use onelauncher_entity::clusters;

use crate::error::LauncherResult;
use crate::store::Dirs;
use crate::utils::io;

pub async fn get_screenshots(cluster: &clusters::Model) -> LauncherResult<Vec<String>> {
	let dir = cluster.folder_name.clone();
	let path = Dirs::get_clusters_dir()
		.await?
		.join(dir)
		.join("screenshots");

	if !path.exists() {
		io::create_dir(&path).await?;
		return Ok(Vec::new());
	}

	let mut list = vec![];
	let mut files = io::read_dir(path).await?;
	while let Ok(Some(entry)) = files.next_entry().await {
		list.push(entry.file_name().to_string_lossy().to_string());
	}

	Ok(list)
}

pub async fn get_worlds(cluster: &clusters::Model) -> LauncherResult<Vec<String>> {
	let dir = cluster.folder_name.clone();
	let path = Dirs::get_clusters_dir().await?.join(dir).join("saves");

	if !path.exists() {
		io::create_dir(&path).await?;
		return Ok(Vec::new());
	}

	let mut list = vec![];
	let mut files = io::read_dir(path).await?;
	while let Ok(Some(entry)) = files.next_entry().await {
		list.push(entry.file_name().to_string_lossy().to_string());
	}

	Ok(list)
}
