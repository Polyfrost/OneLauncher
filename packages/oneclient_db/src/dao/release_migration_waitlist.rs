use sqlx::SqlitePool;

use crate::models::ReleaseMigrationWaitlistRow;

#[allow(clippy::too_many_arguments)]
pub async fn upsert(
	pool: &SqlitePool,
	target_cluster_id: i64,
	provider: i64,
	project_id: &str,
	content_type: i64,
	source_hash: &str,
	display_name: &str,
	added_at: &str,
	expires_at: &str,
) -> Result<(), sqlx::Error> {
	sqlx::query(
		r"
		INSERT INTO release_migration_waitlist (
			target_cluster_id, provider, project_id, content_type,
			source_hash, display_name, added_at, expires_at
		)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(target_cluster_id, provider, project_id) DO UPDATE SET
			content_type = excluded.content_type,
			source_hash = excluded.source_hash,
			display_name = excluded.display_name,
			added_at = excluded.added_at,
			expires_at = excluded.expires_at
		",
	)
	.bind(target_cluster_id)
	.bind(provider)
	.bind(project_id)
	.bind(content_type)
	.bind(source_hash)
	.bind(display_name)
	.bind(added_at)
	.bind(expires_at)
	.execute(pool)
	.await?;
	Ok(())
}

pub async fn list_all(pool: &SqlitePool) -> Result<Vec<ReleaseMigrationWaitlistRow>, sqlx::Error> {
	sqlx::query_as::<_, ReleaseMigrationWaitlistRow>(
		r"
		SELECT
			target_cluster_id, provider, project_id, content_type,
			source_hash, display_name, added_at, expires_at
		FROM release_migration_waitlist
		",
	)
	.fetch_all(pool)
	.await
}

pub async fn delete(
	pool: &SqlitePool,
	target_cluster_id: i64,
	provider: i64,
	project_id: &str,
) -> Result<(), sqlx::Error> {
	sqlx::query(
		"DELETE FROM release_migration_waitlist WHERE target_cluster_id = ? AND provider = ? AND project_id = ?",
	)
	.bind(target_cluster_id)
	.bind(provider)
	.bind(project_id)
	.execute(pool)
	.await?;
	Ok(())
}

pub async fn delete_expired(pool: &SqlitePool, now: &str) -> Result<u64, sqlx::Error> {
	let result = sqlx::query("DELETE FROM release_migration_waitlist WHERE expires_at <= ?")
		.bind(now)
		.execute(pool)
		.await?;
	Ok(result.rows_affected())
}
