use thiserror::Error;

#[derive(Debug, Error)]
pub enum BundleError {
    #[error("unknown loader '{0}' in bundles manifest")]
    UnknownLoader(String),

    #[error("invalid loader value '{0}' in bundles manifest")]
    InvalidLoader(i64),

    #[error("failed to parse polymrpack manifest")]
    InvalidManifest,

    #[error("bundle file missing at {0}")]
    MissingFile(String),

    #[error("bundle not found: {0}")]
    NotFound(String),
}
