use onelauncher_core::api::proxy::ProxyDynamic;
use onelauncher_core::api::{self};
use onelauncher_core::error::LauncherResult;
use onelauncher_core::initialize_core;
use onelauncher_core::store::CoreOptions;
use onelauncher_entity::loader::GameLoader;

#[tokio::main]
async fn main() -> LauncherResult<()> {
	initialize_core(CoreOptions::default(), ProxyDynamic::new()).await?;

	println!("syncing clusters");
	let missing = api::cluster::sync_clusters().await?;
	if !missing.is_empty() {
		println!("missing clusters: {missing:#?}");
	}

	println!("creating cluster");
	let cluster =
		&mut api::cluster::create_cluster("Test Cluster", "1.8.9", GameLoader::Vanilla, None, None)
			.await?;

	println!("cluster created: {cluster:#?}");

	println!("preparing cluster for launch");
	api::cluster::prepare_cluster(cluster, None, Some(true)).await?;

	Ok(())
}
