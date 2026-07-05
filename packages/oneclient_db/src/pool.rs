use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;
use tracing::instrument;

use crate::DbError;

pub type DbPool = SqlitePool;

#[instrument(
    skip(database_path), 
    fields(database_path = %database_path.as_ref().display())
)]
pub async fn connect(database_path: impl AsRef<Path>) -> Result<DbPool, DbError> {
    let database_path = database_path.as_ref();
    if let Some(parent) = database_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let options = SqliteConnectOptions::from_str(&format!(
        "sqlite://{}?mode=rwc",
        dunce::canonicalize(database_path)
            .unwrap_or_else(|_| database_path.to_path_buf())
            .display()
    ))?
    .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?;

    sqlx::migrate!().run(&pool).await?;

    tracing::info!("database ready");

    Ok(pool)
}
