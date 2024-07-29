//! Handles MSA Authentication flow.

use base64::Engine;
use chrono::{DateTime, Utc};
use p256::ecdsa::signature::Signer;
use p256::ecdsa::SigningKey;
use p256::pkcs8::{DecodePrivateKey, EncodePrivateKey};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::Digest;
use std::collections::HashMap;
use std::fmt::Write;
use uuid::Uuid;

use crate::constants::{AUTH_FILE, MINECRAFT_CLIENT_ID, MINECRAFT_REDIRECT_URL, MINECRAFT_SCOPES};

/// The core state of Microsoft authentication for the launcher
#[derive(Serialize, Deserialize, Debug)]
pub struct MinecraftState {
	/// A [`HashMap`] of all logged in users by [`Uuid`] and their Minecraft credentials.
	pub users: HashMap<Uuid, MinecraftCredentials>,
	/// The current stored device token to be used while logging in.
	pub token: Option<SaveDeviceToken>,
	/// The default Minecraft user which the launcher uses, by [`Uuid`].
	pub default_user: Option<Uuid>,
}

impl MinecraftState {
	/// Initialize a new Minecraft global state.
	#[tracing::instrument]
	pub async fn initialize(
		dirs: &super::Directories,
		io_semaphore: &crate::utils::http::IoSemaphore,
	) -> crate::Result<Self> {
		let path = dirs.caches_dir().await.join(AUTH_FILE);
		let store = crate::utils::http::read_json(&path, io_semaphore)
			.await
			.ok();

		if let Some(store) = store {
			Ok(store)
		} else {
			Ok(Self {
				users: HashMap::new(),
				token: None,
				default_user: None,
			})
		}
	}

	/// Save the current Minecraft credentials.
	#[tracing::instrument(skip(self))]
	pub async fn save(&self) -> crate::Result<()> {
		let state = crate::State::get().await?;
		let path = state.directories.caches_dir().await.join(AUTH_FILE);
		crate::utils::http::write(&path, &serde_json::to_vec(&self)?, &state.io_semaphore).await?;
		Ok(())
	}

	/// Refresh all tokens in the global state.
	#[tracing::instrument(skip(self))]
	async fn refresh(
		&mut self,
		current_date: DateTime<Utc>,
		force: bool,
	) -> crate::Result<(DeviceKey, DeviceToken, DateTime<Utc>, bool)> {
		macro_rules! device_key {
			($self:ident, $device_key:expr, $device_token:expr, $SaveDeviceToken:path) => {{
				let key = device_key()?;
				let res = device_token(&key, current_date).await?;

				self.token = Some(SaveDeviceToken {
					id: key.id.clone(),
					private_key: key
						.key
						.to_pkcs8_pem(p256::pkcs8::LineEnding::default())
						.map_err(|err| MinecraftAuthError::PKCS8Error(err))?
						.to_string(),
					x: key.x.clone(),
					y: key.y.clone(),
					token: res.value.clone(),
				});
				self.save().await?;

				(key, res.value, res.date, true)
			}};
		}

		let (key, token, date, valid_date) = if let Some(ref token) = self.token {
			if let Ok(private_key) = SigningKey::from_pkcs8_pem(&token.private_key) {
				if token.token.not_after > Utc::now() && !force {
					(
						DeviceKey {
							id: token.id.clone(),
							key: private_key,
							x: token.x.clone(),
							y: token.y.clone(),
						},
						token.token.clone(),
						current_date,
						false,
					)
				} else {
					let key = DeviceKey {
						id: token.id.clone(),
						key: private_key,
						x: token.x.clone(),
						y: token.y.clone(),
					};

					let res = device_token(&key, current_date).await?;

					(key, res.value, res.date, true)
				}
			} else {
				device_key!(self, device_key, device_token, SaveDeviceToken)
			}
		} else {
			device_key!(self, device_key, device_token, SaveDeviceToken)
		};

		Ok((key, token, date, valid_date))
	}

