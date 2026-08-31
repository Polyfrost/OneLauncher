use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures_util::{SinkExt, TryStreamExt};
use rand::Rng;
use reqwest_websocket::{Message, Upgrade};
use uuid::Uuid;

use oneclient_auth::MinecraftAccount;
use oneclient_events::{ChatEvent, EventBus};
use oneclient_net::{RequestClient, RequestError};

use crate::packets::ClientboundPacket;
use crate::{PlusClient, PlusError, base_url};

const MOJANG_JOIN_URL: &str = "https://sessionserver.mojang.com/session/minecraft/join";

const PING_INTERVAL: Duration = Duration::from_secs(30);

const RECONNECT_DELAY: Duration = Duration::from_secs(15);
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(300);

#[derive(serde::Deserialize)]
struct LoginResponse {
    token: String,
}

enum Outcome {
    Connected,
    NoAccount,
    Failed,
    Restart,
}

enum Frame {
    Ping,
    Restart,
    Message(Option<Message>),
}

enum Ended {
    Closed,
    Restart,
}

pub(crate) fn spawn(client: Arc<PlusClient>) {
    tokio::spawn(async move {
        let socket = match build_socket_client() {
            Ok(socket) => socket,
            Err(err) => {
                tracing::error!("[plus] could not build the websocket client: {err}");
                return;
            }
        };

        let mut delay = RECONNECT_DELAY;

        loop {
            let mut restart = client.restart_signal();

            let outcome = session(&client, &socket, &mut restart).await;
            let restarted = matches!(outcome, Outcome::Restart);

            delay = match outcome {
                Outcome::Restart | Outcome::Connected | Outcome::NoAccount => RECONNECT_DELAY,
                Outcome::Failed => (delay * 2).min(MAX_RECONNECT_DELAY),
            };

            if restarted {
                continue;
            }

            tokio::select! {
                () = tokio::time::sleep(delay) => {}
                _ = restart.changed() => {}
            }
        }
    });
}

fn build_socket_client() -> Result<reqwest::Client, reqwest::Error> {
    let builder = reqwest::Client::builder()
        .tcp_keepalive(Some(Duration::from_secs(15)))
        .connect_timeout(Duration::from_secs(10))
        .http1_only()
        .tls_backend_rustls()
        .user_agent(format!(
            "OneClient {} ({})",
            env!("CARGO_PKG_VERSION"),
            env!("CARGO_PKG_HOMEPAGE")
        ));

    builder.no_hickory_dns().build()
}

async fn session(
    client: &PlusClient,
    socket: &reqwest::Client,
    restart: &mut tokio::sync::watch::Receiver<u64>,
) -> Outcome {
    let account = match current_account(&client.auth).await {
        Ok(Some(account)) => account,
        Ok(None) => return Outcome::NoAccount,
        Err(err) => {
            tracing::warn!("[plus] no account to open a session for: {err}");
            return Outcome::Failed;
        }
    };

    let id = account.id;
    let mut connected = false;
    let mut restarted = false;

    match run_session(client, socket, &account, &mut connected, restart).await {
        Ok(Ended::Closed) => tracing::info!("[plus] session for {id} ended"),
        Ok(Ended::Restart) => {
            restarted = true;
            tracing::info!("[plus] restarting the session for {id}");
        }
        Err(err) => {
            if err.is_unauthorized() {
                client.forget_token().await;
            }
            tracing::warn!("[plus] session for {id} failed: {err}");
        }
    }

    if connected {
        client
            .events
            .chat(ChatEvent::ConnectionChanged { connected: false });
    }

    match (restarted, connected) {
        (true, _) => Outcome::Restart,
        (_, true) => Outcome::Connected,
        _ => Outcome::Failed,
    }
}

async fn run_session(
    client: &PlusClient,
    socket: &reqwest::Client,
    account: &MinecraftAccount,
    connected: &mut bool,
    restart: &mut tokio::sync::watch::Receiver<u64>,
) -> Result<Ended, PlusError> {
    let token = client.authorize().await?;

    let mut websocket = socket
        .get(format!("{}/websocket", base_url()))
        .bearer_auth(&token)
        .upgrade()
        .send()
        .await?
        .into_websocket()
        .await?;

    *connected = true;
    client
        .events
        .chat(ChatEvent::ConnectionChanged { connected: true });
    tracing::info!("[plus] websocket connected for {}", account.id);

    let account_id = account.id;
    pump(
        &mut websocket,
        &client.events,
        PING_INTERVAL,
        account_id,
        restart,
        || account_changed(&client.auth, account_id),
    )
    .await
}

async fn pump<F, Fut>(
    websocket: &mut reqwest_websocket::WebSocket,
    events: &EventBus,
    ping_interval: Duration,
    account_id: Uuid,
    restart: &mut tokio::sync::watch::Receiver<u64>,
    should_stop: F,
) -> Result<Ended, PlusError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let mut ping = tokio::time::interval(ping_interval);
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    ping.tick().await;

    loop {
        let frame = tokio::select! {
            _ = ping.tick() => Frame::Ping,
            _ = restart.changed() => Frame::Restart,
            message = websocket.try_next() => Frame::Message(message?),
        };

        match frame {
            Frame::Ping => {
                if should_stop().await {
                    tracing::info!("[plus] default account changed, ending session for {account_id}");
                    return Ok(Ended::Restart);
                }

                websocket.send(Message::Ping(Bytes::new())).await?;
            }
            Frame::Restart => return Ok(Ended::Restart),
            Frame::Message(Some(Message::Text(payload))) => dispatch(events, &payload),
            Frame::Message(Some(Message::Close { code, reason })) => {
                tracing::info!("[plus] websocket closed by the server: {code:?} {reason}");
                break;
            }
            Frame::Message(Some(_)) => {}
            Frame::Message(None) => break,
        }
    }

    Ok(Ended::Closed)
}

