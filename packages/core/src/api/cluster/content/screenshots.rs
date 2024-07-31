use std::path::PathBuf;

use crate::{utils::io, Result};

use crate::store::ClusterPath;

/// Gets a list of screenshot file names from the [`ClusterPath`]
#[tracing::instrument]
pub async fn get_screenshots(cluster: &ClusterPath) -> Result<Vec<String>> {
	let dir = cluster.full_path().await?.join("screenshots");

	if !dir.exists() {
		io::create_dir(dir).await?;
		return Ok(vec![]);
	}

	let mut list = vec![];
	let mut files = io::read_dir(dir).await?;
	while let Some(file) = files.next_entry().await? {
		list.push(file.file_name().to_string_lossy().to_string());
	}

	Ok(list)
}

#[tracing::instrument]
pub async fn get_screenshot_path(cluster: &ClusterPath, file_name: String) -> Result<PathBuf> {
	let dir = cluster.full_path().await?.join("screenshots");
	Ok(dir.join(file_name))
}