use sqlx::SqlitePool;

pub async fn is_set(pool: &SqlitePool, cluster_id: i64, key: &str) -> Result<bool, sqlx::Error> {
    let row = sqlx::query("SELECT 1 FROM cluster_flags WHERE cluster_id = ? AND key = ?")
        .bind(cluster_id)
        .bind(key)
        .fetch_optional(pool)
        .await?;

    Ok(row.is_some())
}

pub async fn get(
    pool: &SqlitePool,
    cluster_id: i64,
    key: &str,
) -> Result<Option<String>, sqlx::Error> {
    let value = sqlx::query_scalar::<_, Option<String>>(
        "SELECT value FROM cluster_flags WHERE cluster_id = ? AND key = ?",
    )
    .bind(cluster_id)
    .bind(key)
    .fetch_optional(pool)
    .await?;

    Ok(value.flatten())
}

pub async fn set(
    pool: &SqlitePool,
    cluster_id: i64,
    key: &str,
    value: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO cluster_flags (cluster_id, key, value) VALUES (?, ?, ?)
         ON CONFLICT(cluster_id, key) DO UPDATE SET value = excluded.value",
    )
    .bind(cluster_id)
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn clear(pool: &SqlitePool, cluster_id: i64, key: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM cluster_flags WHERE cluster_id = ? AND key = ?")
        .bind(cluster_id)
        .bind(key)
        .execute(pool)
        .await?;

    Ok(())
}
