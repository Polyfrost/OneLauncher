use std::{collections::HashMap, error::Error};

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

    async fn setup(&self) -> Result<(), Box<dyn Error>> {
        // vanilla_impl::setup_libraries().await?;
        // vanilla_impl::setup_natives().await?;
        // vanilla_impl::setup_assets().await?;
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

pub mod vanilla_impl {
    use std::error::Error;

    use serde_json::{Map, Value};
    use tauri::AppHandle;

    pub async fn setup_libraries(manifest: &Value) -> Result<Vec<String>, Box<dyn Error>> {
        let libraries = match manifest.get("libraries") {
            Some(libraries) => {
                if libraries.is_array() { 
                    libraries
                } else { 
                    return Err("Invalid libraries object".into()) 
                }
            },
            None => return Ok(vec![])
        };

        let mut natives: Vec<&Map<String, Value>> = vec![];
        for library in libraries.as_array().unwrap() {
            let library: &Map<String, Value> = library.as_object().unwrap();
            
            if let Some(_) = library.get("natives") {
                natives.push(library);
                continue;
            }

            let name = library.get("name").ok_or("No name object")?
                .as_str().ok_or("Invalid name object")?;
            
            let artifact = library.get("downloads").ok_or("No downloads object")?
                .as_object().ok_or("Invalid downloads object")?
                .get("artifact").ok_or("No artifact object")?
                .as_object().ok_or("Invalid artifact object")?;

            let path = artifact.get("path").ok_or("No path object")?
                .as_str().ok_or("Invalid path object")?;
            let url = artifact.get("url").ok_or("No url object")?
                .as_str().ok_or("Invalid url object")?;
            
            // TODO: Add checks for rules + platform
            
        }

        Ok(vec![])
    }

    pub async fn setup_natives() -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    pub async fn setup_assets() -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}