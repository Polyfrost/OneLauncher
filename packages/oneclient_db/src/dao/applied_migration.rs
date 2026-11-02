use chrono::Utc;
use sqlx::SqlitePool;

pub async fn is_applied(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
    let row = sqlx::query!("SELECT id FROM applied_migrations WHERE id = ?", id)
        .fetch_optional(pool)
        .await?;

    Ok(row.is_some())
}

pub async fn mark_applied(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    let applied_at = Utc::now().to_rfc3339();

    sqlx::query!(
        "INSERT OR IGNORE INTO applied_migrations (id, applied_at) VALUES (?, ?)",
        id,
        applied_at
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn forget(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM applied_migrations WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn forget_prefix(pool: &SqlitePool, prefix: &str) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM applied_migrations WHERE substr(id, 1, length(?)) = ?")
        .bind(prefix)
        .bind(prefix)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}
