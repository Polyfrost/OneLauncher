use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::anyhow;
use async_trait::async_trait;
use chrono::{DateTime, Local};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::Uuid;

use crate::utils::dirs;

use super::{
	minecraft::{MinecraftManifest, ReleaseType},
	vanilla::{self, VanillaClientProps},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
	id: String,
	url: String,
	#[serde(default)]
	release_type: ReleaseType,
	#[serde(default)]
	release_time: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
	pub id: Uuid,
	pub manifest: MinecraftManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
	pub id: Uuid,
	pub created_at: DateTime<Local>,
	pub name: String,
	pub cover: Option<String>,
	pub group: Option<Uuid>,
	pub client: ClientType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientType {
	Vanilla(VanillaClientProps),
}

impl ClientType {
	pub async fn get_versions(&self, file: Option<&PathBuf>) -> crate::Result<Vec<Version>> {
		match self {
			ClientType::Vanilla(_) => vanilla::get_versions(file).await,
		}
	}
}

pub fn get_impl<'a>(
	client: &'a ClientType,
	instance: &'a Instance,
	manifest: &'a Manifest,
) -> Box<dyn ClientTrait<'a> + 'a> {
	Box::new(match client {
		ClientType::Vanilla(_) => vanilla::VanillaClient::new(instance, manifest),
	})
}

#[async_trait]
pub trait ClientTrait<'a>: Send + Sync {
	fn new(instance: &'a Instance, manifest: &'a Manifest) -> Self
	where
		Self: Sized;

	async fn launch(&self) -> crate::Result<()>;
	async fn setup(&self) -> crate::Result<()>;

	async fn install_game(&self) -> crate::Result<PathBuf>;
	async fn install_libraries(&self) -> crate::Result<String>;
	async fn install_natives(&self) -> crate::Result<()>;
	async fn install_assets(&self) -> crate::Result<()>;

	fn get_instance(&self) -> &'a Instance
	where
		Self: Sized;
	fn get_manifest(&self) -> &'a Manifest
	where
		Self: Sized;
}

#[macro_export]
macro_rules! create_game_client {
    ($props:ident { $($props_name:ident: $props_type:ty),* } $client:ident { $($name:ident: $type:ty),* }) => {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub struct $props {
            $(pub $props_name: $props_type),*
        }

        #[derive(Debug, Clone)]
        pub struct $client<'a> {
            pub instance: &'a crate::game::client::Instance,
            pub manifest: &'a crate::game::client::Manifest,
            $(pub $name: $type),*
        }
    };
}

#[macro_export]
macro_rules! impl_game_client {
	() => {
		fn get_instance(&self) -> &'a crate::game::client::Instance
		where
			Self: Sized,
		{
			self.instance
		}

		fn get_manifest(&self) -> &'a crate::game::client::Manifest
		where
			Self: Sized,
		{
			self.manifest
		}
	};
}

#[derive(Debug, thiserror::Error)]
pub enum ClientManagerError {
	#[error("Instance not found")]
	InstanceNotFound,
	#[error("Manifest not found")]
	ManifestNotFound,
}

pub struct ClientManager {
	instances: HashMap<Uuid, Instance>,
	manifests: HashMap<Uuid, Manifest>,
}

impl ClientManager {
	pub fn new() -> crate::Result<Self> {
		let instances = load_and_serialize::<Instance>(&dirs::instances_dir()?)?;

		let manifests = load_and_serialize::<Manifest>(&dirs::manifests_dir()?)?;

		Ok(Self {
			instances,
			manifests,
		})
	}

	pub fn get_impl_uuid<'a>(&'a self, uuid: Uuid) -> crate::Result<Box<dyn ClientTrait<'a> + 'a>> {
		let instance = self.get_instance(uuid)?;
		let manifest = self.get_manifest(uuid)?;

		Ok(get_impl(&instance.client, &instance, &manifest))
	}

	pub fn get_instances(&self) -> Vec<&Instance> {
		self.instances.values().collect()
	}

	pub fn get_instances_owned(&self) -> Vec<Instance> {
		self.instances.values().cloned().collect()
	}

	pub fn get_instance(&self, uuid: Uuid) -> crate::Result<&Instance> {
		self.instances
			.get(&uuid)
			.ok_or(ClientManagerError::InstanceNotFound.into())
	}

	pub fn get_instance_mut(&mut self, uuid: Uuid) -> crate::Result<&mut Instance> {
		self.instances
			.get_mut(&uuid)
			.ok_or(ClientManagerError::InstanceNotFound.into())
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

	pub async fn create_instance(
		&mut self,
		name: String,
		version: String,
		cover: Option<String>,
		group: Option<Uuid>,
		client: ClientType,
	) -> crate::Result<Uuid> {
		let version_cache = dirs::app_config_dir()?.join("versions_cache.json");
		let versions = client.get_versions(Some(&version_cache)).await?;
		let url = &versions
			.iter()
			.find(|v| v.id == version)
			.ok_or(anyhow!("Version not found"))?
			.url;

		let uuid = Uuid::new_v4();

		// Save the manifest
		let minecraft_manifest = vanilla::retrieve_version_manifest(url).await?;
		let manifest = Manifest {
			id: uuid,
			manifest: minecraft_manifest,
		};

		save(
			&dirs::manifests_dir()?.join(format!("{}.json", uuid)),
			&manifest,
		)?;
		self.manifests.insert(uuid, manifest);

		// Save the instance
		let instance = Instance {
			id: uuid,
			created_at: Local::now(),
			name,
			cover,
			group,
			client,
		};

		save(
			&dirs::instances_dir()?.join(format!("{}.json", uuid)),
			&instance,
		)?;
		self.instances.insert(uuid, instance);

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
				eprintln!("Couldn't parse file content as JSON: '{:?}'", content);
				continue;
			}
		};

		result.insert(uuid, parsed);
	}

	Ok(result)
}
