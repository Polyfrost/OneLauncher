use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use bytes::Bytes;
use futures_util::{SinkExt, TryStreamExt};
use rand::Rng;
use reqwest_websocket::{Message, Upgrade};
use uuid::Uuid;

use crate::LauncherResult;
use crate::auth::MinecraftAccount;
use crate::constants;

const MOJANG_JOIN_URL: &str = "https://sessionserver.mojang.com/session/minecraft/join";

const PING_INTERVAL: Duration = Duration::from_secs(30);

const RECONNECT_DELAY: Duration = Duration::from_secs(15);
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(300);

static STARTED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, thiserror::Error)]
pub enum PlusError {
    #[error("Mojang rejected the Poly+ session join (HTTP {status}): {body}")]
    JoinRejected { status: u16, body: String },

    #[error("Poly+ rejected the login for {username} (HTTP {status}): {body}")]
    LoginRejected {
        username: String,
        status: u16,
        body: String,
    },

    #[error(transparent)]
    Request(#[from] reqwest::Error),

    #[error(transparent)]
    Websocket(#[from] reqwest_websocket::Error),

    #[error(transparent)]
    Url(#[from] url::ParseError),
}

#[derive(serde::Deserialize)]
struct LoginResponse {
    token: String,
}

enum Outcome {
    Connected,
    NoAccount,
    Failed,
}

enum Event {
    Ping,
    Message(Option<Message>),
}

pub fn start() {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }

    tokio::spawn(async move {
        let client = match build_client() {
            Ok(client) => client,
            Err(err) => {
                tracing::error!("[plus] could not build the playtime client: {err}");
                return;
            }
        };

        let mut delay = RECONNECT_DELAY;

        loop {
            delay = match session(&client).await {
                Outcome::Connected | Outcome::NoAccount => RECONNECT_DELAY,
                Outcome::Failed => (delay * 2).min(MAX_RECONNECT_DELAY),
            };

            tokio::time::sleep(delay).await;
        }
    });
}

fn build_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .tcp_keepalive(Some(Duration::from_secs(15)))
        .connect_timeout(Duration::from_secs(10))
        // reqwest-websocket only supports Http <1.1
        .http1_only()
        .tls_backend_rustls()
        .user_agent(format!(
            "OneClient {} ({})",
            env!("CARGO_PKG_VERSION"),
            env!("CARGO_PKG_HOMEPAGE")
        ))
        .build()
}

fn base_url() -> &'static str {
    constants::PLUS_BACKEND_URL.trim_end_matches('/')
}

async fn session(client: &reqwest::Client) -> Outcome {
    let account = match current_account().await {
        Ok(Some(account)) => account,
        Ok(None) => return Outcome::NoAccount,
        Err(err) => {
            tracing::warn!("[plus] no account to track playtime for: {err}");
            return Outcome::Failed;
        }
    };

    let id = account.id;
    let mut connected = false;

    match run_session(client, &account, &mut connected).await {
        Ok(()) => tracing::info!("[plus] playtime session for {id} ended"),
        Err(err) => tracing::warn!("[plus] playtime session for {id} failed: {err}"),
    }

    if connected {
        Outcome::Connected
    } else {
        Outcome::Failed
    }
}

async fn current_account() -> LauncherResult<Option<MinecraftAccount>> {
    let Some(account) = crate::auth::default_account_for_launch().await? else {
        return Ok(None);
    };

    Ok(account.is_microsoft().then_some(account))
}

async fn run_session(
    client: &reqwest::Client,
    account: &MinecraftAccount,
    connected: &mut bool,
) -> Result<(), PlusError> {
    let token = login(client, account).await?;

    let mut websocket = client
        .get(format!("{}/websocket", base_url()))
        .bearer_auth(&token)
        .upgrade()
        .send()
        .await?
        .into_websocket()
        .await?;

    *connected = true;
    tracing::info!("[plus] playtime websocket connected for {}", account.id);

    pump(&mut websocket, PING_INTERVAL, account.id).await
}

