use thiserror::Error;

use super::stage::ClusterStage;
use crate::packages::domain::GameLoader;

#[derive(Debug, Error)]
pub enum ClusterError {
	#[error("cluster {0} not found")]
	NotFound(i64),

	#[error("cluster folder '{0}' not found on disk")]
	FolderNotFound(String),

	#[error("cluster is in {0} stage")]
	Busy(ClusterStage),

    #[error("invalid Minecraft version '{0}'")]
    InvalidVersion(String),

    #[error("cluster is missing a required Java version")]
    MissingJavaVersion,

	#[error("loader {0} is not compatible with {1}")]
	MismatchedLoader(GameLoader, GameLoader),

	#[error("setting profile '{0}' not found")]
	ProfileNotFound(String),

	#[error("cluster has no settings profile assigned")]
	NoProfile,

	#[error("cluster name is empty after sanitization")]
	EmptyName,

	#[error("unknown loader id {0} in database")]
	InvalidLoader(i64),

	#[error("unknown stage id {0} in database")]
	InvalidStage(i64),
}
