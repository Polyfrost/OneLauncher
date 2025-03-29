use std::path::PathBuf;

use chrono::Duration;
use sqlx::{migrate::MigrateDatabase, sqlite::SqlitePoolOptions, Sqlite, SqlitePool};

use crate::LauncherResult;

use super::Dirs;

pub async fn create_pool_from_path(path: PathBuf) -> LauncherResult<SqlitePool> {
	let protocol = String::from("sqlite:");
	let connection_str = protocol + path.to_str().expect("invalid path used for sqlite connection");

	if !Sqlite::database_exists(&connection_str).await? {

		Sqlite::create_database(&connection_str).await?;
	}

	let pool = SqlitePoolOptions::new()
		.max_connections(40)
		.idle_timeout(Some(Duration::hours(6).to_std().unwrap()))
		.connect(&connection_str).await?;

	sqlx::migrate!().run(&pool).await?;

	Ok(pool)
}

pub async fn create_pool() -> LauncherResult<SqlitePool> {
	let path = Dirs::get().await?.db_file();
	create_pool_from_path(path).await
}