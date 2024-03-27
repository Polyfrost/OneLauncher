use std::{
	fs::File,
	io::{self, Cursor},
	path::Path,
};

use reqwest::{Client, ClientBuilder};

use crate::{constants::USER_AGENT, ErrorKind};

pub fn create_client() -> crate::Result<Client> {
	Ok(ClientBuilder::new()
		.user_agent(USER_AGENT)
		.build()
		.map_err(|err| ErrorKind::HTTPError(err))?)
}

// can we not make mapping the errors automatic, isnt that the whole point
pub async fn download_file(url: &str, path: &Path) -> crate::Result<()> {
	let response = create_client()?
		.get(url)
		.send()
		.await
		.map_err(|err| ErrorKind::HTTPError(err))?;
	let mut file = File::create(path).map_err(|err| ErrorKind::IOError(err))?;
	let mut content = Cursor::new(
		response
			.bytes()
			.await
			.map_err(|err| ErrorKind::HTTPError(err))?,
	);
	io::copy(&mut content, &mut file).map_err(|err| ErrorKind::IOError(err))?;
	Ok(())
}
