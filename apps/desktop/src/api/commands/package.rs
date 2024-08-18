use onelauncher::cluster::content::package;
use onelauncher::data::{Loader, ManagedPackage};
use onelauncher::package::content::Providers;
use uuid::Uuid;

#[specta::specta]
#[tauri::command]
pub async fn get_package(
	provider: Providers,
	project_id: String
) -> Result<ManagedPackage, String> {
	Ok(provider.get(&project_id).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn get_packages(
	provider: Providers,
	project_ids: Vec<String>
) -> Result<Vec<ManagedPackage>, String> {
	Ok(provider.get_multiple(&project_ids).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn search_packages(
	provider: Providers,
	query: Option<String>,
	game_versions: Option<Vec<String>>,
	categories: Option<Vec<String>>,
	loaders: Option<Vec<Loader>>,
	open_source: Option<bool>,
) -> Result<Vec<ManagedPackage>, String> {
	Ok(provider.search(query, game_versions, categories, loaders, open_source).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn download_package(
	provider: Providers,
	package_id: String,
	cluster_id: Uuid,
	game_version: Option<String>,
	loader: Option<Loader>,
	package_version: Option<String>,
) -> Result<(), String> {
	let mut cluster = onelauncher::cluster::get_by_uuid(cluster_id, None)
		.await?
		.ok_or("cluster not found")?;

	let pkg = provider.get(&package_id).await?;

	package::download_package(&pkg, &mut cluster, game_version, loader, package_version).await?;

	Ok(())
}
