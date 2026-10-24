use sqlx::SqlitePool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClusterFlag {
    /// Set from the first destructive step until the content is back, so a reset that
    /// failed or was cut short by a crash still keeps the half-emptied cluster from launching
    ResetUnfinished,
    BundleJavaOverride,
    LoaderUpdate,
}

impl ClusterFlag {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResetUnfinished => "reset-unfinished",
            Self::BundleJavaOverride => "bundle-java-override",
            Self::LoaderUpdate => "loader-update",
        }
    }
}

pub async fn is_set(
    pool: &SqlitePool,
    cluster_id: i64,
    flag: ClusterFlag,
) -> Result<bool, sqlx::Error> {
    let row = sqlx::query("SELECT 1 FROM cluster_flags WHERE cluster_id = ? AND key = ?")
        .bind(cluster_id)
        .bind(flag.as_str())
        .fetch_optional(pool)
        .await?;

    Ok(row.is_some())
}

pub async fn get(
    pool: &SqlitePool,
    cluster_id: i64,
    flag: ClusterFlag,
) -> Result<Option<String>, sqlx::Error> {
    let value = sqlx::query_scalar::<_, Option<String>>(
        "SELECT value FROM cluster_flags WHERE cluster_id = ? AND key = ?",
    )
    .bind(cluster_id)
    .bind(flag.as_str())
    .fetch_optional(pool)
    .await?;

    Ok(value.flatten())
}

pub async fn set(
    pool: &SqlitePool,
    cluster_id: i64,
    flag: ClusterFlag,
    value: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO cluster_flags (cluster_id, key, value) VALUES (?, ?, ?)
         ON CONFLICT(cluster_id, key) DO UPDATE SET value = excluded.value",
    )
    .bind(cluster_id)
    .bind(flag.as_str())
    .bind(value)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn clear(
    pool: &SqlitePool,
    cluster_id: i64,
    flag: ClusterFlag,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM cluster_flags WHERE cluster_id = ? AND key = ?")
        .bind(cluster_id)
        .bind(flag.as_str())
        .execute(pool)
        .await?;

    Ok(())
}
