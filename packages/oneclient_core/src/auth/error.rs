use serde::Serialize;

use crate::http::RequestError;

#[derive(Debug, Clone, Copy, Serialize)]
pub enum MinecraftAuthStep {
    DeviceCodeRequest,
    DeviceCodePoll,
    AuthCodeExchange,
    RefreshToken,
    XblAuthenticate,
    XstsAuthorize,
    MinecraftToken,
    MinecraftEntitlements,
    MinecraftProfile,
}

#[derive(Debug, thiserror::Error)]
pub enum MinecraftAuthError {
    #[error("failed to serialize JSON during MSA step {step:?}: {source}")]
    SerializeError {
        step: MinecraftAuthStep,
        #[source]
        source: serde_json::Error,
    },
    #[error(
        "failed to deserialize JSON during MSA step {step:?}: {source}! status {status_code} - body: {raw}"
    )]
    DeserializeError {
        step: MinecraftAuthStep,
        raw: String,
        #[source]
        source: serde_json::Error,
        status_code: reqwest::StatusCode,
    },
    #[error("failed to request using HTTP during MSA step {step:?}: {source}")]
    RequestError {
        step: MinecraftAuthStep,
        #[source]
        source: reqwest::Error,
    },
    #[error("waiting for user to complete device authorization")]
    DeviceAuthorizationPending,
    #[error("device authorization polling interval increased; retrying")]
    DeviceAuthorizationSlowDown,
    #[error("device authorization expired before the user signed in")]
    DeviceAuthorizationExpired,
    #[error("device authorization failed: {error}")]
    DeviceAuthorizationFailed { error: String },
    #[error("failed to start the loopback redirect server: {0}")]
    LoopbackBind(String),
    #[error("browser sign-in was cancelled or timed out before completing")]
    BrowserAuthorizationExpired,
    #[error("this browser sign-in is no longer active")]
    BrowserLoginNotFound,
    #[error("browser authorization failed: {error}")]
    BrowserAuthorizationFailed { error: String },
    #[error("failed to read user hash from Xbox token")]
    HashError,
    #[error("Minecraft authentication error {error_code} during MSA step {step:?}: {message}")]
    XboxError {
        step: MinecraftAuthStep,
        error_code: u64,
        message: String,
        redirect: Option<String>,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error(transparent)]
    Minecraft(#[from] MinecraftAuthError),

    #[error(transparent)]
    Request(#[from] RequestError),

    #[error("offline mode requires at least one Microsoft account to be signed in")]
    OfflineRequiresMicrosoft,

    #[error("invalid offline username: {reason}")]
    InvalidOfflineUsername { reason: String },

    #[error("account {0} is not registered")]
    AccountNotFound(uuid::Uuid),

    #[error("account already exists for username {username:?}")]
    DuplicateUsername { username: String },
}
