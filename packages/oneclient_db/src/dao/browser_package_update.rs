use chrono::Utc;
use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use crate::models::BrowserPackageUpdateRow;

pub async fn list_for_cluster(
    pool: &SqlitePool,
    cluster_id: i64,
) -> Result<Vec<BrowserPackageUpdateRow>, sqlx::Error> {
    sqlx::query_as!(
        BrowserPackageUpdateRow,
        r#"
		SELECT
			cluster_id, hash, provider as "provider!: i64", project_id,
			installed_version_id, installed_version_name,
			latest_version_id, latest_version_name,
			display_name, checked_at, skipped, pin_version_id
		FROM browser_package_updates
		WHERE cluster_id = ?
		"#,
        cluster_id
    )
    .fetch_all(pool)
    .await
}

pub async fn list_all(pool: &SqlitePool) -> Result<Vec<BrowserPackageUpdateRow>, sqlx::Error> {
    sqlx::query_as!(
        BrowserPackageUpdateRow,
        r#"
		SELECT
			cluster_id, hash, provider as "provider!: i64", project_id,
			installed_version_id, installed_version_name,
			latest_version_id, latest_version_name,
			display_name, checked_at, skipped, pin_version_id
		FROM browser_package_updates
		"#,
    )
    .fetch_all(pool)
    .await
}

/// Re-upserting the same `latest_version_id` keeps `skipped` a different one
/// resets it since it is a new question for the user
#[allow(clippy::too_many_arguments)]
pub async fn upsert(
    pool: &SqlitePool,
    cluster_id: i64,
    hash: &str,
    provider: i64,
    project_id: &str,
    installed_version_id: &str,
    installed_version_name: &str,
    latest_version_id: &str,
    latest_version_name: &str,
    display_name: &str,
    skipped: bool,
    pin_version_id: Option<&str>,
) -> Result<BrowserPackageUpdateRow, sqlx::Error> {
    let checked_at = Utc::now().to_rfc3339();
    let skipped = i64::from(skipped);

    sqlx::query_as!(
        BrowserPackageUpdateRow,
        r#"
		INSERT INTO browser_package_updates (
			cluster_id, hash, provider, project_id,
			installed_version_id, installed_version_name,
			latest_version_id, latest_version_name,
			display_name, checked_at, skipped, pin_version_id
		)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(cluster_id, hash) DO UPDATE SET
			provider = excluded.provider,
			project_id = excluded.project_id,
			installed_version_id = excluded.installed_version_id,
			installed_version_name = excluded.installed_version_name,
			latest_version_name = excluded.latest_version_name,
			display_name = excluded.display_name,
			checked_at = excluded.checked_at,
			skipped = CASE
				WHEN browser_package_updates.latest_version_id = excluded.latest_version_id
					AND (browser_package_updates.pin_version_id IS NOT NULL OR excluded.pin_version_id IS NULL)
				THEN MAX(browser_package_updates.skipped, excluded.skipped)
				ELSE excluded.skipped
			END,
			latest_version_id = excluded.latest_version_id,
			pin_version_id = excluded.pin_version_id
		RETURNING
			cluster_id, hash, provider as "provider!: i64", project_id,
			installed_version_id, installed_version_name,
			latest_version_id, latest_version_name,
			display_name, checked_at, skipped, pin_version_id
		"#,
        cluster_id,
        hash,
        provider,
        project_id,
        installed_version_id,
        installed_version_name,
        latest_version_id,
        latest_version_name,
        display_name,
        checked_at,
        skipped,
        pin_version_id
    )
    .fetch_one(pool)
    .await
}

