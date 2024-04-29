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
use uuid::Uuid;

const AUTH_STORE: &str = "authentication.json";

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
		let path = dirs.caches_dir().await.join(AUTH_STORE);
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
		let path = state.directories.caches_dir().await.join(AUTH_STORE);
		crate::utils::http::write(&path, &serde_json::to_vec(&self)?, &state.io_semaphore).await?;
		Ok(())
	}

	/// Refresh all tokens in the global state.
	#[tracing::instrument(skip(self))]
	async fn refresh(&mut self) -> crate::Result<(DeviceKey, DeviceToken)> {
		macro_rules! device_key {
			($self:ident, $device_key:expr, $device_token:expr, $SaveDeviceToken:path) => {{
				let key = device_key()?;
				let token = device_token(&key).await?;

				self.token = Some(SaveDeviceToken {
					id: key.id.clone(),
					private_key: key
						.key
						.to_pkcs8_pem(p256::pkcs8::LineEnding::default())
						.map_err(|err| MinecraftAuthError::PKCS8Error(err))?
						.to_string(),
					x: key.x.clone(),
					y: key.y.clone(),
					token: token.clone(),
					modern: true,
				});
				self.save().await?;

				(key, token)
			}};
		}

		let (key, token) = if let Some(ref token) = self.token {
			if token.token.not_after > Utc::now() {
				if let Ok(private_key) = SigningKey::from_pkcs8_pem(&token.private_key) {
					(
						DeviceKey {
							id: token.id.clone(),
							key: private_key,
							x: token.x.clone(),
							y: token.y.clone(),
						},
						token.token.clone(),
					)
				} else {
					device_key!(self, device_key, device_token, SaveDeviceToken)
				}
			} else {
				device_key!(self, device_key, device_token, SaveDeviceToken)
			}
		} else {
			device_key!(self, device_key, device_token, SaveDeviceToken)
		};

		Ok((key, token))
	}

	/// Begin a Microsoft authentication flow.
	#[tracing::instrument(skip(self))]
	pub async fn begin(&mut self) -> crate::Result<MinecraftLogin> {
		let (key, token) = self.refresh().await?;
		let verify = generate_oauth_challenge();
		let mut hash = sha2::Sha256::new();
		hash.update(&verify);
		let hash_result = hash.finalize();
		let challenge = base64::prelude::BASE64_URL_SAFE_NO_PAD.encode(&hash_result);
		let (sid, ruri) = sisu_authenticate(&token.token, &challenge, &key).await?;
		Ok(MinecraftLogin {
			verify,
			challenge,
			session_id: sid,
			redirect_uri: ruri.msa_oauth_redirect,
		})
	}

	/// Finish a Microsoft authentication flow.
	#[tracing::instrument(skip(self))]
	pub async fn finish(
		&mut self,
		code: &str,
		flow: MinecraftLogin,
	) -> crate::Result<MinecraftCredentials> {
		let (key, token) = self.refresh().await?;
		let oauth_token = oauth_token(code, &flow.verify).await?;
		let sisu_authorize = sisu_authorize(
			Some(&flow.session_id),
			&oauth_token.access_token,
			&token.token,
			&key,
		)
		.await?;
		let xbox_token = xsts_authorize(sisu_authorize, &token.token, &key).await?;
		let minecraft_token = minecraft_token(xbox_token).await?;
		minecraft_entitlements(&minecraft_token.access_token).await?;
		let profile = minecraft_profile(&minecraft_token.access_token).await?;
		let profile_id = profile.id.unwrap_or_default();
		let credentials = MinecraftCredentials {
			id: profile_id,
			username: profile.name,
			access_token: minecraft_token.access_token,
			refresh_token: oauth_token.refresh_token,
			#[allow(deprecated)]
			expires: Utc::now() + chrono::TimeDelta::seconds(oauth_token.expires_in as i64),
		};

		self.users.insert(profile_id, credentials.clone());

		if self.default_user.is_none() {
			self.default_user = Some(profile_id);
		}

		self.save().await?;

		Ok(credentials)
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
				let cred_id = creds.id;
				let profile_name = creds.username.clone();

				let oauth_token = oauth_refresh(&creds.refresh_token).await?;
				let (key, token) = self.refresh().await?;
				let sisu_authorize =
					sisu_authorize(None, &oauth_token.access_token, &token.token, &key).await?;

				let xbox_token = xsts_authorize(sisu_authorize, &token.token, &key).await?;
				let minecraft_token = minecraft_token(xbox_token).await?;
				let val = MinecraftCredentials {
					id: cred_id,
					username: profile_name,
					access_token: minecraft_token.access_token,
					refresh_token: oauth_token.refresh_token,
					#[allow(deprecated)]
					expires: Utc::now() + chrono::TimeDelta::seconds(oauth_token.expires_in as i64),
				};

				self.users.insert(val.id, val.clone());
				self.save().await?;

				Ok(Some(val))
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
		self.save().await?;
		Ok(val)
	}
}

// TODO: get MICROSOFT_CLIENT_ID
/// Publically used Minecraft client ID for OneLauncher.
const MICROSOFT_CLIENT_ID: &str = "";
/// Microsoft login redirect URI.
const REDIRECT_URL: &str = "https://login.live.com/oauth20_desktop.srf";
/// Microsoft login xboxlive scopes to get tokens.
const SCOPES: &str = "service::user.auth.xboxlive.com::MBI_SSL";

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
	/// Whether or not we should patch the auth device token to use a modern auth flow
	#[serde(default)]
	modern: bool,
}

