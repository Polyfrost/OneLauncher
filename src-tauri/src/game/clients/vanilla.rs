use std::error::Error;

use async_trait::async_trait;
use tauri::AppHandle;
use tokio::io::{stdout, AsyncWriteExt};

use crate::{create_client, create_manifest, game::client::{GameClient, GameClientDetails, GameClientType}};

create_manifest!(VanillaManifest {});
create_client!(VanillaClient {});

#[async_trait]
impl GameClient for VanillaClient {
    fn new(handle: AppHandle, details: GameClientDetails) -> Self {
        Self {
            handle: handle.clone(),
            details
        }
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
        println!("Installing Vanilla client");
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        println!("Vanilla client installed");
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