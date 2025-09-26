use std::path::Path;

use onelauncher_core::entity::clusters::Model as ClusterModel;
use onelauncher_core::entity::loader::GameLoader;
use onelauncher_core::error::LauncherResult;
use onelauncher_core::send_error;
use onelauncher_core::utils::http::fetch_json;
use reqwest::Method;
use serde::Deserialize;

///
/// e.g.
/// ```json
/// {
/// 	"clusters": [
/// 		{
/// 			"major_version": 21,
/// 			"entries": [
/// 				{
/// 					"minor_version": 5,
/// 					"loader": "fabric"
/// 				},
/// 				{
/// 					"minor_version": 5,
/// 					"loader": "forge"
/// 				}
/// 			]
/// 		},
/// 		{
/// 			"major_version": 20,
/// 			"entries": [
/// 				{
/// 					"minor_version": 5,
/// 					"loader": "fabric"
/// 				}
/// 			]
/// 		}
/// 	]
/// }
/// ```
#[derive(Deserialize)]
struct OnlineClusterManifest {
	clusters: Vec<OnlineCluster>,
}

#[derive(Deserialize)]
struct OnlineCluster {
	major_version: u8,
	entries: Vec<OnlineClusterEntry>,
}

#[derive(Deserialize)]
struct OnlineClusterEntry {
	minor_version: u8,
	loader: GameLoader,
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
