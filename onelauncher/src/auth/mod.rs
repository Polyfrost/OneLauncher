use anyhow::anyhow;
use serde::{Deserialize, Serialize};

use crate::{utils::http, ErrorKind};

pub mod microsoft_auth;

#[derive(thiserror::Error, Debug)]
pub enum AuthenticationError {
	#[error(transparent)]
	MicrosoftError(#[from] microsoft_auth::MicrosoftAuthError),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Account {
	pub username: String,
	pub uuid: String,
	pub skins: Vec<AccountSkin>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountSkin {
	pub id: String,
	pub state: String,
	pub url: String,
	pub variant: String,
}

#[allow(async_fn_in_trait)]
pub trait AuthenticationMethod {
	/// Authenticate with a given method. Stage is a function that takes a string and a u8.
	/// The string is the message to display to the user, and the u8 is the progress of the authentication.
	async fn auth<F>(stage: F) -> crate::Result<Account>
	where
		F: Fn(String, u8, bool) -> ();

	async fn get_profile(access_token: String) -> crate::Result<Account> {
		let response = http::create_client()?
			.get("https://api.minecraftservices.com/minecraft/profile")
			.header("Authorization", format!("Bearer {}", access_token))
			.send()
			.await?;

		let response = response.json::<serde_json::Value>().await?;
		if let Some(error) = response.get("error") {
			return Err(ErrorKind::AnyhowError(anyhow!(error.to_string())).into());
		}

		let account = serde_json::from_value::<Account>(response)?;
		Ok(account)
	}
}
