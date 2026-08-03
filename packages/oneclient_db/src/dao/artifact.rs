use sqlx::SqlitePool;

use crate::models::{
	ArtifactRow, ClusterArtifactRow, ClusterDependencyEdgeRow, OverrideType, ProviderReleaseRow,
	ReleaseDependencyRow, UnsyncedReleaseRow,
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

/// Drops an artifact's row when no cluster refers to it any more.
///
/// `provider_releases` cascades, so the metadata goes with it. The cached file
/// is the caller's to delete — this layer does not touch the disk.
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

/// Every artifact no cluster refers to any more.
///
/// Clusters are deleted with an `ON DELETE CASCADE` on `cluster_artifacts`, and
/// bundle updates swap one version for another, so artifacts are orphaned in
/// bulk and in places too far from the store to evict them one at a time.
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

/// The stored path of every artifact, used to spot cached files that no row
/// accounts for.
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

/// Replaces a release's recorded dependency list.
///
/// Delete-then-insert rather than an upsert per row: a version that dropped a
/// dependency between two passes has to lose the row, or the reverse lookup
/// would keep reporting a package as needed by something that no longer needs
/// it. Empty `dependencies` is meaningful and still marks the release synced.
pub async fn replace_release_dependencies(
	pool: &SqlitePool,
	provider_id: i64,
	project_id: &str,
	version_id: &str,
	dependencies: &[(String, String, String)],
) -> Result<(), sqlx::Error> {
	let mut tx = pool.begin().await?;

	sqlx::query!(
		r#"
		DELETE FROM provider_release_dependencies
		WHERE provider = ? AND project_id = ? AND version_id = ?
		"#,
		provider_id,
		project_id,
		version_id
	)
	.execute(&mut *tx)
	.await?;

	for (dependency_project_id, dependency_version_id, kind) in dependencies {
		sqlx::query!(
			r#"
			INSERT INTO provider_release_dependencies (
				provider, project_id, version_id,
				dependency_project_id, dependency_version_id, kind
			)
			VALUES (?, ?, ?, ?, ?, ?)
			ON CONFLICT(provider, project_id, version_id, dependency_project_id, dependency_version_id)
			DO UPDATE SET kind = excluded.kind
			"#,
			provider_id,
			project_id,
			version_id,
			dependency_project_id,
			dependency_version_id,
			kind
		)
		.execute(&mut *tx)
		.await?;
	}

	sqlx::query!(
		r#"
		UPDATE provider_releases
		SET dependencies_synced_at = datetime('now')
		WHERE provider = ? AND project_id = ? AND version_id = ?
		"#,
		provider_id,
		project_id,
		version_id
	)
	.execute(&mut *tx)
	.await?;

	tx.commit().await
}

pub async fn list_release_dependencies(
	pool: &SqlitePool,
	provider_id: i64,
	project_id: &str,
	version_id: &str,
) -> Result<Vec<ReleaseDependencyRow>, sqlx::Error> {
	sqlx::query_as!(
		ReleaseDependencyRow,
		r#"
		SELECT dependency_project_id, dependency_version_id, kind
		FROM provider_release_dependencies
		WHERE provider = ? AND project_id = ? AND version_id = ?
		"#,
		provider_id,
		project_id,
		version_id
	)
	.fetch_all(pool)
	.await
}

/// Every dependency declared by an artifact this cluster has installed.
///
/// One query for the whole cluster: the reverse graph is only useful whole, and
/// walking it one package at a time would be a round trip per hop.
pub async fn list_cluster_dependency_edges(
	pool: &SqlitePool,
	cluster_id: i64,
) -> Result<Vec<ClusterDependencyEdgeRow>, sqlx::Error> {
	sqlx::query_as!(
		ClusterDependencyEdgeRow,
		r#"
		SELECT
			ca.hash,
			pr.provider as "provider!: i64",
			prd.dependency_project_id,
			prd.dependency_version_id,
			prd.kind
		FROM cluster_artifacts ca
		JOIN provider_releases pr ON pr.hash = ca.hash
		JOIN provider_release_dependencies prd
			ON prd.provider = pr.provider
			AND prd.project_id = pr.project_id
			AND prd.version_id = pr.version_id
		WHERE ca.cluster_id = ?
		"#,
		cluster_id
	)
	.fetch_all(pool)
	.await
}

/// The cluster's releases whose dependency list has never been recorded.
///
/// Everything installed before the dependency table existed lands here, which
/// is what the backfill walks.
pub async fn list_releases_missing_dependencies(
	pool: &SqlitePool,
	cluster_id: i64,
) -> Result<Vec<UnsyncedReleaseRow>, sqlx::Error> {
	sqlx::query_as!(
		UnsyncedReleaseRow,
		r#"
		SELECT DISTINCT
			pr.provider as "provider!: i64",
			pr.project_id,
			pr.version_id
		FROM cluster_artifacts ca
		JOIN provider_releases pr ON pr.hash = ca.hash
		WHERE ca.cluster_id = ? AND pr.dependencies_synced_at IS NULL
		"#,
		cluster_id
	)
	.fetch_all(pool)
	.await
}

/// Switches off a set of the cluster's artifacts, and records the matching
/// bundle overrides, in one transaction.
///
/// The set is written whole or not at all. It is handed in as a set because a
/// package is disabled together with everything that requires it, and a
/// half-applied closure would leave the cluster in a state the user never
/// picked — mods enabled with their library gone is exactly the crash the
/// warning exists to prevent.
///
/// Bundle bookkeeping happens here rather than a layer up for the same reason:
/// the override says "the user turned this off", and it belongs in the
/// transaction that turns it off. `overrides` carries the rows that have no
/// artifact of their own, i.e. a bundle package the cluster has not installed.
pub async fn disable_cluster_artifacts(
	pool: &SqlitePool,
	cluster_id: i64,
	hashes: &[String],
	overrides: &[(String, String)],
) -> Result<Vec<ClusterArtifactRow>, sqlx::Error> {
	let mut tx = pool.begin().await?;
	let mut updated = Vec::with_capacity(hashes.len());

	for hash in hashes {
		let Some(row) = sqlx::query!(
			r#"
			SELECT cluster_file_name, bundle_name, package_id
			FROM cluster_artifacts
			WHERE cluster_id = ? AND hash = ?
			"#,
			cluster_id,
			hash
		)
		.fetch_optional(&mut *tx)
		.await?
		else {
			continue;
		};

		// The suffix is how a disabled file used to be marked in the game
		// folder; the flag is authoritative now, so the stored name is kept
		// clean either way.
		let file_name = row.cluster_file_name.trim_end_matches(".disabled").to_string();

		updated.push(
			sqlx::query_as!(
				ClusterArtifactRow,
				r#"
				UPDATE cluster_artifacts
				SET cluster_file_name = ?, enabled = 0
				WHERE cluster_id = ? AND hash = ?
				RETURNING cluster_id, hash, cluster_file_name, enabled
				"#,
				file_name,
				cluster_id,
				hash
			)
			.fetch_one(&mut *tx)
			.await?,
		);

		if let (Some(bundle_name), Some(package_id)) = (row.bundle_name, row.package_id) {
			save_override_tx(&mut tx, cluster_id, &bundle_name, &package_id).await?;
		}
	}

	for (bundle_name, package_id) in overrides {
		save_override_tx(&mut tx, cluster_id, bundle_name, package_id).await?;
	}

	tx.commit().await?;
	Ok(updated)
}

async fn save_override_tx(
	tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
	cluster_id: i64,
	bundle_name: &str,
	package_id: &str,
) -> Result<(), sqlx::Error> {
	let override_type = OverrideType::Disabled.as_str();
	sqlx::query!(
		r#"
		INSERT INTO cluster_bundle_overrides (cluster_id, bundle_name, package_id, override_type)
		VALUES (?, ?, ?, ?)
		ON CONFLICT(cluster_id, bundle_name, package_id) DO UPDATE SET
			override_type = excluded.override_type
		"#,
		cluster_id,
		bundle_name,
		package_id,
		override_type
	)
	.execute(&mut **tx)
	.await?;
	Ok(())
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
		RETURNING cluster_id, hash, cluster_file_name, enabled
		"#,
		cluster_id,
		hash,
		cluster_file_name
	)
	.fetch_one(pool)
	.await
}

/// Every artifact the cluster has from the same project as `exclude_hash`,
/// excluding that one.
///
/// `provider_releases` is keyed by version, so one artifact can join several
/// rows of it. Without `DISTINCT` a package would be reported once per release
/// row and a caller unlinking the results would work from an inflated list.
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
		SELECT DISTINCT ca.cluster_id, ca.hash, ca.cluster_file_name, ca.enabled
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

pub async fn list_cluster_artifacts(
	pool: &SqlitePool,
	cluster_id: i64,
) -> Result<Vec<ClusterArtifactRow>, sqlx::Error> {
	sqlx::query_as!(
		ClusterArtifactRow,
		r#"
		SELECT cluster_id, hash, cluster_file_name, enabled
		FROM cluster_artifacts
		WHERE cluster_id = ?
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
		SELECT cluster_id, hash, cluster_file_name, enabled
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
		RETURNING cluster_id, hash, cluster_file_name, enabled
		"#,
		cluster_file_name,
		enabled,
		cluster_id,
		hash
	)
	.fetch_one(pool)
	.await
}
