use std::{path::PathBuf, time::Duration};

use onelauncher_migration::MigratorTrait;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};

use crate::{utils::io::{self, IOError}, LauncherResult};

use super::Dirs;

pub async fn create_pool_from_path(path: PathBuf) -> LauncherResult<DatabaseConnection> {
	if !path.is_absolute() {
		return Err(IOError::InvalidAbsolutePath(path).into());
	}

	let address = format!("sqlite://{}?mode=rwc", path.to_string_lossy());
	let mut opts = ConnectOptions::new(address);
	opts.min_connections(5)
		.max_connections(40)
		.connect_timeout(Duration::from_secs(5))
		.idle_timeout(Duration::from_secs(6 * 60 * 60 /* 6 hours */));

	let db = Database::connect(opts).await?;

	onelauncher_migration::Migrator::up(&db, None).await?;

	Ok(db)
}

#[tracing::instrument]
pub async fn create_pool() -> LauncherResult<DatabaseConnection> {
	let path = Dirs::get_db_file().await?;
	create_pool_from_path(path).await
}