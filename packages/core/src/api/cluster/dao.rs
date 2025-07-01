use std::path::Path;

use onelauncher_entity::{cluster_groups, clusters, icon::Icon, loader::GameLoader};
use sea_orm::{ActiveValue::Set, IntoActiveModel, prelude::*};

use crate::{error::{DaoError, LauncherResult}, store::State, utils::io};

pub type ClusterId = i64;

/// Inserts a cluster in the database.
pub async fn insert_cluster(
	name: &str,
	folder_name: &str,
	mc_version: &str,
	mc_loader: GameLoader,
	mc_loader_version: Option<&str>,
	icon_url: Option<Icon>,
) -> LauncherResult<clusters::Model> {
	let state = State::get().await?;
	let db = &state.db;

	let model = clusters::ActiveModel {
		name: Set(name.to_string()),
		folder_name: Set(folder_name.to_string()),
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
	id: ClusterId,
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

pub async fn update_cluster<B>(
	cluster: &mut clusters::Model,
	block: B,
) -> LauncherResult<&mut clusters::Model>
where B: AsyncFnOnce(clusters::ActiveModel) -> LauncherResult<clusters::ActiveModel> {
	let state = State::get().await?;
	let db = &state.db;

	let model = cluster.clone().into_active_model();
	let model = block(model).await?;
	let model = model.update(db).await?;

	*cluster = model;

	Ok(cluster)
}

/// Deletes a cluster by its ID from the **database**.
pub async fn delete_cluster_by_id(id: ClusterId) -> LauncherResult<()> {
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
pub async fn get_cluster_by_id(id: ClusterId) -> LauncherResult<Option<clusters::Model>> {
	let state = State::get().await?;
	let db = &state.db;

	let model = clusters::Entity::find_by_id(id)
		.one(db)
		.await?;

	Ok(model)
}

/// Gets a cluster by its path from the database.
pub async fn get_cluster_by_folder_name(folder_name: &Path) -> LauncherResult<Option<clusters::Model>> {
	let state = State::get().await?;
	let db = &state.db;

	let model = clusters::Entity::find()
		.filter(clusters::Column::FolderName.eq(folder_name.to_string_lossy().to_string()))
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

/// Caches an icon from a file path and updates the cluster to use the cached icon.
/// This function copies the image to the icon cache directory and sets the cluster's icon_url
/// to reference the cached version.
pub async fn set_cluster_icon_by_path(
	cluster: &mut clusters::Model,
	icon_path: &str,
) -> LauncherResult<()> {
	let icon_path = Path::new(icon_path);

	if !icon_path.exists() {
		return Err(anyhow::anyhow!("Icon file does not exist: {}", icon_path.display()).into());
	}

	let image_bytes = io::read(icon_path).await?;

	let hash = crate::utils::crypto::HashAlgorithm::Sha1.hash(&image_bytes).await?;

	let cache_dir = crate::store::Dirs::get_caches_dir().await?.join("icons");
	io::create_dir_all(&cache_dir).await?;

	let cached_file_path = cache_dir.join(format!("{}.png", hash));
	io::write(&cached_file_path, &image_bytes).await?;

	let file_icon = Icon::try_from_path(&cached_file_path)
		.ok_or_else(|| anyhow::anyhow!("Failed to create file icon from path: {}", cached_file_path.display()))?;

	update_cluster(cluster, |mut active_model: clusters::ActiveModel| async move {
		active_model.icon_url = Set(Some(file_icon));
		Ok(active_model)
	}).await?;

	Ok(())
}

/// Caches an icon from a file path and updates the cluster by ID to use the cached icon.
/// This function copies the image to the icon cache directory and sets the cluster's icon_url
/// to reference the cached version.
pub async fn set_icon_by_id(
	cluster_id: ClusterId,
	icon_path: &str,
) -> LauncherResult<clusters::Model> {
	let icon_path = Path::new(icon_path);

	if !icon_path.exists() {
		return Err(anyhow::anyhow!("Icon file does not exist: {}", icon_path.display()).into());
	}

	let image_bytes = io::read(icon_path).await?;

	let hash = crate::utils::crypto::HashAlgorithm::Sha1.hash(&image_bytes).await?;

	let cache_dir = crate::store::Dirs::get_caches_dir().await?.join("icons");
	io::create_dir_all(&cache_dir).await?;

	let cached_file_path = cache_dir.join(format!("{}.png", hash));
	io::write(&cached_file_path, &image_bytes).await?;

	let file_icon = Icon::try_from_path(&cached_file_path)
		.ok_or_else(|| anyhow::anyhow!("Failed to create file icon from path: {}", cached_file_path.display()))?;

	let updated_cluster = update_cluster_by_id(cluster_id, |mut active_model: clusters::ActiveModel| async move {
		active_model.icon_url = Set(Some(file_icon));
		Ok(active_model)
	}).await?;

	Ok(updated_cluster)
}
