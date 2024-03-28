use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use self::vanilla::VanillaClientProps;

use super::client::{ClientTrait, Cluster, Manifest, MinecraftVersion};

pub mod vanilla;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientType {
	Vanilla(VanillaClientProps),
}

pub async fn get_versions(client: &ClientType, file: Option<&PathBuf>) -> crate::Result<Vec<MinecraftVersion>> {
    match client {
        ClientType::Vanilla(_) => vanilla::get_versions(file).await,
    }
}

pub fn get_impl<'a>(client: &'a ClientType, cluster: &'a Cluster, manifest: &'a Manifest) -> Box<dyn ClientTrait<'a> + 'a> {
	Box::new(match client {
		ClientType::Vanilla(_) => vanilla::VanillaClient::new(cluster, manifest),
	})
}