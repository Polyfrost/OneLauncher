use sqlx::SqlitePool;

use crate::models::{BisectExit, ClusterBisectModRow, ClusterBisectSessionRow};

pub struct BisectModState<'a> {
    pub hash: &'a str,
    pub cleared_round: Option<i64>,
    pub testing: bool,
}

pub async fn get_session(
    pool: &SqlitePool,
    cluster_id: i64,
) -> Result<Option<ClusterBisectSessionRow>, sqlx::Error> {
    sqlx::query_as!(
        ClusterBisectSessionRow,
        r#"
        SELECT cluster_id, round, pending_exit, started_at, retry_note
        FROM cluster_bisect_sessions
        WHERE cluster_id = ?
        "#,
        cluster_id
    )
    .fetch_optional(pool)
    .await
}

pub async fn list_mods(
    pool: &SqlitePool,
    cluster_id: i64,
) -> Result<Vec<ClusterBisectModRow>, sqlx::Error> {
    sqlx::query_as!(
        ClusterBisectModRow,
        r#"
        SELECT cluster_id, hash, cleared_round, testing
        FROM cluster_bisect_mods
        WHERE cluster_id = ?
        ORDER BY hash
        "#,
        cluster_id
    )
    .fetch_all(pool)
    .await
}

pub async fn session_hashes(
    pool: &SqlitePool,
    cluster_id: i64,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        SELECT hash FROM cluster_bisect_mods
        WHERE cluster_id = ?
        "#,
        cluster_id
    )
    .fetch_all(pool)
    .await
}

pub async fn is_active(pool: &SqlitePool, cluster_id: i64) -> Result<bool, sqlx::Error> {
    let found = sqlx::query_scalar!(
        r#"
        SELECT cluster_id FROM cluster_bisect_sessions
        WHERE cluster_id = ?
        "#,
        cluster_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(found.is_some())
}

pub async fn start(
    pool: &SqlitePool,
    cluster_id: i64,
    hashes: &[String],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query!(
        r#"
        INSERT INTO cluster_bisect_sessions (cluster_id) VALUES (?)
        "#,
        cluster_id
    )
    .execute(&mut *tx)
    .await?;
    for hash in hashes {
        sqlx::query!(
            r#"
            INSERT INTO cluster_bisect_mods (cluster_id, hash) VALUES (?, ?)
            "#,
            cluster_id,
            hash
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

pub async fn save_round(
    pool: &SqlitePool,
    cluster_id: i64,
    round: i64,
    mods: &[BisectModState<'_>],
) -> Result<(), sqlx::Error> {
    let none = BisectExit::None.as_i64();
    let mut tx = pool.begin().await?;
    sqlx::query!(
        r#"
        UPDATE cluster_bisect_sessions SET round = ?, pending_exit = ?, retry_note = NULL
        WHERE cluster_id = ?
        "#,
        round,
        none,
        cluster_id
    )
    .execute(&mut *tx)
    .await?;
    for state in mods {
        let testing = i64::from(state.testing);
        sqlx::query!(
            r#"
            UPDATE cluster_bisect_mods SET cleared_round = ?, testing = ?
            WHERE cluster_id = ? AND hash = ?
            "#,
            state.cleared_round,
            testing,
            cluster_id,
            state.hash
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

pub async fn set_pending_exit(
    pool: &SqlitePool,
    cluster_id: i64,
    exit: BisectExit,
) -> Result<bool, sqlx::Error> {
    let exit = exit.as_i64();
    let affected = sqlx::query!(
        r#"
        UPDATE cluster_bisect_sessions SET pending_exit = ?
        WHERE cluster_id = ?
        "#,
        exit,
        cluster_id
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(affected > 0)
}

pub async fn list_edges(
    pool: &SqlitePool,
    cluster_id: i64,
) -> Result<Vec<(String, String)>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT from_hash, to_hash FROM cluster_bisect_edges
        WHERE cluster_id = ?
        "#,
        cluster_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.from_hash, row.to_hash))
        .collect())
}

pub async fn add_edges(
    pool: &SqlitePool,
    cluster_id: i64,
    edges: &[(String, String)],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    for (from_hash, to_hash) in edges {
        sqlx::query!(
            r#"
            INSERT OR IGNORE INTO cluster_bisect_edges (cluster_id, from_hash, to_hash)
            VALUES (?, ?, ?)
            "#,
            cluster_id,
            from_hash,
            to_hash
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

pub async fn set_retry_note(
    pool: &SqlitePool,
    cluster_id: i64,
    note: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE cluster_bisect_sessions SET retry_note = ?
        WHERE cluster_id = ?
        "#,
        note,
        cluster_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn end(pool: &SqlitePool, cluster_id: i64) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query!(
        r#"
        DELETE FROM cluster_bisect_edges WHERE cluster_id = ?
        "#,
        cluster_id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        r#"
        DELETE FROM cluster_bisect_mods WHERE cluster_id = ?
        "#,
        cluster_id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        r#"
        DELETE FROM cluster_bisect_sessions WHERE cluster_id = ?
        "#,
        cluster_id
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}
