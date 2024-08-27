use onelauncher::cluster::content::package;
use onelauncher::data::{Loader, ManagedPackage, ManagedUser, PackageType};
use onelauncher::package::content::Providers;
use onelauncher::store::{Author, ClusterPath, Package, PackagePath, ProviderSearchResults};
use uuid::Uuid;

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
pub async fn search_provider_packages(
	provider: Providers,
	query: Option<String>,
	limit: Option<u8>,
	game_versions: Option<Vec<String>>,
	categories: Option<Vec<String>>,
	loaders: Option<Vec<Loader>>,
	open_source: Option<bool>,
) -> Result<ProviderSearchResults, String> {
	Ok(provider
		.search(
			query,
			limit,
			game_versions,
			categories,
			loaders,
			open_source,
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

	let pkg = provider.get(&package_id).await?;

	package::download_package(&pkg, &mut cluster, game_version, loader, package_version).await?;

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
