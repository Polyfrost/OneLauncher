use onelauncher_entity::loader::GameLoader;
use onelauncher_entity::package::PackageType;
use onelauncher_entity::{cluster_packages, clusters, packages};
use sea_orm::{
	ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, TryInsertResult,
};

use crate::error::{DaoError, LauncherResult};
use crate::store::State;

use super::PackageError;

pub type PackageId = String;

/// Inserts a new package into the database.
pub async fn insert_package(package: packages::ActiveModel) -> LauncherResult<packages::Model> {
	let state = State::get().await?;
	let db = &state.db;

	Ok(package.insert(db).await?)
}

/// Links a package to a cluster in database.
pub async fn link_package_to_cluster(
	package: &packages::Model,
	cluster: &clusters::Model,
) -> LauncherResult<()> {
	let state = State::get().await?;
	let db = &state.db;

	let cluster_package = cluster_packages::Model {
		cluster_id: cluster.id,
		package_hash: package.hash.clone(),
	}
	.into_active_model();

	cluster_packages::Entity::insert(cluster_package)
		.exec(db)
		.await?;

	Ok(())
}

/// Links multiple packages to a cluster in database. Returns the number of packages linked.
pub async fn link_many_packages_to_cluster(
	packages: &[packages::Model],
	cluster: &clusters::Model,
) -> LauncherResult<u64> {
	let state = State::get().await?;
	let db = &state.db;

	let cluster_packages = packages
		.iter()
		.map(|package| {
			cluster_packages::Model {
				cluster_id: cluster.id,
				package_hash: package.hash.clone(),
			}
			.into_active_model()
		})
		.collect::<Vec<_>>();

	let inserted = cluster_packages::Entity::insert_many(cluster_packages)
		.on_conflict_do_nothing()
		.exec_without_returning(db)
		.await?;

	Ok(match inserted {
		TryInsertResult::Inserted(rows) => rows,
		_ => 0,
	})
}

/// Retrieves a clusters linked packages from the database.
pub async fn get_linked_packages(
	cluster: &clusters::Model,
) -> LauncherResult<Vec<packages::Model>> {
	let state = State::get().await?;
	let db = &state.db;

	let linked_packages = cluster_packages::Entity::find()
		.filter(cluster_packages::Column::ClusterId.eq(cluster.id))
		.find_with_related(packages::Entity)
		.all(db)
		.await?
		.into_iter()
		.flat_map(|(_cluster_packages, packages)| packages)
		.collect::<Vec<_>>();

	Ok(linked_packages)
}

/// Is the package linked to the cluster?
pub async fn is_package_linked_to_cluster(
	package: &packages::Model,
	cluster: &clusters::Model,
) -> LauncherResult<bool> {
	let state = State::get().await?;
	let db = &state.db;

	let linked_packages = cluster_packages::Entity::find()
		.filter(cluster_packages::Column::ClusterId.eq(cluster.id))
		.filter(cluster_packages::Column::PackageHash.eq(package.hash.clone()))
		.one(db)
		.await?;

	Ok(linked_packages.is_some())
}

/// Retrieves a package by its hash from the database.
pub async fn get_package_by_hash(hash: PackageId) -> LauncherResult<Option<packages::Model>> {
	let state = State::get().await?;
	let db = &state.db;

	Ok(packages::Entity::find_by_id(hash).one(db).await?)
}

/// Retrieves all packages that match the package type, loader and version(s) in the database.
pub async fn get_packages_for(
	package_type: PackageType,
	loader: GameLoader,
	versions: Vec<String>,
) -> LauncherResult<Vec<packages::Model>> {
	let state = State::get().await?;
	let db = &state.db;

	Ok(packages::Entity::find()
		.filter(packages::Column::PackageType.eq(package_type))
		.filter(packages::Column::McLoader.eq(loader))
		.all(db)
		.await?
		.into_iter()
		.filter(|p| {
			let mc_versions = p.mc_versions.clone();
			versions.iter().any(|v| mc_versions.0.contains(v))
		})
		.collect())
}

/// Updates an existing package in the database.
pub async fn update_package_by_hash<B>(id: PackageId, block: B) -> LauncherResult<packages::Model>
where
	B: AsyncFnOnce(packages::ActiveModel) -> LauncherResult<packages::ActiveModel>,
{
	let state = State::get().await?;
	let db = &state.db;

	let model = get_package_by_hash(id).await?.ok_or(DaoError::NotFound)?;
	let model = block(model.into_active_model()).await?;
	let model = model.update(db).await?;

	Ok(model)
}

/// Updates an existing package in the database.
pub async fn update_package<B>(
	package: &mut packages::Model,
	block: B,
) -> LauncherResult<&mut packages::Model>
where
	B: AsyncFnOnce(packages::ActiveModel) -> LauncherResult<packages::ActiveModel>,
{
	let state = State::get().await?;
	let db = &state.db;

	let model = package.clone().into_active_model();
	let model = block(model).await?;
	let model = model.update(db).await?;

	*package = model;

	Ok(package)
}

/// Deletes a package by its hash from the **database**.
pub async fn delete_package_by_id(id: PackageId) -> LauncherResult<()> {
	let state = State::get().await?;
	let db = &state.db;

	let deleted = packages::Entity::delete_by_id(id).exec(db).await?;

	if deleted.rows_affected == 0 {
		return Err(DaoError::NotFound.into());
	}

	Ok(())
}
