use std::{error::Error, fs::File, io::{self, Cursor}, path::Path};

use tauri_plugin_http::reqwest::{Client, ClientBuilder};

use crate::constants::USER_AGENT;

pub fn create_client() -> Client {
    ClientBuilder::new()
        .user_agent(USER_AGENT)
        .build().expect("Failed to create HTTP client. Please report this issue on GitHub.")
}

pub async fn download_file(url: &str, path: &Path) -> Result<(), Box<dyn Error>> {
    let response = create_client().get(url).send().await?;
    let mut file = File::create(path)?;
    let mut content = Cursor::new(response.bytes().await?);
    io::copy(&mut content, &mut file)?;
    Ok(())
}
