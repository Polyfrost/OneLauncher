use sqlx::SqlitePool;

use crate::DbError;
use crate::error::DbResult;
use crate::models::SettingProfileRow;

const GLOBAL_PROFILE_NAME: &str = "Global";

pub fn is_reserved_global_name(name: &str) -> bool {
    name == GLOBAL_PROFILE_NAME
}

pub async fn list_all(pool: &SqlitePool) -> DbResult<Vec<SettingProfileRow>> {
    let rows = sqlx::query_as!(
        SettingProfileRow,
        r#"
		SELECT name, java_path, resolution, force_fullscreen, mem_max, launch_args, launch_env,
		       hook_pre, hook_wrapper, hook_post, os_extra
		FROM setting_profiles
		ORDER BY name ASC
		"#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn get_by_name(pool: &SqlitePool, name: &str) -> DbResult<Option<SettingProfileRow>> {
    let row = sqlx::query_as!(
        SettingProfileRow,
        r#"
		SELECT name, java_path, resolution, force_fullscreen, mem_max, launch_args, launch_env,
		       hook_pre, hook_wrapper, hook_post, os_extra
		FROM setting_profiles
		WHERE name = ?
		"#,
        name,
    )
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn upsert(
    pool: &SqlitePool,
    row: &SettingProfileRow,
) -> Result<SettingProfileRow, DbError> {
    if row.name.is_empty() || is_reserved_global_name(&row.name) {
        return Err(DbError::InvalidValue {
            field: "name".into(),
            value: row.name.clone(),
        });
    }

    sqlx::query_as!(
        SettingProfileRow,
        r#"
		INSERT INTO setting_profiles (
			name, java_path, resolution, force_fullscreen, mem_max, launch_args, launch_env,
			hook_pre, hook_wrapper, hook_post, os_extra
		)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(name) DO UPDATE SET
			java_path = excluded.java_path,
			resolution = excluded.resolution,
			force_fullscreen = excluded.force_fullscreen,
			mem_max = excluded.mem_max,
			launch_args = excluded.launch_args,
			launch_env = excluded.launch_env,
			hook_pre = excluded.hook_pre,
			hook_wrapper = excluded.hook_wrapper,
			hook_post = excluded.hook_post,
			os_extra = excluded.os_extra
		RETURNING name, java_path, resolution, force_fullscreen, mem_max, launch_args, launch_env,
                  hook_pre, hook_wrapper, hook_post, os_extra
		"#,
        &row.name,
        &row.java_path,
        &row.resolution,
        row.force_fullscreen,
        row.mem_max,
        &row.launch_args,
        &row.launch_env,
        &row.hook_pre,
        &row.hook_wrapper,
        &row.hook_post,
        &row.os_extra,
    )
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

pub async fn delete_by_name(pool: &SqlitePool, name: &str) -> Result<(), DbError> {
    let result = sqlx::query!(r#"DELETE FROM setting_profiles WHERE name = ?"#, name,)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(DbError::NotFound);
    }

    Ok(())
}
