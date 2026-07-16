use chrono::Utc;
use sqlx::SqlitePool;

use crate::models::{ClusterPatch, ClusterRow, NewCluster};

pub async fn get_by_id(pool: &SqlitePool, id: i64) -> Result<Option<ClusterRow>, sqlx::Error> {
	sqlx::query_as!(
		ClusterRow,
		r#"
		SELECT
			id, name, folder_name, setting_profile_name, mc_version, mc_loader,
			stage, mc_loader_version, created_at, last_played, overall_played, linked_modpack_hash
		FROM clusters
		WHERE id = ?
		"#,
		id
	)
	.fetch_optional(pool)
	.await
}

pub async fn get_by_folder_name(
	pool: &SqlitePool,
	folder_name: &str,
) -> Result<Option<ClusterRow>, sqlx::Error> {
	sqlx::query_as!(
		ClusterRow,
		r#"
		SELECT
			id, name, folder_name, setting_profile_name, mc_version, mc_loader,
			stage, mc_loader_version, created_at, last_played, overall_played, linked_modpack_hash
		FROM clusters
		WHERE folder_name = ?
		"#,
		folder_name
	)
	.fetch_optional(pool)
	.await
}

pub async fn list_all(pool: &SqlitePool) -> Result<Vec<ClusterRow>, sqlx::Error> {
	sqlx::query_as!(
		ClusterRow,
		r#"
		SELECT
			id, name, folder_name, setting_profile_name, mc_version, mc_loader,
			stage, mc_loader_version, created_at, last_played, overall_played, linked_modpack_hash
		FROM clusters
		ORDER BY last_played IS NULL, last_played DESC, name ASC
		"#
	)
	.fetch_all(pool)
	.await
}

pub async fn find_by_version_loader(
	pool: &SqlitePool,
	mc_version: &str,
	mc_loader: i64,
) -> Result<Option<ClusterRow>, sqlx::Error> {
	sqlx::query_as!(
		ClusterRow,
		r#"
		SELECT
			id, name, folder_name, setting_profile_name, mc_version, mc_loader,
			stage, mc_loader_version, created_at, last_played, overall_played, linked_modpack_hash
		FROM clusters
		WHERE mc_version = ? AND mc_loader = ?
		LIMIT 1
		"#,
		mc_version,
		mc_loader
	)
	.fetch_optional(pool)
	.await
}

pub async fn insert(pool: &SqlitePool, new: &NewCluster<'_>) -> Result<ClusterRow, sqlx::Error> {
	let created_at = Utc::now().to_rfc3339();

	sqlx::query_as!(
		ClusterRow,
		r#"
		INSERT INTO clusters (
			name, folder_name, mc_version, mc_loader, mc_loader_version,
			setting_profile_name, stage, created_at
		)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?)
		RETURNING
			id, name, folder_name, setting_profile_name, mc_version, mc_loader,
			stage, mc_loader_version, created_at, last_played, overall_played, linked_modpack_hash
		"#,
		new.name,
		new.folder_name,
		new.mc_version,
		new.mc_loader,
		new.mc_loader_version,
		new.setting_profile_name,
		new.stage,
		created_at
	)
	.fetch_one(pool)
	.await
}

pub async fn update(
	pool: &SqlitePool,
	id: i64,
	patch: &ClusterPatch,
) -> Result<ClusterRow, sqlx::Error> {
	let existing = get_by_id(pool, id)
		.await?
		.ok_or(sqlx::Error::RowNotFound)?;

	let name = patch.name.as_deref().unwrap_or(&existing.name);
	let setting_profile_name = patch
		.setting_profile_name
		.clone()
		.unwrap_or(existing.setting_profile_name);
	let mc_loader_version = patch
		.mc_loader_version
		.clone()
		.unwrap_or(existing.mc_loader_version);
	let linked_modpack_hash = patch
		.linked_modpack_hash
		.clone()
		.unwrap_or(existing.linked_modpack_hash);

	sqlx::query_as!(
		ClusterRow,
		r#"
		UPDATE clusters
		SET name = ?,
		    setting_profile_name = ?,
		    mc_loader_version = ?,
		    linked_modpack_hash = ?
		WHERE id = ?
		RETURNING
			id, name, folder_name, setting_profile_name, mc_version, mc_loader,
			stage, mc_loader_version, created_at, last_played, overall_played, linked_modpack_hash
		"#,
		name,
		setting_profile_name,
		mc_loader_version,
		linked_modpack_hash,
		id
	)
	.fetch_one(pool)
	.await
}

pub async fn migrate_version(
	pool: &SqlitePool,
	id: i64,
	mc_version: &str,
	name: Option<&str>,
	folder_name: &str,
) -> Result<ClusterRow, sqlx::Error> {
	let existing = get_by_id(pool, id)
		.await?
		.ok_or(sqlx::Error::RowNotFound)?;

	let name = name.unwrap_or(&existing.name);

	sqlx::query_as!(
		ClusterRow,
		r#"
		UPDATE clusters
		SET mc_version = ?,
		    name = ?,
		    folder_name = ?
		WHERE id = ?
		RETURNING
			id, name, folder_name, setting_profile_name, mc_version, mc_loader,
			stage, mc_loader_version, created_at, last_played, overall_played, linked_modpack_hash
		"#,
		mc_version,
		name,
		folder_name,
		id
	)
	.fetch_one(pool)
	.await
}

