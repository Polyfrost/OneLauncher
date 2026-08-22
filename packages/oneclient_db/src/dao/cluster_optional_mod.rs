use sqlx::SqlitePool;

use crate::models::{ClusterOptionalModRow, OptionalModStatus};

pub async fn queue(
    pool: &SqlitePool,
    cluster_id: i64,
    bundle_name: &str,
    package_id: &str,
    bundle_version_id: &str,
) -> Result<(), sqlx::Error> {
    let new = OptionalModStatus::New.as_i64();

    sqlx::query!(
        r#"
        INSERT INTO cluster_optional_mods
            (cluster_id, bundle_name, package_id, bundle_version_id, seen_status)
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(cluster_id, package_id) DO UPDATE SET
            bundle_name = excluded.bundle_name,
            bundle_version_id = excluded.bundle_version_id
        "#,
        cluster_id,
        bundle_name,
        package_id,
        bundle_version_id,
        new
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_pending(
    pool: &SqlitePool,
    cluster_id: i64,
) -> Result<Vec<ClusterOptionalModRow>, sqlx::Error> {
    sqlx::query_as!(
        ClusterOptionalModRow,
        r#"
        SELECT id, cluster_id, bundle_name, package_id, bundle_version_id,
               seen_status, queued_at
        FROM cluster_optional_mods
        WHERE cluster_id = ?
        ORDER BY queued_at, id
        "#,
        cluster_id
    )
    .fetch_all(pool)
    .await
}

// if a mod is marked as skipped, it will never show in a modal again (we mark it as seen by a user, so he doesn't have to see it again)
pub async fn mark_skipped(
    pool: &SqlitePool,
    cluster_id: i64,
    package_ids: &[String],
) -> Result<(), sqlx::Error> {
    if package_ids.is_empty() {
        return Ok(());
    }

    let seen = OptionalModStatus::Skipped.as_i64();
    let mut tx = pool.begin().await?;
    for package_id in package_ids {
        sqlx::query!(
            r#"
            UPDATE cluster_optional_mods SET seen_status = ?
            WHERE cluster_id = ? AND package_id = ?
            "#,
            seen,
            cluster_id,
            package_id
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

pub async fn resolve(
    pool: &SqlitePool,
    cluster_id: i64,
    package_ids: &[String],
) -> Result<(), sqlx::Error> {
    if package_ids.is_empty() {
        return Ok(());
    }

    let mut tx = pool.begin().await?;
    for package_id in package_ids {
        sqlx::query!(
            r#"
            DELETE FROM cluster_optional_mods
            WHERE cluster_id = ? AND package_id = ?
            "#,
            cluster_id,
            package_id
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

pub async fn retain(
    pool: &SqlitePool,
    cluster_id: i64,
    keep: &[String],
) -> Result<(), sqlx::Error> {
    let stale: Vec<String> = list_pending(pool, cluster_id)
        .await?
        .into_iter()
        .filter(|row| row.status() != OptionalModStatus::Skipped)
        .map(|row| row.package_id)
        .filter(|package_id| !keep.contains(package_id))
        .collect();

    resolve(pool, cluster_id, &stale).await
}
