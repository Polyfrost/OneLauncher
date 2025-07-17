use onelauncher_core::{api::{cluster, packages::{self, data::{SearchQuery, Sort}, download_package, provider::ProviderExt}, proxy::ProxyDynamic}, error::LauncherResult, initialize_core, store::{CoreOptions, Dirs}};
use onelauncher_entity::{loader::GameLoader, package::Provider};

#[tokio::main]
pub async fn main() -> LauncherResult<()> {
	initialize_core(CoreOptions::default(), ProxyDynamic::default()).await?;

	let dirs = Dirs::get().await?;

	// Here we create the cluster
	let cluster_name = "Chatting Profile";
	let path = dirs.clusters_dir().join(cluster_name);

	let cluster = if let Some(cluster) = cluster::dao::get_cluster_by_folder_name(&path).await? {
		cluster
	} else {
		cluster::create_cluster(cluster_name, "1.8.9", GameLoader::Forge, None, None).await?
	};

	// Fetch our package and its versions
	let provider = Provider::Modrinth;

	println!("search results: {:#?}", provider.search(&SearchQuery {
		query: Some("chatting".to_string()),
		offset: Some(0),
		limit: Some(10),
		sort: Some(Sort::Newest),
		filters: None,
	}).await?);

	let package = &provider.get("chatting").await?;
	let versions = provider.get_versions(package.version_ids.as_slice()).await?;

	let version = versions.iter().find(|v| v.mc_versions.contains(&"1.8.9".to_string())).expect("Version supporting 1.8.9 not found");

	// We can now download the package
	let model = download_package(package, version, None).await?;

	// Link the package to the cluster
	packages::link_package(&model, &cluster, None).await?;

	// Check if the package is hard linked
	let cluster_dir = dirs.clusters_dir().join(cluster.folder_name.clone());
	let pkg_path = cluster_dir.join(model.package_type.folder_name()).join(model.file_name.as_str());
	assert!(tokio::fs::metadata(&pkg_path).await.is_ok(), "Package not found in cluster directory");

	Ok(())
}