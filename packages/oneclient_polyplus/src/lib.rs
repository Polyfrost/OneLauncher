mod models;
mod packets;
mod rest;
mod session;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::RwLock;

use oneclient_common::constants;
use oneclient_events::EventBus;
use oneclient_net::{RequestClient, RequestError};

pub use models::{
    BlockedPlayer, Friend, FriendRequest, GroupKind, GroupMessage, GroupSummary, LastMessage,
    MAX_MESSAGE_LENGTH, MAX_PAGE_SIZE, MAX_RESOLVE_BATCH, RelationshipKind, ResolvedPlayer,
    SessionInvite, SpecialChatStatus,
};
pub use packets::ClientboundPacket;

static STARTED: AtomicBool = AtomicBool::new(false);
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

    #[error(transparent)]
    Request(#[from] RequestError),

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error(transparent)]
    Websocket(#[from] reqwest_websocket::Error),

    #[error(transparent)]
    Url(#[from] url::ParseError),
}

impl PlusError {
    #[must_use]
    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Request(RequestError::HttpStatus { status, .. }) => Some(*status),
            Self::JoinRejected { status } | Self::LoginRejected { status, .. } => Some(*status),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_not_found(&self) -> bool {
        self.status() == Some(404)
    }

    #[must_use]
    pub fn is_unauthorized(&self) -> bool {
        self.status() == Some(401)
    }
}

pub struct PlusClient {
    requester: RequestClient,
    auth: Arc<oneclient_auth::AuthService>,
    events: EventBus,
    token: RwLock<Option<String>>,
}

impl PlusClient {
    fn new(
        auth: Arc<oneclient_auth::AuthService>,
        requester: RequestClient,
        events: EventBus,
    ) -> Self {
        Self {
            requester,
            auth,
            events,
            token: RwLock::new(None),
        }
    }

    pub fn events(&self) -> &EventBus {
        &self.events
    }

    pub(crate) async fn authorize(&self) -> Result<String, PlusError> {
        if let Some(token) = self.token.read().await.clone() {
            return Ok(token);
        }

        let mut guard = self.token.write().await;
        if let Some(token) = guard.clone() {
            return Ok(token);
        }

        let account = session::current_account(&self.auth)
            .await?
            .ok_or(PlusError::NoAccount)?;
        let token = session::login(&self.requester, &account).await?;
        *guard = Some(token.clone());

        Ok(token)
    }

    pub(crate) async fn forget_token(&self) {
        self.token.write().await.take();
    }
}

pub fn start(
    auth: Arc<oneclient_auth::AuthService>,
    requester: RequestClient,
    events: EventBus,
) -> Arc<PlusClient> {
    let client = CLIENT.get_or_init(|| Arc::new(PlusClient::new(auth, requester, events)));

    if !STARTED.swap(true, Ordering::SeqCst) {
        session::spawn(Arc::clone(client));
    }

    Arc::clone(client)
}

#[must_use]
pub fn client() -> Option<Arc<PlusClient>> {
    CLIENT.get().map(Arc::clone)
}

pub async fn forget_token() {
    if let Some(client) = client() {
        client.forget_token().await;
    }
}

fn base_url() -> &'static str {
    constants::PLUS_BACKEND_URL.trim_end_matches('/')
}
