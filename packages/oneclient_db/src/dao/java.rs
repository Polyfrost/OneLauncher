use sqlx::SqlitePool;

use crate::DbError;
use crate::models::JavaVersionRow;

pub async fn insert(
    pool: &SqlitePool,
    absolute_path: &str,
    major: u32,
    version: &str,
    vendor: &str,
    os_arch: &str,
) -> Result<JavaVersionRow, DbError> {
    sqlx::query_as!(
        JavaVersionRow,
        r#"
		INSERT INTO java_versions (absolute_path, major, version, vendor, os_arch)
		VALUES (?, ?, ?, ?, ?)
		ON CONFLICT(absolute_path) DO UPDATE SET
			major = excluded.major,
			version = excluded.version,
			vendor = excluded.vendor,
			os_arch = excluded.os_arch
		RETURNING absolute_path, major, version, vendor, os_arch
		"#,
        absolute_path,
        i64::from(major),
        version,
        vendor,
        os_arch,
    )
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

pub async fn get_by_path(
    pool: &SqlitePool,
    absolute_path: &str,
) -> Result<Option<JavaVersionRow>, DbError> {
    let row = sqlx::query_as!(
        JavaVersionRow,
        r#"
        SELECT absolute_path, major, version, vendor, os_arch FROM java_versions 
        WHERE absolute_path = ?
        "#,
        absolute_path,
    )
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn get_latest_by_major(
    pool: &SqlitePool,
    major: u32,
) -> Result<Option<JavaVersionRow>, DbError> {
    let row = sqlx::query_as!(
        JavaVersionRow,
        r#"
		SELECT absolute_path, major, version, vendor, os_arch
		FROM java_versions
		WHERE major = ?
		ORDER BY version DESC
		LIMIT 1
		"#,
        i64::from(major)
    )
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn delete_by_path(pool: &SqlitePool, absolute_path: &str) -> Result<(), DbError> {
    sqlx::query!(
        r#"
        DELETE FROM java_versions WHERE absolute_path = ?
        "#,
        absolute_path,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_all(pool: &SqlitePool) -> Result<Vec<JavaVersionRow>, DbError> {
    let rows = sqlx::query_as!(
        JavaVersionRow,
        r#"
        SELECT absolute_path, major, version, vendor, os_arch FROM java_versions 
        ORDER BY major DESC, version DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
