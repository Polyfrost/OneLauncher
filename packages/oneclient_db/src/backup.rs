//! Snapshots taken before a migration

use std::path::{Path, PathBuf};

use crate::{DbError, DbPool};

pub const KEEP: usize = 5;

const SIDECARS: &[&str] = &["-wal", "-shm", "-journal"];

pub fn dir(database_path: &Path) -> PathBuf {
    database_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("backups")
}

fn stem(database_path: &Path) -> String {
    database_path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "database".to_owned())
}

fn timestamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

/// A consistent copy without blocking on the journal, unlike a file copy
pub async fn snapshot(pool: &DbPool, database_path: &Path) -> Result<PathBuf, DbError> {
    let dir = dir(database_path);
    std::fs::create_dir_all(&dir)?;

    let target = dir.join(format!("{}-{}.db", stem(database_path), timestamp()));

    sqlx::query("VACUUM INTO ?")
        .bind(target.to_string_lossy().as_ref())
        .execute(pool)
        .await?;

    Ok(target)
}

/// Newest first
pub fn list(database_path: &Path) -> Vec<PathBuf> {
    let prefix = format!("{}-", stem(database_path));

    let Ok(entries) = std::fs::read_dir(dir(database_path)) else {
        return Vec::new();
    };

    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.ends_with(".db")
                        && name.strip_prefix(&prefix).is_some_and(|rest| {
                            rest.starts_with(|c: char| c.is_ascii_digit())
                        })
                })
        })
        .collect();

    found.sort();
    found.reverse();
    found
}

pub fn prune(database_path: &Path, keep: usize) {
    for old in list(database_path).into_iter().skip(keep) {
        if let Err(err) = std::fs::remove_file(&old) {
            tracing::warn!(path = %old.display(), %err, "could not remove old snapshot");
        }
    }
}

/// Moves the live database into `backups/` and returns where it went. The
/// caller is left with no database at `database_path`, which is what both
/// restore and reset want before they continue.
pub fn set_aside(database_path: &Path) -> Result<Option<PathBuf>, DbError> {
    if !database_path.exists() {
        return Ok(None);
    }

    let dir = dir(database_path);
    std::fs::create_dir_all(&dir)?;

    let target = dir.join(format!(
        "{}-replaced-{}.db",
        stem(database_path),
        timestamp()
    ));
    std::fs::rename(database_path, &target)?;

    for suffix in SIDECARS {
        let sidecar = sidecar(database_path, suffix);
        if sidecar.exists() {
            let _ = std::fs::remove_file(sidecar);
        }
    }

    Ok(Some(target))
}

fn sidecar(database_path: &Path, suffix: &str) -> PathBuf {
    let mut name = database_path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

/// Puts `snapshot` back, keeping the current database in `backups/` first
pub fn restore(database_path: &Path, snapshot: &Path) -> Result<Option<PathBuf>, DbError> {
    let replaced = set_aside(database_path)?;
    std::fs::copy(snapshot, database_path)?;

    tracing::info!(
        snapshot = %snapshot.display(),
        "restored the database from a snapshot"
    );

    Ok(replaced)
}

/// Starts over. The old database is moved aside, never deleted, so it is
/// recoverable by hand afterwards.
pub fn reset(database_path: &Path) -> Result<Option<PathBuf>, DbError> {
    let replaced = set_aside(database_path)?;

    tracing::warn!(
        replaced = ?replaced.as_ref().map(|path| path.display().to_string()),
        "reset the database"
    );

    Ok(replaced)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_tempfile::TempDir;
    use sqlx::SqlitePool;

    async fn pool_at(path: &Path) -> SqlitePool {
        SqlitePool::connect(&format!("sqlite://{}?mode=rwc", path.display()))
            .await
            .expect("open sqlite")
    }

    #[tokio::test]
    async fn a_snapshot_is_a_usable_database() {
        let dir = TempDir::new().await.expect("tempdir");
        let db = dir.dir_path().join("user_data.db");

        let pool = pool_at(&db).await;
        sqlx::query("CREATE TABLE demo (id INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await
            .expect("create");

        let snapshot = snapshot(&pool, &db).await.expect("snapshot");
        assert!(snapshot.exists());

        let restored = pool_at(&snapshot).await;
        let found: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM sqlite_master WHERE name = 'demo'")
                .fetch_optional(&restored)
                .await
                .expect("probe");
        assert!(found.is_some());
    }

    #[tokio::test]
    async fn restoring_keeps_the_replaced_database() {
        let dir = TempDir::new().await.expect("tempdir");
        let db = dir.dir_path().join("user_data.db");

        let pool = pool_at(&db).await;
        sqlx::query("CREATE TABLE before_snapshot (id INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await
            .expect("create");
        let snapshot = snapshot(&pool, &db).await.expect("snapshot");

        sqlx::query("CREATE TABLE after_snapshot (id INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await
            .expect("create");
        pool.close().await;

        let replaced = restore(&db, &snapshot).expect("restore").expect("moved");

        let now = pool_at(&db).await;
        let after: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM sqlite_master WHERE name = 'after_snapshot'")
                .fetch_optional(&now)
                .await
                .expect("probe");
        assert!(after.is_none(), "the snapshot should be what is live now");

        let kept = pool_at(&replaced).await;
        let after: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM sqlite_master WHERE name = 'after_snapshot'")
                .fetch_optional(&kept)
                .await
                .expect("probe");
        assert!(after.is_some(), "the replaced database must still be intact");
    }

    #[tokio::test]
    async fn a_reset_moves_the_database_rather_than_deleting_it() {
        let dir = TempDir::new().await.expect("tempdir");
        let db = dir.dir_path().join("user_data.db");

        let pool = pool_at(&db).await;
        sqlx::query("CREATE TABLE demo (id INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await
            .expect("create");
        pool.close().await;

        let moved = reset(&db).expect("reset").expect("moved");

        assert!(!db.exists());
        assert!(moved.exists());
    }

    #[tokio::test]
    async fn a_set_aside_file_is_not_offered_as_a_snapshot() {
        let dir = TempDir::new().await.expect("tempdir");
        let db = dir.dir_path().join("user_data.db");

        let pool = pool_at(&db).await;
        sqlx::query("CREATE TABLE demo (id INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await
            .expect("create");
        let snapshot = snapshot(&pool, &db).await.expect("snapshot");
        pool.close().await;

        reset(&db).expect("reset");

        assert_eq!(list(&db), vec![snapshot]);
    }

    #[tokio::test]
    async fn pruning_keeps_the_newest() {
        let dir = TempDir::new().await.expect("tempdir");
        let db = dir.dir_path().join("user_data.db");
        let backups = self::dir(&db);
        std::fs::create_dir_all(&backups).expect("mkdir");

        for stamp in ["20260101-000000", "20260102-000000", "20260103-000000"] {
            std::fs::write(backups.join(format!("user_data-{stamp}.db")), b"").expect("write");
        }

        prune(&db, 2);

        let left = list(&db);
        assert_eq!(left.len(), 2);
        assert!(
            left[0].ends_with("user_data-20260103-000000.db"),
            "the newest must survive"
        );
    }
}
