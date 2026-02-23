use chrono::Utc;
use onelauncher_entity::{cluster_packages, clusters, packages};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

use crate::error::{DaoError, LauncherResult};
use crate::store::State;

pub async fn track_bundle_package(
	cluster: &clusters::Model,
	package: &packages::Model,
	bundle_name: &str,
	bundle_version_id: &str,
) -> LauncherResult<cluster_packages::Model> {
	let state = State::get().await?;
	let db = &state.db;

	let existing = cluster_packages::Entity::find_by_id((cluster.id, package.hash.clone()))
		.one(db)
		.await?;

	let model = if let Some(existing) = existing {
		let mut active: cluster_packages::ActiveModel = existing.into();
		active.bundle_name = Set(Some(bundle_name.to_string()));
		active.bundle_version_id = Set(Some(bundle_version_id.to_string()));
		active.package_id = Set(Some(package.package_id.clone()));
		active.installed_at = Set(Some(Utc::now()));
		active.update(db).await?
	} else {
		let active = cluster_packages::ActiveModel {
			cluster_id: Set(cluster.id),
			package_hash: Set(package.hash.clone()),
			bundle_name: Set(Some(bundle_name.to_string())),
			bundle_version_id: Set(Some(bundle_version_id.to_string())),
			package_id: Set(Some(package.package_id.clone())),
			installed_at: Set(Some(Utc::now())),
		};
		active.insert(db).await?
	};

	Ok(model)
}

pub async fn get_bundle_packages_for_cluster(
	cluster_id: i64,
) -> LauncherResult<Vec<cluster_packages::Model>> {
	let state = State::get().await?;
	let db = &state.db;

	let records = cluster_packages::Entity::find()
		.filter(cluster_packages::Column::ClusterId.eq(cluster_id))
		.filter(cluster_packages::Column::BundleName.is_not_null())
		.all(db)
		.await?;

	Ok(records)
}

pub async fn get_packages_from_bundle(
	cluster_id: i64,
	bundle_name: &str,
) -> LauncherResult<Vec<cluster_packages::Model>> {
	let state = State::get().await?;
	let db = &state.db;

	let records = cluster_packages::Entity::find()
		.filter(cluster_packages::Column::ClusterId.eq(cluster_id))
		.filter(cluster_packages::Column::BundleName.eq(Some(bundle_name.to_string())))
		.all(db)
		.await?;

	Ok(records)
}

pub async fn get_bundle_package(
	cluster_id: i64,
	package_hash: &str,
) -> LauncherResult<Option<cluster_packages::Model>> {
	let state = State::get().await?;
	let db = &state.db;

	let record = cluster_packages::Entity::find_by_id((cluster_id, package_hash.to_string()))
		.filter(cluster_packages::Column::BundleName.is_not_null())
		.one(db)
		.await?;

	Ok(record)
}

pub async fn get_bundle_package_by_package_id(
	cluster_id: i64,
	package_id: &str,
) -> LauncherResult<Option<cluster_packages::Model>> {
	let state = State::get().await?;
	let db = &state.db;

	let record = cluster_packages::Entity::find()
		.filter(cluster_packages::Column::ClusterId.eq(cluster_id))
		.filter(cluster_packages::Column::PackageId.eq(Some(package_id.to_string())))
		.filter(cluster_packages::Column::BundleName.is_not_null())
		.one(db)
		.await?;

	Ok(record)
}

pub async fn remove_bundle_package_tracking(
	cluster_id: i64,
	package_hash: &str,
) -> LauncherResult<()> {
	let state = State::get().await?;
	let db = &state.db;

	let existing = cluster_packages::Entity::find_by_id((cluster_id, package_hash.to_string()))
		.one(db)
		.await?;

	if let Some(existing) = existing {
		let mut active: cluster_packages::ActiveModel = existing.into();
		active.bundle_name = Set(None);
		active.bundle_version_id = Set(None);
		active.package_id = Set(None);
		active.installed_at = Set(None);
		active.update(db).await?;
		Ok(())
	} else {
		Err(DaoError::NotFound.into())
	}
}

pub async fn remove_all_bundle_packages_for_cluster(cluster_id: i64) -> LauncherResult<u64> {
	let state = State::get().await?;
	let db = &state.db;

	let records = cluster_packages::Entity::find()
		.filter(cluster_packages::Column::ClusterId.eq(cluster_id))
		.filter(cluster_packages::Column::BundleName.is_not_null())
		.all(db)
		.await?;

	let count = records.len() as u64;

	for record in records {
		let mut active: cluster_packages::ActiveModel = record.into();
		active.bundle_name = Set(None);
		active.bundle_version_id = Set(None);
		active.package_id = Set(None);
		active.installed_at = Set(None);
		active.update(db).await?;
	}

	Ok(count)
}

pub async fn remove_bundle_packages_for_bundle(
	cluster_id: i64,
	bundle_name: &str,
) -> LauncherResult<u64> {
	let state = State::get().await?;
	let db = &state.db;

	let records = cluster_packages::Entity::find()
		.filter(cluster_packages::Column::ClusterId.eq(cluster_id))
		.filter(cluster_packages::Column::BundleName.eq(Some(bundle_name.to_string())))
		.all(db)
		.await?;

	let count = records.len() as u64;

	for record in records {
		let mut active: cluster_packages::ActiveModel = record.into();
		active.bundle_name = Set(None);
		active.bundle_version_id = Set(None);
		active.package_id = Set(None);
		active.installed_at = Set(None);
		active.update(db).await?;
	}

	Ok(count)
}

pub async fn is_package_from_bundle(cluster_id: i64, package_hash: &str) -> LauncherResult<bool> {
	let record = get_bundle_package(cluster_id, package_hash).await?;
	Ok(record.is_some())
}
