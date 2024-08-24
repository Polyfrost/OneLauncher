use onelauncher::{cluster, data::{Loader, PackageType}, package::content::Providers, store::{Cluster, Package, PackagePath}, Result};

const CLUSTER_NAME: &str = "Test Packages";
const MODRINTH_PACKAGE: &str = "behindyou";
const MODRINTH_PACKAGE_VERSION: &str = "v3.2.2";

#[tokio::main]
async fn main() -> Result<()> {
	let mut cluster = create_cluster().await?;
	let (package_path, package) = download_crashpatch(&mut cluster).await?;
	cluster::content::package::add_package_to_cluster(package_path, package, &cluster, Some(PackageType::Mod)).await?;

	// cluster::sync_packages(&cluster.cluster_path(), true).await;

	let packages = cluster::content::package::get_packages_by_type(&cluster.cluster_path(), PackageType::Mod).await?;

	println!("Cluster Packages: {:#?}", packages);

	Ok(())
}

async fn create_cluster() -> Result<Cluster> {

	if let Some(cluster) = cluster::get_by_name(CLUSTER_NAME).await? {
		return Ok(cluster);
	}

	let cluster_path = cluster::create::create_cluster(
		CLUSTER_NAME.to_owned(),
		"1.8.9".to_owned(),
		Loader::Forge,
		None, None, None, None,
		Some(true), Some(false)
	).await?;

	let cluster = cluster::get(&cluster_path).await?.expect("Cluster not found");

	Ok(cluster)
}

async fn download_crashpatch(cluster: &mut Cluster) -> Result<(PackagePath, Package)> {
	let package = Providers::Modrinth.get(MODRINTH_PACKAGE).await?;

	cluster::content::package::download_package(&package, cluster, None, None, Some(MODRINTH_PACKAGE_VERSION.to_owned())).await
}
