use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
	#[error(transparent)]
	Io(#[from] std::io::Error),

	#[error(transparent)]
	Sqlx(#[from] sqlx::Error),

	#[error(transparent)]
	Migrate(#[from] sqlx::migrate::MigrateError),

	#[error("record not found")]
	NotFound,

	#[error("invalid value for {field}: {value}")]
	InvalidValue { field: String, value: String },
}

pub(crate) type DbResult<T> = Result<T, DbError>;
