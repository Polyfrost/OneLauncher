use std::path::PathBuf;

use async_trait::async_trait;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{clients::ClientType, minecraft::{MinecraftManifest, ReleaseType}};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftVersion {
	pub id: String,
	pub url: String,
	#[serde(default)]
	pub release_type: ReleaseType,
	#[serde(default)]
	pub release_time: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
	pub id: Uuid,
	pub manifest: MinecraftManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cluster {
	pub id: Uuid,
	pub created_at: DateTime<Local>,
	pub name: String,
	pub cover: Option<String>,
	pub group: Option<Uuid>,
	pub client: ClientType,
}

#[async_trait]
pub trait ClientTrait<'a>: Send + Sync {
	fn new(cluster: &'a Cluster, manifest: &'a Manifest) -> Self
	where
		Self: Sized;

	async fn launch(&self) -> crate::Result<()>;
	async fn setup(&self) -> crate::Result<()>;

	async fn install_game(&self) -> crate::Result<PathBuf>;
	async fn install_libraries(&self) -> crate::Result<String>;
	async fn install_natives(&self) -> crate::Result<()>;
	async fn install_assets(&self) -> crate::Result<()>;

	fn get_cluster(&self) -> &'a Cluster
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
            pub cluster: &'a crate::game::client::Cluster,
            pub manifest: &'a crate::game::client::Manifest,
            $(pub $name: $type),*
        }
    };
}

#[macro_export]
macro_rules! impl_game_client {
	() => {
		fn get_cluster(&self) -> &'a crate::game::client::Cluster
		where
			Self: Sized,
		{
			self.cluster
		}

		fn get_manifest(&self) -> &'a crate::game::client::Manifest
		where
			Self: Sized,
		{
			self.manifest
		}
	};
}