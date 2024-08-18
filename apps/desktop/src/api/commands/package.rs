use onelauncher::cluster::content::package;
use onelauncher::data::{Loader, ManagedPackage};
use onelauncher::package::content::Providers;
use uuid::Uuid;

#[specta::specta]
#[tauri::command]
pub async fn get_package(project_id: String) -> Result<ManagedPackage, String> {
	let provider = Providers::Modrinth;
	Ok(provider.get(&project_id).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn download_package(
	package_id: String,
	provider: Providers,
	cluster_id: Uuid,
	game_version: Option<String>,
	loader: Option<Loader>,
	package_version: Option<String>
) -> Result<(), String> {
	let mut cluster = onelauncher::cluster::get_by_uuid(cluster_id, None)
		.await?
		.ok_or("cluster not found")?;

	let pkg = provider.get(&package_id).await?;

	package::download_package(&pkg, &mut cluster, game_version, loader, package_version).await?;

	Ok(())
}