async fn pump(
    websocket: &mut reqwest_websocket::WebSocket,
    ping_interval: Duration,
    account_id: Uuid,
) -> Result<(), PlusError> {
    let mut ping = tokio::time::interval(ping_interval);
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    ping.tick().await;

    loop {
        let event = tokio::select! {
            _ = ping.tick() => Event::Ping,
            message = websocket.try_next() => Event::Message(message?),
        };

        match event {
            Event::Ping => {
                if account_changed(account_id).await {
                    tracing::info!(
                        "[plus] default account changed, ending playtime session for {account_id}"
                    );
                    break;
                }

                websocket.send(Message::Ping(Bytes::new())).await?;
            }
            Event::Message(Some(Message::Close { code, reason })) => {
                tracing::info!("[plus] playtime websocket closed by the server: {code:?} {reason}");
                break;
            }
            Event::Message(Some(_)) => {}
            Event::Message(None) => break,
        }
    }

    Ok(())
}

async fn login(
    client: &reqwest::Client,
    account: &MinecraftAccount,
) -> Result<String, PlusError> {
    let server_id = generate_server_id();

    let join = client
        .post(MOJANG_JOIN_URL)
        .header("Accept", "application/json")
        .json(&serde_json::json!({
            "accessToken": account.access_token,
            "selectedProfile": account.id.simple().to_string(),
            "serverId": server_id,
        }))
        .send()
        .await?;

    if !join.status().is_success() {
        return Err(PlusError::JoinRejected {
            status: join.status().as_u16(),
            body: join.text().await.unwrap_or_default(),
        });
    }

    let mut url = reqwest::Url::parse(&format!("{}/account/login", base_url()))?;
    url.query_pairs_mut()
        .append_pair("username", &account.username)
        .append_pair("server_id", &server_id);

    let login = client
        .post(url)
        .header("Accept", "application/json")
        .send()
        .await?;

    if !login.status().is_success() {
        return Err(PlusError::LoginRejected {
            username: account.username.clone(),
            status: login.status().as_u16(),
            body: login.text().await.unwrap_or_default(),
        });
    }

    Ok(login.json::<LoginResponse>().await?.token)
}

async fn account_changed(id: Uuid) -> bool {
    match crate::auth::get_default_account().await {
        Ok(Some(account)) => account.id != id,
        Ok(None) => true,
        Err(_) => false,
    }
}

fn generate_server_id() -> String {
    let mut bytes = [0u8; 20];
    rand::rng().fill_bytes(&mut bytes);
    crate::crypto::to_hex(&bytes)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;

    use async_tungstenite::tokio::accept_async;
    use async_tungstenite::tungstenite::Message as ServerMessage;
    use futures_util::StreamExt;
    use tokio::net::TcpListener;

    use super::*;

    async fn ping_counting_server(close_after_pings: usize) -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let pings = Arc::new(AtomicUsize::new(0));

        tokio::spawn({
            let pings = Arc::clone(&pings);
            async move {
                let (stream, _) = listener.accept().await.expect("accept");
                let mut server = accept_async(stream).await.expect("handshake");

                while let Some(Ok(message)) = server.next().await {
                    if let ServerMessage::Ping(_) = message
                        && pings.fetch_add(1, Ordering::SeqCst) + 1 >= close_after_pings
                    {
                        let _ = server.close(None).await;
                        break;
                    }
                }
            }
        });

        (format!("ws://127.0.0.1:{port}/websocket"), pings)
    }

    async fn connect(url: &str) -> reqwest_websocket::WebSocket {
        build_client()
            .expect("client")
            .get(url)
            .upgrade()
            .send()
            .await
            .expect("upgrade")
            .into_websocket()
            .await
            .expect("websocket")
    }

    #[tokio::test]
    async fn pings_on_every_interval_and_stops_when_the_server_closes() {
        assert!(!account_changed(Uuid::nil()).await);

        let (url, pings) = ping_counting_server(3).await;
        let mut websocket = connect(&url).await;

        let pumped = tokio::time::timeout(
            Duration::from_secs(10),
            pump(&mut websocket, Duration::from_millis(50), Uuid::nil()),
        )
        .await
        .expect("pump should return once the server closes the socket");

        pumped.expect("pump should treat a server close as a clean end");
        assert_eq!(pings.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn server_ids_are_random_20_byte_hex() {
        let id = generate_server_id();
        assert_eq!(id.len(), 40);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(id, generate_server_id());
    }
}
