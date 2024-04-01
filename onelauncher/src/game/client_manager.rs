use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::anyhow;
use chrono::Local;
use serde::Serialize;
use uuid::Uuid;

use crate::{auth::Account, utils::dirs};

use super::{client::{ClientTrait, Cluster, LaunchCallbacks, LaunchInfo, Manifest}, clients::{self, vanilla, ClientType}, java};

#[derive(Debug, thiserror::Error)]
pub enum ClientManagerError {
	#[error("Cluster not found")]
	ClusterNotFound,

	#[error("Manifest not found")]
	ManifestNotFound,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClusterWithManifest {
    pub cluster: Cluster, 
    pub manifest: Manifest
}

pub struct ClientManager {
	clusters: HashMap<Uuid, ClusterWithManifest>,
}

impl ClientManager {
    pub fn new() -> crate::Result<Self> {
		Ok(Self {
            clusters: load_and_serialize()?
        })
	}

	pub fn get_impl_uuid<'a>(&'a self, uuid: Uuid) -> crate::Result<Box<dyn ClientTrait<'a> + 'a>> {
		let ClusterWithManifest { cluster, manifest } = self.get_cluster(uuid)?;

		Ok(clients::get_impl(&cluster.client, cluster, manifest))
	}

	pub fn get_clusters(&self) -> Vec<&ClusterWithManifest> {
		self.clusters.values().collect()
	}

    pub fn get_clusters_owned(&self) -> Vec<ClusterWithManifest> {
		self.clusters.values().cloned().collect()
	}

    pub fn get_cluster(&self, uuid: Uuid) -> crate::Result<&ClusterWithManifest> {
        self.clusters
            .get(&uuid)
            .ok_or(ClientManagerError::ClusterNotFound.into())
    }
}

impl ClientManager {
    pub async fn launch_cluster(
        &mut self,
        uuid: Uuid,
        callbacks: LaunchCallbacks,
    ) -> crate::Result<i32> {
        let client = self.get_impl_uuid(uuid)?;
        
        let java = java::download_java(&dirs::java_dir()?, client.get_manifest().minecraft_manifest.java_version.major_version).await?;
        let setup = client.setup().await?;

        // TODO: Make this configurable
        let info = LaunchInfo {
            java,
            setup,
            account: Account {
                username: "Player 0".to_string(),
                access_token: "0".to_string(),
                uuid: Uuid::new_v4().to_string(),
                skins: vec![]
            },
            mem_min: 1024,
            mem_max: 4096,
        };

        let exit_code = client.launch(info, callbacks).await?;
        Ok(exit_code)
    }

	pub async fn create_cluster(
		&mut self,
		name: String,
		version: String,
		cover: Option<String>,
		group: Option<Uuid>,
		client: ClientType,
	) -> crate::Result<Uuid> {
		let version_cache = dirs::app_config_dir()?.join("versions_cache.json");
		let versions = clients::get_versions(&client, Some(&version_cache)).await?;
		let url = &versions
			.iter()
			.find(|v| v.id == version)
			.ok_or(anyhow!("Version not found"))?
			.url;

		let uuid = Uuid::new_v4();
        let dir = &dirs::cluster_dir(uuid.to_string())?;

		// Save the manifest
        // TODO: Is this correct?
		let minecraft_manifest = vanilla::retrieve_version_manifest(url).await?;
		let manifest = Manifest {
			id: uuid,
			minecraft_manifest,
		};

		save(
			&dir.join("manifest.json"),
			&manifest,
		)?;

		// Save the cluster
		let cluster = Cluster {
			id: uuid,
			created_at: Local::now(),
			name,
			cover,
			group,
			client,
		};

		save(
			&dir.join("cluster.json"),
			&cluster,
		)?;

		self.clusters.insert(uuid, ClusterWithManifest { cluster, manifest });

		Ok(uuid)
	}
}

fn save<T>(path: &PathBuf, object: T) -> crate::Result<()>
where
	T: Serialize,
{
	if !path.exists() {
		fs::create_dir_all(
			path.parent()
				.expect("Couldn't get parent of path whilst saving object. 'client.rs'"),
		)?;
	}

	let content = serde_json::to_string(&object)?;
	fs::write(path, content)?;
	Ok(())
}

fn load_and_serialize() -> crate::Result<HashMap<Uuid, ClusterWithManifest>> {
	let mut result = HashMap::<Uuid, ClusterWithManifest>::new();

    let dir = dirs::clusters_dir()?;
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
        return Ok(result);
    }
	
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            match get_entry(&path) {
                Ok((uuid, cluster)) => {
                    result.insert(uuid, cluster);
                }
                Err(e) => {
                    eprintln!("Failed to load cluster: {}", e);
                }
            }
        }
    }

	Ok(result)
}

fn get_entry(path: &PathBuf) -> crate::Result<(Uuid, ClusterWithManifest)> {
    let manifest_path = path.join("manifest.json");
    let cluster_path = path.join("cluster.json");

    let manifest: Manifest = serde_json::from_str(&fs::read_to_string(manifest_path)?)?;
    let cluster: Cluster = serde_json::from_str(&fs::read_to_string(cluster_path)?)?;

    Ok((cluster.id, ClusterWithManifest { cluster, manifest }))
}
