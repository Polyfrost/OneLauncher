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
		       hook_pre, hook_wrapper, hook_post, os_extra, browser_update_mode
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
		       hook_pre, hook_wrapper, hook_post, os_extra, browser_update_mode
		FROM setting_profiles
		WHERE name = ?
		"#,
        name,
    )
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn replace_mem_max(pool: &SqlitePool, from: u32, to: u32) -> DbResult<u64> {
    let (from, to) = (i64::from(from), i64::from(to));

    let result = sqlx::query!(
        r#"
		UPDATE setting_profiles
		SET mem_max = ?
		WHERE mem_max = ?
		"#,
        to,
        from,
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
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
			hook_pre, hook_wrapper, hook_post, os_extra, browser_update_mode
		)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
			os_extra = excluded.os_extra,
			browser_update_mode = excluded.browser_update_mode
		RETURNING name, java_path, resolution, force_fullscreen, mem_max, launch_args, launch_env,
                  hook_pre, hook_wrapper, hook_post, os_extra, browser_update_mode
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
        &row.browser_update_mode,
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

#[cfg(test)]
mod tests {
    use super::*;

    async fn pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        sqlx::migrate!().run(&pool).await.expect("migrations run");
        pool
    }

    async fn seed(pool: &SqlitePool, name: &str, mem_max: Option<i64>) {
        upsert(
            pool,
            &SettingProfileRow {
                name: name.into(),
                java_path: None,
                resolution: None,
                force_fullscreen: None,
                mem_max,
                launch_args: None,
                launch_env: None,
                hook_pre: None,
                hook_wrapper: None,
                hook_post: None,
                os_extra: None,
                browser_update_mode: None,
            },
        )
        .await
        .expect("insert profile");
    }

    #[tokio::test]
    async fn only_the_legacy_heap_is_replaced() {
        let pool = pool().await;
        seed(&pool, "stale", Some(4096)).await;
        seed(&pool, "also stale", Some(4096)).await;
        seed(&pool, "chosen", Some(8192)).await;
        seed(&pool, "inherits", None).await;

        let replaced = replace_mem_max(&pool, 4096, 3072).await.expect("replace");
        assert_eq!(replaced, 2);

        let mem_max = |name: &'static str| {
            let pool = pool.clone();
            async move { get_by_name(&pool, name).await.unwrap().unwrap().mem_max }
        };

        assert_eq!(mem_max("stale").await, Some(3072));
        assert_eq!(mem_max("also stale").await, Some(3072));
        assert_eq!(mem_max("chosen").await, Some(8192));
        assert_eq!(mem_max("inherits").await, None);
    }
}
