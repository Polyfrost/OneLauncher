use onelauncher_core::{api::{self, proxy::ProxyDynamic}, error::LauncherResult, initialize_core, store::CoreOptions};
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
	let cluster = api::cluster::create_cluster(
		"Test Cluster",
		"1.8.9",
		GameLoader::Vanilla,
		None,
		None,
	).await?;

	println!("cluster created: {cluster:#?}");

	println!("preparing cluster for launch");
	api::cluster::prepare_cluster(&cluster).await?;

	Ok(())
}