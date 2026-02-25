use std::path::Path;

use onelauncher_core::entity::clusters::Model as ClusterModel;
use onelauncher_core::entity::loader::GameLoader;
use onelauncher_core::error::LauncherResult;
use onelauncher_core::send_error;
use onelauncher_core::store::Dirs;
use onelauncher_core::utils::http::{fetch, fetch_json};
use reqwest::Method;
use serde::{Deserialize, Serialize};

///
/// e.g.
/// ```json
/// {
/// 	"clusters": [
/// 		{
/// 			"major_version": 21,
/// 			"name": "Tricky Trials",
/// 			"art": "/versions/art/Tricky_Trials.png",
/// 			"entries": [
/// 				{
/// 					"minor_version": 5,
/// 					"loader": "fabric",
/// 					"tags": ["PvP", "Survival"]
/// 				},
/// 				{
/// 					"minor_version": 5,
/// 					"loader": "forge",
/// 					"tags": ["PvP", "Survival"]
/// 				}
/// 			]
/// 		},
/// 		{
/// 			"major_version": 20,
/// 			"name": "Trails & Tales",
/// 			"art": "/versions/art/Trails_Tales.png",
/// 			"entries": [
/// 				{
/// 					"minor_version": 5,
/// 					"loader": "fabric",
/// 					"tags": ["PvP", "Survival"]
/// 				}
/// 			]
/// 		}
/// 	]
/// }
/// ```
#[derive(specta::Type, Deserialize, Serialize)]
pub struct OnlineClusterManifest {
	clusters: Vec<OnlineCluster>,
}

#[derive(specta::Type, Deserialize, Serialize)]
pub struct OnlineCluster {
	major_version: u8,
	name: String,
	art: String,
	entries: Vec<OnlineClusterEntry>,
}

#[derive(specta::Type, Deserialize, Serialize)]
pub struct OnlineClusterEntry {
	minor_version: u8,
	#[serde(skip_serializing_if = "Option::is_none")]
	name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	art: Option<String>,
	loader: GameLoader,
	tags: Vec<String>,
}

pub async fn get_data_storage_versions() -> LauncherResult<OnlineClusterManifest> {
	let manifest = match fetch_json::<OnlineClusterManifest>(
		Method::GET,
		&format!("{}/versions/versions.json", crate::constants::META_URL_BASE),
		None,
		None,
	)
	.await
	{
		Ok(m) => m,
		Err(e) => {
			send_error!("failed to fetch clusters manifest: {}", e);
			return Err(e);
		}
	};

	Ok(manifest)
}

/// Download an art image (e.g. `/versions/art/Foo.png`) from the data storage CDN and
/// save it to the local cache directory. Returns the OS-native path to the cached file,
/// which the frontend converts to an `asset://` URL via `convertFileSrc`.
/// If the file is already cached it is returned immediately without a network request.
pub async fn cache_art_image(path: &str) -> LauncherResult<String> {
	let art_dir = Dirs::get_caches_dir().await?.join("oneclient").join("art");
	tokio::fs::create_dir_all(&art_dir)
		.await
		.map_err(anyhow::Error::from)?;

	// Flatten "/versions/art/Foo.png" → "versions_art_Foo.png"
	let filename = path.trim_start_matches('/').replace('/', "_");
	let cached_path = art_dir.join(&filename);

	if cached_path.exists() {
		return Ok(cached_path.to_string_lossy().into_owned());
	}

	let url = format!("{}{}", crate::constants::META_URL_BASE, path);
	let bytes = fetch(Method::GET, &url).await?;
	tokio::fs::write(&cached_path, bytes)
		.await
		.map_err(anyhow::Error::from)?;

	Ok(cached_path.to_string_lossy().into_owned())
}

/// Re-download an art image and overwrite the on-disk cache.
/// Errors are silently ignored — the existing cached version is kept on failure.
/// Call this from `tokio::spawn` so it runs fully in the background.
pub async fn refresh_art_cache(path: &str) {
	let Ok(caches_dir) = Dirs::get_caches_dir().await else {
		return;
	};
	let art_dir = caches_dir.join("oneclient").join("art");
	let filename = path.trim_start_matches('/').replace('/', "_");
	let cached_path = art_dir.join(&filename);

	let url = format!("{}{}", crate::constants::META_URL_BASE, path);
	if let Ok(bytes) = fetch(Method::GET, &url).await {
		let _ = tokio::fs::write(&cached_path, bytes).await;
	}
}

pub async fn init_clusters() -> LauncherResult<()> {
	let manifest = match fetch_json::<OnlineClusterManifest>(
		Method::GET,
		&format!("{}/versions/versions.json", crate::constants::META_URL_BASE),
		None,
		None,
	)
	.await
	{
		Ok(m) => m,
		Err(e) => {
			send_error!("failed to fetch clusters manifest: {}", e);
			return Err(e);
		}
	};

	for cluster in manifest.clusters {
		for entry in cluster.entries {
			let mc_version = format!("1.{}.{}", cluster.major_version, entry.minor_version);

			if let Err(e) = create_cluster_if_not_exist(&mc_version, entry.loader, None).await {
				send_error!("failed to create cluster for {}: {}", mc_version, e);
			}
		}
	}

	Ok(())
}

fn cluster_name(mc_version: &str, loader: &GameLoader) -> String {
	format!("{mc_version} {loader}")
}

async fn create_cluster_if_not_exist(
	mc_version: &str,
	mc_loader: GameLoader,
	mc_loader_version: Option<&str>,
) -> LauncherResult<Option<ClusterModel>> {
	let name = cluster_name(mc_version, &mc_loader);

	let cluster =
		onelauncher_core::api::cluster::dao::get_cluster_by_folder_name(Path::new(&name)).await?;
	if cluster.is_some() {
		return Ok(cluster);
	}

	onelauncher_core::api::cluster::create_cluster(
		&name,
		mc_version,
		mc_loader,
		mc_loader_version,
		None,
	)
	.await
	.map(Some)
}
