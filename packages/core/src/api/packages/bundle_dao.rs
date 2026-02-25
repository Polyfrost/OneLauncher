use chrono::Utc;
use onelauncher_entity::{cluster_packages, clusters, packages};
use sea_orm::sea_query::{Expr, Value};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

use crate::error::{DaoError, LauncherResult};
use crate::store::State;
use onelauncher_entity::cluster_bundle_overrides::{self, OverrideType};

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

	let result = cluster_packages::Entity::update_many()
		.col_expr(
			cluster_packages::Column::BundleName,
			Expr::value(Value::String(None)),
		)
		.col_expr(
			cluster_packages::Column::BundleVersionId,
			Expr::value(Value::String(None)),
		)
		.col_expr(
			cluster_packages::Column::PackageId,
			Expr::value(Value::String(None)),
		)
		.col_expr(
			cluster_packages::Column::InstalledAt,
			Expr::value(Value::ChronoDateTimeUtc(None)),
		)
		.filter(cluster_packages::Column::ClusterId.eq(cluster_id))
		.filter(cluster_packages::Column::BundleName.is_not_null())
		.exec(db)
		.await?;

	Ok(result.rows_affected)
}

pub async fn remove_bundle_packages_for_bundle(
	cluster_id: i64,
	bundle_name: &str,
) -> LauncherResult<u64> {
	let state = State::get().await?;
	let db = &state.db;

	let result = cluster_packages::Entity::update_many()
		.col_expr(
			cluster_packages::Column::BundleName,
			Expr::value(Value::String(None)),
		)
		.col_expr(
			cluster_packages::Column::BundleVersionId,
			Expr::value(Value::String(None)),
		)
		.col_expr(
			cluster_packages::Column::PackageId,
			Expr::value(Value::String(None)),
		)
		.col_expr(
			cluster_packages::Column::InstalledAt,
			Expr::value(Value::ChronoDateTimeUtc(None)),
		)
		.filter(cluster_packages::Column::ClusterId.eq(cluster_id))
		.filter(cluster_packages::Column::BundleName.eq(Some(bundle_name.to_string())))
		.exec(db)
		.await?;

	Ok(result.rows_affected)
}

pub async fn is_package_from_bundle(cluster_id: i64, package_hash: &str) -> LauncherResult<bool> {
	let record = get_bundle_package(cluster_id, package_hash).await?;
	Ok(record.is_some())
}

pub async fn save_bundle_override(
	cluster_id: i64,
	bundle_name: &str,
	package_id: &str,
	override_type: OverrideType,
) -> LauncherResult<()> {
	let state = State::get().await?;
	let db = &state.db;

	let existing = cluster_bundle_overrides::Entity::find()
		.filter(cluster_bundle_overrides::Column::ClusterId.eq(cluster_id))
		.filter(cluster_bundle_overrides::Column::BundleName.eq(bundle_name))
		.filter(cluster_bundle_overrides::Column::PackageId.eq(package_id))
		.one(db)
		.await?;

	if let Some(existing) = existing {
		let mut active: cluster_bundle_overrides::ActiveModel = existing.into();
		active.override_type = Set(override_type);
		active.update(db).await?;
	} else {
		let active = cluster_bundle_overrides::ActiveModel {
			cluster_id: Set(cluster_id),
			bundle_name: Set(bundle_name.to_string()),
			package_id: Set(package_id.to_string()),
			override_type: Set(override_type),
			..Default::default()
		};
		active.insert(db).await?;
	}

	Ok(())
}

pub async fn remove_bundle_override(
	cluster_id: i64,
	bundle_name: &str,
	package_id: &str,
) -> LauncherResult<()> {
	let state = State::get().await?;
	let db = &state.db;

	cluster_bundle_overrides::Entity::delete_many()
		.filter(cluster_bundle_overrides::Column::ClusterId.eq(cluster_id))
		.filter(cluster_bundle_overrides::Column::BundleName.eq(bundle_name))
		.filter(cluster_bundle_overrides::Column::PackageId.eq(package_id))
		.exec(db)
		.await?;

	Ok(())
}

/// Returns whether a specific bundle package mapping still exists in the cluster.
///
/// Useful to avoid deleting overrides during update replacements where the old hash
/// is removed after a new hash has already been linked and tracked.
pub async fn has_bundle_package_mapping(
	cluster_id: i64,
	bundle_name: &str,
	package_id: &str,
) -> LauncherResult<bool> {
	let state = State::get().await?;
	let db = &state.db;

	let record = cluster_packages::Entity::find()
		.filter(cluster_packages::Column::ClusterId.eq(cluster_id))
		.filter(cluster_packages::Column::BundleName.eq(Some(bundle_name.to_string())))
		.filter(cluster_packages::Column::PackageId.eq(Some(package_id.to_string())))
		.one(db)
		.await?;

	Ok(record.is_some())
}

/// Removes all bundle overrides for a specific package in a cluster.
/// This is a broad cleanup helper that applies across all bundles.
pub async fn remove_overrides_for_package(
	cluster_id: i64,
	package_id: &str,
) -> LauncherResult<u64> {
	let state = State::get().await?;
	let db = &state.db;

	let result = cluster_bundle_overrides::Entity::delete_many()
		.filter(cluster_bundle_overrides::Column::ClusterId.eq(cluster_id))
		.filter(cluster_bundle_overrides::Column::PackageId.eq(package_id))
		.exec(db)
		.await?;

	Ok(result.rows_affected)
}

pub async fn get_bundle_overrides(
	cluster_id: i64,
) -> LauncherResult<Vec<cluster_bundle_overrides::Model>> {
	let state = State::get().await?;
	let db = &state.db;

	let overrides = cluster_bundle_overrides::Entity::find()
		.filter(cluster_bundle_overrides::Column::ClusterId.eq(cluster_id))
		.all(db)
		.await?;

	Ok(overrides)
}
