use sqlx::QueryBuilder;
use sqlx::SqlitePool;

use crate::models::{BundleRow, NewBundle};

pub async fn upsert_bundle(
    pool: &SqlitePool,
    bundle: NewBundle<'_>,
) -> Result<BundleRow, sqlx::Error> {
    let hidden = i64::from(bundle.hidden);
    sqlx::query_as!(
        BundleRow,
        r#"
        INSERT INTO bundles (
            remote_path, mc_version, mc_loader, file_name, name, version_id,
            category, loader_version, disk_path, hidden, etag, synced_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(remote_path) DO UPDATE SET
            mc_version = excluded.mc_version,
            mc_loader = excluded.mc_loader,
            file_name = excluded.file_name,
            name = COALESCE(excluded.name, bundles.name),
            version_id = COALESCE(excluded.version_id, bundles.version_id),
            category = COALESCE(excluded.category, bundles.category),
            loader_version = COALESCE(excluded.loader_version, bundles.loader_version),
            disk_path = excluded.disk_path,
            hidden = excluded.hidden,
            etag = COALESCE(excluded.etag, bundles.etag),
            synced_at = COALESCE(excluded.synced_at, bundles.synced_at)
        RETURNING
            remote_path, mc_version, mc_loader, file_name, name, version_id,
            category, loader_version, disk_path, hidden, etag, synced_at
        "#,
        bundle.remote_path,
        bundle.mc_version,
        bundle.mc_loader,
        bundle.file_name,
        bundle.name,
        bundle.version_id,
        bundle.category,
        bundle.loader_version,
        bundle.disk_path,
        hidden,
        bundle.etag,
        bundle.synced_at,
    )
    .fetch_one(pool)
    .await
}

pub async fn hide_bundles_not_in(
    pool: &SqlitePool,
    remote_paths: &[String],
) -> Result<u64, sqlx::Error> {
    if remote_paths.is_empty() {
        let result = sqlx::query("UPDATE bundles SET hidden = 1 WHERE hidden = 0")
            .execute(pool)
            .await?;
        return Ok(result.rows_affected());
    }

    let mut builder = QueryBuilder::new(
        "UPDATE bundles SET hidden = 1 WHERE hidden = 0 AND remote_path NOT IN (",
    );
    let mut separated = builder.separated(", ");
    for path in remote_paths {
        separated.push_bind(path);
    }
    separated.push_unseparated(")");

    let result = builder.build().execute(pool).await?;
    Ok(result.rows_affected())
}

pub async fn list_visible_for_version_loader(
    pool: &SqlitePool,
    mc_version: &str,
    mc_loader: i64,
) -> Result<Vec<BundleRow>, sqlx::Error> {
    sqlx::query_as!(
        BundleRow,
        r#"
        SELECT
            remote_path, mc_version, mc_loader, file_name, name, version_id,
            category, loader_version, disk_path, hidden, etag, synced_at
        FROM bundles
        WHERE mc_version = ? AND mc_loader = ? AND hidden = 0
        ORDER BY name, file_name
        "#,
        mc_version,
        mc_loader,
    )
    .fetch_all(pool)
    .await
}

pub async fn get_by_remote_path(
    pool: &SqlitePool,
    remote_path: &str,
) -> Result<Option<BundleRow>, sqlx::Error> {
    sqlx::query_as!(
        BundleRow,
        r#"
        SELECT
            remote_path, mc_version, mc_loader, file_name, name, version_id,
            category, loader_version, disk_path, hidden, etag, synced_at
        FROM bundles
        WHERE remote_path = ?
        "#,
        remote_path
    )
    .fetch_optional(pool)
    .await
}

#[derive(Debug, Clone)]
pub struct BundleVersionLoaderGroup {
    pub mc_version: String,
    pub mc_loader: i64,
}

pub async fn list_distinct_version_loaders(
    pool: &SqlitePool,
) -> Result<Vec<BundleVersionLoaderGroup>, sqlx::Error> {
    sqlx::query_as!(
        BundleVersionLoaderGroup,
        r#"
        SELECT DISTINCT mc_version, mc_loader
        FROM bundles
        WHERE hidden = 0
        ORDER BY mc_version, mc_loader
        "#
    )
    .fetch_all(pool)
    .await
}

pub async fn list_all(pool: &SqlitePool) -> Result<Vec<BundleRow>, sqlx::Error> {
    sqlx::query_as!(
        BundleRow,
        r#"
        SELECT
            remote_path, mc_version, mc_loader, file_name, name, version_id,
            category, loader_version, disk_path, hidden, etag, synced_at
        FROM bundles
        ORDER BY mc_version, mc_loader, name, file_name
        "#
    )
    .fetch_all(pool)
    .await
}
