use std::{collections::HashMap, path::PathBuf};

use onelauncher_entity::java_versions;
use sea_orm::{ActiveValue::Set, ActiveModelTrait, ColumnTrait, EntityTrait, Order, QueryFilter, QueryOrder};
use serde::Serialize;

use crate::{api::ingress::{init_ingress, send_ingress}, store::{ingress::IngressType, State}, utils::io::{self, IOError}, LauncherResult};

pub async fn get_java_all() -> LauncherResult<Vec<java_versions::Model>> {
	let db = &State::get().await?.db;

	Ok(java_versions::Entity::find()
		.order_by(java_versions::Column::MajorVersion, Order::Desc)
		.all(db)
		.await?)
}

pub async fn get_java_by_id(id: i32) -> LauncherResult<Option<java_versions::Model>> {
	let db = &State::get().await?.db;

	Ok(java_versions::Entity::find()
		.filter(java_versions::Column::Id.eq(id))
		.one(db)
		.await?)
}

pub async fn get_java_by_major(major: i32) -> LauncherResult<Vec<java_versions::Model>> {
	let db = &State::get().await?.db;

	Ok(java_versions::Entity::find()
		.filter(java_versions::Column::MajorVersion.eq(major))
		.order_by(java_versions::Column::FullVersion, Order::Desc)
		.all(db)
		.await?)
}

pub async fn insert_java(absolute_path: PathBuf) -> LauncherResult<java_versions::Model> {
	let db = &State::get().await?.db;

	let java_info = check_java_runtime(&absolute_path).await?;

	let major_version = if java_info.java_version[0..2].eq("1.") {
		java_info.java_version[2..3].parse::<i32>().unwrap_or(0)
	} else {
		let split = java_info.java_version.split_once('.').unwrap_or_default();
		split.0.parse::<i32>().unwrap_or(0)
	};

	let model = java_versions::ActiveModel {
		absolute_path: Set(absolute_path.to_string_lossy().to_string()),
		major_version: Set(major_version),
		full_version: Set(java_info.java_version.clone()),
		vendor_name: Set(java_info.java_vendor.clone()),
		..Default::default()
	};

	Ok(model.insert(db).await?)
}

#[onelauncher_macro::specta]
#[derive(Debug, Clone, Serialize)]
pub struct JavaInfo {
	pub os_arch: String,
	pub java_version: String,
	pub java_vendor: String,
}

const JAVA_INFO_CLASS: &[u8] = include_bytes!("../../assets/java/JavaInfo.class");

pub async fn check_java_runtime(absolute_path: &PathBuf) -> LauncherResult<JavaInfo> {
	let id = init_ingress(IngressType::JavaCheck, "Checking JRE information", 100.0).await?;

	let dir = io::tempdir()?;
	let file = dir.path().join("JavaInfo.class");
	send_ingress(&id, 25.0).await?;

	io::write(&file, JAVA_INFO_CLASS).await?;
	send_ingress(&id, 25.0).await?;

	let java_info = tokio::process::Command::new(absolute_path)
		.arg("-cp")
		.arg(dir.path())
		.arg("JavaInfo")
		.env_remove("_JAVA_OPTIONS")
		.output()
		.await
		.map_err(IOError::from)?;

	let java_info = String::from_utf8_lossy(&java_info.stdout);
	send_ingress(&id, 50.0).await?;

	let info = java_info.lines()
		.map(|line| {
			let mut parts = line.splitn(2, '=');
			let key = parts.next().unwrap_or("unknown");
			let value = parts.next().unwrap_or("unknown");

			(key.to_string(), value.to_string())
		})
		.collect::<HashMap<_, _>>();

	Ok(JavaInfo {
		os_arch: info.get("os.arch").cloned().unwrap_or_else(|| String::from("unknown")),
		java_version: info.get("java.version").cloned().unwrap_or_else(|| String::from("unknown")),
		java_vendor: info.get("java.vendor").cloned().unwrap_or_else(|| String::from("unknown")),
	})
}

