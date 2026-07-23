use thiserror::Error;

#[derive(Debug, Error)]
pub enum LauncherError {
    #[error("Unable to parse JSON: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Database execution failed: {0}")]
    SqlError(#[from] sqlx::Error),

    #[error(transparent)]
    IoError(#[from] polyio::IOError),

    #[error(transparent)]
    StdIoError(#[from] std::io::Error),

    #[error(transparent)]
    RequestError(#[from] crate::http::RequestError),

    #[error("Could not resolve launcher data directory")]
    DataDirUnavailable,

    #[error(transparent)]
    DbError(#[from] oneclient_db::DbError),

    #[error("launcher core is not initialized")]
    NotInitialized,

    #[error("launcher core is already initialized")]
    AlreadyInitialized,

    #[error("invalid settings profile: {reason}")]
    InvalidSettingsProfile { reason: String },

    #[error(transparent)]
    JavaError(#[from] crate::java::JavaError),

    #[error(transparent)]
    UrlError(#[from] url::ParseError),

    #[error(transparent)]
    NotificationError(#[from] crate::notification::NotificationError),

    #[error(transparent)]
    PackageError(#[from] crate::packages::PackageError),

    #[error(transparent)]
    AuthError(#[from] crate::auth::AuthError),

    #[error(transparent)]
    ClusterError(#[from] crate::clusters::ClusterError),

    #[error(transparent)]
    MetadataError(#[from] crate::metadata::MetadataError),

    #[error(transparent)]
    BundleError(#[from] crate::bundles::BundleError),

    #[error(transparent)]
    GameError(#[from] crate::game::GameError),

    #[error(transparent)]
    LogsError(#[from] crate::logs::LogsError),

    #[error(transparent)]
    ScreenshotsError(#[from] crate::screenshots::ScreenshotsError),

    #[error("minecraft: {0}")]
    Minecraft(String),
}

impl LauncherError {
    #[must_use]
    pub fn auth_guidance(&self) -> Option<crate::auth::AuthErrorGuidance> {
        match self {
            LauncherError::AuthError(crate::auth::AuthError::Minecraft(err)) => {
                crate::auth::diagnose_auth_error(err)
            }
            _ => None,
        }
    }
}

pub trait SentryExclusion {
    /// Whether this error is expected/environmental noise that should be kept out
    /// of Sentry rather than reported as a crash.
    fn is_sentry_excluded(&self) -> bool {
        false
    }
}

impl SentryExclusion for LauncherError {
    fn is_sentry_excluded(&self) -> bool {
        match self {
            LauncherError::StdIoError(e) => e.is_sentry_excluded(),
            LauncherError::IoError(e) => e.is_sentry_excluded(),
            LauncherError::RequestError(e) => e.is_sentry_excluded(),
            LauncherError::JavaError(e) => e.is_sentry_excluded(),
            _ => false,
        }
    }
}

impl SentryExclusion for std::io::Error {
    fn is_sentry_excluded(&self) -> bool {
        use std::io::ErrorKind;

        // Out of disk space. `ErrorKind::StorageFull` is still unstable, so match
        // the raw OS codes instead. Codes are platform-gated so a Unix errno can't
        // collide with a Windows code (112 is the unrelated EHOSTDOWN on Linux).
        if let Some(code) = self.raw_os_error() {
            #[cfg(unix)]
            if code == 28 {
                // ENOSPC
                return true;
            }
            #[cfg(windows)]
            if code == 112 || code == 39 {
                // ERROR_DISK_FULL / ERROR_HANDLE_DISK_FULL
                return true;
            }
            let _ = code;
        }

        // Lost/refused network connections during downloads.
        matches!(
            self.kind(),
            ErrorKind::ConnectionRefused
                | ErrorKind::ConnectionReset
                | ErrorKind::ConnectionAborted
                | ErrorKind::NotConnected
                | ErrorKind::TimedOut
        )
    }
}

impl SentryExclusion for reqwest::Error {
    fn is_sentry_excluded(&self) -> bool {
        // Connectivity problems (offline, connection refused, timed out) are the
        // user's network, not a launcher bug.
        self.is_timeout() || self.is_connect()
    }
}

impl SentryExclusion for polyio::IOError {
    fn is_sentry_excluded(&self) -> bool {
        match self {
            polyio::IOError::IOError(source)
            | polyio::IOError::PathIOError { source, .. } => source.is_sentry_excluded(),
            _ => false,
        }
    }
}

pub type LauncherResult<T> = Result<T, LauncherError>;
