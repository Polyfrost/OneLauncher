use onelauncher_entity::clusters;
use onelauncher_entity::icon::Icon;
use onelauncher_entity::loader::GameLoader;

use crate::error::LauncherResult;
use crate::store::Dirs;
use crate::utils::io;

pub mod dao;

mod sync;
pub use sync::*;

#[must_use]
pub fn sanitize_name(name: &str) -> String {
	let mut name = name.to_string();
	name.retain(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ' ' | '.' | '(' | ')'));
	name
}

#[tracing::instrument]
pub async fn create_cluster(
	name: &str,
	mc_version: &str,
	mc_loader: GameLoader,
	mc_loader_version: Option<&str>,
	icon_url: Option<Icon>,
) -> LauncherResult<clusters::Model> {
	let name = sanitize_name(name);
	let cluster_dir = Dirs::get_clusters_dir().await?;

	// Get the directory for the cluster
	let mut path = cluster_dir.join(&name);

	// Folder name conflict resolution
	if path.exists() {
		let mut which = 1;
		loop {
			let new_name = format!("{name} ({which})");
			path = cluster_dir.join(&new_name);
			if !path.exists() {
				break;
			}
			which += 1;
		}

		tracing::warn!(
			"collision while creating new cluster: {}, renaming to {}",
			cluster_dir.display(),
			path.display()
		);
	}

	let result = async {
		io::create_dir_all(&path).await?;

		tracing::info!("creating cluster at path {}", path.display());

		// Finally add the cluster to the database
		dao::insert_cluster(
			name.as_str(),
			path.to_string_lossy().into_owned().as_str(),
			mc_version,
			mc_loader,
			mc_loader_version,
			icon_url,
		)
		.await
	}
	.await;

	match result {
		Ok(result) => Ok(result),
		Err(err) => {
			tracing::error!("failed to create cluster: {}", err);
			let _ = io::remove_dir_all(&path).await;
			Err(err)
		}
	}
}


