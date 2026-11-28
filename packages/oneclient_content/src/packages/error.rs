use thiserror::Error;

use super::types::PackageBody;
use oneclient_common::domain::ProviderId;

#[derive(Debug, Error)]
pub enum PackageError {
    #[error("no primary download file on version")]
    NoPrimaryFile,
    #[error("{0} can't be downloaded from inside the launcher because its author turned off third-party downloads. Download it from its CurseForge page instead")]
    DownloadBlocked(String),
    #[error("package is not a modpack")]
    NotModpack,
    #[error("unsupported modpack format")]
    UnsupportedModpackFormat,
    #[error("missing API key for provider {0:?}")]
    MissingApiKey(ProviderId),
    #[error("unsupported package body type")]
    UnsupportedBody(PackageBody),
    #[error("package is not compatible with cluster Minecraft version")]
    IncompatibleMcVersion,
    #[error("package is not compatible with cluster loader")]
    IncompatibleLoader,
    #[error("cluster {0} not found")]
    ClusterNotFound(i64),
    #[error("artifact file missing at {0}")]
    ArtifactMissing(String),
    #[error("hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },
    #[error("provider {0:?} does not support this operation")]
    UnsupportedProvider(ProviderId),
    #[error("package provider {0:?} is not registered")]
    ProviderNotRegistered(ProviderId),
    #[error("invalid local file: {0}")]
    InvalidLocalFile(String),
    #[error("Stop mod diagnostics before adding, removing or updating mods in this cluster")]
    DiagnosticsRunning,
}

pub type PackageResult<T> = Result<T, PackageError>;
