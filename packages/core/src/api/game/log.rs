use std::collections::HashMap;

use crate::store::credentials::MinecraftCredentials;

pub type CensorMap = HashMap<String, String>;

#[must_use]
pub fn create_censors(
	creds: &MinecraftCredentials,
) -> CensorMap {
	let mut map = HashMap::new();

	let username = whoami::username();
	let realname = whoami::realname();

	map.insert(format!("/{username}/"), "/{ENV_USERNAME}/".to_string());
	map.insert(format!("\\{username}\\"), "\\{ENV_USERNAME}\\".to_string());
	map.insert(format!("/{realname}/"), "/{ENV_REALNAME}/".to_string());
	map.insert(format!("\\{realname}\\"), "\\{ENV_REALNAME}\\".to_string());

	if !creds.access_token.is_empty() {
		map.insert(creds.access_token.clone(), "{MC_ACCESS_TOKEN}".to_string());
	}

	map.insert(creds.username.clone(), "{MC_USERNAME}".to_string());
	map.insert(creds.id.as_simple().to_string(), "{MC_UUID}".to_string());
	map.insert(creds.id.as_hyphenated().to_string(), "{MC_UUID}".to_string());

	map
}

pub fn censor_line(censors: &CensorMap, line: &mut String) {
	for (key, value) in censors {
		*line = line.replace(key, value);
	}
}