use sqlx::SqlitePool;

use crate::models::{
	ArtifactRow, ClusterArtifactRow, LinkedArtifactRow, ProviderReleaseRow, SeenStatus,
};

pub async fn get_artifact_by_hash(
	pool: &SqlitePool,
	hash: &str,
) -> Result<Option<ArtifactRow>, sqlx::Error> {
	sqlx::query_as!(
		ArtifactRow,
		r#"
		SELECT hash, content_type, path, file_name, size_bytes
		FROM artifacts
		WHERE hash = ?
		"#,
		hash
	)
	.fetch_optional(pool)
	.await
}

pub async fn insert_artifact(
	pool: &SqlitePool,
	hash: &str,
	content_type: i64,
	path: &str,
	file_name: &str,
	size_bytes: Option<i64>,
) -> Result<ArtifactRow, sqlx::Error> {
	sqlx::query_as!(
		ArtifactRow,
		r#"
		INSERT INTO artifacts (hash, content_type, path, file_name, size_bytes)
		VALUES (?, ?, ?, ?, ?)
		ON CONFLICT(hash) DO UPDATE SET
			content_type = excluded.content_type,
			path = excluded.path,
			file_name = excluded.file_name,
			size_bytes = COALESCE(excluded.size_bytes, artifacts.size_bytes)
		RETURNING hash, content_type, path, file_name, size_bytes
		"#,
		hash,
		content_type,
		path,
		file_name,
		size_bytes
	)
	.fetch_one(pool)
	.await
}

/// `provider_releases` cascades the cached file is the caller's to delete
/// this layer does not touch the disk
pub async fn delete_artifact_if_unused(pool: &SqlitePool, hash: &str) -> Result<bool, sqlx::Error> {
	let linked: (i64,) = sqlx::query_as(
		"SELECT COUNT(*) FROM cluster_artifacts WHERE hash = ?",
	)
	.bind(hash)
	.fetch_one(pool)
	.await?;

	if linked.0 > 0 {
		return Ok(false);
	}

	sqlx::query!("DELETE FROM artifacts WHERE hash = ?", hash)
		.execute(pool)
		.await?;

	Ok(true)
}

pub async fn list_unused_artifacts(pool: &SqlitePool) -> Result<Vec<ArtifactRow>, sqlx::Error> {
	sqlx::query_as::<_, ArtifactRow>(
		r#"
		SELECT hash, content_type, path, file_name, size_bytes
		FROM artifacts
		WHERE hash NOT IN (SELECT hash FROM cluster_artifacts)
		"#,
	)
	.fetch_all(pool)
	.await
}

pub async fn list_artifact_paths(pool: &SqlitePool) -> Result<Vec<String>, sqlx::Error> {
	let rows: Vec<(String,)> = sqlx::query_as("SELECT path FROM artifacts")
		.fetch_all(pool)
		.await?;

	Ok(rows.into_iter().map(|(path,)| path).collect())
}

#[allow(clippy::too_many_arguments)]
pub async fn upsert_provider_release(
	pool: &SqlitePool,
	provider_id: i64,
	project_id: &str,
	version_id: &str,
	hash: &str,
	display_name: &str,
	display_version: &str,
	published_at: Option<&str>,
	mc_versions: &str,
	mc_loaders: &str,
) -> Result<ProviderReleaseRow, sqlx::Error> {
	sqlx::query_as!(
		ProviderReleaseRow,
		r#"
		INSERT INTO provider_releases (
			provider, project_id, version_id, hash,
			display_name, display_version, published_at, mc_versions, mc_loaders
		)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(provider, project_id, version_id) DO UPDATE SET
			hash = excluded.hash,
			display_name = excluded.display_name,
			display_version = excluded.display_version,
			published_at = excluded.published_at,
			mc_versions = excluded.mc_versions,
			mc_loaders = excluded.mc_loaders
		RETURNING
			provider as "provider!: i64", project_id, version_id, hash,
			display_name, display_version, published_at, mc_versions, mc_loaders
		"#,
		provider_id,
		project_id,
		version_id,
		hash,
		display_name,
		display_version,
		published_at,
		mc_versions,
		mc_loaders
	)
	.fetch_one(pool)
	.await
}

pub async fn get_provider_release(
	pool: &SqlitePool,
	provider_id: i64,
	project_id: &str,
	version_id: &str,
) -> Result<Option<ProviderReleaseRow>, sqlx::Error> {
	sqlx::query_as!(
		ProviderReleaseRow,
		r#"
		SELECT
			provider as "provider!: i64", project_id, version_id, hash,
			display_name, display_version, published_at, mc_versions, mc_loaders
		FROM provider_releases
		WHERE provider = ? AND project_id = ? AND version_id = ?
		"#,
		provider_id,
		project_id,
		version_id
	)
	.fetch_optional(pool)
	.await
}

