//! The database side of [`oneclient_java::JavaStore`].
//!
//! `oneclient_java` defines what it needs; this maps it onto the launcher's
//! `java_versions` table. Keeping the adapter here rather than the trait there
//! lets the Java crate build without sqlx, and lets its tests run against
//! `MemoryJavaStore` instead of a real database file.

use std::str::FromStr;

use oneclient_db::{DbPool, dao, models::JavaVersionRow};
use oneclient_java::{JavaRuntime, JavaStore, JavaVendor, StoreError, StoreResult};

pub struct SqlJavaStore {
	pool: DbPool,
}

impl SqlJavaStore {
	#[must_use]
	pub fn new(pool: DbPool) -> Self {
		Self { pool }
	}
}

/// The one place a stored row becomes a domain value. Keeping it here spares
/// `JavaRuntime` from knowing the database's row type.
fn row_to_runtime(row: JavaVersionRow) -> JavaRuntime {
	JavaRuntime {
		absolute_path: row.absolute_path,
		major: row.major as u32,
		version: row.version,
		vendor: JavaVendor::from_str(&row.vendor).unwrap_or(JavaVendor::Other(row.vendor)),
		os_arch: row.os_arch,
	}
}

#[async_trait::async_trait]
impl JavaStore for SqlJavaStore {
	async fn upsert(&self, runtime: &JavaRuntime) -> StoreResult<JavaRuntime> {
		let row = dao::java::insert(
			&self.pool,
			&runtime.absolute_path,
			runtime.major,
			&runtime.version,
			&runtime.vendor.to_string(),
			&runtime.os_arch,
		)
		.await
		.map_err(StoreError::new)?;

		Ok(row_to_runtime(row))
	}

	async fn get_by_path(&self, absolute_path: &str) -> StoreResult<Option<JavaRuntime>> {
		Ok(dao::java::get_by_path(&self.pool, absolute_path)
			.await
			.map_err(StoreError::new)?
			.map(row_to_runtime))
	}

	async fn latest_by_major(&self, major: u32) -> StoreResult<Option<JavaRuntime>> {
		Ok(dao::java::get_latest_by_major(&self.pool, major)
			.await
			.map_err(StoreError::new)?
			.map(row_to_runtime))
	}

	async fn delete_by_path(&self, absolute_path: &str) -> StoreResult<()> {
		dao::java::delete_by_path(&self.pool, absolute_path)
			.await
			.map_err(StoreError::new)
	}

	async fn list(&self) -> StoreResult<Vec<JavaRuntime>> {
		Ok(dao::java::list_all(&self.pool)
			.await
			.map_err(StoreError::new)?
			.into_iter()
			.map(row_to_runtime)
			.collect())
	}
}
