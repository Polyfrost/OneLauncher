use base64::Engine;
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::http::RequestClient;
use crate::{LauncherError, LauncherResult};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MojangPlayerProfile {
    pub uuid: String,
    pub username: String,
    pub is_slim: bool,
    pub skin_url: Option<String>,
    pub cape_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PlayerProfileView {
    pub uuid: String,
    pub username: String,
    pub is_slim: bool,
    pub skin_url: Option<String>,
    pub cape_url: Option<String>,
    pub skins: Vec<MojangSkin>,
    pub capes: Vec<MojangCape>,
}

impl PlayerProfileView {
    pub fn placeholder() -> Self {
        Self {
            uuid: "00000000-0000-0000-0000-000000000000".into(),
            username: "Player".into(),
            is_slim: false,
            skin_url: None,
            cape_url: None,
            skins: Vec::new(),
            capes: Vec::new(),
        }
    }
}

impl From<MojangPlayerProfile> for PlayerProfileView {
    fn from(profile: MojangPlayerProfile) -> Self {
        Self {
            uuid: profile.uuid,
            username: profile.username,
            is_slim: profile.is_slim,
            skin_url: profile.skin_url,
            cape_url: profile.cape_url,
            skins: Vec::new(),
            capes: Vec::new(),
        }
    }
}

impl From<MojangFullPlayerProfile> for PlayerProfileView {
    fn from(profile: MojangFullPlayerProfile) -> Self {
        let active_skin = profile
            .skins
            .iter()
            .find(|skin| skin.state.eq_ignore_ascii_case("ACTIVE"));
        let active_cape = profile
            .capes
            .iter()
            .find(|cape| cape.state.eq_ignore_ascii_case("ACTIVE"));

        Self {
            uuid: profile.id,
            username: profile.username,
            is_slim: active_skin
                .is_some_and(|skin| skin.variant == SkinVariant::Slim),
            skin_url: active_skin.map(|skin| skin.url.clone()),
            cape_url: active_cape.map(|cape| cape.url.clone()),
            skins: profile.skins,
            capes: profile.capes,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SkinVariant {
    #[serde(alias = "CLASSIC")]
    Classic,
    #[serde(alias = "SLIM")]
    Slim,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MojangFullPlayerProfile {
    pub id: String,
    #[serde(alias = "name")]
    pub username: String,
    pub skins: Vec<MojangSkin>,
    pub capes: Vec<MojangCape>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MojangSkin {
    pub id: String,
    pub state: String,
    pub url: String,
    pub variant: SkinVariant,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MojangCape {
    pub id: String,
    pub state: String,
    pub url: String,
    pub alias: String,
}

fn normalize_texture_url(url: &str) -> String {
    url.replace(
        "http://textures.minecraft.net",
        "https://textures.minecraft.net",
    )
}

fn normalize_uuid(uuid: &str) -> &str {
    uuid.trim().trim_matches(|c: char| c == '-' || c.is_whitespace())
}

fn auth_headers(access_token: &str) -> [(&'static str, String); 1] {
    [("Authorization", format!("Bearer {access_token}"))]
}

#[tracing::instrument(level = "debug", skip(client))]
pub async fn fetch_player_profile(
    client: &RequestClient,
    uuid: &str,
) -> LauncherResult<MojangPlayerProfile> {
    #[derive(Deserialize)]
    struct Properties {
        name: String,
        value: String,
    }

    #[derive(Deserialize)]
    struct Response {
        id: String,
        name: String,
        properties: Vec<Properties>,
    }

    #[derive(Default, Deserialize)]
    #[serde(default)]
    struct TextureMetadata {
        model: Option<String>,
    }

    #[derive(Deserialize)]
    struct Texture {
        url: String,
        #[serde(default)]
        metadata: TextureMetadata,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "UPPERCASE")]
    struct Textures {
        skin: Option<Texture>,
        cape: Option<Texture>,
    }

    #[derive(Deserialize)]
    struct DecodedProperties {
        textures: Textures,
    }

    let uuid = normalize_uuid(uuid);
    let url = format!("https://sessionserver.mojang.com/session/minecraft/profile/{uuid}")
        .parse()
        .map_err(LauncherError::UrlError)?;

    let response = client
        .send_as::<Response>(reqwest::Request::new(Method::GET, url))
        .await?;

    let texture_property = response
        .properties
        .iter()
        .find(|p| p.name == "textures")
        .ok_or_else(|| LauncherError::Minecraft("no textures in player profile".into()))?;

    let decoded = base64::prelude::BASE64_STANDARD
        .decode(texture_property.value.as_bytes())
        .map_err(|_| {
            LauncherError::Minecraft("failed to decode profile texture property".into())
        })?;

    let decoded = serde_json::from_slice::<DecodedProperties>(&decoded)
        .map_err(LauncherError::JsonError)?
        .textures;

    let is_slim = decoded
        .skin
        .as_ref()
        .is_some_and(|s| s.metadata.model.as_deref() == Some("slim"));

    Ok(MojangPlayerProfile {
        uuid: response.id,
        username: response.name,
        is_slim,
        skin_url: decoded
            .skin
            .as_ref()
            .map(|s| normalize_texture_url(&s.url)),
        cape_url: decoded
            .cape
            .as_ref()
            .map(|c| normalize_texture_url(&c.url)),
    })
}

#[tracing::instrument(level = "debug", skip(client, access_token))]
pub async fn fetch_logged_in_profile(
    client: &RequestClient,
    access_token: &str,
) -> LauncherResult<MojangFullPlayerProfile> {
    let url = "https://api.minecraftservices.com/minecraft/profile"
        .parse()
        .map_err(LauncherError::UrlError)?;

    let headers = auth_headers(access_token);
    let header_refs: Vec<(&str, &str)> = headers
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();

    let mut profile = client
        .send_json::<MojangFullPlayerProfile>(Method::GET, url, None, &header_refs)
        .await?;

    for skin in &mut profile.skins {
        skin.url = normalize_texture_url(&skin.url);
    }
    for cape in &mut profile.capes {
        cape.url = normalize_texture_url(&cape.url);
    }

    Ok(profile)
}

#[tracing::instrument(level = "debug", skip(client, access_token))]
pub async fn fetch_player_profile_view(
    client: &RequestClient,
    uuid: &str,
    access_token: Option<&str>,
) -> LauncherResult<PlayerProfileView> {
    let fetched = async {
        if let Some(token) = access_token.filter(|t| !t.is_empty()) {
            let full = fetch_logged_in_profile(client, token).await?;
            return Ok(PlayerProfileView::from(full));
        }
        let summary = fetch_player_profile(client, uuid).await?;
        Ok(PlayerProfileView::from(summary))
    }
    .await;

    match fetched {
        Ok(view) => {
            write_cached_profile(uuid, &view).await;
            Ok(view)
        }
        Err(err) => match read_cached_profile(uuid).await {
            Some(cached) => {
                tracing::warn!("using cached player profile for {uuid}: {err}");
                Ok(cached)
            }
            None => Err(err),
        },
    }
}

fn profile_cache_path(uuid: &str) -> Option<std::path::PathBuf> {
    let key = crate::crypto::sha1_bytes(normalize_uuid(uuid).as_bytes());
    Some(crate::paths::profiles_cache_dir().ok()?.join(format!("{key}.json")))
}

async fn write_cached_profile(uuid: &str, view: &PlayerProfileView) {
    let Some(path) = profile_cache_path(uuid) else {
        return;
    };
    let Ok(bytes) = serde_json::to_vec(view) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = polyio::create_dir_all(parent).await;
    }
    if let Err(err) = polyio::write(&path, &bytes).await {
        tracing::warn!("failed to cache player profile for {uuid}: {err}");
    }
}

async fn read_cached_profile(uuid: &str) -> Option<PlayerProfileView> {
    let path = profile_cache_path(uuid)?;
    let bytes = polyio::read(&path).await.ok()?;
    serde_json::from_slice(&bytes).ok()
}
