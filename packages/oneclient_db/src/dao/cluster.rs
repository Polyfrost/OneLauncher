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
