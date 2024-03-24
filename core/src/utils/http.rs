use std::{
	fs::File,
	io::{self, Cursor},
	path::Path,
};

use reqwest::{Client, ClientBuilder};

use crate::{constants::USER_AGENT, PolyError, PolyResult};

pub fn create_client() -> PolyResult<Client> {
	ClientBuilder::new()
		.user_agent(USER_AGENT)
		.build()
		.map_err(|err| PolyError::HTTPError(err))
}

// can we not make mapping the errors automatic, isnt that the whole point
pub async fn download_file(url: &str, path: &Path) -> PolyResult<()> {
	let response = create_client()?
		.get(url)
		.send()
		.await
		.map_err(|err| PolyError::HTTPError(err))?;
	let mut file = File::create(path).map_err(|err| PolyError::IOError(err))?;
	let mut content = Cursor::new(
		response
			.bytes()
			.await
			.map_err(|err| PolyError::HTTPError(err))?,
	);
	io::copy(&mut content, &mut file).map_err(|err| PolyError::IOError(err))?;
	Ok(())
}
