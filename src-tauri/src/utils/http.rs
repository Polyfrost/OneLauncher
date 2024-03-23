use std::{fs::File, io::{self, Cursor}, path::Path};

use tauri_plugin_http::reqwest::{Client, ClientBuilder};

use crate::{constants::USER_AGENT, PolyError, PolyResult};

pub fn create_client() -> PolyResult<Client> {
    ClientBuilder::new()
        .user_agent(USER_AGENT)
        .build().map_err(|err| PolyError::HTTPError(err))
}

pub async fn download_file(url: &str, path: &Path) -> PolyResult<()> {
    let response = create_client()?.get(url).send().await?;
    let mut file = File::create(path)?;
    let mut content = Cursor::new(response.bytes().await?);
    io::copy(&mut content, &mut file)?;
    Ok(())
}
