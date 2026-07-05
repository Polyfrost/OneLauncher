use thiserror::Error;

use super::domain::ProviderId;
use super::types::PackageBody;

#[derive(Debug, Error)]
pub enum PackageError {
	#[error("no primary download file on version")]
	NoPrimaryFile,
	#[error("package is not a modpack")]
	NotModpack,
	#[error("unsupported modpack format")]
	UnsupportedModpackFormat,
	#[error("modpack install had {failed} of {total} file failures")]
	PartialModpackInstall { failed: u64, total: u64 },
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
}

pub type PackageResult<T> = Result<T, PackageError>;
