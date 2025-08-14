use onelauncher_entity::clusters;

use crate::error::LauncherResult;
use crate::store::Dirs;
use crate::utils::io;

/// Returns a list of screenshot file names
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

/// Returns a list of world filenames
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

/// Returns a list of log file names
pub async fn get_logs(cluster: &clusters::Model) -> LauncherResult<Vec<String>> {
	let dir = cluster.folder_name.clone();
	let path = Dirs::get_clusters_dir().await?.join(dir).join("logs");

	if !path.exists() {
		io::create_dir(&path).await?;
		return Ok(Vec::new());
	}

	let mut list = vec![];
	let mut files = io::read_dir(path).await?;
	while let Ok(Some(entry)) = files.next_entry().await {
		list.push(entry.file_name().to_string_lossy().to_string());
	}

	list.sort_by(|a, b| {
		if a == "latest.log" {
			std::cmp::Ordering::Less
		} else if b == "latest.log" {
			std::cmp::Ordering::Greater
		} else {
			a.cmp(b)
		}
	});

	Ok(list)
}

/// returns a log from a cluster and file name
pub async fn get_log_by_name(
	cluster: &clusters::Model,
	file_name: &str,
) -> LauncherResult<Option<String>> {
	let dir = cluster.folder_name.clone();
	let path = Dirs::get_clusters_dir()
		.await?
		.join(dir)
		.join("logs")
		.join(file_name);

	if !path.exists() {
		return Ok(None);
	}

	let content = if file_name.ends_with(".gz") {
		io::read_gz_to_string(&path).await?
	} else {
		io::read_to_string(&path).await?
	};

	Ok(Some(content))
}