fn dispatch(events: &EventBus, payload: &str) {
    let packet = match serde_json::from_str::<ClientboundPacket>(payload) {
        Ok(packet) => packet,
        Err(err) => {
            tracing::warn!("[plus] could not decode a websocket packet: {err}");
            return;
        }
    };

    match packet {
        ClientboundPacket::GroupMessageReceived {
            group_id,
            message_id,
            sender,
            content,
            ..
        } => events.chat(ChatEvent::MessageReceived {
            group_id,
            message_id,
            sender,
            content,
        }),
        ClientboundPacket::GroupMessageEdited {
            group_id,
            message_id,
            content,
            ..
        } => events.chat(ChatEvent::MessageEdited {
            group_id,
            message_id,
            content,
        }),
        ClientboundPacket::GroupMessageDeleted {
            group_id,
            message_id,
        } => events.chat(ChatEvent::MessageDeleted {
            group_id,
            message_id,
        }),
        ClientboundPacket::PlayerPresence { player, online } => {
            events.chat(ChatEvent::PresenceChanged { player, online });
        }
        ClientboundPacket::FriendRequestReceived { .. }
        | ClientboundPacket::FriendRequestUpdated { .. }
        | ClientboundPacket::FriendRemoved { .. } => events.chat(ChatEvent::RosterChanged),
        ClientboundPacket::Error {
            error_code,
            message,
            ..
        } => tracing::warn!("[plus] server error {error_code}: {message}"),
        ClientboundPacket::Unhandled => {}
    }
}

pub(crate) async fn current_account(
    auth: &oneclient_auth::AuthService,
) -> Result<Option<MinecraftAccount>, PlusError> {
    let Some(account) = auth.default_account_for_launch().await? else {
        return Ok(None);
    };

    Ok(account.is_microsoft().then_some(account))
}

async fn account_changed(auth: &oneclient_auth::AuthService, id: Uuid) -> bool {
    match auth.default_account().await {
        Ok(Some(account)) => account.id != id,
        Ok(None) => true,
        Err(_) => false,
    }
}

pub(crate) async fn login(
    requester: &RequestClient,
    account: &MinecraftAccount,
) -> Result<String, PlusError> {
    let server_id = generate_server_id();

    let mut join = reqwest::Request::new(
        reqwest::Method::POST,
        reqwest::Url::parse(MOJANG_JOIN_URL)?,
    );
    join.headers_mut().insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("application/json"),
    );
    let payload = serde_json::json!({
        "accessToken": account.access_token,
        "selectedProfile": account.id.simple().to_string(),
        "serverId": server_id,
    });
    *join.body_mut() = Some(
        serde_json::to_vec(&payload)
            .map_err(RequestError::SerializeError)?
            .into(),
    );

    let joined = requester.send(join).await?;
    if !joined.status().is_success() {
        return Err(PlusError::JoinRejected {
            status: joined.status().as_u16(),
        });
    }

    let mut url = reqwest::Url::parse(&format!("{}/account/login", base_url()))?;
    url.query_pairs_mut()
        .append_pair("username", &account.username)
        .append_pair("server_id", &server_id);

    requester
        .send_json::<LoginResponse>(reqwest::Method::POST, url, None, &[])
        .await
        .map(|response| response.token)
        .map_err(|err| match err {
            RequestError::HttpStatus { status, .. } => PlusError::LoginRejected {
                username: account.username.clone(),
                status,
            },
            other => PlusError::Request(other),
        })
}

fn generate_server_id() -> String {
    let mut bytes = [0u8; 20];
    rand::rng().fill_bytes(&mut bytes);
    polyio::to_hex(&bytes)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

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
        build_socket_client()
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
        let (url, pings) = ping_counting_server(3).await;
        let mut websocket = connect(&url).await;
        let (events, _receiver) = EventBus::channel();

        let restart = tokio::sync::watch::Sender::new(0u64);
        let mut quiet = restart.subscribe();

        let pumped = tokio::time::timeout(
            Duration::from_secs(10),
            pump(
                &mut websocket,
                &events,
                Duration::from_millis(50),
                Uuid::nil(),
                &mut quiet,
                || async { false },
            ),
        )
        .await
        .expect("pump should return once the server closes the socket");

        assert!(matches!(
            pumped.expect("pump should treat a server close as a clean end"),
            Ended::Closed
        ));
        assert_eq!(pings.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn server_ids_are_random_20_byte_hex() {
        let id = generate_server_id();
        assert_eq!(id.len(), 40);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(id, generate_server_id());
    }

    #[test]
    fn group_messages_reach_the_bus() {
        let (events, mut receiver) = EventBus::channel();
        let sender = Uuid::nil();

        dispatch(
            &events,
            &format!(
                r#"{{"type":"GroupMessageReceived","group_id":3,"message_id":9,"sender":"{sender}","content":"hey"}}"#
            ),
        );

        match receiver.try_recv().expect("an event should have been emitted") {
            oneclient_events::Event::Chat(ChatEvent::MessageReceived {
                group_id,
                message_id,
                content,
                ..
            }) => {
                assert_eq!(group_id, 3);
                assert_eq!(message_id, 9);
                assert_eq!(content, "hey");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn cosmetic_traffic_stays_off_the_bus() {
        let (events, mut receiver) = EventBus::channel();

        dispatch(
            &events,
            r#"{"type":"PlayerEmoteStopped","player":"00000000-0000-0000-0000-000000000000"}"#,
        );

        assert!(receiver.try_recv().is_err());
    }
}