/// Core variables passed throughout the Minecraft login flow.
#[derive(Serialize, Deserialize, Debug)]
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
pub async fn device_token(key: &DeviceKey) -> Result<DeviceToken, MinecraftAuthError> {
	Ok(send_signed_request(
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
	)
	.await?
	.1)
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
) -> Result<(String, RedirectUri), MinecraftAuthError> {
	// what was microsoft on when writing this??????????
	let (headers, res) = send_signed_request(
		None,
		"https://sisu.xboxlive.com/authenticate",
		"/authenticate",
		json!({
			"AppId": MICROSOFT_CLIENT_ID,
			"DeviceToken": token,
			"Offers": [SCOPES],
			"Query": {
				"code_challenge": challenge,
				"code_challenge_method": "S256",
				"state": generate_oauth_challenge(),
				"prompt": "select_account"
			},
			"RedirectUri": REDIRECT_URL,
			"Sandbox": "RETAIL",
			"TokenType": "code",
			"TitleId": "1794566092"
		}),
		key,
		MinecraftAuthStep::SisuAuthenicate,
	)
	.await?;

	let session_id = headers
		.get("X-SessionId")
		.and_then(|x| x.to_str().ok())
		.ok_or_else(|| MinecraftAuthError::SessionIdError)?
		.to_string();

	Ok((session_id, res))
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
async fn oauth_token(code: &str, verify: &str) -> Result<OAuthToken, MinecraftAuthError> {
	let mut query = HashMap::new();
	query.insert("client_id", MICROSOFT_CLIENT_ID);
	query.insert("code", code);
	query.insert("code_verifier", &*verify);
	query.insert("grant_type", "authorization_code");
	query.insert("redirect_uri", REDIRECT_URL);
	query.insert("scope", SCOPES);
	let res = auth_retry(|| {
		crate::utils::http::REQWEST_CLIENT
			.post(REDIRECT_URL)
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
	let text = res
		.text()
		.await
		.map_err(|source| MinecraftAuthError::RequestError {
			step: MinecraftAuthStep::OAuthToken,
			source,
		})?;

	serde_json::from_str(&text).map_err(|source| MinecraftAuthError::DeserializeError {
		step: MinecraftAuthStep::OAuthToken,
		raw: text,
		source,
		status_code: status,
	})
}

#[tracing::instrument]
async fn oauth_refresh(refresh_token: &str) -> Result<OAuthToken, MinecraftAuthError> {
	let mut query = HashMap::new();
	query.insert("client_id", MICROSOFT_CLIENT_ID);
	query.insert("refresh_token", refresh_token);
	query.insert("grant_type", "refresh_token");
	query.insert("redirect_uri", REDIRECT_URL);
	query.insert("scope", SCOPES);
	let res = auth_retry(|| {
		crate::utils::http::REQWEST_CLIENT
			.post(REDIRECT_URL)
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
	let text = res
		.text()
		.await
		.map_err(|source| MinecraftAuthError::RequestError {
			step: MinecraftAuthStep::RefreshOAuthToken,
			source,
		})?;

	serde_json::from_str(&text).map_err(|source| MinecraftAuthError::DeserializeError {
		step: MinecraftAuthStep::RefreshOAuthToken,
		raw: text,
		source,
		status_code: status,
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
) -> Result<SisuAuthorize, MinecraftAuthError> {
	Ok(send_signed_request(
		None,
		"https://sisu.xboxlive.com/authorize",
		"/authorize",
		json!({
			"AccessToken": format!("t={access_token}"),
			"AppId": MICROSOFT_CLIENT_ID,
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
	)
	.await?
	.1)
}

#[tracing::instrument(skip(key))]
async fn xsts_authorize(
	authorize: SisuAuthorize,
	device_token: &str,
	key: &DeviceKey,
) -> Result<DeviceToken, MinecraftAuthError> {
	Ok(send_signed_request(
		None,
		"https://xsts.auth.xboxlive.com/xsts/authorize",
		"/xsts/authorize",
		json!({
			"RelyingParty": "rp://api.minecraftservices.com/",
			"TokenType": "JWT",
			"Properties": {
				"SandboxId": "RETAIL",
				"UserTokens": [authorize.user_token.token],
				"DeviceTokens": device_token,
				"TitleToken": authorize.title_token.token,
			},
		}),
		key,
		MinecraftAuthStep::XstsAuthorize,
	)
	.await?
	.1)
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
			.get("https//api.minecraftservices.com/minecraft/profile")
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

async fn send_signed_request<T: serde::de::DeserializeOwned>(
	authorization: Option<&str>,
	url: &str,
	url_path: &str,
	raw_body: serde_json::Value,
	key: &DeviceKey,
	step: MinecraftAuthStep,
) -> Result<(reqwest::header::HeaderMap, T), MinecraftAuthError> {
	let auth = authorization
		.clone()
		.map_or(Vec::new(), |v| v.as_bytes().to_vec());
	let body = serde_json::to_vec(&raw_body)
		.map_err(|source| MinecraftAuthError::SerializeError { step, source })?;
	let time: u128 = { ((Utc::now().timestamp() as u128) + 11644473600) * 10000000 };

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
	Ok((headers, body))
}

#[tracing::instrument]
fn generate_oauth_challenge() -> String {
	let mut rng = rand::thread_rng();
	let bytes: Vec<u8> = (0..64).map(|_| rng.gen::<u8>()).collect();
	bytes.iter().map(|b| format!("{:02x}", b)).collect()
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
