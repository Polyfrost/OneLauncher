use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures_util::{SinkExt, TryStreamExt};
use rand::Rng;
use reqwest_websocket::{Message, Upgrade};
use uuid::Uuid;

use oneclient_auth::MinecraftAccount;
use oneclient_events::{ChatEvent, EventBus};
use oneclient_net::RequestError;

use crate::packets::{ClientboundPacket, ServerboundPacket};
use crate::{
    PlusClient, PlusError, REQUEST_TIMEOUT, api, base_url, party, plus_blocked, social_allowed,
};

const MOJANG_JOIN_URL: &str = "https://sessionserver.mojang.com/session/minecraft/join";

const PING_INTERVAL: Duration = Duration::from_secs(30);

const RECONNECT_DELAY: Duration = Duration::from_secs(15);
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(300);

const MAX_PLAYERS_PER_PACKET: usize = 64;
const MAX_WATCHED_PLAYERS: usize = 512;

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
    Watch,
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
            if plus_blocked() {
                tracing::info!("[plus] ending the session: terms and privacy policy declined");
                return;
            }

            let mut restart = client.restart_signal();

            let outcome = session(&client, &socket, &mut restart).await;
            let restarted = matches!(outcome, Outcome::Restart);

            delay = next_delay(&outcome, delay);

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

pub(crate) fn build_socket_client() -> Result<reqwest::Client, reqwest::Error> {
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
        Ok(None) => {
            client.set_session_account(None);
            return Outcome::NoAccount;
        }
        Err(err) => {
            client.set_session_account(None);
            tracing::warn!("[plus] no account to open a session for: {err}");
            return Outcome::Failed;
        }
    };
    client.set_session_account(Some(account.id));

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
                api::forget_token(id).await;
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
    let session_url = base_url();
    let token = api::token_for(account, None).await?;

    let mut websocket = socket
        .get(format!("{session_url}/websocket"))
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
    party::connected();

    let account_id = account.id;
    let mut watched = client.watched_players();
    let url = session_url.as_str();
    pump(
        &mut websocket,
        &client.events,
        PING_INTERVAL,
        account_id,
        restart,
        &mut watched,
        || async move {
            plus_blocked()
                || !social_allowed()
                || base_url() != url
                || account_changed(&client.auth, account_id).await
        },
    )
    .await
}

