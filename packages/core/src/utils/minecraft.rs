use base64::Engine;
use reqwest::Method;

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