pub async fn set_skipped(
    pool: &SqlitePool,
    cluster_id: i64,
    hash: &str,
    skipped: bool,
) -> Result<(), sqlx::Error> {
    let skipped = i64::from(skipped);
    sqlx::query!(
        r#"
		UPDATE browser_package_updates
		SET skipped = ?
		WHERE cluster_id = ? AND hash = ?
		"#,
        skipped,
        cluster_id,
        hash
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn delete(pool: &SqlitePool, cluster_id: i64, hash: &str) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM browser_package_updates WHERE cluster_id = ? AND hash = ?",
        cluster_id,
        hash
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Drops rows for hashes the latest check did not report an empty list clears
/// the cluster
pub async fn retain_hashes(
    pool: &SqlitePool,
    cluster_id: i64,
    hashes: &[String],
) -> Result<(), sqlx::Error> {
    if hashes.is_empty() {
        sqlx::query!(
            "DELETE FROM browser_package_updates WHERE cluster_id = ?",
            cluster_id
        )
        .execute(pool)
        .await?;
        return Ok(());
    }

    let mut builder: QueryBuilder<Sqlite> =
        QueryBuilder::new("DELETE FROM browser_package_updates WHERE cluster_id = ");
    builder.push_bind(cluster_id);
    builder.push(" AND hash NOT IN (");
    let mut separated = builder.separated(", ");
    for hash in hashes {
        separated.push_bind(hash);
    }
    separated.push_unseparated(")");

    builder.build().execute(pool).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        sqlx::migrate!().run(&pool).await.expect("migrations run");
        // Runtime query not the macro the offline cache is library-only
        sqlx::query(
            r#"
			INSERT INTO clusters (id, name, folder_name, mc_version)
			VALUES (1, 'test', 'test', '1.21.4')
			"#,
        )
        .execute(&pool)
        .await
        .expect("cluster row");
        pool
    }

    async fn record(pool: &SqlitePool, hash: &str, latest: &str) -> BrowserPackageUpdateRow {
        upsert(
            pool, 1, hash, 0, "proj", "v1", "1.0.0", latest, "2.0.0", "Mod", false, None,
        )
        .await
        .expect("upsert")
    }

    #[tokio::test]
    async fn skip_survives_a_recheck_of_the_same_version() {
        let pool = pool().await;
        record(&pool, "abc", "v2").await;
        set_skipped(&pool, 1, "abc", true).await.expect("skip");

        let row = record(&pool, "abc", "v2").await;
        assert_eq!(row.skipped, 1, "re-running the check must not re-ask");
    }

    #[tokio::test]
    async fn a_newer_version_asks_again() {
        let pool = pool().await;
        record(&pool, "abc", "v2").await;
        set_skipped(&pool, 1, "abc", true).await.expect("skip");

        let row = record(&pool, "abc", "v3").await;
        assert_eq!(row.skipped, 0, "a different version is a new question");
    }

    #[tokio::test]
    async fn a_hold_survives_taking_a_new_version_id() {
        let pool = pool().await;
        record(&pool, "abc", "1.2.0").await;
        set_skipped(&pool, 1, "abc", true).await.expect("skip");

        let row = upsert(
            &pool, 1, "abc", 0, "proj", "v1", "1.0.0", "AbC123", "1.2.0", "Mod", true, None,
        )
        .await
        .expect("upsert");

        assert_eq!(row.skipped, 1);
    }

    #[tokio::test]
    async fn a_pin_on_a_skipped_version_asks_again() {
        let pool = pool().await;
        record(&pool, "abc", "v2").await;
        set_skipped(&pool, 1, "abc", true).await.expect("skip");

        let pin = || {
            upsert(
                &pool,
                1,
                "abc",
                0,
                "proj",
                "v1",
                "1.0.0",
                "v2",
                "2.0.0",
                "Mod",
                false,
                Some("v2"),
            )
        };
        assert_eq!(pin().await.expect("upsert").skipped, 0);

        set_skipped(&pool, 1, "abc", true).await.expect("skip");
        assert_eq!(
            pin().await.expect("upsert").skipped,
            1,
            "a skip made against the pin is a hold"
        );
    }

    #[tokio::test]
    async fn retain_drops_hashes_the_check_no_longer_reports() {
        let pool = pool().await;
        record(&pool, "abc", "v2").await;
        record(&pool, "def", "v2").await;

        retain_hashes(&pool, 1, &["abc".to_string()])
            .await
            .expect("retain");

        let rows = list_for_cluster(&pool, 1).await.expect("list");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].hash, "abc");
    }

    #[tokio::test]
    async fn retain_with_nothing_clears_the_cluster() {
        let pool = pool().await;
        record(&pool, "abc", "v2").await;

        retain_hashes(&pool, 1, &[]).await.expect("retain");

        assert!(list_for_cluster(&pool, 1).await.expect("list").is_empty());
    }
}
