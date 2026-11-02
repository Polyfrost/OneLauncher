use sqlx::SqlitePool;

use crate::models::{BisectEdgeKind, BisectExit, ClusterBisectModRow, ClusterBisectSessionRow};

pub struct BisectModState<'a> {
    pub hash: &'a str,
    pub cleared_round: Option<i64>,
    pub pinned_round: Option<i64>,
    pub testing: bool,
}

pub async fn get_session(
    pool: &SqlitePool,
    cluster_id: i64,
) -> Result<Option<ClusterBisectSessionRow>, sqlx::Error> {
    sqlx::query_as!(
        ClusterBisectSessionRow,
        r#"
        SELECT cluster_id, round, pending_exit, started_at, retry_note, confirmed, finishing
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
        SELECT cluster_id, hash, cleared_round, testing, name, pinned_round, disable_on_finish
        FROM cluster_bisect_mods
        WHERE cluster_id = ?
        ORDER BY hash
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
    mods: &[(String, String)],
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
    for (hash, name) in mods {
        sqlx::query!(
            r#"
            INSERT INTO cluster_bisect_mods (cluster_id, hash, name) VALUES (?, ?, ?)
            "#,
            cluster_id,
            hash,
            name
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
    confirmed: bool,
    mods: &[BisectModState<'_>],
) -> Result<(), sqlx::Error> {
    let none = BisectExit::None.as_i64();
    let confirmed = i64::from(confirmed);
    let mut tx = pool.begin().await?;
    sqlx::query!(
        r#"
        UPDATE cluster_bisect_sessions
        SET round = ?, pending_exit = ?, retry_note = NULL, confirmed = ?
        WHERE cluster_id = ?
        "#,
        round,
        none,
        confirmed,
        cluster_id
    )
    .execute(&mut *tx)
    .await?;
    for state in mods {
        let testing = i64::from(state.testing);
        sqlx::query!(
            r#"
            UPDATE cluster_bisect_mods SET cleared_round = ?, pinned_round = ?, testing = ?
            WHERE cluster_id = ? AND hash = ?
            "#,
            state.cleared_round,
            state.pinned_round,
            testing,
            cluster_id,
            state.hash
        )
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query!(
        r#"
        DELETE FROM cluster_bisect_suggestions WHERE cluster_id = ?
        "#,
        cluster_id
    )
    .execute(&mut *tx)
    .await?;
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
) -> Result<Vec<(String, String, BisectEdgeKind)>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT from_hash, to_hash, kind FROM cluster_bisect_edges
        WHERE cluster_id = ?
        "#,
        cluster_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let kind = BisectEdgeKind::from_repr(row.kind).unwrap_or(BisectEdgeKind::Guessed);
            (row.from_hash, row.to_hash, kind)
        })
        .collect())
}

pub async fn add_edges(
    pool: &SqlitePool,
    cluster_id: i64,
    edges: &[(String, String, BisectEdgeKind)],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    for (from_hash, to_hash, kind) in edges {
        let kind = kind.as_i64();
        sqlx::query!(
            r#"
            INSERT INTO cluster_bisect_edges (cluster_id, from_hash, to_hash, kind)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(cluster_id, from_hash, to_hash) DO UPDATE SET
                kind = MIN(kind, excluded.kind)
            "#,
            cluster_id,
            from_hash,
            to_hash,
            kind
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

pub async fn list_suggestions(
    pool: &SqlitePool,
    cluster_id: i64,
) -> Result<Vec<(Option<String>, String)>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT from_hash, to_hash FROM cluster_bisect_suggestions
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

pub async fn set_suggestions(
    pool: &SqlitePool,
    cluster_id: i64,
    suggestions: &[(Option<String>, String)],
    exit: BisectExit,
) -> Result<(), sqlx::Error> {
    let exit = exit.as_i64();
    let mut tx = pool.begin().await?;
    sqlx::query!(
        r#"
        DELETE FROM cluster_bisect_suggestions WHERE cluster_id = ?
        "#,
        cluster_id
    )
    .execute(&mut *tx)
    .await?;
    for (from_hash, to_hash) in suggestions {
        sqlx::query!(
            r#"
            INSERT INTO cluster_bisect_suggestions (cluster_id, from_hash, to_hash)
            VALUES (?, ?, ?)
            "#,
            cluster_id,
            from_hash,
            to_hash
        )
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query!(
        r#"
        UPDATE cluster_bisect_sessions SET pending_exit = ?
        WHERE cluster_id = ?
        "#,
        exit,
        cluster_id
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

pub async fn clear_suggestions(pool: &SqlitePool, cluster_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        DELETE FROM cluster_bisect_suggestions WHERE cluster_id = ?
        "#,
        cluster_id
    )
    .execute(pool)
    .await?;
    Ok(())
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

pub async fn set_names(
    pool: &SqlitePool,
    cluster_id: i64,
    names: &[(String, String)],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    for (hash, name) in names {
        sqlx::query!(
            r#"
            UPDATE cluster_bisect_mods SET name = ?
            WHERE cluster_id = ? AND hash = ?
            "#,
            name,
            cluster_id,
            hash
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

pub async fn set_confirmed(
    pool: &SqlitePool,
    cluster_id: i64,
    confirmed: bool,
) -> Result<(), sqlx::Error> {
    let confirmed = i64::from(confirmed);
    let none = BisectExit::None.as_i64();
    sqlx::query!(
        r#"
        UPDATE cluster_bisect_sessions SET confirmed = ?, pending_exit = ?
        WHERE cluster_id = ?
        "#,
        confirmed,
        none,
        cluster_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Recorded before any mod is switched back so an interrupted finish can be
/// completed at the next startup
pub async fn begin_finish(
    pool: &SqlitePool,
    cluster_id: i64,
    disable: &[String],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query!(
        r#"
        UPDATE cluster_bisect_sessions SET finishing = 1
        WHERE cluster_id = ?
        "#,
        cluster_id
    )
    .execute(&mut *tx)
    .await?;
    for hash in disable {
        sqlx::query!(
            r#"
            UPDATE cluster_bisect_mods SET disable_on_finish = 1
            WHERE cluster_id = ? AND hash = ?
            "#,
            cluster_id,
            hash
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

pub async fn list_finishing(pool: &SqlitePool) -> Result<Vec<i64>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        SELECT cluster_id FROM cluster_bisect_sessions
        WHERE finishing = 1
        "#
    )
    .fetch_all(pool)
    .await
}

pub async fn end(pool: &SqlitePool, cluster_id: i64) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query!(
        r#"
        DELETE FROM cluster_bisect_suggestions WHERE cluster_id = ?
        "#,
        cluster_id
    )
    .execute(&mut *tx)
    .await?;
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