async fn pump<F, Fut>(
    websocket: &mut reqwest_websocket::WebSocket,
    events: &EventBus,
    ping_interval: Duration,
    account_id: Uuid,
    restart: &mut tokio::sync::watch::Receiver<u64>,
    watched: &mut tokio::sync::watch::Receiver<Vec<Uuid>>,
    should_stop: F,
) -> Result<Ended, PlusError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let mut ping = tokio::time::interval(ping_interval);
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    ping.tick().await;

    let mut subscribed = HashSet::new();
    let wanted = watched.borrow_and_update().clone();
    sync_watched(websocket, &mut subscribed, &wanted).await?;

    loop {
        let frame = tokio::select! {
            _ = ping.tick() => Frame::Ping,
            _ = restart.changed() => Frame::Restart,
            Ok(()) = watched.changed() => Frame::Watch,
            message = websocket.try_next() => Frame::Message(message?),
        };

        match frame {
            Frame::Ping => {
                if should_stop().await {
                    tracing::info!("[plus] ending session for {account_id}");
                    return Ok(Ended::Restart);
                }

                websocket.send(Message::Ping(Bytes::new())).await?;
            }
            Frame::Restart => return Ok(Ended::Restart),
            Frame::Watch => {
                let wanted = watched.borrow_and_update().clone();
                sync_watched(websocket, &mut subscribed, &wanted).await?;
            }
            Frame::Message(Some(Message::Text(payload))) => {
                dispatch(events, account_id, &payload);
                party::dispatch(&payload);
            }
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

async fn sync_watched(
    websocket: &mut reqwest_websocket::WebSocket,
    subscribed: &mut HashSet<Uuid>,
    wanted: &[Uuid],
) -> Result<(), PlusError> {
    let wanted: HashSet<Uuid> = wanted.iter().copied().take(MAX_WATCHED_PLAYERS).collect();

    let removed: Vec<Uuid> = subscribed.difference(&wanted).copied().collect();
    let added: Vec<Uuid> = wanted.difference(subscribed).copied().collect();

    for players in removed.chunks(MAX_PLAYERS_PER_PACKET) {
        send(
            websocket,
            &ServerboundPacket::UnsubscribePlayers { players },
        )
        .await?;
    }
    for players in added.chunks(MAX_PLAYERS_PER_PACKET) {
        send(websocket, &ServerboundPacket::SubscribePlayers { players }).await?;
    }

    *subscribed = wanted;
    Ok(())
}

async fn send(
    websocket: &mut reqwest_websocket::WebSocket,
    packet: &ServerboundPacket<'_>,
) -> Result<(), PlusError> {
    let payload = serde_json::to_string(packet)
        .map_err(|err| PlusError::Request(RequestError::SerializeError(err)))?;
    websocket.send(Message::Text(payload)).await?;
    Ok(())
}

fn dispatch(events: &EventBus, account: Uuid, payload: &str) {
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
            session_invite_id,
            session_invite_status,
        } => events.chat(ChatEvent::MessageReceived {
            account,
            group_id,
            message_id,
            sender,
            content,
            invite_id: session_invite_id,
            invite_status: session_invite_status,
        }),
        ClientboundPacket::GroupMessageEdited {
            group_id,
            message_id,
            content,
            session_invite_status,
        } => events.chat(ChatEvent::MessageEdited {
            group_id,
            message_id,
            content,
            invite_status: session_invite_status,
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
        ClientboundPacket::SubscriptionSnapshot { users } => {
            for player in users {
                events.chat(ChatEvent::PresenceChanged {
                    player,
                    online: true,
                });
            }
        }
        ClientboundPacket::FriendRequestReceived { sender, .. } => {
            events.chat(ChatEvent::FriendRequestReceived { account, sender });
        }
        ClientboundPacket::FriendRequestUpdated { .. }
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
    if !social_allowed() {
        return Ok(None);
    }

    let Some(account) = auth.default_account_for_launch().await? else {
        return Ok(None);
    };

    Ok(usable(&account, social_allowed()).then_some(account))
}

fn next_delay(outcome: &Outcome, delay: Duration) -> Duration {
    match outcome {
        Outcome::Restart | Outcome::Connected | Outcome::NoAccount => RECONNECT_DELAY,
        Outcome::Failed => (delay * 2).min(MAX_RECONNECT_DELAY),
    }
}

pub(crate) fn usable(account: &MinecraftAccount, allowed: bool) -> bool {
    allowed
        && account.is_microsoft()
        && !account.needs_sign_in()
        && !account.access_token.is_empty()
}

async fn account_changed(auth: &oneclient_auth::AuthService, id: Uuid) -> bool {
    match auth.default_account().await {
        Ok(Some(account)) => account.id != id || account.needs_sign_in(),
        Ok(None) => true,
        Err(_) => false,
    }
}

pub(crate) async fn login(
    client: &reqwest::Client,
    account: &MinecraftAccount,
) -> Result<String, PlusError> {
    if plus_blocked() {
        return Err(PlusError::ConsentRequired);
    }

    let server_id = generate_server_id();

    let join = client
        .post(MOJANG_JOIN_URL)
        .timeout(REQUEST_TIMEOUT)
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
        });
    }

    let mut url = reqwest::Url::parse(&format!("{}/account/login", base_url()))?;
    url.query_pairs_mut()
        .append_pair("username", &account.username)
        .append_pair("server_id", &server_id);

    let login = client
        .post(url)
        .timeout(REQUEST_TIMEOUT)
        .header("Accept", "application/json")
        .send()
        .await?;

    if !login.status().is_success() {
        return Err(PlusError::LoginRejected {
            username: account.username.clone(),
            status: login.status().as_u16(),
        });
    }

    Ok(login.json::<LoginResponse>().await?.token)
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

    #[test]
    fn failures_back_off_up_to_the_cap_and_any_connection_resets_the_wait() {
        assert_eq!(next_delay(&Outcome::Failed, RECONNECT_DELAY), RECONNECT_DELAY * 2);
        assert_eq!(next_delay(&Outcome::Failed, MAX_RECONNECT_DELAY), MAX_RECONNECT_DELAY);
        assert_eq!(next_delay(&Outcome::Connected, MAX_RECONNECT_DELAY), RECONNECT_DELAY);
        assert_eq!(next_delay(&Outcome::NoAccount, MAX_RECONNECT_DELAY), RECONNECT_DELAY);
        assert_eq!(next_delay(&Outcome::Restart, MAX_RECONNECT_DELAY), RECONNECT_DELAY);
    }

    fn account(kind: oneclient_auth::AccountKind, token: &str, signed_out: bool) -> MinecraftAccount {
        MinecraftAccount {
            id: Uuid::from_u128(1),
            username: "player".to_string(),
            access_token: token.to_string(),
            refresh_token: String::new(),
            expires: chrono::Utc::now(),
            kind,
            signed_out,
        }
    }

    #[test]
    fn only_a_signed_in_microsoft_account_with_a_token_is_usable() {
        use oneclient_auth::AccountKind::{Microsoft, Offline};

        assert!(usable(&account(Microsoft, "token", false), true));
        assert!(!usable(&account(Microsoft, "token", false), false));
        assert!(!usable(&account(Microsoft, "token", true), true));
        assert!(!usable(&account(Microsoft, "", false), true));
        assert!(!usable(&account(Offline, "token", false), true));
    }

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
        let watched = tokio::sync::watch::Sender::new(Vec::new());
        let mut nobody = watched.subscribe();

        let pumped = tokio::time::timeout(
            Duration::from_secs(10),
            pump(
                &mut websocket,
                &events,
                Duration::from_millis(50),
                Uuid::nil(),
                &mut quiet,
                &mut nobody,
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

    async fn text_recording_server(
        close_after: usize,
    ) -> (
        String,
        tokio::sync::oneshot::Receiver<Vec<serde_json::Value>>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let (done, received) = tokio::sync::oneshot::channel();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut server = accept_async(stream).await.expect("handshake");
            let mut texts = Vec::new();

            while let Some(Ok(message)) = server.next().await {
                if let ServerMessage::Text(text) = message {
                    texts.push(serde_json::from_str(text.as_str()).expect("json packet"));
                    if texts.len() >= close_after {
                        let _ = server.close(None).await;
                        break;
                    }
                }
            }

            let _ = done.send(texts);
        });

        (format!("ws://127.0.0.1:{port}/websocket"), received)
    }

    fn players_of(packet: &serde_json::Value) -> HashSet<Uuid> {
        serde_json::from_value(packet["players"].clone()).expect("players")
    }

    #[tokio::test]
    async fn watched_friends_are_subscribed_and_kept_in_step() {
        let first = Uuid::from_u128(1);
        let second = Uuid::from_u128(2);
        let third = Uuid::from_u128(3);

        let (url, received) = text_recording_server(3).await;
        let mut websocket = connect(&url).await;
        let (events, _receiver) = EventBus::channel();

        let restart = tokio::sync::watch::Sender::new(0u64);
        let mut quiet = restart.subscribe();
        let watched = tokio::sync::watch::Sender::new(vec![first, second]);
        let mut friends = watched.subscribe();

        let pumping = tokio::spawn(async move {
            pump(
                &mut websocket,
                &events,
                Duration::from_secs(60),
                Uuid::nil(),
                &mut quiet,
                &mut friends,
                || async { false },
            )
            .await
            .map(|_| ())
        });

        tokio::time::sleep(Duration::from_millis(100)).await;
        watched.send_replace(vec![second, third]);

        let texts = tokio::time::timeout(Duration::from_secs(10), received)
            .await
            .expect("server should see three packets")
            .expect("server result");
        let _ = pumping.await;

        assert_eq!(texts[0]["type"], "SubscribePlayers");
        assert_eq!(players_of(&texts[0]), HashSet::from([first, second]));
        assert_eq!(texts[1]["type"], "UnsubscribePlayers");
        assert_eq!(players_of(&texts[1]), HashSet::from([first]));
        assert_eq!(texts[2]["type"], "SubscribePlayers");
        assert_eq!(players_of(&texts[2]), HashSet::from([third]));
    }

    #[tokio::test]
    async fn party_packets_reach_the_lobby_over_the_chat_socket() {
        let party_id = Uuid::from_u128(7);
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("addr").port();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut server = accept_async(stream).await.expect("handshake");
            let packet =
                format!(r#"{{"type":"PartyLeft","party_id":"{party_id}","reason":"left"}}"#);
            server
                .send(ServerMessage::Text(packet.into()))
                .await
                .expect("send");
            let _ = server.close(None).await;
        });

        let mut lobby = party::subscribe();
        let mut websocket = connect(&format!("ws://127.0.0.1:{port}/websocket")).await;
        let (events, mut bus) = EventBus::channel();

        let restart = tokio::sync::watch::Sender::new(0u64);
        let mut quiet = restart.subscribe();
        let watched = tokio::sync::watch::Sender::new(Vec::new());
        let mut nobody = watched.subscribe();

        tokio::time::timeout(
            Duration::from_secs(10),
            pump(
                &mut websocket,
                &events,
                Duration::from_secs(60),
                Uuid::nil(),
                &mut quiet,
                &mut nobody,
                || async { false },
            ),
        )
        .await
        .expect("pump should end with the socket")
        .expect("a clean close");

        let received = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let party::LiveEvent::Packet(packet) = lobby.recv().await.expect("lobby event")
                    && let party::PartyPacket::PartyLeft { party_id: id, .. } = *packet
                    && id == party_id
                {
                    return id;
                }
            }
        })
        .await
        .expect("the lobby should hear the packet");

        assert_eq!(received, party_id);
        assert!(bus.try_recv().is_err(), "party traffic stays off the chat bus");
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
            sender,
            &format!(
                r#"{{"type":"GroupMessageReceived","group_id":3,"message_id":9,"sender":"{sender}","content":"hey"}}"#
            ),
        );

        match receiver
            .try_recv()
            .expect("an event should have been emitted")
        {
            oneclient_events::Event::Chat(ChatEvent::MessageReceived {
                account,
                group_id,
                message_id,
                content,
                ..
            }) => {
                assert_eq!(account, sender);
                assert_eq!(group_id, 3);
                assert_eq!(message_id, 9);
                assert_eq!(content, "hey");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn a_live_invite_keeps_its_id_and_status() {
        let (events, mut receiver) = EventBus::channel();
        let sender = Uuid::nil();

        dispatch(
            &events,
            sender,
            &format!(
                r#"{{"type":"GroupMessageReceived","group_id":3,"message_id":9,"sender":"{sender}","content":"","session_invite_id":12,"session_invite_status":"pending"}}"#
            ),
        );

        match receiver
            .try_recv()
            .expect("an event should have been emitted")
        {
            oneclient_events::Event::Chat(ChatEvent::MessageReceived {
                invite_id,
                invite_status,
                ..
            }) => {
                assert_eq!(invite_id, Some(12));
                assert_eq!(invite_status.as_deref(), Some("pending"));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn cosmetic_traffic_stays_off_the_bus() {
        let (events, mut receiver) = EventBus::channel();

        dispatch(
            &events,
            Uuid::nil(),
            r#"{"type":"PlayerEmoteStopped","player":"00000000-0000-0000-0000-000000000000"}"#,
        );

        assert!(receiver.try_recv().is_err());
    }
}
