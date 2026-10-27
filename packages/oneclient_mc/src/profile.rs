use base64::Engine;
use chrono::{DateTime, TimeDelta, Utc};
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::error::{McError, McResult};
use oneclient_net::RequestClient;

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
            is_slim: active_skin.is_some_and(|skin| skin.variant == SkinVariant::Slim),
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

const CAPE_ACTIVE_URL: &str = "https://api.minecraftservices.com/minecraft/profile/capes/active";
const SKINS_URL: &str = "https://api.minecraftservices.com/minecraft/profile/skins";

fn normalize_texture_url(url: &str) -> String {
    url.replace(
        "http://textures.minecraft.net",
        "https://textures.minecraft.net",
    )
}

fn normalize_uuid(uuid: &str) -> &str {
    uuid.trim()
        .trim_matches(|c: char| c == '-' || c.is_whitespace())
}

fn auth_headers(access_token: &str) -> [(&'static str, String); 1] {
    [("Authorization", format!("Bearer {access_token}"))]
}

#[tracing::instrument(level = "debug", skip(client))]
pub async fn fetch_player_profile(
    client: &RequestClient,
    uuid: &str,
) -> McResult<MojangPlayerProfile> {
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
        .map_err(McError::Url)?;

    let response = client
        .send_as::<Response>(reqwest::Request::new(Method::GET, url))
        .await?;

    let texture_property = response
        .properties
        .iter()
        .find(|p| p.name == "textures")
        .ok_or_else(|| McError::Minecraft("no textures in player profile".into()))?;

    let decoded = base64::prelude::BASE64_STANDARD
        .decode(texture_property.value.as_bytes())
        .map_err(|_| McError::Minecraft("failed to decode profile texture property".into()))?;

    let decoded = serde_json::from_slice::<DecodedProperties>(&decoded)
        .map_err(McError::ParseError)?
        .textures;

    let is_slim = decoded
        .skin
        .as_ref()
        .is_some_and(|s| s.metadata.model.as_deref() == Some("slim"));

    Ok(MojangPlayerProfile {
        uuid: response.id,
        username: response.name,
        is_slim,
        skin_url: decoded.skin.as_ref().map(|s| normalize_texture_url(&s.url)),
        cape_url: decoded.cape.as_ref().map(|c| normalize_texture_url(&c.url)),
    })
}

#[tracing::instrument(level = "debug", skip(client, access_token))]
pub async fn fetch_logged_in_profile(
    client: &RequestClient,
    access_token: &str,
) -> McResult<MojangFullPlayerProfile> {
    let url = "https://api.minecraftservices.com/minecraft/profile"
        .parse()
        .map_err(McError::Url)?;

    let headers = auth_headers(access_token);
    let header_refs: Vec<(&str, &str)> = headers
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();

    let profile = client
        .send_json::<MojangFullPlayerProfile>(Method::GET, url, None, &header_refs)
        .await?;

    Ok(normalize_profile(profile))
}

fn normalize_profile(mut profile: MojangFullPlayerProfile) -> MojangFullPlayerProfile {
    for skin in &mut profile.skins {
        skin.url = normalize_texture_url(&skin.url);
    }
    for cape in &mut profile.capes {
        cape.url = normalize_texture_url(&cape.url);
    }
    profile
}

async fn send_profile_request(
    client: &RequestClient,
    access_token: &str,
    method: Method,
    url: &str,
    body: Option<serde_json::Value>,
) -> McResult<MojangFullPlayerProfile> {
    let url: reqwest::Url = url.parse().map_err(McError::Url)?;
    let mut request = client
        .http()
        .request(method, url)
        .bearer_auth(access_token)
        .header("Accept", "application/json");
    if let Some(body) = body {
        request = request.json(&body);
    }

    let response = request
        .send()
        .await
        .map_err(|err| McError::Minecraft(err.to_string()))?;
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|err| McError::Minecraft(err.to_string()))?;
    if !status.is_success() {
        return Err(McError::MinecraftStatus {
            status: status.as_u16(),
            body: String::from_utf8_lossy(&bytes).into_owned(),
        });
    }

    let profile = serde_json::from_slice::<MojangFullPlayerProfile>(&bytes)?;
    Ok(normalize_profile(profile))
}

#[tracing::instrument(level = "debug", skip(client, access_token))]
pub async fn fetch_logged_in_profile_once(
    client: &RequestClient,
    access_token: &str,
) -> McResult<MojangFullPlayerProfile> {
    send_profile_request(
        client,
        access_token,
        Method::GET,
        "https://api.minecraftservices.com/minecraft/profile",
        None,
    )
    .await
}

