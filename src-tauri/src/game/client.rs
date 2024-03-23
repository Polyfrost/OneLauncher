use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::anyhow;
use async_trait::async_trait;
use chrono::{DateTime, Local};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::PolyResult;

use super::{minecraft::MinecraftManifest, vanilla};

#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub id: Uuid,
    pub manifest: MinecraftManifest
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    Vanilla
}

#[async_trait]
pub trait ClientTrait: Send + Sync {
    fn from_instance(instance: Instance) -> Self where Self: Sized;

    async fn launch(&self) -> PolyResult<()>;
    async fn setup(&self) -> PolyResult<()>;

    async fn install_game(&self) -> PolyResult<()>;
    async fn install_libraries(&self) -> PolyResult<String>;
    async fn install_natives(&self) -> PolyResult<()>;
    async fn install_assets(&self) -> PolyResult<()>;
}

pub struct ClientManager {
    instances: HashMap<Uuid, Instance>,
    instances_dir: PathBuf,

    manifests: HashMap<Uuid, Manifest>,
    manifests_dir: PathBuf,
}

impl ClientManager {
    pub fn new(handle: &AppHandle) -> PolyResult<Self> {
        let instances_dir = handle.path().app_config_dir()?.join("instances");
        let instances = load_and_serialize::<Instance>(&instances_dir)?;

        let manifests_dir = handle.path().app_config_dir()?.join("manifests");
        let manifests = load_and_serialize::<Manifest>(&manifests_dir)?;

        Ok(Self {
            instances,
            instances_dir,
            manifests,
            manifests_dir,
        })
    }

    pub fn get_instances(&self) -> Vec<&Instance> {
        self.instances.values().collect()
    }

    pub fn get_instances_owned(&self) -> Vec<Instance> {
        self.instances.values().cloned().collect()
    }

    pub fn get_instance(&self, uuid: Uuid) -> Option<&Instance> {
        self.instances.get(&uuid)
    }

    pub fn get_instance_mut(&mut self, uuid: Uuid) -> Option<&mut Instance> {
        self.instances.get_mut(&uuid)
    }

    pub fn get_manifest(&self, uuid: Uuid) -> Option<&Manifest> {
        self.manifests.get(&uuid)
    }

    pub fn get_manifest_mut(&mut self, uuid: Uuid) -> Option<&mut Manifest> {
        self.manifests.get_mut(&uuid)
    }

    pub async fn create_instance(&mut self, 
        name: String, 
        version: String, 
        cover: Option<String>, 
        group: Option<Uuid>,
        client: ClientType
    ) -> PolyResult<Uuid> {
        // TODO: Implement cahcing
        let url = String::from("https://piston-meta.mojang.com/v1/packages/d546f1707a3f2b7d034eece5ea2e311eda875787/1.8.9.json");
        let uuid = Uuid::new_v4();

        // Save the manifest
        let minecraft_manifest = vanilla::retrieve_version_manifest(url).await?;
        let manifest = Manifest {
            id: uuid,
            manifest: minecraft_manifest
        };

        save(&self.manifests_dir.join(format!("{}.json", uuid)), &manifest)?;
        self.manifests.insert(uuid, manifest);

        // Save the instance
        let instance = Instance {
            id: uuid,
            created_at: Local::now(),
            name,
            cover,
            group,
            client
        };

        save(&self.instances_dir.join(format!("{}.json", uuid)), &instance)?;
        self.instances.insert(uuid, instance);

        Ok(uuid)
    }
}

fn save<T>(path: &PathBuf, object: T) -> PolyResult<()> where T: Serialize {
    if !path.exists() {
        fs::create_dir_all(path.parent().expect("Couldn't get parent of path whilst saving object. 'client.rs'"))?;
    }

    let content = serde_json::to_string(&object)?;
    fs::write(path, content)?;
    Ok(())
}

fn load_and_serialize<T>(dir: &PathBuf) -> PolyResult<HashMap<Uuid, T>> where T: DeserializeOwned {
    let mut result = HashMap::<Uuid, T>::new();

    if !dir.exists() {
        fs::create_dir_all(dir)?;
        return Ok(result);
    }
    
    for file in fs::read_dir(dir)? {
        let file = file?;
        let file_name = file.file_name();
        let file_name = file_name.to_str().ok_or(anyhow!("Couldn't convert OsStr to str"))?.replace(".json", "");
        let uuid = match Uuid::parse_str(&file_name) {
            Ok(uuid) => uuid,
            Err(_) => {
                eprintln!("Couldn't parse file name as UUID: '{:?}'", file_name);
                continue;
            }
        };

        let path = file.path();
        let content = fs::read_to_string(&path)?;
        let parsed = serde_json::from_str::<T>(&content)?;

        result.insert(uuid, parsed);
    }

    Ok(result)
}