	/// Begin a Microsoft authentication flow.
	#[tracing::instrument(skip(self))]
	pub async fn begin(&mut self) -> crate::Result<MinecraftLogin> {
		let (key, token, current_date, valid_date) = self.refresh(Utc::now(), false).await?;
		let verify = generate_oauth_challenge();
		let mut hash = sha2::Sha256::new();
		hash.update(&verify);
		let hash_result = hash.finalize();
		let challenge = base64::prelude::BASE64_URL_SAFE_NO_PAD.encode(hash_result);

		match sisu_authenticate(&token.token, &challenge, &key, current_date).await {
			Ok((session_id, redirect_uri)) => Ok(MinecraftLogin {
				verify,
				challenge,
				session_id,
				redirect_uri: redirect_uri.value.msa_oauth_redirect,
			}),
			Err(err) => {
				if !valid_date {
					let (key, token, current_date, _) = self.refresh(Utc::now(), false).await?;
					let verify = generate_oauth_challenge();
					let mut hash = sha2::Sha256::new();
					hash.update(&verify);
					let hash_result = hash.finalize();
					let challenge = base64::prelude::BASE64_URL_SAFE_NO_PAD.encode(hash_result);
					let (session_id, redirect_uri) =
						sisu_authenticate(&token.token, &challenge, &key, current_date).await?;

					Ok(MinecraftLogin {
						verify,
						challenge,
						session_id,
						redirect_uri: redirect_uri.value.msa_oauth_redirect,
					})
				} else {
					Err(err.into())
				}
			}
		}
	}

	/// Finish a Microsoft authentication flow.
	#[tracing::instrument(skip(self))]
	pub async fn finish(
		&mut self,
		code: &str,
		flow: MinecraftLogin,
	) -> crate::Result<MinecraftCredentials> {
		let (key, token, _, _) = self.refresh(Utc::now(), false).await?;
		let oauth_token = oauth_token(code, &flow.verify).await?;
		let sisu_authorize = sisu_authorize(
			Some(&flow.session_id),
			&oauth_token.value.access_token,
			&token.token,
			&key,
			oauth_token.date,
		)
		.await?;
		let xbox_token = xsts_authorize(
			sisu_authorize.value,
			&token.token,
			&key,
			sisu_authorize.date,
		)
		.await?;
		let minecraft_token = minecraft_token(xbox_token.value).await?;
		minecraft_entitlements(&minecraft_token.access_token).await?;
		let profile = minecraft_profile(&minecraft_token.access_token).await?;
		let profile_id = profile.id.unwrap_or_default();
		let credentials = MinecraftCredentials {
			id: profile_id,
			username: profile.name,
			access_token: minecraft_token.access_token,
			refresh_token: oauth_token.value.refresh_token,
			#[allow(deprecated)]
			expires: oauth_token.date
				+ chrono::TimeDelta::seconds(oauth_token.value.expires_in as i64),
		};

		self.users.insert(profile_id, credentials.clone());

		if self.default_user.is_none() {
			self.default_user = Some(profile_id);
		}

		self.save().await?;

		Ok(credentials)
	}

	/// Refresh the current stored [`MinecraftCredentials`].
	async fn refresh_token(
		&mut self,
		creds: &MinecraftCredentials,
	) -> crate::Result<Option<MinecraftCredentials>> {
		let cred_id = creds.id;
		let cred_name = creds.username.clone();
		let oauth_token = oauth_refresh(&creds.refresh_token).await?;
		let (key, token, current_date, _) = self.refresh(oauth_token.date, false).await?;
		let sisu_authorize = sisu_authorize(
			None,
			&oauth_token.value.access_token,
			&token.token,
			&key,
			current_date,
		)
		.await?;
		let xbox_token = xsts_authorize(
			sisu_authorize.value,
			&token.token,
			&key,
			sisu_authorize.date,
		)
		.await?;
		let minecraft_token = minecraft_token(xbox_token.value).await?;
		let val = MinecraftCredentials {
			id: cred_id,
			username: cred_name,
			access_token: minecraft_token.access_token,
			refresh_token: oauth_token.value.refresh_token,
			#[allow(deprecated)]
			expires: oauth_token.date
				+ chrono::TimeDelta::seconds(oauth_token.value.expires_in as i64),
		};

		self.users.insert(val.id, val.clone());
		self.save().await?;

		Ok(Some(val))
	}

