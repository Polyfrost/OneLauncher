use onelauncher::data::ManagedPackage;
use onelauncher::package::content;
use uuid::Uuid;

#[specta::specta]
#[tauri::command]
pub async fn random_mods() -> Result<Vec<ManagedPackage>, String> {
	let provider = content::Providers::Modrinth;
	Ok(provider.list().await?)
}

#[specta::specta]
#[tauri::command]
pub async fn get_mod(project_id: String) -> Result<ManagedPackage, String> {
	let provider = content::Providers::Modrinth;
	Ok(provider.get(&project_id).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn download_mod(cluster_id: Uuid, version_id: String) -> Result<(), String> {
	let cluster = onelauncher::cluster::get_by_uuid(cluster_id, None)
		.await?
		.ok_or("cluster not found")?;
	let provider = content::Providers::Modrinth;
	let game_version = cluster.meta.mc_version.clone();

	provider
		.get_version_for_game_version(&version_id, &game_version)
		.await?
		.files
		.first()
		.ok_or("no files found")?
		.download_to_cluster(&cluster)
		.await?;

	Ok(())
}
