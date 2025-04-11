use std::path::Path;

use onelauncher_entity::{cluster_groups, clusters, icon::Icon, loader::GameLoader};
use sea_orm::{ActiveValue::Set, IntoActiveModel, prelude::*};

use crate::{error::{DaoError, LauncherResult}, store::State};

/// Inserts a cluster in the database.
pub async fn insert_cluster(
	name: &str,
	path: &str,
	mc_version: &str,
	mc_loader: GameLoader,
	mc_loader_version: Option<&str>,
	icon_url: Option<Icon>,
) -> LauncherResult<clusters::Model> {
	let state = State::get().await?;
	let db = &state.db;

	let model = clusters::ActiveModel {
		name: Set(name.to_string()),
		path: Set(path.to_string()),
		mc_version: Set(mc_version.to_string()),
		mc_loader: Set(mc_loader),
		mc_loader_version: Set(mc_loader_version.map(String::from)),
		icon_url: Set(icon_url),
		..Default::default()
	};

	let model = model.insert(db).await?;

	Ok(model)
}

/// Updates an existing cluster in the database.
pub async fn update_cluster_by_id<B>(
	id: i32,
	block: B,
) -> LauncherResult<clusters::Model>
where B: AsyncFnOnce(clusters::ActiveModel) -> LauncherResult<clusters::ActiveModel> {
	let state = State::get().await?;
	let db = &state.db;

	let model = get_cluster_by_id(id).await?.ok_or(DaoError::NotFound)?;
	let model = block(model.into_active_model()).await?;
	let model = model.update(db).await?;

	Ok(model)
}

/// Deletes a cluster by its ID from the **database**.
pub async fn delete_cluster_by_id(id: i32) -> LauncherResult<()> {
	let state = State::get().await?;
	let db = &state.db;

	let deleted = clusters::Entity::delete_by_id(id)
		.exec(db)
		.await?;

	if deleted.rows_affected == 0 {
		return Err(DaoError::NotFound.into());
	}

	Ok(())
}

/// Gets a cluster by its ID from the database.
pub async fn get_cluster_by_id(id: i32) -> LauncherResult<Option<clusters::Model>> {
	let state = State::get().await?;
	let db = &state.db;

	let model = clusters::Entity::find_by_id(id)
		.one(db)
		.await?;

	Ok(model)
}

/// Gets a cluster by its path from the database.
pub async fn get_cluster_by_path(path: &Path) -> LauncherResult<Option<clusters::Model>> {
	let state = State::get().await?;
	let db = &state.db;

	let model = clusters::Entity::find()
		.filter(clusters::Column::Path.eq(path.to_string_lossy().to_string()))
		.one(db)
		.await?;

	Ok(model)
}

/// Gets all clusters from the database.
pub async fn get_all_clusters() -> LauncherResult<Vec<clusters::Model>> {
	let state = State::get().await?;
	let db = &state.db;

	let clusters = clusters::Entity::find()
		.all(db)
		.await?;

	Ok(clusters)
}

/// Gets all clusters grouped by their group from the database.
pub async fn get_clusters_grouped() -> LauncherResult<Vec<(cluster_groups::Model, Vec<clusters::Model>)>> {
	let state = State::get().await?;
	let db = &state.db;

	let groups = cluster_groups::Entity::find()
		.all(db)
		.await?;

	let clusters = groups.load_many(clusters::Entity, db)
		.await?;

	let mut grouped = Vec::new();

	for (index, group) in groups.iter().enumerate() {
		let group_clusters: Vec<clusters::Model> = clusters.get(index).cloned().unwrap_or_else(Vec::new);

		grouped.push((group.clone(), group_clusters));
	}

	Ok(grouped)
}
