use thiserror::Error;

use crate::stage::ClusterStage;
use oneclient_common::domain::GameLoader;

pub type ClusterResult<T> = Result<T, ClusterError>;

#[derive(Debug, Error)]
pub enum ClusterError {
	#[error(transparent)]
	Io(#[from] polyio::IOError),

	#[error(transparent)]
	StdIo(#[from] std::io::Error),

	#[error(transparent)]
	Db(#[from] oneclient_db::DbError),

	#[error("Database execution failed: {0}")]
	Sql(#[from] sqlx::Error),

	#[error(transparent)]
	Paths(#[from] oneclient_common::PathsError),

	#[error("Unable to parse JSON: {0}")]
	Json(#[from] serde_json::Error),

	#[error("invalid settings profile: {reason}")]
	InvalidProfile { reason: String },

	#[error(transparent)]
	Logs(#[from] crate::logs::LogsError),

	#[error(transparent)]
	Screenshots(#[from] crate::screenshots::ScreenshotsError),

	#[error(transparent)]
	Request(#[from] oneclient_net::RequestError),

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

	#[error("cluster {0} is already running")]
	AlreadyRunning(i64),

	#[error("cluster name cannot be empty")]
	EmptyName,

	#[error("this version's default instance cannot be renamed or deleted")]
	Provisioned,

	#[error("unknown loader id {0} in database")]
	InvalidLoader(i64),

	#[error("unknown stage id {0} in database")]
	InvalidStage(i64),
}
