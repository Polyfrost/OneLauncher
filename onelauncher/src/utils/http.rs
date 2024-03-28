use std::{
	fs::File,
	io::{self, Cursor},
	path::PathBuf,
};

use reqwest::{Client, ClientBuilder};

use crate::{constants::USER_AGENT, ErrorKind};

use super::file::file_sha1;

pub fn create_client() -> crate::Result<Client> {
	Ok(ClientBuilder::new().user_agent(USER_AGENT).build()?)
}

pub async fn download_file_sha1_check(url: &str, path: &PathBuf, sha1: &str) -> crate::Result<()> {
    download_file(url, path).await?;
    
    let file_hash = file_sha1(path)?;
    if file_hash != sha1 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Hashes do not match",
        ).into());
    }

    Ok(())
}

pub async fn download_file(url: &str, path: &PathBuf) -> crate::Result<()> {
	let response = create_client()?.get(url).send().await?;
	let mut file = File::create(path).map_err(|err| ErrorKind::IOError(err))?;
	let mut content = Cursor::new(response.bytes().await?);

	io::copy(&mut content, &mut file).map_err(|err| ErrorKind::IOError(err))?;

	Ok(())
}
