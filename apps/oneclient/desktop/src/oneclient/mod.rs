use onelauncher_core::{entity::{clusters::Model as ClusterModel, icon::Icon, loader::GameLoader}, error::LauncherResult, send_error};

pub async fn initialize_oneclient() {
	init_clusters().await;
}

struct OnlineCluster {
	mc_major_version: u8,
	mc_minor_versions: Vec<u8>,
	mc_loaders: Vec<GameLoader>,
}

async fn init_clusters() {
	// TODO: use online metadata stuff
	let online_clusters: Vec<OnlineCluster> = vec![
		OnlineCluster {
			mc_major_version: 21,
			mc_minor_versions: vec![1, 2, 4, 7],
			mc_loaders: vec![GameLoader::Fabric, GameLoader::Forge],
		},
		OnlineCluster {
			mc_major_version: 8,
			mc_minor_versions: vec![9],
			mc_loaders: vec![GameLoader::Forge],
		},
	];

	for cluster in online_clusters {
		for minor_version in cluster.mc_minor_versions {
			let mc_version = format!("1.{}.{}", cluster.mc_major_version, minor_version);

			for loader in &cluster.mc_loaders {
				if let Err(e) = create_cluster_if_not_exist(
					&format!("{} {}", mc_version, loader),
					&mc_version,
					*loader,
					None,
					None,
				).await {
					send_error!("failed to create cluster for {}: {}", mc_version, e);
				}
			}
		}
	}
}

async fn create_cluster_if_not_exist(
	name: &str,
	mc_version: &str,
	mc_loader: GameLoader,
	mc_loader_version: Option<&str>,
	icon_url: Option<Icon>,
) -> LauncherResult<Option<ClusterModel>> {
	let clusters = onelauncher_core::api::cluster::dao::get_clusters_by_version(mc_version).await?;
	if clusters.len() > 1 {
		return Ok(None);
	}

	onelauncher_core::api::cluster::create_cluster(name, mc_version, mc_loader, mc_loader_version, icon_url)
		.await
		.map(Some)
}