pub async fn set_stage(pool: &SqlitePool, id: i64, stage: i64) -> Result<ClusterRow, sqlx::Error> {
	sqlx::query_as!(
		ClusterRow,
		r#"
		UPDATE clusters
		SET stage = ?
		WHERE id = ?
		RETURNING
			id, name, folder_name, setting_profile_name, mc_version, mc_loader,
			stage, mc_loader_version, created_at, last_played, overall_played, linked_modpack_hash
		"#,
		stage,
		id
	)
	.fetch_one(pool)
	.await
}

pub async fn add_playtime(
	pool: &SqlitePool,
	id: i64,
	seconds: i64,
) -> Result<ClusterRow, sqlx::Error> {
	let now = Utc::now().to_rfc3339();

	sqlx::query_as!(
		ClusterRow,
		r#"
		UPDATE clusters
		SET overall_played = COALESCE(overall_played, 0) + ?,
		    last_played = ?
		WHERE id = ?
		RETURNING
			id, name, folder_name, setting_profile_name, mc_version, mc_loader,
			stage, mc_loader_version, created_at, last_played, overall_played, linked_modpack_hash
		"#,
		seconds,
		now,
		id
	)
	.fetch_one(pool)
	.await
}

pub async fn delete_by_id(pool: &SqlitePool, id: i64) -> Result<bool, sqlx::Error> {
	let result = sqlx::query!("DELETE FROM clusters WHERE id = ?", id)
		.execute(pool)
		.await?;

	Ok(result.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::dao::applied_migration;

	async fn pool() -> SqlitePool {
		let pool = SqlitePool::connect("sqlite::memory:")
			.await
			.expect("in-memory sqlite");
		sqlx::migrate!().run(&pool).await.expect("migrations run");
		pool
	}

	async fn seed(pool: &SqlitePool, mc_version: &str, folder: &str) -> ClusterRow {
		insert(
			pool,
			&NewCluster {
				name: &format!("{mc_version} fabric"),
				folder_name: folder,
				mc_version,
				mc_loader: 1,
				mc_loader_version: None,
				setting_profile_name: None,
				stage: 0,
			},
		)
		.await
		.expect("insert cluster")
	}

	#[tokio::test]
	async fn migrate_version_rewrites_identity_and_keeps_history() {
		let pool = pool().await;
		let cluster = seed(&pool, "26.1", "26.1 fabric").await;
		add_playtime(&pool, cluster.id, 3600)
			.await
			.expect("record playtime");

		let migrated = migrate_version(
			&pool,
			cluster.id,
			"26.1.2",
			Some("26.1.2 fabric"),
			"26.1.2 fabric",
		)
		.await
		.expect("migrate");

		assert_eq!(migrated.id, cluster.id, "must move the row, not replace it");
		assert_eq!(migrated.mc_version, "26.1.2");
		assert_eq!(migrated.folder_name, "26.1.2 fabric");
		assert_eq!(migrated.name, "26.1.2 fabric");
		assert_eq!(migrated.overall_played, Some(3600));
		assert_eq!(migrated.created_at, cluster.created_at);

		assert!(
			find_by_version_loader(&pool, "26.1", 1)
				.await
				.unwrap()
				.is_none()
		);
		assert_eq!(
			find_by_version_loader(&pool, "26.1.2", 1)
				.await
				.unwrap()
				.unwrap()
				.id,
			cluster.id
		);
	}

	#[tokio::test]
	async fn migrate_version_can_leave_name_and_folder_untouched() {
		let pool = pool().await;
		let cluster = seed(&pool, "26.1", "my cool pack").await;

		let migrated = migrate_version(&pool, cluster.id, "26.1.2", None, "my cool pack")
			.await
			.expect("migrate");

		assert_eq!(migrated.mc_version, "26.1.2");
		assert_eq!(migrated.name, "26.1 fabric", "custom name must survive");
		assert_eq!(migrated.folder_name, "my cool pack");
	}

	#[tokio::test]
	async fn applied_migrations_ledger_is_idempotent() {
		let pool = pool().await;

		assert!(!applied_migration::is_applied(&pool, "rule-a").await.unwrap());
		applied_migration::mark_applied(&pool, "rule-a").await.unwrap();
		assert!(applied_migration::is_applied(&pool, "rule-a").await.unwrap());

		applied_migration::mark_applied(&pool, "rule-a").await.unwrap();
		assert!(applied_migration::is_applied(&pool, "rule-a").await.unwrap());
		assert!(!applied_migration::is_applied(&pool, "rule-b").await.unwrap());
	}
}
