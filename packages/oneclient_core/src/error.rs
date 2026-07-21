use thiserror::Error;

#[derive(Debug, Error)]
pub enum LauncherError {
    #[error("Unable to parse JSON: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Database execution failed: {0}")]
    DatabaseError(#[from] sqlx::Error),

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

pub type LauncherResult<T> = Result<T, LauncherError>;
