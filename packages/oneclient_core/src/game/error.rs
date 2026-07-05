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

    #[error("invalid game version {0}")]
    InvalidVersion(String),

    #[error("cluster {0} is already running")]
    AlreadyRunning(i64),

    #[error("failed to spawn the game process: {0}")]
    Spawn(String),
}
