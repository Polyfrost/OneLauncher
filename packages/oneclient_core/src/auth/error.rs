use serde::Serialize;

use crate::http::RequestError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
    #[error("{message}")]
    XboxError {
        step: MinecraftAuthStep,
        error_code: u64,
        message: String,
        redirect: Option<String>,
    },
    #[error(
        "The sign-in service returned an error (HTTP {status_code}) during step {step:?}. Please wait a moment and try again."
    )]
    ServiceError {
        step: MinecraftAuthStep,
        status_code: reqwest::StatusCode,
    },
}

#[must_use]
pub fn friendly_xbox_error(code: u64) -> Option<&'static str> {
    Some(match code {
        2_148_916_227 => "This account has been banned or suspended from Xbox.",
        2_148_916_229 => "This account is a child account that must be added to a Family group by an adult before signing in.",
        2_148_916_233 => "This Microsoft account does not have an Xbox profile yet. Create one at xbox.com, then try again.",
        2_148_916_234 => "This account has not accepted the Xbox Terms of Service. Sign in at xbox.com to accept them first.",
        2_148_916_235 => "Xbox Live is not available in your country or region, so sign-in is blocked.",
        2_148_916_236 | 2_148_916_237 => "This account requires adult verification (South Korea) before it can sign in.",
        2_148_916_238 => "This is a child account. An adult must add it to a Microsoft Family group before it can sign in.",
        _ => return None,
    })
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
