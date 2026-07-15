use chrono::Utc;
use sqlx::SqlitePool;

use crate::models::{BundleTrackedArtifactRow, ClusterBundleOverrideRow, OverrideType};

pub async fn track_bundle_artifact(
    pool: &SqlitePool,
    cluster_id: i64,
    hash: &str,
    bundle_name: &str,
    bundle_version_id: &str,
    package_id: &str,
) -> Result<BundleTrackedArtifactRow, sqlx::Error> {
    let installed_at = Utc::now().to_rfc3339();
    sqlx::query_as!(
        BundleTrackedArtifactRow,
        r#"
        UPDATE cluster_artifacts
        SET
            bundle_name = ?,
            bundle_version_id = ?,
            package_id = ?,
            installed_at = ?
        WHERE cluster_id = ? AND hash = ?
        RETURNING
            cluster_id, hash, cluster_file_name, enabled,
            bundle_name, bundle_version_id, package_id, installed_at
        "#,
        bundle_name,
        bundle_version_id,
        package_id,
        installed_at,
        cluster_id,
        hash
    )
    .fetch_one(pool)
    .await
}

pub async fn clear_bundle_tracking(
    pool: &SqlitePool,
    cluster_id: i64,
    hash: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE cluster_artifacts
        SET bundle_name = NULL, bundle_version_id = NULL, package_id = NULL, installed_at = NULL
        WHERE cluster_id = ? AND hash = ?
        "#,
        cluster_id,
        hash
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_bundle_tracked(
    pool: &SqlitePool,
    cluster_id: i64,
) -> Result<Vec<BundleTrackedArtifactRow>, sqlx::Error> {
    sqlx::query_as!(
        BundleTrackedArtifactRow,
        r#"
        SELECT
            cluster_id, hash, cluster_file_name, enabled,
            bundle_name, bundle_version_id, package_id, installed_at
        FROM cluster_artifacts
        WHERE cluster_id = ? AND bundle_name IS NOT NULL
        "#,
        cluster_id
    )
    .fetch_all(pool)
    .await
}

pub async fn get_bundle_tracked(
    pool: &SqlitePool,
    cluster_id: i64,
    hash: &str,
) -> Result<Option<BundleTrackedArtifactRow>, sqlx::Error> {
    sqlx::query_as!(
        BundleTrackedArtifactRow,
        r#"
        SELECT
            cluster_id, hash, cluster_file_name, enabled,
            bundle_name, bundle_version_id, package_id, installed_at
        FROM cluster_artifacts
        WHERE cluster_id = ? AND hash = ? AND bundle_name IS NOT NULL
        "#,
        cluster_id,
        hash
    )
    .fetch_optional(pool)
    .await
}

pub async fn has_bundle_mapping(
    pool: &SqlitePool,
    cluster_id: i64,
    bundle_name: &str,
    package_id: &str,
) -> Result<bool, sqlx::Error> {
    let row: Option<(i64,)> = sqlx::query_as(
        r#"
        SELECT 1 FROM cluster_artifacts
        WHERE cluster_id = ? AND bundle_name = ? AND package_id = ?
        LIMIT 1
        "#,
    )
    .bind(cluster_id)
    .bind(bundle_name)
    .bind(package_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.is_some())
}

pub async fn save_override(
    pool: &SqlitePool,
    cluster_id: i64,
    bundle_name: &str,
    package_id: &str,
    override_type: OverrideType,
) -> Result<(), sqlx::Error> {
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
        override_type.as_str()
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn save_overrides(
    pool: &SqlitePool,
    cluster_id: i64,
    overrides: &[(String, String, OverrideType)],
) -> Result<(), sqlx::Error> {
    if overrides.is_empty() {
        return Ok(());
    }

    let mut tx = pool.begin().await?;
    for (bundle_name, package_id, override_type) in overrides {
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
            override_type.as_str()
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

pub async fn remove_override(
    pool: &SqlitePool,
    cluster_id: i64,
    bundle_name: &str,
    package_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        DELETE FROM cluster_bundle_overrides
        WHERE cluster_id = ? AND bundle_name = ? AND package_id = ?
        "#,
        cluster_id,
        bundle_name,
        package_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_overrides(
    pool: &SqlitePool,
    cluster_id: i64,
) -> Result<Vec<ClusterBundleOverrideRow>, sqlx::Error> {
    sqlx::query_as!(
        ClusterBundleOverrideRow,
        r#"
        SELECT id, cluster_id, bundle_name, package_id, override_type
        FROM cluster_bundle_overrides
        WHERE cluster_id = ?
        "#,
        cluster_id
    )
    .fetch_all(pool)
    .await
}