#[tracing::instrument(level = "debug", skip(client))]
pub async fn resolve_username(
    client: &RequestClient,
    username: &str,
) -> McResult<(String, String)> {
    #[derive(Deserialize)]
    struct Response {
        id: String,
        name: String,
    }

    let name = username.trim();
    if name.is_empty()
        || name.len() > 16
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(McError::Minecraft(format!("invalid username {name}")));
    }

    let url = format!("https://api.mojang.com/users/profiles/minecraft/{name}")
        .parse()
        .map_err(McError::Url)?;
    let response = client
        .send_as::<Response>(reqwest::Request::new(Method::GET, url))
        .await?;
    Ok((response.id, response.name))
}

#[tracing::instrument(level = "debug", skip(client, access_token))]
pub async fn select_cape(
    client: &RequestClient,
    access_token: &str,
    cape_id: &str,
) -> McResult<MojangFullPlayerProfile> {
    send_profile_request(
        client,
        access_token,
        Method::PUT,
        CAPE_ACTIVE_URL,
        Some(serde_json::json!({ "capeId": cape_id })),
    )
    .await
}

#[tracing::instrument(level = "debug", skip(client, access_token))]
pub async fn hide_cape(
    client: &RequestClient,
    access_token: &str,
) -> McResult<MojangFullPlayerProfile> {
    send_profile_request(client, access_token, Method::DELETE, CAPE_ACTIVE_URL, None).await
}

#[tracing::instrument(level = "debug", skip(client, access_token))]
pub async fn set_skin_from_url(
    client: &RequestClient,
    access_token: &str,
    skin_url: &str,
    variant: SkinVariant,
) -> McResult<MojangFullPlayerProfile> {
    let response = client
        .http()
        .get(skin_url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|err| McError::Minecraft(err.to_string()))?;
    let png = response
        .bytes()
        .await
        .map_err(|err| McError::Minecraft(err.to_string()))?;
    upload_skin(client, access_token, png.to_vec(), variant).await
}

#[tracing::instrument(level = "debug", skip(client, access_token, png))]
pub async fn upload_skin(
    client: &RequestClient,
    access_token: &str,
    png: Vec<u8>,
    variant: SkinVariant,
) -> McResult<MojangFullPlayerProfile> {
    let variant = match variant {
        SkinVariant::Classic => "classic",
        SkinVariant::Slim => "slim",
    };
    let part = reqwest::multipart::Part::bytes(png)
        .file_name("skin.png")
        .mime_str("image/png")
        .map_err(|err| McError::Minecraft(err.to_string()))?;
    let form = reqwest::multipart::Form::new()
        .text("variant", variant)
        .part("file", part);

    let response = client
        .http()
        .post(SKINS_URL)
        .bearer_auth(access_token)
        .multipart(form)
        .send()
        .await
        .map_err(|err| McError::Minecraft(err.to_string()))?;

    let status = response.status();
    let body = response
        .bytes()
        .await
        .map_err(|err| McError::Minecraft(err.to_string()))?;
    if !status.is_success() {
        return Err(McError::MinecraftStatus {
            status: status.as_u16(),
            body: String::from_utf8_lossy(&body).into_owned(),
        });
    }

    let profile = serde_json::from_slice::<MojangFullPlayerProfile>(&body)?;
    Ok(normalize_profile(profile))
}

/// Served without revalidating so cold start doesn't wait on a Mojang round
/// trip before the first avatar paints
const PROFILE_CACHE_TTL_HOURS: i64 = 6;

#[derive(Debug, Serialize, Deserialize)]
struct CachedProfile {
    fetched_at: DateTime<Utc>,
    view: PlayerProfileView,
}

impl CachedProfile {
    fn is_fresh(&self) -> bool {
        Utc::now() - self.fetched_at < TimeDelta::hours(PROFILE_CACHE_TTL_HOURS)
    }
}

async fn fetch_profile_view(
    client: &RequestClient,
    uuid: &str,
    access_token: Option<&str>,
) -> McResult<PlayerProfileView> {
    if let Some(token) = access_token.filter(|t| !t.is_empty()) {
        let full = fetch_logged_in_profile(client, token).await?;
        return Ok(PlayerProfileView::from(full));
    }
    let summary = fetch_player_profile(client, uuid).await?;
    Ok(PlayerProfileView::from(summary))
}

