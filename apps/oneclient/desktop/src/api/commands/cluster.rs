use onelauncher_core::{api::{cluster, setting_profiles}, entity::{clusters::Model, icon::Icon, loader::GameLoader}, migration::sea_orm};
use serde::{Deserialize, Serialize};
use specta::Type;
use crate::api::error::SerializableResult;

#[derive(Serialize, Deserialize, Type)]
pub struct CreateCluster {
	name: String,
	mc_version: String,
	mc_loader: GameLoader,
	mc_loader_version: Option<String>,
	icon_url: Option<Icon>
}

#[specta::specta]
#[tauri::command]
pub async fn create_cluster(options: CreateCluster) -> SerializableResult<Model> {
    let thing = cluster::create_cluster(&options.name, &options.mc_version, options.mc_loader, options.mc_loader_version.as_deref(), options.icon_url).await?;

	if setting_profiles::dao::get_profile_by_name(&options.name).await?.is_none() {
		setting_profiles::create_profile(&options.name, async |mut profile| {
			profile.mem_max = sea_orm::ActiveValue::Set(Some(2048));
			Ok(profile)
		}).await?;
	}

    Ok(thing)
}

#[specta::specta]
#[tauri::command]
pub async fn get_clusters() -> SerializableResult<Vec<Model>> {
    Ok(cluster::dao::get_all_clusters().await?)
}

#[specta::specta]
#[tauri::command]
pub async fn get_cluster_by_id(id: i64) -> SerializableResult<Option<Model>> {
    Ok(cluster::dao::get_cluster_by_id(id).await?)
}