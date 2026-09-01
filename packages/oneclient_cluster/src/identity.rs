use serde::{Deserialize, Serialize};

use oneclient_common::domain::GameLoader;
use oneclient_common::paths;

use crate::error::ClusterResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterIdentity {
	pub name: String,
	pub mc_version: String,
	pub mc_loader: GameLoader,
	#[serde(default)]
	pub mc_loader_version: Option<String>,
	#[serde(default)]
	pub dedicated: bool,
}

impl ClusterIdentity {
	pub async fn write(&self, folder_name: &str) -> ClusterResult<()> {
		let path = paths::cluster_identity_file(folder_name)?;
		if let Some(parent) = path.parent() {
			polyio::create_dir_all(parent).await?;
		}
		polyio::write(&path, serde_json::to_vec_pretty(self)?).await?;
		Ok(())
	}

	pub async fn read(folder_name: &str) -> Option<Self> {
		let path = paths::cluster_identity_file(folder_name).ok()?;
		let bytes = polyio::read(&path).await.ok()?;

		match serde_json::from_slice(&bytes) {
			Ok(identity) => Some(identity),
			Err(err) => {
				tracing::warn!(
					folder = folder_name,
					error = %err,
					"unreadable cluster identity file; falling back to folder name"
				);
				None
			}
		}
	}

	pub async fn amend(folder_name: &str, apply: impl FnOnce(&mut Self)) {
		let Some(mut identity) = Self::read(folder_name).await else {
			return;
		};

		apply(&mut identity);

		if let Err(err) = identity.write(folder_name).await {
			tracing::warn!(
				folder = folder_name,
				error = %err,
				"failed to update cluster identity file"
			);
		}
	}
}
