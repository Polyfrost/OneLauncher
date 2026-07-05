use thiserror::Error;

#[derive(Debug, Error)]
pub enum LogsError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("invalid log file name: {0}")]
    InvalidName(String),

    #[error("log file is too large to upload")]
    TooLarge,

    #[error(transparent)]
    Request(#[from] crate::http::RequestError),

    #[error("mclo.gs upload failed: {0}")]
    Upload(String),
}
