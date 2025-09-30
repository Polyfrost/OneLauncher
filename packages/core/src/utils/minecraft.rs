use std::collections::HashMap;

use base64::Engine;
use reqwest::{Method, multipart};
use serde_json::json;

use crate::error::LauncherResult;
use crate::utils::http;

#[onelauncher_macro::specta]
#[derive(Debug, serde::Serialize, Clone)]
pub struct MojangPlayerProfile {
	pub uuid: String,
	pub username: String,
	pub is_slim: bool,
	pub skin_url: Option<String>,
	pub cape_url: Option<String>,
}

#[onelauncher_macro::specta]
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum SkinVariant {
	Classic,
	Slim,
}

impl std::fmt::Display for SkinVariant {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			SkinVariant::Classic => write!(f, "classic"),
			SkinVariant::Slim => write!(f, "slim"),
		}
	}
}

#[onelauncher_macro::specta]
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct MojangFullPlayerProfile {
	pub uuid: String,
	pub username: String,
	pub skins: Vec<MojangSkin>,
	pub capes: Vec<MojangCape>,
}

#[onelauncher_macro::specta]
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct MojangSkin {
	pub id: String,
	pub state: String,
	pub url: String,
	pub variant: SkinVariant,
}

#[onelauncher_macro::specta]
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct MojangCape {
	pub id: String,
	pub state: String,
	pub url: String,
	pub alias: String,
}

pub async fn fetch_player_profile(uuid: &str) -> LauncherResult<MojangPlayerProfile> {
	#[derive(serde::Deserialize)]
	struct Properties {
		name: String,
		value: String,
	}

	#[derive(serde::Deserialize)]
	struct Response {
		id: String,
		name: String,
		properties: Vec<Properties>,
	}

	let response = http::fetch_json::<Response>(
		Method::GET,
		&format!("https://sessionserver.mojang.com/session/minecraft/profile/{uuid}"),
		None,
		None,
	)
	.await?;

	#[derive(Default, serde::Deserialize)]
	#[serde(default)]
	struct TextureMetadata {
		model: Option<String>,
	}

	#[derive(serde::Deserialize)]
	struct Texture {
		url: String,
		#[serde(default)]
		metadata: TextureMetadata,
	}

	#[derive(serde::Deserialize)]
	#[serde(rename_all = "UPPERCASE")]
	struct Textures {
		skin: Option<Texture>,
		cape: Option<Texture>,
	}

	#[derive(serde::Deserialize)]
	struct DecodedProperties {
		textures: Textures,
	}

	let texture_property = response
		.properties
		.iter()
		.find(|p| p.name == "textures")
		.ok_or_else(|| anyhow::anyhow!("no textures found in player profile"))?;

	let decoded = base64::prelude::BASE64_STANDARD
		.decode(texture_property.value.as_bytes())
		.map_err(|_| {
			anyhow::anyhow!(
				"failed to decode base64 texture property: {}",
				texture_property.value
			)
		})?;

	let decoded = serde_json::from_slice::<DecodedProperties>(&decoded)
		.map_err(|_| {
			anyhow::anyhow!(
				"failed to parse JSON from decoded texture property: {}",
				texture_property.value
			)
		})?
		.textures;

	let is_slim = decoded
		.skin
		.as_ref()
		.map_or(false, |s| s.metadata.model.as_deref() == Some("slim"));

	Ok(MojangPlayerProfile {
		uuid: response.id,
		username: response.name,
		is_slim,
		skin_url: decoded.skin.as_ref().map(|s| s.url.clone()),
		cape_url: decoded.cape.as_ref().map(|c| c.url.clone()),
	})
}

pub async fn fetch_logged_in_profile(
	access_token: &str,
) -> LauncherResult<MojangFullPlayerProfile> {
	let mut headers: HashMap<&str, &str> = HashMap::with_capacity(1);

	let bearer = &format!("Bearer {access_token}");
	headers.insert("Authorization", bearer);

	http::fetch_json_advanced::<MojangFullPlayerProfile>(
		Method::GET,
		"https://api.minecraftservices.com/minecraft/profile",
		None,
		Some(headers),
		None,
		None,
	)
	.await
}

pub async fn upload_skin_bytes(
	access_token: &str,
	skin_data: Vec<u8>,
	image_format: &str,
	skin_variant: SkinVariant,
) -> LauncherResult<MojangSkin> {
	let bytes_part =
		multipart::Part::bytes(skin_data).mime_str(&format!("image/{}", image_format))?;

	let form = multipart::Form::new()
		.text("variant", skin_variant.to_string())
		.part("data", bytes_part);

	let req = http::build_request(
		Method::POST,
		"https://api.minecraftservices.com/minecraft/profile/skins",
	)
	.header("Authorization", &format!("Bearer {access_token}"))
	.header("Content-Type", "multipart/form-data")
	.multipart(form)
	.build()?;

	let res = http::send_request(req).await?;

	if !res.status().is_success() {
		let status = res.status();
		let text = res.text().await.unwrap_or_default();
		return Err(
			anyhow::anyhow!("failed to upload skin: HTTP {}: {}", status.as_u16(), text).into(),
		);
	}

	let full_profile = res.json::<MojangFullPlayerProfile>().await?;
	dbg!(&full_profile);
	full_profile
		.skins
		.first()
		.cloned()
		.ok_or_else(|| anyhow::anyhow!("no skins found in response").into())
}

pub async fn change_skin(
	access_token: &str,
	skin_url: &str,
	skin_variant: SkinVariant,
) -> LauncherResult<MojangSkin> {
	let mut headers: HashMap<&str, &str> = HashMap::with_capacity(1);

	let bearer = &format!("Bearer {access_token}");
	headers.insert("Authorization", bearer);

	let profile = http::fetch_json_advanced::<MojangFullPlayerProfile>(
		Method::POST,
		"https://api.minecraftservices.com/minecraft/profile/skins",
		Some(json!({
			"url": skin_url,
			"variant": skin_variant.to_string(),
		})),
		Some(headers),
		None,
		None,
	)
	.await?;

	profile
		.skins
		.first()
		.cloned()
		.ok_or_else(|| anyhow::anyhow!("no skins found in response").into())
}
