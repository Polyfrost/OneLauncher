use std::path::Path;

use sqlx::migrate::{MigrateError, Migrator};

use crate::{DbError, DbPool, backup};

const MAX_ATTEMPTS: u32 = 3;

pub async fn run(pool: &DbPool, database_path: &Path) -> Result<(), DbError> {
    let mut migrator = sqlx::migrate!();
    migrator.set_ignore_missing(true);

    if has_pending(pool, &migrator).await {
        match backup::snapshot(pool, database_path).await {
            Ok(path) => {
                tracing::info!(path = %path.display(), "snapshotted the database before migrating");
                backup::prune(database_path, backup::KEEP);
            }
            // A migration that then succeeds is the common case, so a failed
            // snapshot is not worth refusing to start over
            Err(err) => tracing::warn!(%err, "could not snapshot the database before migrating"),
        }
    }

    let mut attempt = 1;
    loop {
        let error = match migrator.run(pool).await {
            Ok(()) => return Ok(()),
            Err(error) => error,
        };

        if attempt >= MAX_ATTEMPTS || !matches!(error, MigrateError::ExecuteMigration(..)) {
            return Err(error.into());
        }

        tracing::warn!(attempt, %error, "migration failed, retrying");
        attempt += 1;
    }
}

/// False on a fresh database, where there is nothing yet worth snapshotting
async fn has_pending(pool: &DbPool, migrator: &Migrator) -> bool {
    let applied: Vec<i64> = sqlx::query_scalar("SELECT version FROM _sqlx_migrations")
        .fetch_all(pool)
        .await
        .unwrap_or_default();

    !applied.is_empty() && migrator.iter().any(|m| !applied.contains(&m.version))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_tempfile::TempDir;
    use sqlx::SqlitePool;

    async fn migrated() -> (TempDir, std::path::PathBuf, SqlitePool) {
        let dir = TempDir::new().await.expect("tempdir");
        let path = dir.dir_path().join("user_data.db");
        let pool = SqlitePool::connect(&format!("sqlite://{}?mode=rwc", path.display()))
            .await
            .expect("open sqlite");

        run(&pool, &path).await.expect("migrations run");
        (dir, path, pool)
    }

    #[tokio::test]
    async fn migrates_a_fresh_database() {
        let (_dir, _path, pool) = migrated().await;

        let column: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM pragma_table_info('setting_profiles') WHERE name = ?")
                .bind("browser_update_mode")
                .fetch_optional(&pool)
                .await
                .expect("probe");

        assert!(column.is_some());
    }

    #[tokio::test]
    async fn running_twice_is_a_no_op() {
        let (_dir, path, pool) = migrated().await;
        run(&pool, &path).await.expect("second run");
    }

    #[tokio::test]
    async fn tolerates_a_migration_this_build_does_not_ship() {
        let (_dir, path, pool) = migrated().await;

        sqlx::query(
            "INSERT INTO _sqlx_migrations
             (version, description, success, checksum, execution_time)
             VALUES (?, 'from a newer build', TRUE, x'00', -1)",
        )
        .bind(20990101120000i64)
        .execute(&pool)
        .await
        .expect("record a migration from the future");

        run(&pool, &path).await.expect("a downgrade must still start");
    }

    #[tokio::test]
    async fn a_fresh_database_is_not_snapshotted() {
        let (_dir, path, _pool) = migrated().await;
        assert!(backup::list(&path).is_empty());
    }

    #[tokio::test]
    async fn nothing_outstanding_once_migrated() {
        let (_dir, _path, pool) = migrated().await;
        assert!(!has_pending(&pool, &sqlx::migrate!()).await);
    }

    #[tokio::test]
    async fn an_unapplied_migration_is_outstanding() {
        let (_dir, _path, pool) = migrated().await;

        let latest: i64 = sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .expect("latest version");
        sqlx::query("DELETE FROM _sqlx_migrations WHERE version = ?")
            .bind(latest)
            .execute(&pool)
            .await
            .expect("forget it");

        assert!(has_pending(&pool, &sqlx::migrate!()).await);
    }
}
