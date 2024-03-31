use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::anyhow;
use chrono::Local;
use serde::{de::DeserializeOwned, Serialize};
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

pub struct ClientManager {
	clusters: HashMap<Uuid, Cluster>,
	manifests: HashMap<Uuid, Manifest>,
}

impl ClientManager {
    pub fn new() -> crate::Result<Self> {
		Ok(Self {
			clusters: load_and_serialize::<Cluster>(&dirs::clusters_dir()?)?,
			manifests: load_and_serialize::<Manifest>(&dirs::manifests_dir()?)?,
		})
	}

	pub fn get_impl_uuid<'a>(&'a self, uuid: Uuid) -> crate::Result<Box<dyn ClientTrait<'a> + 'a>> {
		let cluster = self.get_cluster(uuid)?;
		let manifest = self.get_manifest(uuid)?;

		Ok(clients::get_impl(&cluster.client, &cluster, &manifest))
	}

	pub fn get_clusters(&self) -> Vec<&Cluster> {
		self.clusters.values().collect()
	}

	pub fn get_clusters_owned(&self) -> Vec<Cluster> {
		self.clusters.values().cloned().collect()
	}

	pub fn get_cluster(&self, uuid: Uuid) -> crate::Result<&Cluster> {
		self.clusters
			.get(&uuid)
			.ok_or(ClientManagerError::ClusterNotFound.into())
	}

	pub fn get_cluster_mut(&mut self, uuid: Uuid) -> crate::Result<&mut Cluster> {
		self.clusters
			.get_mut(&uuid)
			.ok_or(ClientManagerError::ClusterNotFound.into())
	}

	pub fn get_manifest(&self, uuid: Uuid) -> crate::Result<&Manifest> {
		self.manifests
			.get(&uuid)
			.ok_or(ClientManagerError::ManifestNotFound.into())
	}

	pub fn get_manifest_mut(&mut self, uuid: Uuid) -> crate::Result<&mut Manifest> {
		self.manifests
			.get_mut(&uuid)
			.ok_or(ClientManagerError::ManifestNotFound.into())
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

		// Save the manifest
        // TODO: Is this correct?
		let minecraft_manifest = vanilla::retrieve_version_manifest(url).await?;
		let manifest = Manifest {
			id: uuid,
			minecraft_manifest,
		};

		save(
			&dirs::manifests_dir()?.join(format!("{}.json", uuid)),
			&manifest,
		)?;
		self.manifests.insert(uuid, manifest);

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
			&dirs::clusters_dir()?.join(format!("{}.json", uuid)),
			&cluster,
		)?;
		self.clusters.insert(uuid, cluster);

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

fn load_and_serialize<T>(dir: &PathBuf) -> crate::Result<HashMap<Uuid, T>>
where
	T: DeserializeOwned,
{
	let mut result = HashMap::<Uuid, T>::new();

	if !dir.exists() {
		fs::create_dir_all(dir)?;
		return Ok(result);
	}

	for file in fs::read_dir(dir)? {
		let file = file?;
		let file_name = file.file_name();
		let file_name = file_name
			.to_str()
			.ok_or(anyhow!("Couldn't convert OsStr to str"))?
			.replace(".json", "");
		let uuid = match Uuid::parse_str(&file_name) {
			Ok(uuid) => uuid,
			Err(_) => {
				eprintln!("Couldn't parse file name as UUID: '{:?}'", file_name);
				continue;
			}
		};

		let path = file.path();
		let content = match fs::read_to_string(&path) {
			Ok(content) => content,
			Err(_) => {
				eprintln!("Couldn't read file: '{:?}'", path);
				continue;
			}
		};

		let parsed = match serde_json::from_str::<T>(&content) {
			Ok(parsed) => parsed,
			Err(_) => {
				eprintln!("Couldn't parse file '{}' content as JSON", path.display());
				continue;
			}
		};

		result.insert(uuid, parsed);
	}

	Ok(result)
}