#[tracing::instrument(level = "debug", skip(client, access_token))]
pub async fn fetch_player_profile_view(
    client: &RequestClient,
    uuid: &str,
    access_token: Option<&str>,
) -> McResult<PlayerProfileView> {
    let authenticated = access_token.is_some_and(|t| !t.is_empty());

    if !authenticated && let Some(cached) = read_cached_profile(uuid).await {
        if cached.is_fresh() {
            return Ok(cached.view);
        }

        return match fetch_profile_view(client, uuid, access_token).await {
            Ok(view) => {
                write_cached_profile(uuid, &view).await;
                Ok(view)
            }
            Err(err) => {
                tracing::warn!("serving stale cached profile for {uuid}: {err}");
                Ok(cached.view)
            }
        };
    }

    match fetch_profile_view(client, uuid, access_token).await {
        Ok(view) => {
            if !authenticated {
                write_cached_profile(uuid, &view).await;
            }
            Ok(view)
        }
        Err(err) => match read_cached_profile(uuid).await {
            Some(cached) => {
                tracing::warn!("using cached player profile for {uuid}: {err}");
                Ok(cached.view)
            }
            None => Err(err),
        },
    }
}

pub async fn store_profile_view(uuid: &str, profile: MojangFullPlayerProfile) -> PlayerProfileView {
    let view = PlayerProfileView::from(profile);
    write_cached_profile(uuid, &view).await;
    view
}

fn profile_cache_path(uuid: &str) -> Option<std::path::PathBuf> {
    let key = polyio::sha1_bytes(normalize_uuid(uuid).as_bytes());
    Some(
        oneclient_common::paths::profiles_cache_dir()
            .ok()?
            .join(format!("{key}.json")),
    )
}

async fn write_cached_profile(uuid: &str, view: &PlayerProfileView) {
    let Some(path) = profile_cache_path(uuid) else {
        return;
    };
    let entry = CachedProfile {
        fetched_at: Utc::now(),
        view: view.clone(),
    };
    let Ok(bytes) = serde_json::to_vec(&entry) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = polyio::create_dir_all(parent).await;
    }
    if let Err(err) = polyio::write(&path, &bytes).await {
        tracing::warn!("failed to cache player profile for {uuid}: {err}");
    }
}

async fn read_cached_profile(uuid: &str) -> Option<CachedProfile> {
    let path = profile_cache_path(uuid)?;
    let bytes = polyio::read(&path).await.ok()?;

    if let Ok(entry) = serde_json::from_slice::<CachedProfile>(&bytes) {
        return Some(entry);
    }

    let view = serde_json::from_slice::<PlayerProfileView>(&bytes).ok()?;
    Some(CachedProfile {
        fetched_at: DateTime::UNIX_EPOCH,
        view,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_launcher_dir() {
        let dir =
            std::env::temp_dir().join(format!("oneclient-profile-test-{}", uuid::Uuid::new_v4()));
        oneclient_common::paths::set_launcher_dir(dir);
    }

    fn sample_view(username: &str) -> PlayerProfileView {
        PlayerProfileView {
            uuid: "069a79f4-44e9-4726-a5be-fca90e38aaf5".into(),
            username: username.into(),
            is_slim: true,
            skin_url: Some("https://textures.minecraft.net/texture/abc".into()),
            cape_url: None,
            skins: Vec::new(),
            capes: Vec::new(),
        }
    }

    #[tokio::test]
    async fn fresh_cache_entry_is_served_without_network() {
        temp_launcher_dir();
        let uuid = uuid::Uuid::new_v4().to_string();
        let view = sample_view("Cached");
        write_cached_profile(&uuid, &view).await;

        let client = RequestClient::new(oneclient_net::NetConfig::default()).unwrap();
        let resolved = fetch_player_profile_view(&client, &uuid, None)
            .await
            .expect("fresh cache entry should resolve offline");

        assert_eq!(resolved, view);
    }

    #[tokio::test]
    async fn entry_past_ttl_is_not_fresh() {
        let stale = CachedProfile {
            fetched_at: Utc::now() - TimeDelta::hours(PROFILE_CACHE_TTL_HOURS + 1),
            view: sample_view("Stale"),
        };
        assert!(!stale.is_fresh());

        let recent = CachedProfile {
            fetched_at: Utc::now() - TimeDelta::hours(PROFILE_CACHE_TTL_HOURS - 1),
            view: sample_view("Recent"),
        };
        assert!(recent.is_fresh());
    }

    #[tokio::test]
    async fn legacy_cache_entry_loads_and_revalidates() {
        temp_launcher_dir();
        let uuid = uuid::Uuid::new_v4().to_string();
        let view = sample_view("Legacy");

        let path = profile_cache_path(&uuid).unwrap();
        polyio::create_dir_all(path.parent().unwrap())
            .await
            .unwrap();
        polyio::write(&path, &serde_json::to_vec(&view).unwrap())
            .await
            .unwrap();

        let cached = read_cached_profile(&uuid)
            .await
            .expect("legacy entry should still parse");

        assert_eq!(cached.view, view);
        assert!(
            !cached.is_fresh(),
            "legacy entry should be due for revalidation"
        );
    }
}
