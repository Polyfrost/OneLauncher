use crate::utils::io;
use crate::Result;

use crate::store::ClusterPath;

/// Gets a list of screenshot file names from the [`ClusterPath`]
#[tracing::instrument]
pub async fn get_worlds(cluster: &ClusterPath) -> Result<Vec<String>> {
	let dir = cluster.full_path().await?.join("saves");

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
