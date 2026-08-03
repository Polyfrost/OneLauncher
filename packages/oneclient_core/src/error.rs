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
    RequestError(#[from] oneclient_net::RequestError),

    #[error(transparent)]
    PathsError(#[from] oneclient_common::PathsError),

    #[error(transparent)]
    DbError(#[from] oneclient_db::DbError),

    #[error("launcher core is not initialized")]
    NotInitialized,

    #[error("launcher core is already initialized")]
    AlreadyInitialized,

    #[error("invalid settings profile: {reason}")]
    InvalidSettingsProfile { reason: String },

    #[error(transparent)]
    JavaError(#[from] oneclient_java::JavaError),

    #[error(transparent)]
    UrlError(#[from] url::ParseError),

    #[error(transparent)]
    EventError(#[from] oneclient_events::EventError),

    #[error(transparent)]
    PackageError(#[from] oneclient_content::packages::PackageError),

    #[error(transparent)]
    AuthError(#[from] oneclient_auth::AuthError),

    #[error(transparent)]
    ClusterError(#[from] crate::clusters::ClusterError),

    #[error(transparent)]
    McError(#[from] oneclient_mc::McError),

    #[error(transparent)]
    ContentError(#[from] oneclient_content::ContentError),

    #[error(transparent)]
    BundleError(#[from] oneclient_content::bundles::BundleError),

    #[error(transparent)]
    GameError(#[from] crate::game::GameError),

    #[error(transparent)]
    LogsError(#[from] oneclient_cluster::logs::LogsError),

    #[error(transparent)]
    ScreenshotsError(#[from] oneclient_cluster::screenshots::ScreenshotsError),

    #[error("minecraft: {0}")]
    Minecraft(String),
}

impl LauncherError {
    #[must_use]
    pub fn auth_guidance(&self) -> Option<oneclient_auth::AuthErrorGuidance> {
        match self {
            LauncherError::AuthError(oneclient_auth::AuthError::Minecraft(err)) => {
                oneclient_auth::diagnose_auth_error(err)
            }
            _ => None,
        }
    }

    /// Whether this is the user backing out of their own sign-in.
    ///
    /// Worth telling apart because it is not something to report: by the time it
    /// lands the dialog is already gone, and putting a red line under the button
    /// would be complaining about the button the user just pressed.
    #[must_use]
    pub fn is_login_cancelled(&self) -> bool {
        matches!(
            self,
            LauncherError::AuthError(oneclient_auth::AuthError::LoginCancelled)
        )
    }

    /// Whether this failure reads as "the install is missing pieces", and so is
    /// worth repairing rather than just reporting.
    ///
    /// Something like
    /// `An error occurred whilst accessing path '.../metadata/assets': No such
    /// file or directory (os error 2)` is a dead end as a message: it names a
    /// path the user did not create and cannot fix, when the actual answer is
    /// that a file the install needs is not there and should be downloaded
    /// again.
    ///
    /// Deliberately narrow. Only a genuine not-found reaches here: a permission
    /// error, a full disk, or an unreachable network share would all survive a
    /// repair unchanged, and re-downloading the game to rediscover that would
    /// waste a lot of the user's time.
    #[must_use]
    pub fn indicates_missing_files(&self) -> bool {
        let mut source: Option<&(dyn std::error::Error + 'static)> = Some(self);

        // Walk the chain rather than matching the top variant: by the time this
        // surfaces it has usually been wrapped two or three layers deep, and the
        // `io::Error` that actually carries the kind is at the bottom.
        while let Some(err) = source {
            if let Some(io) = err.downcast_ref::<std::io::Error>()
                && io.kind() == std::io::ErrorKind::NotFound
            {
                return true;
            }
            source = err.source();
        }

        false
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
            LauncherError::RequestError(e) => e.is_transient(),
            LauncherError::JavaError(e) => e.is_transient(),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The OS-level "no such file or directory" the filesystem actually
    /// returns, rather than a synthesized `ErrorKind`: they map to the same
    /// kind, but only this one renders the `(os error 2)` text users report.
    fn not_found(path: &str) -> LauncherError {
        LauncherError::IoError(polyio::IOError::PathIOError {
            source: std::io::Error::from_raw_os_error(2),
            path: path.to_string(),
        })
    }

    #[test]
    fn a_missing_asset_directory_asks_for_a_repair() {
        // The exact shape users report: a path deep in the launcher's own
        // metadata that they never created and cannot fix by hand.
        let path =
            "/Users/someone/Library/Application Support/org.Polyfrost.OneClient-dev/metadata/assets";
        let err = not_found(path);

        assert!(err.indicates_missing_files(), "{err}");
        assert!(err.to_string().contains(path), "{err}");
    }

    #[test]
    fn a_not_found_nested_several_layers_deep_is_still_found() {
        // By the time this surfaces it has usually been wrapped on its way up,
        // so matching only the outermost variant would miss it.
        let err = LauncherError::McError(oneclient_mc::McError::Io(polyio::IOError::PathIOError {
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
            path: "metadata/libraries/foo.jar".to_string(),
        }));

        assert!(err.indicates_missing_files(), "{err}");
    }

    #[test]
    fn failures_a_repair_could_not_fix_are_left_alone() {
        // Re-downloading the entire game to rediscover that the disk is full,
        // or that the folder is not writable, would waste a lot of the user's
        // time and end at the same error.
        for kind in [
            std::io::ErrorKind::PermissionDenied,
            std::io::ErrorKind::ConnectionRefused,
            std::io::ErrorKind::TimedOut,
        ] {
            let err = LauncherError::IoError(polyio::IOError::PathIOError {
                source: std::io::Error::from(kind),
                path: "metadata/assets".to_string(),
            });
            assert!(!err.indicates_missing_files(), "{kind:?} should not repair");
        }

        assert!(!LauncherError::NotInitialized.indicates_missing_files());
        assert!(
            !LauncherError::Minecraft("bad manifest".into()).indicates_missing_files()
        );
    }
}