	/// Get the default user account as a set of optional [`MinecraftCredentials`].
	#[tracing::instrument(skip(self))]
	pub async fn get_default(&mut self) -> crate::Result<Option<MinecraftCredentials>> {
		let credentials = if let Some(default_user) = self.default_user {
			if let Some(creds) = self.users.get(&default_user) {
				Some(creds)
			} else {
				self.users.values().next()
			}
		} else {
			self.users.values().next()
		};

		if let Some(creds) = credentials {
			if self.default_user != Some(creds.id) {
				self.default_user = Some(creds.id);
				self.save().await?;
			}

			if creds.expires < Utc::now() {
				let old_creds = creds.clone();
				let res = self.refresh_token(&old_creds).await;

				match res {
					Ok(val) => Ok(val),
					Err(err) => {
						if let crate::ErrorKind::AuthError(MinecraftAuthError::RequestError {
							ref source,
							..
						}) = *err.raw
						{
							if source.is_connect() || source.is_timeout() {
								return Ok(Some(old_creds));
							}
						}

						Err(err)
					}
				}
			} else {
				Ok(Some(creds.clone()))
			}
		} else {
			Ok(None)
		}
	}

	/// Remove a set of [`MinecraftCredentials`] from a Uuid.
	#[tracing::instrument(skip(self))]
	pub async fn remove(&mut self, id: Uuid) -> crate::Result<Option<MinecraftCredentials>> {
		let val = self.users.remove(&id);

		if self.default_user == Some(id) {
			self.default_user = None;
		}

		self.save().await?;
		Ok(val)
	}
}

/// A [`reqwest::Request`] with a [`DateTime<Utc>`] attached to it.
struct RequestWithDate<T> {
	pub date: DateTime<Utc>,
	pub value: T,
}

/// A device token used for Microsoft authentication.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct DeviceToken {
	/// The time that the token was issued in [`DateTime<Utc>`].
	pub issue_instant: DateTime<Utc>,
	/// The expiration date of this token in [`DateTime<Utc>`].
	pub not_after: DateTime<Utc>,
	/// The token as a string.
	pub token: String,
	/// The JSON-encoded [`HashMap<String, Value>`] of display claims.
	pub display_claims: HashMap<String, serde_json::Value>,
}

/// A transferrable EC key translated to a [`SaveDeviceToken`]
#[derive(Debug)]
pub struct DeviceKey {
	/// The ID of this device key.
	pub id: String,
	/// The EC private key.
	pub key: SigningKey,
	/// The EC `x` value.
	pub x: String,
	/// The EC `y` value.
	pub y: String,
}

/// A serializable device token that is used to finish authenticaton.
#[derive(Serialize, Deserialize, Debug)]
pub struct SaveDeviceToken {
	/// The ID of the token.
	pub id: String,
	/// The private key associated with the token.
	pub private_key: String,
	/// The EC `x` value of the key.
	pub x: String,
	/// The EC `y` value of the key.
	pub y: String,
	/// The assocated [`DeviceToken`].
	pub token: DeviceToken,
}

/// Core variables passed throughout the Minecraft login flow.
#[derive(Serialize, Deserialize, Debug)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
pub struct MinecraftLogin {
	pub verify: String,
	/// The OAuth challenge generated at the start of the flow.
	pub challenge: String,
	/// The generated session id.
	pub session_id: String,
	/// The xboxlive redirect URI.
	pub redirect_uri: String,
}

/// A structure of all needed Minecraft credentials for logging in and account management.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
pub struct MinecraftCredentials {
	/// The uuid of the credentials.
	pub id: Uuid,
	/// The username of the Minecraft account.
	pub username: String,
	/// The access token as a String.
	pub access_token: String,
	/// The refresh token as a string for [`MinecraftState#refresh`].
	pub refresh_token: String,
	/// The time that the access token expires as a [`DateTime<Utc>`].
	pub expires: DateTime<Utc>,
}