pub async fn get_release_by_hash(
	pool: &SqlitePool,
	hash: &str,
) -> Result<Option<ProviderReleaseRow>, sqlx::Error> {
	sqlx::query_as!(
		ProviderReleaseRow,
		r#"
		SELECT
			provider as "provider!: i64", project_id, version_id, hash,
			display_name, display_version, published_at, mc_versions, mc_loaders
		FROM provider_releases
		WHERE hash = ?
		LIMIT 1
		"#,
		hash
	)
	.fetch_optional(pool)
	.await
}

pub async fn link_cluster_artifact(
	pool: &SqlitePool,
	cluster_id: i64,
	hash: &str,
	cluster_file_name: &str,
) -> Result<ClusterArtifactRow, sqlx::Error> {
	sqlx::query_as!(
		ClusterArtifactRow,
		r#"
		INSERT INTO cluster_artifacts (cluster_id, hash, cluster_file_name, enabled)
		VALUES (?, ?, ?, 1)
		ON CONFLICT(cluster_id, hash) DO UPDATE SET
			cluster_file_name = excluded.cluster_file_name
		RETURNING cluster_id, hash, cluster_file_name, enabled, seen_status
		"#,
		cluster_id,
		hash,
		cluster_file_name
	)
	.fetch_one(pool)
	.await
}

/// `DISTINCT` is required `provider_releases` is keyed by version so one
/// artifact joins several rows and would otherwise be reported repeatedly
pub async fn list_cluster_artifacts_for_project(
	pool: &SqlitePool,
	cluster_id: i64,
	provider: i64,
	project_id: &str,
	exclude_hash: &str,
) -> Result<Vec<ClusterArtifactRow>, sqlx::Error> {
	sqlx::query_as!(
		ClusterArtifactRow,
		r#"
		SELECT DISTINCT ca.cluster_id, ca.hash, ca.cluster_file_name, ca.enabled, ca.seen_status
		FROM cluster_artifacts ca
		JOIN provider_releases pr ON pr.hash = ca.hash
		WHERE ca.cluster_id = ?
			AND pr.provider = ?
			AND pr.project_id = ?
			AND ca.hash <> ?
		"#,
		cluster_id,
		provider,
		project_id,
		exclude_hash
	)
	.fetch_all(pool)
	.await
}

pub async fn is_cluster_linked(
	pool: &SqlitePool,
	cluster_id: i64,
	hash: &str,
) -> Result<bool, sqlx::Error> {
	let row: Option<(i64,)> = sqlx::query_as(
		"SELECT 1 FROM cluster_artifacts WHERE cluster_id = ? AND hash = ? LIMIT 1",
	)
	.bind(cluster_id)
	.bind(hash)
	.fetch_optional(pool)
	.await?;

	Ok(row.is_some())
}

pub async fn unlink_cluster_artifact(
	pool: &SqlitePool,
	cluster_id: i64,
	hash: &str,
) -> Result<(), sqlx::Error> {
	sqlx::query!(
		"DELETE FROM cluster_artifacts WHERE cluster_id = ? AND hash = ?",
		cluster_id,
		hash
	)
	.execute(pool)
	.await?;

	Ok(())
}

/// One artifact can carry several `provider_releases` rows so the release
/// columns are taken from a single row picked by a total order rather than
/// grouped which lets SQLite mix columns from different rows
pub async fn list_cluster_artifacts_detailed(
	pool: &SqlitePool,
	cluster_id: i64,
) -> Result<Vec<LinkedArtifactRow>, sqlx::Error> {
	sqlx::query_as!(
		LinkedArtifactRow,
		r#"
		SELECT
			ca.hash AS "hash!: String",
			ca.cluster_file_name AS "cluster_file_name!: String",
			ca.enabled AS "enabled!: i64",
			ca.seen_status AS "seen_status!: i64",
			a.content_type AS "content_type!: i64",
			a.file_name AS "file_name!: String",
			pr.provider AS "provider?: i64",
			pr.project_id AS "project_id?: String",
			pr.version_id AS "version_id?: String",
			pr.display_name AS "display_name?: String",
			pr.display_version AS "display_version?: String",
			pr.published_at AS "published_at?: String"
		FROM cluster_artifacts ca
		JOIN artifacts a ON a.hash = ca.hash
		LEFT JOIN provider_releases pr ON pr.rowid = (
			SELECT rowid
			FROM provider_releases
			WHERE hash = ca.hash
			ORDER BY published_at DESC, provider, project_id, version_id
			LIMIT 1
		)
		WHERE ca.cluster_id = ?
		"#,
		cluster_id
	)
	.fetch_all(pool)
	.await
}

pub async fn get_cluster_artifact(
	pool: &SqlitePool,
	cluster_id: i64,
	hash: &str,
) -> Result<Option<ClusterArtifactRow>, sqlx::Error> {
	sqlx::query_as!(
		ClusterArtifactRow,
		r#"
		SELECT cluster_id, hash, cluster_file_name, enabled, seen_status
		FROM cluster_artifacts
		WHERE cluster_id = ? AND hash = ?
		"#,
		cluster_id,
		hash
	)
	.fetch_optional(pool)
	.await
}

