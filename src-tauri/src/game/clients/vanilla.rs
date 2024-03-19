use std::{collections::HashMap, error::Error};

use async_trait::async_trait;
use tauri::AppHandle;
use tokio::io::{stdout, AsyncWriteExt};

use crate::{create_client, create_manifest, game::client::{GameClient, GameClientDetails, GameClientType}};

create_manifest!(VanillaManifest {});
create_client!(VanillaClient {});

pub mod vanilla_impl {
    use std::error::Error;

    pub async fn install_libraries() -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    pub async fn install_natives() -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    pub async fn install_assets() -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

#[async_trait]
impl GameClient for VanillaClient {
    fn new(handle: AppHandle, details: GameClientDetails) -> Self {
        Self {
            handle: handle.clone(),
            details
        }
    }

    fn get_version_urls() -> HashMap<String, String> {
        HashMap::new()
    }

    fn get_details_from_version(version: String) -> Result<GameClientDetails, Box<dyn Error>> {
        Err("a".into())
    }

    fn get_handle(&self) -> &AppHandle {
        &self.handle
    }

    fn get_details(&self) -> &GameClientDetails {
        &self.details
    }

    fn get_client(&self) -> &GameClientType {
        &self.get_details().client_type
    }

    async fn install(&self) -> Result<(), Box<dyn Error>> {
        vanilla_impl::install_libraries().await?;
        vanilla_impl::install_natives().await?;
        vanilla_impl::install_assets().await?;
        Ok(())
    }

    async fn launch(&self) -> Result<(), Box<dyn Error>> {
        println!("Launching Vanilla client");
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        println!("Vanilla client launched");
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        println!("Vanilla client exited");
        stdout().flush().await?; // why...
        Ok(())
    }
}