#[tracing::instrument(skip(key))]
pub async fn device_token(
	key: &DeviceKey,
	current_date: DateTime<Utc>,
) -> Result<RequestWithDate<DeviceToken>, MinecraftAuthError> {
	let res = send_signed_request(
		None,
		"https://device.auth.xboxlive.com/device/authenticate",
		"/device/authenticate",
		json!({
			"Properties": {
				"AuthMethod": "ProofOfPossession",
				"Id": format!("{{{}}}", key.id),
				"DeviceType": "Win32",
				"Version": "10.16.0",
				"ProofKey": {
					"kty": "EC",
					"x": key.x,
					"y": key.y,
					"crv": "P-256",
					"alg": "ES256",
					"use": "sig"
				}
			},
			"RelyingParty": "http://auth.xboxlive.com",
			"TokenType": "JWT"
		}),
		key,
		MinecraftAuthStep::DeviceToken,
		current_date,
	)
	.await?;

	Ok(RequestWithDate {
		date: res.current_date,
		value: res.body,
	})
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RedirectUri {
	pub msa_oauth_redirect: String,
}

#[tracing::instrument(skip(key))]
async fn sisu_authenticate(
	token: &str,
	challenge: &str,
	key: &DeviceKey,
	current_date: DateTime<Utc>,
) -> Result<(String, RequestWithDate<RedirectUri>), MinecraftAuthError> {
	// what was microsoft on when writing this??????????
	let res = send_signed_request::<RedirectUri>(
		None,
		"https://sisu.xboxlive.com/authenticate",
		"/authenticate",
		json!({
			"AppId": MINECRAFT_CLIENT_ID,
			"DeviceToken": token,
			"Offers": [MINECRAFT_SCOPES],
			"Query": {
				"code_challenge": challenge,
				"code_challenge_method": "S256",
				"state": generate_oauth_challenge(),
				"prompt": "select_account"
			},
			"RedirectUri": MINECRAFT_REDIRECT_URL,
			"Sandbox": "RETAIL",
			"TokenType": "code",
			"TitleId": "1794566092"
		}),
		key,
		MinecraftAuthStep::SisuAuthenicate,
		current_date,
	)
	.await?;

	let session_id = res
		.headers
		.get("X-SessionId")
		.and_then(|x| x.to_str().ok())
		.ok_or_else(|| MinecraftAuthError::SessionIdError)?
		.to_string();

	Ok((
		session_id,
		RequestWithDate {
			date: res.current_date,
			value: res.body,
		},
	))
}

#[derive(Deserialize)]
struct OAuthToken {
	// pub token_type: String,
	pub expires_in: u64,
	// pub scope: String,
	pub access_token: String,
	pub refresh_token: String,
	// pub user_id: String,
	// pub foci: String,
}

#[tracing::instrument]
async fn oauth_token(
	code: &str,
	verify: &str,
) -> Result<RequestWithDate<OAuthToken>, MinecraftAuthError> {
	let mut query = HashMap::new();
	query.insert("client_id", MINECRAFT_CLIENT_ID);
	query.insert("code", code);
	query.insert("code_verifier", verify);
	query.insert("grant_type", "authorization_code");
	query.insert("redirect_uri", MINECRAFT_REDIRECT_URL);
	query.insert("scope", MINECRAFT_SCOPES);
	let res = auth_retry(|| {
		crate::utils::http::REQWEST_CLIENT
			.post("https://login.live.com/oauth20_token.srf")
			.header("Accept", "application/json")
			.form(&query)
			.send()
	})
	.await
	.map_err(|source| MinecraftAuthError::RequestError {
		source,
		step: MinecraftAuthStep::OAuthToken,
	})?;

	let status = res.status();
	let current_date = get_date_header(res.headers());
	let text = res
		.text()
		.await
		.map_err(|source| MinecraftAuthError::RequestError {
			step: MinecraftAuthStep::OAuthToken,
			source,
		})?;

	let body =
		serde_json::from_str(&text).map_err(|source| MinecraftAuthError::DeserializeError {
			step: MinecraftAuthStep::OAuthToken,
			raw: text,
			source,
			status_code: status,
		})?;

	Ok(RequestWithDate {
		date: current_date,
		value: body,
	})
}

#[tracing::instrument]
async fn oauth_refresh(
	refresh_token: &str,
) -> Result<RequestWithDate<OAuthToken>, MinecraftAuthError> {
	let mut query = HashMap::new();
	query.insert("client_id", MINECRAFT_CLIENT_ID);
	query.insert("refresh_token", refresh_token);
	query.insert("grant_type", "refresh_token");
	query.insert("redirect_uri", MINECRAFT_REDIRECT_URL);
	query.insert("scope", MINECRAFT_SCOPES);
	let res = auth_retry(|| {
		crate::utils::http::REQWEST_CLIENT
			.post("https://login.live.com/oauth20_token.srf")
			.header("Accept", "application/json")
			.form(&query)
			.send()
	})
	.await
	.map_err(|source| MinecraftAuthError::RequestError {
		step: MinecraftAuthStep::RefreshOAuthToken,
		source,
	})?;

	let status = res.status();
	let current_date = get_date_header(res.headers());
	let text = res
		.text()
		.await
		.map_err(|source| MinecraftAuthError::RequestError {
			step: MinecraftAuthStep::RefreshOAuthToken,
			source,
		})?;

	let body =
		serde_json::from_str(&text).map_err(|source| MinecraftAuthError::DeserializeError {
			step: MinecraftAuthStep::RefreshOAuthToken,
			raw: text,
			source,
			status_code: status,
		})?;

	Ok(RequestWithDate {
		date: current_date,
		value: body,
	})
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct SisuAuthorize {
	// pub authorization_token: DeviceToken,
	// pub device_token: String,
	// pub sandbox: String,
	pub title_token: DeviceToken,
	pub user_token: DeviceToken,
	// pub web_page: string,
}

#[tracing::instrument(skip(key))]
async fn sisu_authorize(
	session_id: Option<&str>,
	access_token: &str,
	device_token: &str,
	key: &DeviceKey,
	current_date: DateTime<Utc>,
) -> Result<RequestWithDate<SisuAuthorize>, MinecraftAuthError> {
	let res = send_signed_request(
		None,
		"https://sisu.xboxlive.com/authorize",
		"/authorize",
		json!({
			"AccessToken": format!("t={access_token}"),
			"AppId": MINECRAFT_CLIENT_ID,
			"DeviceToken": device_token,
			"ProofKey": {
				// kibty (kty)
				"kty": "EC",
				"x": key.x,
				"y": key.y,
				"crv": "P-256",
				"alg": "ES256",
				"use": "sig"
			},
			"Sandbox": "RETAIL",
			"SessionId": session_id,
			"SiteName": "user.auth.xboxlive.com",
			"RelyingParty": "http://xboxlive.com",
			"UseModernGamertag": true
		}),
		key,
		MinecraftAuthStep::SisuAuthorize,
		current_date,
	)
	.await?;

	Ok(RequestWithDate {
		date: res.current_date,
		value: res.body,
	})
}

#[tracing::instrument(skip(key))]
async fn xsts_authorize(
	authorize: SisuAuthorize,
	device_token: &str,
	key: &DeviceKey,
	current_date: DateTime<Utc>,
) -> Result<RequestWithDate<DeviceToken>, MinecraftAuthError> {
	let res = send_signed_request(
		None,
		"https://xsts.auth.xboxlive.com/xsts/authorize",
		"/xsts/authorize",
		json!({
			"RelyingParty": "rp://api.minecraftservices.com/",
			"TokenType": "JWT",
			"Properties": {
				"SandboxId": "RETAIL",
				"UserTokens": [authorize.user_token.token],
				"DeviceToken": device_token,
				"TitleToken": authorize.title_token.token,
			},
		}),
		key,
		MinecraftAuthStep::XstsAuthorize,
		current_date,
	)
	.await?;

	Ok(RequestWithDate {
		date: res.current_date,
		value: res.body,
	})
}

#[derive(Deserialize)]
struct MinecraftToken {
	// pub username: String,
	pub access_token: String,
	// pub token_type: String,
	// pub expires_in: u64,
}

#[tracing::instrument]
async fn minecraft_token(token: DeviceToken) -> Result<MinecraftToken, MinecraftAuthError> {
	let uhs = token
		.display_claims
		.get("xui")
		.and_then(|x| x.get(0))
		.and_then(|x| x.get("uhs"))
		.and_then(|x| x.as_str().map(String::from))
		.ok_or_else(|| MinecraftAuthError::HashError)?;

	let token = token.token;
	let res = auth_retry(|| {
		crate::utils::http::REQWEST_CLIENT
			.post("https://api.minecraftservices.com/launcher/login")
			.header("Accept", "application/json")
			.json(&json!({
				"platform": "PC_LAUNCHER",
				"xtoken": format!("XBL3.0 x={uhs};{token}"),
			}))
			.send()
	})
	.await
	.map_err(|source| MinecraftAuthError::RequestError {
		step: MinecraftAuthStep::MinecraftToken,
		source,
	})?;

	let status = res.status();
	let text = res
		.text()
		.await
		.map_err(|source| MinecraftAuthError::RequestError {
			step: MinecraftAuthStep::MinecraftToken,
			source,
		})?;

	serde_json::from_str(&text).map_err(|source| MinecraftAuthError::DeserializeError {
		step: MinecraftAuthStep::MinecraftToken,
		raw: text,
		source,
		status_code: status,
	})
}

#[derive(Deserialize)]
struct MinecraftProfile {
	pub id: Option<Uuid>,
	pub name: String,
}

#[tracing::instrument]
async fn minecraft_profile(token: &str) -> Result<MinecraftProfile, MinecraftAuthError> {
	let res = auth_retry(|| {
		crate::utils::http::REQWEST_CLIENT
			.get("https://api.minecraftservices.com/minecraft/profile")
			.header("Accept", "application/json")
			.bearer_auth(token)
			.send()
	})
	.await
	.map_err(|source| MinecraftAuthError::RequestError {
		step: MinecraftAuthStep::MinecraftProfile,
		source,
	})?;

	let status = res.status();
	let text = res
		.text()
		.await
		.map_err(|source| MinecraftAuthError::RequestError {
			step: MinecraftAuthStep::MinecraftProfile,
			source,
		})?;

	serde_json::from_str(&text).map_err(|source| MinecraftAuthError::DeserializeError {
		step: MinecraftAuthStep::MinecraftProfile,
		raw: text,
		source,
		status_code: status,
	})
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MinecraftEntitlements {}

#[tracing::instrument]
async fn minecraft_entitlements(token: &str) -> Result<MinecraftEntitlements, MinecraftAuthError> {
	let res = auth_retry(|| {
		crate::utils::http::REQWEST_CLIENT
			.get("https://api.minecraftservices.com/entitlements/mcstore")
			.header("Accept", "application/json")
			.bearer_auth(token)
			.send()
	})
	.await
	.map_err(|source| MinecraftAuthError::RequestError {
		step: MinecraftAuthStep::MinecraftEntitlements,
		source,
	})?;

	let status = res.status();
	let text = res
		.text()
		.await
		.map_err(|source| MinecraftAuthError::RequestError {
			step: MinecraftAuthStep::MinecraftEntitlements,
			source,
		})?;

	serde_json::from_str(&text).map_err(|source| MinecraftAuthError::DeserializeError {
		step: MinecraftAuthStep::MinecraftEntitlements,
		raw: text,
		source,
		status_code: status,
	})
}

#[tracing::instrument(skip(request_fn))]
async fn auth_retry<F>(request_fn: impl Fn() -> F) -> Result<reqwest::Response, reqwest::Error>
where
	F: std::future::Future<Output = Result<reqwest::Response, reqwest::Error>>,
{
	const RETRY_COUNT: usize = 5;
	const RETRY_WAIT: tokio::time::Duration = tokio::time::Duration::from_millis(250);
	let mut resp = request_fn().await;

	for i in 0..RETRY_COUNT {
		match &resp {
			Ok(_) => {
				break;
			}
			Err(err) => {
				if err.is_connect() || err.is_timeout() {
					if i < RETRY_COUNT - 1 {
						tracing::debug!(
							"request failed due to a connection error, retrying request..."
						);
						tokio::time::sleep(RETRY_WAIT).await;
						resp = request_fn().await;
					} else {
						break;
					}
				}
			}
		}
	}

	resp
}

#[tracing::instrument]
fn device_key() -> Result<DeviceKey, MinecraftAuthError> {
	let id = Uuid::new_v4().to_string().to_uppercase();
	let signing_key = SigningKey::random(&mut rand::rngs::OsRng);
	let public_key = p256::ecdsa::VerifyingKey::from(&signing_key);
	let encoded_point = public_key.to_encoded_point(false);

	Ok(DeviceKey {
		id,
		key: signing_key,
		x: base64::prelude::BASE64_STANDARD_NO_PAD.encode(
			encoded_point
				.x()
				.ok_or_else(|| MinecraftAuthError::PublicKeyReading)?,
		),
		y: base64::prelude::BASE64_STANDARD_NO_PAD.encode(
			encoded_point
				.y()
				.ok_or_else(|| MinecraftAuthError::PublicKeyReading)?,
		),
	})
}

struct SignedRequestResponse<T> {
	pub headers: reqwest::header::HeaderMap,
	pub current_date: DateTime<Utc>,
	pub body: T,
}

async fn send_signed_request<T: serde::de::DeserializeOwned>(
	authorization: Option<&str>,
	url: &str,
	url_path: &str,
	raw_body: serde_json::Value,
	key: &DeviceKey,
	step: MinecraftAuthStep,
	current_date: DateTime<Utc>,
) -> Result<SignedRequestResponse<T>, MinecraftAuthError> {
	let auth = authorization.map_or(Vec::new(), |v| v.as_bytes().to_vec());
	let body = serde_json::to_vec(&raw_body)
		.map_err(|source| MinecraftAuthError::SerializeError { step, source })?;
	let time: u128 = { ((current_date.timestamp() as u128) + 11644473600) * 10000000 };

	use byteorder::WriteBytesExt;
	let mut buffer = Vec::new();
	buffer
		.write_u32::<byteorder::BigEndian>(1)
		.map_err(|source| MinecraftAuthError::SigningError { step, source })?;
	buffer
		.write_u8(0)
		.map_err(|source| MinecraftAuthError::SigningError { step, source })?;
	buffer
		.write_u64::<byteorder::BigEndian>(time as u64)
		.map_err(|source| MinecraftAuthError::SigningError { step, source })?;
	buffer
		.write_u8(0)
		.map_err(|source| MinecraftAuthError::SigningError { step, source })?;
	buffer.extend_from_slice("POST".as_bytes());
	buffer
		.write_u8(0)
		.map_err(|source| MinecraftAuthError::SigningError { step, source })?;
	buffer.extend_from_slice(url_path.as_bytes());
	buffer
		.write_u8(0)
		.map_err(|source| MinecraftAuthError::SigningError { step, source })?;
	buffer.extend_from_slice(&auth);
	buffer
		.write_u8(0)
		.map_err(|source| MinecraftAuthError::SigningError { step, source })?;
	buffer.extend_from_slice(&body);
	buffer
		.write_u8(0)
		.map_err(|source| MinecraftAuthError::SigningError { step, source })?;

	let ecdsa_sig: p256::ecdsa::Signature = key.key.sign(&buffer);
	let mut sig_buffer = Vec::new();

	sig_buffer
		.write_i32::<byteorder::BigEndian>(1)
		.map_err(|source| MinecraftAuthError::SigningError { step, source })?;
	sig_buffer
		.write_u64::<byteorder::BigEndian>(time as u64)
		.map_err(|source| MinecraftAuthError::SigningError { step, source })?;
	sig_buffer.extend_from_slice(&ecdsa_sig.r().to_bytes());
	sig_buffer.extend_from_slice(&ecdsa_sig.s().to_bytes());

	let signature = base64::prelude::BASE64_STANDARD.encode(&sig_buffer);

	let res = auth_retry(|| {
		let mut request = crate::utils::http::REQWEST_CLIENT
			.post(url)
			.header("Content-Type", "application/json; charset=utf-8")
			.header("Accept", "application/json")
			.header("Signature", &signature);

		if url != "https://sisu.xboxlive.com/authorize" {
			request = request.header("x-xbl-contract-version", "1");
		}

		if let Some(auth) = authorization {
			request = request.header("Authorization", auth);
		}

		request.body(body.clone()).send()
	})
	.await
	.map_err(|source| MinecraftAuthError::RequestError { step, source })?;

	let status = res.status();
	let headers = res.headers().clone();
	let current_date = get_date_header(&headers);
	let body = res
		.text()
		.await
		.map_err(|source| MinecraftAuthError::RequestError { step, source })?;

	let body =
		serde_json::from_str(&body).map_err(|source| MinecraftAuthError::DeserializeError {
			step,
			raw: body,
			source,
			status_code: status,
		})?;
	Ok(SignedRequestResponse {
		headers,
		current_date,
		body,
	})
}

#[tracing::instrument]
fn get_date_header(headers: &reqwest::header::HeaderMap) -> DateTime<Utc> {
	headers
		.get(reqwest::header::DATE)
		.and_then(|x| x.to_str().ok())
		.and_then(|x| DateTime::parse_from_rfc2822(x).ok())
		.map(|x| x.with_timezone(&Utc))
		.unwrap_or(Utc::now())
}

#[tracing::instrument]
fn generate_oauth_challenge() -> String {
	let mut rng = rand::thread_rng();
	let bytes: Vec<u8> = (0..64).map(|_| rng.gen::<u8>()).collect();
	bytes.iter().fold(String::new(), |mut output, b| {
		let _ = write!(output, "{b:02x}");
		output
	})
}

/// An ordered list of all MSA authentication steps.
#[derive(Debug, Clone, Copy)]
pub enum MinecraftAuthStep {
	DeviceToken,
	SisuAuthenicate,
	OAuthToken,
	RefreshOAuthToken,
	SisuAuthorize,
	XstsAuthorize,
	MinecraftToken,
	MinecraftEntitlements,
	MinecraftProfile,
}

/// Wrapper around all `Error`s that can occur during the Microsoft authentication process.
#[derive(thiserror::Error, Debug)]
pub enum MinecraftAuthError {
	#[error("failed to read public key during key generation")]
	PublicKeyReading,
	#[error("failed to serialize private key using PKCS8: {0}")]
	PKCS8Error(#[from] p256::pkcs8::Error),
	#[error("failed to serialize JSON during MSA step {step:?}: {source}")]
	SerializeError {
		step: MinecraftAuthStep,
		#[source]
		source: serde_json::Error,
	},
	#[error("failed to deserialize JSON during MSA step {step:?}: {source}! status code {status_code} - body: {raw}")]
	DeserializeError {
		step: MinecraftAuthStep,
		raw: String,
		#[source]
		source: serde_json::Error,
		status_code: reqwest::StatusCode,
	},
	#[error("failed to request using HTTP during MSA step {step:?}: {source}")]
	RequestError {
		step: MinecraftAuthStep,
		#[source]
		source: reqwest::Error,
	},
	#[error("failed to create signed buffer during MSA step {step:?}: {source}")]
	SigningError {
		step: MinecraftAuthStep,
		#[source]
		source: std::io::Error,
	},
	#[error("failed to read user hash")]
	HashError,
	#[error("failed to read user xbox session ID")]
	SessionIdError,
}
