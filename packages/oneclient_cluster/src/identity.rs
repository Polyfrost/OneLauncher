use std::path::Path;

use serde::{Deserialize, Serialize};

use oneclient_common::domain::GameLoader;
use oneclient_common::paths;
use oneclient_db::models::ClusterKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceIdentity {
    pub name: String,
    pub mc_version: String,
    pub mc_loader: GameLoader,
    #[serde(default)]
    pub mc_loader_version: Option<String>,
    #[serde(default)]
    pub kind: ClusterKind,
    #[serde(default)]
    pub user_created: bool,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub cover_path: Option<String>,
}

pub async fn read(cluster_dir: &Path) -> Option<InstanceIdentity> {
    let bytes = polyio::read(cluster_dir.join(paths::INSTANCE_FILE))
        .await
        .ok()?;
    match serde_json::from_slice(&bytes) {
        Ok(identity) => Some(identity),
        Err(err) => {
            tracing::warn!(dir = %cluster_dir.display(), error = %err, "instance file is unreadable");
            None
        }
    }
}

pub async fn write(cluster_dir: &Path, identity: &InstanceIdentity) {
    let Ok(bytes) = serde_json::to_vec_pretty(identity) else {
        return;
    };

    if let Err(err) = polyio::write(cluster_dir.join(paths::INSTANCE_FILE), &bytes).await {
        tracing::warn!(dir = %cluster_dir.display(), error = %err, "failed to write the instance file");
    }
}
