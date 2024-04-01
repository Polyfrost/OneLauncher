use std::{path::PathBuf, process::Stdio};

use anyhow::anyhow;
use async_trait::async_trait;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use tokio::{io::{AsyncBufReadExt, BufReader}, process};
use tracing::info;
use uuid::Uuid;

use crate::auth::Account;

use super::{clients::ClientType, minecraft::{MinecraftManifest, ModernArgumentsItemExt}};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
	pub id: Uuid,
    #[serde(rename = "manifest")]
	pub minecraft_manifest: MinecraftManifest,
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

impl Cluster {
    pub fn dir(&self) -> crate::Result<PathBuf> {
        Ok(crate::utils::dirs::cluster_dir(self.id.to_string())?)
    }
}

#[derive(Debug)]
pub struct LaunchInfo {
    pub java: PathBuf,
    pub account: Account,
    pub mem_min: u32,
    pub mem_max: u32,
    pub setup: SetupInfo,
}

#[derive(Debug)]
pub struct SetupInfo {
    pub version: String,
    pub libraries: String,
    pub game_dir: PathBuf,
    pub natives_dir: PathBuf,
    pub assets_dir: PathBuf,
    pub asset_index: String,
}

pub struct LaunchCallbacks {
    pub on_launch: Box<dyn FnMut(u32) + Send>,
    pub on_stdout: Box<dyn FnMut(String) + Send>,
    pub on_stderr: Box<dyn FnMut(String) + Send>,
}

#[async_trait]
pub trait ClientTrait<'a>: Send + Sync {
	fn new(cluster: &'a Cluster, manifest: &'a Manifest) -> Self where Self: Sized;

	async fn launch(&self, info: LaunchInfo, mut callbacks: LaunchCallbacks) -> crate::Result<i32> {
        let manifest = &self.get_manifest().minecraft_manifest;

        let args = get_arguments(manifest)?
            .replace("${game_assets}", "${assets_root}")
            .replace("${auth_player_name}", &info.account.username)
            .replace("${version_name}", &info.setup.version)
            .replace("${game_directory}", info.setup.game_dir.to_str().unwrap())
            .replace("${assets_root}", info.setup.assets_dir.to_str().unwrap())
            .replace("${assets_index_name}", &manifest.asset_index.id)
            .replace("${auth_uuid}", &info.account.uuid)
            .replace("${auth_access_token}", &info.account.access_token)
            .replace("${auth_session}", "0") // TODO: Figure out how to get this session ID.
            .replace("${user_properties}", "{}") // TODO: Figure out these properties.
            .replace("${user_type}", "1")
            .replace("${version_type}", "OneLauncher");

        let mut process = process::Command::new(&info.java).arg("-XX:-UseAdaptiveSizePolicy")
            .arg("-XX:-OmitStackTraceInFastThrow")
            .arg("-Dminecraft.launcher.brand=onelauncher")
            .arg("-Dminecraft.launcher.version=1")
            .arg(format!("-Djava.library.path={}", info.setup.natives_dir.to_str().unwrap()))
            .arg(format!("-Xms{}M", info.mem_min))
            .arg(format!("-Xmx{}M", info.mem_max))
            .arg("-cp")
            .arg(info.setup.libraries)
            .arg(manifest.main_class.clone())
            .args(args.split_whitespace())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        
        let pid = process.id().unwrap_or(0);
        (callbacks.on_launch)(pid);
        
        let stdout = process.stdout.take().ok_or(anyhow!("Couldn't take stdout"))?;
        let stderr = process.stderr.take().ok_or(anyhow!("Couldn't take stderr"))?;

        let mut stdout_lines = BufReader::new(stdout).lines();
        let mut stderr_lines = BufReader::new(stderr).lines();

        let stdout_task = tokio::spawn(async move {
            while let Some(line) = stdout_lines.next_line().await.expect("Couldn't read line") {
                (callbacks.on_stdout)(line);
            }
        });

        let stderr_task = tokio::spawn(async move {
            while let Some(line) = stderr_lines.next_line().await.expect("Couldn't read line") {
                (callbacks.on_stderr)(line);
            }
        });

        let status = process.wait().await?;
        tokio::try_join!(stdout_task, stderr_task)?;

        let exit_code = status.code().unwrap_or(0);
        Ok(exit_code)
    }

	async fn setup(&self) -> crate::Result<SetupInfo>;

	fn get_cluster(&self) -> &'a Cluster;

	fn get_manifest(&self) -> &'a Manifest;
}

pub fn get_arguments(manifest: &MinecraftManifest) -> crate::Result<String> {
    // Modern
    if let Some(arguments) = &manifest.arguments {
        return Ok(arguments.game.build());
    }

    // Legacy
    if let Some(arguments) = &manifest.minecraft_arguments {
        return Ok(arguments.clone());
    }

    Err(anyhow!("No arguments found").into())
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