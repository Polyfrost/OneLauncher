use std::collections::HashSet;
use std::sync::Mutex;

use sqlx::sqlite::SqliteRow;
use sqlx::{Column, Row, ValueRef};

use crate::{DbError, DbPool};

/// sqlx runs SQLite on a worker thread and requires the SQL string to be
/// `'static`. Intern each distinct console query so repeated runs of the same
/// text don't leak, while still accepting arbitrary runtime strings.
fn intern(sql: &str) -> &'static str {
    static CACHE: Mutex<Option<HashSet<&'static str>>> = Mutex::new(None);

    let mut guard = CACHE.lock().unwrap();
    let cache = guard.get_or_insert_with(HashSet::new);

    if let Some(existing) = cache.get(sql) {
        return existing;
    }

    let leaked: &'static str = Box::leak(sql.to_owned().into_boxed_str());
    cache.insert(leaked);
    leaked
}

/// Outcome of a console query, stringified for display.
#[derive(Debug, Clone, Default)]
pub struct ConsoleQueryResult {
    /// Column headers (empty when the statement returned no rows).
    pub columns: Vec<String>,
    /// Row cells, already converted to display strings.
    pub rows: Vec<Vec<String>>,
    /// Rows affected by a non-select statement.
    pub rows_affected: u64,
    /// Whether the statement returned a result set.
    pub is_select: bool,
}

/// Execute an arbitrary SQL statement.
///
/// Statements that start with a row-returning keyword (`SELECT`, `WITH`,
/// `PRAGMA`, `EXPLAIN`) are fetched into a table; everything else is executed
/// and reports `rows_affected`.
pub async fn run_console_query(pool: &DbPool, sql: &str) -> Result<ConsoleQueryResult, DbError> {
    let sql = intern(sql.trim().trim_end_matches(';').trim());

    if returns_rows(sql) {
        let fetched = sqlx::query(sql).persistent(false).fetch_all(pool).await?;
        Ok(rows_to_result(&fetched))
    } else {
        let done = sqlx::query(sql).persistent(false).execute(pool).await?;
        Ok(ConsoleQueryResult {
            rows_affected: done.rows_affected(),
            is_select: false,
            ..Default::default()
        })
    }
}

fn returns_rows(sql: &str) -> bool {
    let head = sql
        .trim_start()
        .split(|c: char| c.is_whitespace() || c == '(')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();

    matches!(head.as_str(), "SELECT" | "WITH" | "PRAGMA" | "EXPLAIN")
}

fn rows_to_result(fetched: &[SqliteRow]) -> ConsoleQueryResult {
    let columns = fetched
        .first()
        .map(|row| {
            row.columns()
                .iter()
                .map(|c| c.name().to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let rows = fetched
        .iter()
        .map(|row| {
            (0..row.columns().len())
                .map(|idx| cell_to_string(row, idx))
                .collect()
        })
        .collect();

    ConsoleQueryResult {
        columns,
        rows,
        rows_affected: 0,
        is_select: true,
    }
}

/// SQLite is dynamically typed, so probe the raw value and decode accordingly.
fn cell_to_string(row: &SqliteRow, idx: usize) -> String {
    match row.try_get_raw(idx) {
        Ok(raw) if raw.is_null() => return "NULL".to_string(),
        Ok(_) => {}
        Err(_) => return "<err>".to_string(),
    }

    if let Ok(v) = row.try_get::<String, _>(idx) {
        return v;
    }
    if let Ok(v) = row.try_get::<i64, _>(idx) {
        return v.to_string();
    }
    if let Ok(v) = row.try_get::<f64, _>(idx) {
        return v.to_string();
    }
    if let Ok(v) = row.try_get::<Vec<u8>, _>(idx) {
        return format!("<blob {} bytes>", v.len());
    }

    "<?>".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn pool() -> DbPool {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT, ratio REAL)")
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    #[tokio::test]
    async fn select_returns_columns_and_rows() {
        let pool = pool().await;
        sqlx::query("INSERT INTO t (name, ratio) VALUES ('a', 1.5), (NULL, 2.0)")
            .execute(&pool)
            .await
            .unwrap();

        let res = run_console_query(&pool, "SELECT id, name, ratio FROM t ORDER BY id;")
            .await
            .unwrap();

        assert!(res.is_select);
        assert_eq!(res.columns, vec!["id", "name", "ratio"]);
        assert_eq!(res.rows.len(), 2);
        assert_eq!(res.rows[0], vec!["1", "a", "1.5"]);
        assert_eq!(res.rows[1][1], "NULL");
    }

    #[tokio::test]
    async fn non_select_reports_rows_affected() {
        let pool = pool().await;
        let res = run_console_query(&pool, "INSERT INTO t (name) VALUES ('x'), ('y')")
            .await
            .unwrap();

        assert!(!res.is_select);
        assert_eq!(res.rows_affected, 2);
        assert!(res.rows.is_empty());
    }

    #[tokio::test]
    async fn invalid_sql_errors() {
        let pool = pool().await;
        assert!(run_console_query(&pool, "SELECT * FROM nope").await.is_err());
    }
}
