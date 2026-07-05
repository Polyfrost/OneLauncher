use thiserror::Error;

use crate::packages::domain::GameLoader;

#[derive(Debug, Error)]
pub enum MetadataError {
    #[error("failed to fetch metadata")]
    FetchError,

    #[error("loader {0} does not use a modded manifest")]
    NotModdedManifest(GameLoader),

    #[error("loader {0} does not use a vanilla manifest")]
    NotVanillaManifest(GameLoader),

    #[error("failed to parse metadata: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("no matching loader found")]
    NoMatchingLoader,

    #[error("requested loader version '{requested}' was not found")]
    RequestedLoaderVersionNotFound { requested: String },

    #[error("no matching version found")]
    NoMatchingVersion,
}
