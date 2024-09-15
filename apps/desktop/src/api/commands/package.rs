use std::path::PathBuf;

use onelauncher::cluster::content::package;
use onelauncher::data::{Loader, ManagedPackage, ManagedUser, ManagedVersion, PackageType};
use onelauncher::package::content::Providers;
use onelauncher::package::import::{default_launcher_path, ImportType};
use onelauncher::store::{Author, ClusterPath, Package, PackagePath, ProviderSearchResults};
use uuid::Uuid;

#[specta::specta]
#[tauri::command]
pub async fn import_launcher_instances(
	launcher: ImportType,
	path: Option<PathBuf>,
) -> Result<(), String> {
	onelauncher::api::package::import::import_instances(
		launcher,
		path.unwrap_or(
			default_launcher_path(launcher)
				.ok_or("couldn't get a default path for this launcher")?,
		),
	)
	.await?;
	Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn get_provider_package(
	provider: Providers,
	project_id: String,
) -> Result<ManagedPackage, String> {
	Ok(provider.get(&project_id).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn get_provider_packages(
	provider: Providers,
	project_ids: Vec<String>,
) -> Result<Vec<ManagedPackage>, String> {
	Ok(provider.get_multiple(&project_ids).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn get_all_provider_package_versions(
	provider: Providers,
	project_id: String,
	game_versions: Option<Vec<String>>,
	loaders: Option<Vec<Loader>>,
) -> Result<Vec<ManagedVersion>, String> {
	Ok(provider
		.get_all_versions(&project_id, game_versions, loaders)
		.await?)
}

#[specta::specta]
#[tauri::command]
pub async fn get_provider_package_versions(
	provider: Providers,
	versions: Vec<String>,
) -> Result<Vec<ManagedVersion>, String> {
	Ok(provider.get_versions(versions).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn get_provider_package_version(
	provider: Providers,
	version: String,
) -> Result<ManagedVersion, String> {
	Ok(provider.get_version(&version).await?)
}

#[derive(specta::Type, serde::Deserialize, serde::Serialize)]
pub struct ProviderSearchQuery {
	query: Option<String>,
	limit: Option<u8>,
	offset: Option<u32>,
	game_versions: Option<Vec<String>>,
	categories: Option<Vec<String>>,
	loaders: Option<Vec<Loader>>,
	package_types: Option<Vec<PackageType>>,
	open_source: Option<bool>,
}

#[specta::specta]
#[tauri::command]
pub async fn search_provider_packages(
	provider: Providers,
	query: ProviderSearchQuery,
) -> Result<ProviderSearchResults, String> {
	Ok(provider
		.search(
			query.query,
			query.limit,
			query.offset,
			query.game_versions,
			query.categories,
			query.loaders,
			query.package_types,
			query.open_source,
		)
		.await?)
}

#[specta::specta]
#[tauri::command]
pub async fn get_provider_authors(
	provider: Providers,
	author: Author,
) -> Result<Vec<ManagedUser>, String> {
	Ok(provider.get_authors(&author).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn download_provider_package(
	provider: Providers,
	package_id: String,
	cluster_id: Uuid,
	game_version: Option<String>,
	loader: Option<Loader>,
	package_version: Option<String>,
) -> Result<(), String> {
	let mut cluster = onelauncher::cluster::get_by_uuid(cluster_id)
		.await?
		.ok_or("cluster not found")?;

	let mgd_pkg = provider.get(&package_id).await?;

	let (pkg_path, pkg) = package::download_package(
		&mgd_pkg,
		&mut cluster,
		game_version,
		loader,
		package_version,
	)
	.await?;
	package::add_package(
		&cluster.cluster_path(),
		pkg_path,
		pkg,
		Some(mgd_pkg.package_type),
	)
	.await?;

	Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn get_cluster_package(
	cluster_path: ClusterPath,
	package_path: PackagePath,
	package_type: PackageType,
) -> Result<Package, String> {
	Ok(package::get_package(&cluster_path, &package_path, package_type).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn get_cluster_packages(
	cluster_path: ClusterPath,
	package_type: PackageType,
) -> Result<Vec<Package>, String> {
	Ok(package::get_packages(&cluster_path, package_type).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn add_cluster_package(
	cluster_path: ClusterPath,
	package_path: PackagePath,
	pkg: Package,
	package_type: Option<PackageType>,
) -> Result<(), String> {
	package::add_package(&cluster_path, package_path, pkg, package_type).await?;
	Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn remove_cluster_package(
	cluster_path: ClusterPath,
	package_path: PackagePath,
	package_type: PackageType,
) -> Result<(), String> {
	package::remove_package(&cluster_path, &package_path, package_type).await?;
	Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn sync_cluster_packages(cluster_path: ClusterPath) -> Result<(), String> {
	package::sync_packages(&cluster_path).await?;
	Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn sync_cluster_packages_by_type(
	cluster_path: ClusterPath,
	package_type: PackageType,
) -> Result<(), String> {
	package::sync_packages_by_type(&cluster_path, package_type).await?;
	Ok(())
}
