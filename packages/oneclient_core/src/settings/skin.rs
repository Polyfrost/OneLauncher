use oneclient_common::paths::skins_file;
use oneclient_net::{HttpRequest, RequestClient};
use polyio::{read_json, try_exists, write_json_atomic};
use reqwest::{Method, Request, Response};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{LauncherError, LauncherResult};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct MinecraftSkin {
    pub name: String,
    pub src: String,
    pub belongs_to: String,
    pub active: bool,
}

async fn load_skins() -> LauncherResult<Vec<MinecraftSkin>> {
    let path = skins_file()?;
    if !try_exists(&path).await? {
        return Ok(Vec::new());
    }
    Ok(read_json(&path).await?)
}

async fn save_skins(skins: &[MinecraftSkin]) -> LauncherResult<()> {
    write_json_atomic(skins_file()?, skins).await?;
    Ok(())
}

/// Create the skins file if it doesn't exist already
pub async fn initialize_storage_file() -> LauncherResult<()> {
    let path = skins_file()?;
    if !try_exists(&path).await? {
        save_skins(&[]).await?;
    }
    Ok(())
}
pub async fn get_all_skins() -> LauncherResult<Vec<MinecraftSkin>> {
    load_skins().await
}

/// Runs only when the skin storage file is created
/// Adds all current skins being used by all saved accounts into the storage
async fn load_profile_skins() {
    // fetch all accounts, get their skins via get_skin_from_username, and save it
}

/// Applies the skin by sending it to Mojang api
async fn apply_skin() {}

pub async fn add_skin(skin: MinecraftSkin) -> LauncherResult<()> {
    let mut skins = load_skins().await?;
    skins.retain(|s| s.name != skin.name); // dedupe by name
    skins.push(skin);
    save_skins(&skins).await
}
pub async fn get_skin(name: &str) -> LauncherResult<Option<MinecraftSkin>> {
    Ok(load_skins().await?.into_iter().find(|s| s.name == name))
}
pub async fn remove_skin(name: &str) -> LauncherResult<bool> {
    let mut skins = load_skins().await?;
    let before = skins.len();
    skins.retain(|s| s.name != name);
    let removed = skins.len() != before;
    if removed {
        save_skins(&skins).await?;
    }
    Ok(removed)
}

/// Marks skin active and every other skin inactive.
pub async fn set_active_skin(name: &str) -> LauncherResult<()> {
    let mut skins = load_skins().await?;
    for s in &mut skins {
        if s.name == name {
            s.active = true;
        } else {
            s.active = false;
        }
    }
    save_skins(&skins).await
}
pub async fn get_active_skin() -> LauncherResult<Option<MinecraftSkin>> {
    Ok(load_skins().await?.into_iter().find(|s| s.active))
}

/// Fetch skin from Minotaur API
pub async fn get_skin_via_uuid(net: &RequestClient, uuid: &str) -> LauncherResult<MinecraftSkin> {
    let resp = net
        .send(Request::new(
            Method::GET,
            Url::parse(&format!("https://minotar.net/skin/{uuid}"))?,
        ))
        .await?;gg
    let bytes = resp.bytes().await?;
    let hash = polyio::sha1_bytes(&bytes);
    Ok(MinecraftSkin {
        name: uuid.into(),
        src: hash,
        belongs_to: uuid.into(),
        active: false,
    })
}
