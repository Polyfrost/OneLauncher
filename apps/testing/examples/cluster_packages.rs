use onelauncher::data::{Loader, PackageType};
use onelauncher::package::content::Providers;
use onelauncher::store::{Cluster, PackagePath};
use onelauncher::{cluster, Result};

const CLUSTER_NAME: &str = "Test Packages";

const MODRINTH_PACKAGE_ONE: &str = "behindyou";
const MODRINTH_PACKAGE_ONE_VERSION: &str = "v3.2.2";

const MODRINTH_PACKAGE_TWO: &str = "keystrokes";
const MODRINTH_PACKAGE_TWO_VERSION: &str = "v1.0.0";

#[tokio::main]
async fn main() -> Result<()> {
	let mut cluster = create_cluster().await?;

	// Print Packages first
	print_packages(&cluster).await?;

	println!("Download the first mod, adding it to the PackageManager and sync packages");
	download_mod(
		&mut cluster,
		MODRINTH_PACKAGE_ONE,
		MODRINTH_PACKAGE_ONE_VERSION,
		true,
	)
	.await?;
	cluster::content::package::sync_packages_by_type(&cluster.cluster_path(), PackageType::Mod, None)
		.await?;

	// Print Packages again
	print_packages(&cluster).await?;

	println!("Download the second mod, NOT adding it to the PackageManager and sync packages");
	let package_path = download_mod(
		&mut cluster,
		MODRINTH_PACKAGE_TWO,
		MODRINTH_PACKAGE_TWO_VERSION,
		false,
	)
	.await?;
	cluster::content::package::sync_packages_by_type(&cluster.cluster_path(), PackageType::Mod, None)
		.await?;

	// Print Packages again
	print_packages(&cluster).await?;

	println!("Removing the second mod from the PackageManager and sync packages");
	cluster::content::package::remove_package(
		&cluster.cluster_path(),
		&package_path,
	)
	.await?;
	cluster::content::package::sync_packages_by_type(&cluster.cluster_path(), PackageType::Mod, None)
		.await?;

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
		None,
		None,
		None,
		None,
		Some(true),
		Some(false),
	)
	.await?;

	let cluster = cluster::get(&cluster_path)
		.await?
		.expect("Cluster not found");

	Ok(cluster)
}

async fn download_mod(
	cluster: &mut Cluster,
	pkg: &str,
	ver: &str,
	add: bool,
) -> Result<PackagePath> {
	let package = Providers::Modrinth.get(pkg).await?;

	let (package_path, package) = cluster::content::package::download_package(
		&package,
		cluster,
		None,
		None,
		Some(ver.to_owned()),
	)
	.await?;

	if add {
		cluster::content::package::add_package(
			&cluster.cluster_path(),
			package_path.clone(),
			package,
		)
		.await?;
	}

	Ok(package_path)
}

async fn print_packages(cluster: &Cluster) -> Result<()> {
	let packages =
		cluster::content::package::get_packages(&cluster.cluster_path(), PackageType::Mod).await?;

	println!("Packages for cluster: {:?}", cluster.cluster_path());
	for package in packages {
		println!("{:?}: {:?}", package.file_name, package.meta);
	}

	Ok(())
}
