pub mod api;
pub mod instances;
mod models;
mod packets;
pub mod party;
mod rest;
mod session;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use uuid::Uuid;

use oneclient_common::constants;
use oneclient_events::EventBus;
use oneclient_net::RequestError;

pub use models::{
    BlockedPlayer, Friend, FriendRequest, GroupKind, GroupMessage, GroupSummary,
    IncomingSessionInvite, LastMessage,
    MAX_MESSAGE_LENGTH, MAX_PAGE_SIZE, MAX_RESOLVE_BATCH, PlayerStatus, RelationshipKind,
    ResolvedPlayer, SessionInvite,
};
pub use packets::ClientboundPacket;

pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

static STARTED: AtomicBool = AtomicBool::new(false);
static SOCIAL_ALLOWED: AtomicBool = AtomicBool::new(false);
static BASE_URL: std::sync::RwLock<Option<String>> = std::sync::RwLock::new(None);
static CLIENT: std::sync::OnceLock<Arc<PlusClient>> = std::sync::OnceLock::new();

#[derive(Debug, thiserror::Error)]
pub enum PlusError {
    #[error(transparent)]
    Auth(#[from] oneclient_auth::AuthError),

    #[error("no Microsoft account is signed in")]
    NoAccount,

    #[error("Mojang rejected the Poly+ session join (HTTP {status})")]
    JoinRejected { status: u16 },

    #[error("Poly+ rejected the login for {username} (HTTP {status})")]
    LoginRejected { username: String, status: u16 },

    #[error("Poly+ is off because the Terms of Service and Privacy Policy were declined")]
    ConsentRequired,

    #[error("Poly+ needs a Microsoft account")]
    OfflineAccount,

    #[error("Poly+ request failed (HTTP {status}): {body}")]
    Api { status: u16, body: String },

    #[error("could not build the Poly+ client: {0}")]
    Client(String),

    #[error("could not read the Poly+ asset: {0}")]
    Asset(String),

    #[error(transparent)]
    Request(#[from] RequestError),

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error(transparent)]
    Websocket(#[from] reqwest_websocket::Error),

    #[error(transparent)]
    Url(#[from] url::ParseError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl PlusError {
    #[must_use]
    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Api { status, .. } | Self::Request(RequestError::HttpStatus { status, .. }) => {
                Some(*status)
            }
            Self::Websocket(reqwest_websocket::Error::Handshake(
                reqwest_websocket::HandshakeError::UnexpectedStatusCode(status),
            )) => Some(status.as_u16()),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_not_found(&self) -> bool {
        self.status() == Some(404)
    }

    #[must_use]
    pub fn is_forbidden(&self) -> bool {
        self.status() == Some(403)
    }

    #[must_use]
    pub fn is_unauthorized(&self) -> bool {
        self.status() == Some(401)
    }
}

pub struct PlusClient {
    auth: Arc<oneclient_auth::AuthService>,
    events: EventBus,
    session_account: std::sync::Mutex<Option<Uuid>>,
    restart: tokio::sync::watch::Sender<u64>,
    watched: tokio::sync::watch::Sender<Vec<Uuid>>,
}

impl PlusClient {
    fn new(auth: Arc<oneclient_auth::AuthService>, events: EventBus) -> Self {
        Self {
            auth,
            events,
            session_account: std::sync::Mutex::new(None),
            restart: tokio::sync::watch::Sender::new(0),
            watched: tokio::sync::watch::Sender::new(Vec::new()),
        }
    }

    pub fn events(&self) -> &EventBus {
        &self.events
    }

    pub(crate) fn set_session_account(&self, account: Option<Uuid>) {
        if let Ok(mut current) = self.session_account.lock() {
            *current = account;
        }
    }

    async fn restart_if_switched(&self) {
        let owner = self
            .auth
            .default_account()
            .await
            .ok()
            .flatten()
            .filter(|account| account.is_microsoft() && !account.needs_sign_in())
            .map(|account| account.id);

        let current = self
            .session_account
            .lock()
            .ok()
            .and_then(|current| *current);
        if current != owner {
            self.request_restart();
        }
    }

    pub(crate) fn request_restart(&self) {
        self.restart.send_modify(|generation| {
            *generation = generation.wrapping_add(1);
        });
    }

    pub(crate) fn watched_players(&self) -> tokio::sync::watch::Receiver<Vec<Uuid>> {
        self.watched.subscribe()
    }

    pub(crate) fn restart_signal(&self) -> tokio::sync::watch::Receiver<u64> {
        self.restart.subscribe()
    }
}

pub fn start(auth: Arc<oneclient_auth::AuthService>, events: EventBus) -> Arc<PlusClient> {
    let client = CLIENT.get_or_init(|| Arc::new(PlusClient::new(auth, events)));

    if !STARTED.swap(true, Ordering::SeqCst) {
        if plus_blocked() {
            tracing::info!("[plus] session stays off: terms and privacy policy declined");
        } else {
            session::spawn(Arc::clone(client));
        }
    }

    Arc::clone(client)
}

#[must_use]
pub fn client() -> Option<Arc<PlusClient>> {
    CLIENT.get().map(Arc::clone)
}

pub fn watch_players(mut players: Vec<Uuid>) {
    let Some(client) = client() else { return };

    players.sort_unstable();
    players.dedup();
    client.watched.send_if_modified(|watched| {
        if *watched == players {
            return false;
        }
        *watched = players;
        true
    });
}

pub async fn restart_session() {
    if let Some(client) = client() {
        client.restart_if_switched().await;
    }
}

pub fn set_social_allowed(allowed: bool) {
    if SOCIAL_ALLOWED.swap(allowed, Ordering::SeqCst) != allowed
        && let Some(client) = client()
    {
        client.request_restart();
    }
}

#[must_use]
pub fn social_allowed() -> bool {
    SOCIAL_ALLOWED.load(Ordering::SeqCst)
}

pub fn plus_blocked() -> bool {
    oneclient_common::consent::blocks_url(&base_url())
}

pub fn set_base_url(url: &str) {
    if let Ok(mut current) = BASE_URL.write() {
        *current = Some(url.to_string());
    }
}

pub fn base_url() -> String {
    let local = cfg!(debug_assertions)
        .then(|| std::env::var("ONECLIENT_PLUS_URL").ok())
        .flatten()
        .filter(|url| !url.trim().is_empty());
    local
        .or_else(|| BASE_URL.read().ok().and_then(|url| url.clone()))
        .as_deref()
        .unwrap_or(constants::PLUS_BACKEND_URL)
        .trim_end_matches('/')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_api_failure_reports_its_status() {
        let err = PlusError::Api {
            status: 404,
            body: String::new(),
        };
        assert!(err.is_not_found());
    }

    #[test]
    fn a_rejected_login_says_nothing_about_the_resource() {
        assert!(!PlusError::JoinRejected { status: 403 }.is_forbidden());
        let login = PlusError::LoginRejected {
            username: String::new(),
            status: 404,
        };
        assert!(!login.is_not_found());
    }

    #[test]
    fn a_rejected_websocket_handshake_counts_as_unauthorized() {
        let err = PlusError::Websocket(reqwest_websocket::Error::Handshake(
            reqwest_websocket::HandshakeError::UnexpectedStatusCode(
                reqwest::StatusCode::UNAUTHORIZED,
            ),
        ));
        assert!(err.is_unauthorized());
    }

    #[test]
    fn base_url_follows_the_pushed_setting() {
        set_base_url("https://plus-staging.example.test/");
        assert_eq!(base_url(), "https://plus-staging.example.test");
        set_base_url(constants::PLUS_BACKEND_URL);
        assert_eq!(base_url(), constants::PLUS_BACKEND_URL);
    }
}
