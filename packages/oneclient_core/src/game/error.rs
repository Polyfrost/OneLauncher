use thiserror::Error;

#[derive(Debug, Error)]
pub enum GameError {
    #[error("no client download exists for version {0}")]
    NoClientDownload(String),

    #[error("failed to resolve library artifact path for {0}")]
    LibraryPath(String),

    #[error("download hash mismatch for {path}: expected {expected}, got {actual}")]
    HashMismatch {
        path: String,
        expected: String,
        actual: String,
    },

    #[error("cluster is missing a required Java version")]
    MissingJavaVersion,

    #[error("forge processor failed: {0}")]
    ProcessorFailed(String),

    #[error("processor main class not found for {0}")]
    ProcessorMainClass(String),

    #[error("cluster {0} is already running")]
    AlreadyRunning(i64),

    #[error("another cluster ({0}) is already running in the same directory")]
    DirectoryInUse(i64),

    #[error(
        "{0} is already running in the shared game directory; close it before launching another cluster"
    )]
    SharedDirectoryBusy(String),

    #[error("failed to spawn the game process: {0}")]
    Spawn(String),

    #[error(
        "the wrapper command '{program}' could not be started: {reason}. Check the Wrapper Command setting."
    )]
    WrapperSpawn { program: String, reason: String },
}

impl GameError {
    pub(crate) fn wrapper_spawn(program: &str, err: &std::io::Error) -> Self {
        let reason = match err.kind() {
            std::io::ErrorKind::NotFound => "command not found".to_string(),
            std::io::ErrorKind::PermissionDenied => {
                "insufficient permissions or file is not an executable".to_string()
            }
            _ => err.to_string(),
        };

        Self::WrapperSpawn {
            program: program.to_string(),
            reason,
        }
    }
}