pub async fn update_cluster_artifact(
	pool: &SqlitePool,
	cluster_id: i64,
	hash: &str,
	cluster_file_name: &str,
	enabled: i64,
) -> Result<ClusterArtifactRow, sqlx::Error> {
	sqlx::query_as!(
		ClusterArtifactRow,
		r#"
		UPDATE cluster_artifacts
		SET cluster_file_name = ?, enabled = ?
		WHERE cluster_id = ? AND hash = ?
		RETURNING cluster_id, hash, cluster_file_name, enabled, seen_status
		"#,
		cluster_file_name,
		enabled,
		cluster_id,
		hash
	)
	.fetch_one(pool)
	.await
}

pub async fn set_seen_status(
	pool: &SqlitePool,
	cluster_id: i64,
	hash: &str,
	status: SeenStatus,
) -> Result<(), sqlx::Error> {
	let status = status.as_i64();

	let result = sqlx::query!(
		r#"
		UPDATE cluster_artifacts
		SET seen_status = ?
		WHERE cluster_id = ? AND hash = ?
		"#,
		status,
		cluster_id,
		hash
	)
	.execute(pool)
	.await?;

	if result.rows_affected() == 0 {
		tracing::warn!(cluster_id, hash, "no cluster artifact matched the seen status update");
	}

	Ok(())
}

pub async fn mark_all_seen(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
	let seen = SeenStatus::Seen.as_i64();

	let result = sqlx::query!(
		"UPDATE cluster_artifacts SET seen_status = ? WHERE seen_status <> ?",
		seen,
		seen
	)
	.execute(pool)
	.await?;

	Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::dao::cluster as cluster_dao;
	use crate::models::NewCluster;

	async fn pool() -> SqlitePool {
		let pool = SqlitePool::connect("sqlite::memory:")
			.await
			.expect("in-memory sqlite");
		sqlx::migrate!().run(&pool).await.expect("migrations run");
		pool
	}

	async fn seed_cluster(pool: &SqlitePool) -> i64 {
		cluster_dao::insert(
			pool,
			&NewCluster {
				name: "26.1 fabric",
				folder_name: "26.1 fabric",
				mc_version: "26.1",
				mc_loader: 1,
				mc_loader_version: None,
				setting_profile_name: None,
				stage: 0,
			},
		)
		.await
		.expect("insert cluster")
		.id
	}

	#[tokio::test]
	async fn detailed_listing_matches_the_lookups_it_replaced() {
		let pool = pool().await;
		let cluster_id = seed_cluster(&pool).await;

		insert_artifact(&pool, "hash_managed", 0, "a/managed.jar", "managed.jar", Some(1))
			.await
			.expect("insert managed artifact");
		insert_artifact(&pool, "hash_local", 0, "a/local.jar", "local.jar", None)
			.await
			.expect("insert local artifact");

		for version in ["v1", "v2"] {
			upsert_provider_release(
				&pool,
				1,
				"sodium",
				version,
				"hash_managed",
				"Sodium",
				version,
				None,
				"[]",
				"[]",
			)
			.await
			.expect("insert release");
		}

		link_cluster_artifact(&pool, cluster_id, "hash_managed", "managed.jar")
			.await
			.expect("link managed");
		link_cluster_artifact(&pool, cluster_id, "hash_local", "local.jar")
			.await
			.expect("link local");
		update_cluster_artifact(&pool, cluster_id, "hash_local", "local.jar", 0)
			.await
			.expect("disable local");

		let rows = list_cluster_artifacts_detailed(&pool, cluster_id)
			.await
			.expect("detailed listing");

		assert_eq!(rows.len(), 2, "one row per link, not per provider release");

		let managed = rows.iter().find(|r| r.hash == "hash_managed").expect("managed row");
		assert_eq!(managed.file_name, "managed.jar");
		assert_eq!(managed.enabled, 1);
		assert_eq!(managed.project_id.as_deref(), Some("sodium"));
		assert_eq!(managed.provider, Some(1));
		assert_eq!(managed.version_id.as_deref(), Some("v1"));
		assert_eq!(
			managed.display_version.as_deref(),
			Some("v1"),
			"every release column comes from the same release row"
		);

		let local = rows.iter().find(|r| r.hash == "hash_local").expect("local row");
		assert_eq!(local.enabled, 0);
		assert!(local.project_id.is_none(), "a local file has no provider columns");

		let mut conn = pool.acquire().await.expect("connection");
		sqlx::query("PRAGMA foreign_keys = OFF")
			.execute(&mut *conn)
			.await
			.expect("disable foreign keys");
		sqlx::query!("DELETE FROM artifacts WHERE hash = ?", "hash_local")
			.execute(&mut *conn)
			.await
			.expect("drop artifact row");
		let orphans: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cluster_artifacts WHERE hash = ?")
			.bind("hash_local")
			.fetch_one(&mut *conn)
			.await
			.expect("count orphan link");
		assert_eq!(orphans, 1, "the link must outlive its artifact row");
		drop(conn);

		let rows = list_cluster_artifacts_detailed(&pool, cluster_id)
			.await
			.expect("detailed listing after delete");
		assert_eq!(rows.len(), 1);
		assert_eq!(rows[0].hash, "hash_managed");
	}
}
