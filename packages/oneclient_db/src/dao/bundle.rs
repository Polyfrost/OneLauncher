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
        tracing::warn!("refusing to hide bundles for an empty catalog");
        return Ok(0);
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

pub async fn list_delisted_names_for_version_loader(
    pool: &SqlitePool,
    mc_version: &str,
    mc_loader: i64,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        "SELECT name FROM bundles \
         WHERE mc_version = ? AND mc_loader = ? AND hidden = 1 AND name IS NOT NULL",
    )
    .bind(mc_version)
    .bind(mc_loader)
    .fetch_all(pool)
    .await
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

#[cfg(test)]
mod tests {
    use super::*;

    async fn pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        sqlx::migrate!("../oneclient_db/migrations")
            .run(&pool)
            .await
            .expect("migrations run");
        pool
    }

    async fn seed(pool: &SqlitePool, remote_path: &str) {
        upsert_bundle(
            pool,
            NewBundle {
                remote_path,
                mc_version: "1.21.11",
                mc_loader: 1,
                file_name: "bundle.mrpack",
                name: Some("SkyBlock"),
                version_id: Some("1"),
                category: Some("test"),
                loader_version: Some("0.16.0"),
                disk_path: remote_path,
                hidden: false,
                etag: None,
                synced_at: None,
            },
        )
        .await
        .expect("seed bundle");
    }

    #[tokio::test]
    async fn an_empty_catalog_hides_nothing() {
        let pool = pool().await;
        seed(&pool, "bundles/skyblock.mrpack").await;

        assert_eq!(hide_bundles_not_in(&pool, &[]).await.unwrap(), 0);
        assert_eq!(
            list_visible_for_version_loader(&pool, "1.21.11", 1)
                .await
                .unwrap()
                .len(),
            1,
            "an empty catalog must not delist a live bundle"
        );
    }

    #[tokio::test]
    async fn a_catalog_that_drops_one_bundle_hides_only_that_one() {
        let pool = pool().await;
        seed(&pool, "bundles/skyblock.mrpack").await;
        seed(&pool, "bundles/qol.mrpack").await;

        hide_bundles_not_in(&pool, &["bundles/qol.mrpack".to_string()])
            .await
            .unwrap();

        let visible = list_visible_for_version_loader(&pool, "1.21.11", 1)
            .await
            .unwrap();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].remote_path, "bundles/qol.mrpack");
        assert_eq!(
            list_delisted_names_for_version_loader(&pool, "1.21.11", 1)
                .await
                .unwrap(),
            vec!["SkyBlock".to_string()]
        );
    }
}
