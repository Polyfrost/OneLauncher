use std::path::PathBuf;

use onelauncher::{cluster, data::{Loader, PackageData}};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[macro_export]
macro_rules! collect_commands {
    () => {
        {
            use $crate::api::commands::*;
            tauri_specta::ts::builder()
                .commands(tauri_specta::collect_commands![
                    is_dev,
                    create_cluster
                ])
        }
    };
}

#[specta::specta]
#[tauri::command]
pub fn is_dev() -> bool {
	cfg!(debug_assertions)
}

#[derive(Serialize, Deserialize, Type)]
pub struct CreateCluster {
    name: String,
	mc_version: String,
	mod_loader: Loader,
	loader_version: Option<String>,
	icon: Option<PathBuf>,
	icon_url: Option<String>,
	package_data: Option<PackageData>,
	skip: Option<bool>,
	skip_watch: Option<bool>,
}

#[specta::specta]
#[tauri::command]
pub async fn create_cluster(props: CreateCluster) -> Result<Uuid, String> {
    let path = cluster::create::create_cluster(
        props.name,
        props.mc_version,
        props.mod_loader,
        props.loader_version,
        props.icon,
        props.icon_url,
        props.package_data,
        props.skip,
        props.skip_watch,
    ).await?;

    if let Some(cluster) = cluster::get(&path, None).await? {
        Ok(cluster.uuid)
    } else {
        Err("Cluster does not exist".to_string())
    }
}
