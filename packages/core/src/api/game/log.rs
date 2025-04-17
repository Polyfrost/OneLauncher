use std::sync::LazyLock;

use crate::store::credentials::MinecraftCredentials;

static WHOAMI: LazyLock<(String, String)> = LazyLock::new(|| {
	(whoami::username(), whoami::realname())
});

#[must_use]
pub fn censor_line(
	creds: &MinecraftCredentials,
	mut output: String
) -> String {
	let username = &WHOAMI.0;
	let realname = &WHOAMI.1;

	output = output.replace(&format!("/{username}/"), "/{ENV_USERNAME}/");
	output = output.replace(&format!("\\{username}\\"), "\\{ENV_USERNAME}\\");
	output = output.replace(&format!("/{realname}/"), "/{ENV_REALNAME}/");
	output = output.replace(&format!("\\{realname}\\"), "\\{ENV_REALNAME}\\");
	output = output.replace(&creds.access_token, "{MC_ACCESS_TOKEN}");
	output = output.replace(&creds.username, "{MC_USERNAME}");
	output = output.replace(&creds.id.as_simple().to_string(), "{MC_UUID}");
	output = output.replace(&creds.id.as_hyphenated().to_string(), "{MC_UUID}");

	output
}