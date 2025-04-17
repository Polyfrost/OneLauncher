use std::path::{Path, PathBuf};

use onelauncher_entity::java_versions;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, Order, QueryFilter, QueryOrder};

use crate::{error::LauncherResult, store::State};

use super::{JavaError, JavaInfo};

/// Returns the latest Java version
pub async fn get_latest_java() -> LauncherResult<Option<java_versions::Model>> {
	let db = &State::get().await?.db;

	Ok(java_versions::Entity::find()
		.order_by(java_versions::Column::MajorVersion, Order::Desc)
		.order_by(java_versions::Column::FullVersion, Order::Desc)
		.one(db)
		.await?)
}

/// Accepts a path to a JRE folder and a [`JavaInfo`] and converts it to a [`java_versions::ActiveModel`]
pub fn get_java_model(absolute_path: &Path, info: &JavaInfo) -> LauncherResult<java_versions::ActiveModel> {
	let major_version: u32 = if info.java_version[0..2].eq("1.") {
		info.java_version[2..3].parse()
	} else {
		let split = info.java_version.split_once('.').unwrap_or_default();
		split.0.parse()
	}.or_else(
		|_| info.java_version.parse()
	).map_err(|e| JavaError::ParseVersion(info.java_version.clone(), e))?;

	Ok(java_versions::ActiveModel {
		absolute_path: Set(absolute_path.to_string_lossy().to_string()),
		major_version: Set(major_version),
		full_version: Set(info.java_version.clone()),
		vendor_name: Set(info.java_vendor.clone()),
		arch: Set(info.os_arch.clone()),
		..Default::default()
	})
}

/// Returns all Java versions
pub async fn get_java_all() -> LauncherResult<Vec<java_versions::Model>> {
	let db = &State::get().await?.db;

	Ok(java_versions::Entity::find()
		.order_by(java_versions::Column::MajorVersion, Order::Desc)
		.all(db)
		.await?)
}

/// Returns the specific Java version by ID
pub async fn get_java_by_id(id: u64) -> LauncherResult<Option<java_versions::Model>> {
	let db = &State::get().await?.db;

	Ok(java_versions::Entity::find()
		.filter(java_versions::Column::Id.eq(id))
		.one(db)
		.await?)
}

/// Returns all versions of Java for a given major version
pub async fn get_all_java_by_major(major: u32) -> LauncherResult<Vec<java_versions::Model>> {
	let db = &State::get().await?.db;

	Ok(java_versions::Entity::find()
		.filter(java_versions::Column::MajorVersion.eq(major))
		.order_by(java_versions::Column::FullVersion, Order::Desc)
		.all(db)
		.await?)
}

/// Returns the latest Java version for a given major version
pub async fn get_latest_java_by_major(major: u32) -> LauncherResult<Option<java_versions::Model>> {
	let db = &State::get().await?.db;

	Ok(java_versions::Entity::find()
		.filter(java_versions::Column::MajorVersion.eq(major))
		.order_by(java_versions::Column::FullVersion, Order::Desc)
		.one(db)
		.await?)
}

/// Accepts a path to a JRE folder and returns the inserted entry
pub async fn insert_java(absolute_path: PathBuf, info: JavaInfo) -> LauncherResult<java_versions::Model> {
	let db = &State::get().await?.db;

	let model = get_java_model(&absolute_path, &info)?;

	Ok(model.insert(db).await?)
}

/// Accepts a path to JRE folders and returns the inserted entries
pub async fn insert_java_many(java: Vec<(PathBuf, JavaInfo)>) -> LauncherResult<Vec<java_versions::Model>> {
	let db = &State::get().await?.db;

	let mut models = Vec::new();
	for (absolute_path, info) in java {
		let model = get_java_model(&absolute_path, &info)?;
		models.push(model);
	}

	Ok(java_versions::Entity::insert_many(models)
		.exec_with_returning_many(db)
		.await?)
}