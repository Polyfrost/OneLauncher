//! Local skins and capes for offline accounts.
//!
//! Vanilla only asks Mojang's session server for a skin when a profile has no
//! `textures` property. Passing one through `--userProperties` makes the client
//! load it even in offline mode. The files are served from a loopback HTTP
//! server for as long as the launcher runs, so no mod and no public upload is
//! needed.

use std::path::{Path, PathBuf};

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use oneclient_auth::MinecraftAccount;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::OnceCell;
use uuid::Uuid;

use oneclient_common::paths;

const SKIN_FILE: &str = "skin.png";
const CAPE_FILE: &str = "cape.png";
const META_FILE: &str = "skin.json";
const MAX_REQUEST_BYTES: usize = 4096;

static TEXTURE_SERVER: OnceCell<String> = OnceCell::const_new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SkinModel {
    #[default]
    Classic,
    Slim,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct OfflineSkin {
    #[serde(default)]
    pub model: SkinModel,
    #[serde(default)]
    pub has_cape: bool,
}

fn account_dir(account_id: Uuid) -> Result<PathBuf, String> {
    paths::skins_dir()
        .map(|dir| dir.join(account_id.simple().to_string()))
        .map_err(|err| err.to_string())
}

pub async fn load_offline_skin(account_id: Uuid) -> Option<OfflineSkin> {
    let dir = account_dir(account_id).ok()?;
    if !dir.join(SKIN_FILE).is_file() {
        return None;
    }
    let meta = tokio::fs::read(dir.join(META_FILE)).await.ok();
    let mut skin = meta
        .and_then(|bytes| serde_json::from_slice::<OfflineSkin>(&bytes).ok())
        .unwrap_or_default();
    skin.has_cape = dir.join(CAPE_FILE).is_file();
    Some(skin)
}

fn validate_png(path: &Path, allowed: &[(u32, u32)]) -> Result<(), String> {
    let (width, height) = image::image_dimensions(path).map_err(|err| err.to_string())?;
    if allowed.contains(&(width, height)) {
        Ok(())
    } else {
        Err(format!(
            "unsupported image size {width}x{height}; expected one of {}",
            allowed
                .iter()
                .map(|(w, h)| format!("{w}x{h}"))
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}

/// Copies the picked skin/cape into the account's folder. `None` keeps the
/// existing file, so the dialog can change only one of them at a time.
pub async fn save_offline_skin(
    account_id: Uuid,
    skin: Option<PathBuf>,
    cape: Option<PathBuf>,
    model: SkinModel,
) -> Result<OfflineSkin, String> {
    let dir = account_dir(account_id)?;
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|err| err.to_string())?;

    if let Some(source) = skin {
        validate_png(&source, &[(64, 64), (64, 32), (128, 64), (128, 128)])?;
        tokio::fs::copy(&source, dir.join(SKIN_FILE))
            .await
            .map_err(|err| err.to_string())?;
    }
    if !dir.join(SKIN_FILE).is_file() {
        return Err("pick a skin first".to_string());
    }

    if let Some(source) = cape {
        validate_png(&source, &[(64, 32), (22, 17), (128, 64)])?;
        tokio::fs::copy(&source, dir.join(CAPE_FILE))
            .await
            .map_err(|err| err.to_string())?;
    }

    let skin = OfflineSkin {
        model,
        has_cape: dir.join(CAPE_FILE).is_file(),
    };
    let bytes = serde_json::to_vec(&skin).map_err(|err| err.to_string())?;
    tokio::fs::write(dir.join(META_FILE), bytes)
        .await
        .map_err(|err| err.to_string())?;

    Ok(skin)
}

pub async fn remove_offline_cape(account_id: Uuid) -> Result<(), String> {
    let dir = account_dir(account_id)?;
    match tokio::fs::remove_file(dir.join(CAPE_FILE)).await {
        Ok(()) => {
            if let Some(mut skin) = load_offline_skin(account_id).await {
                skin.has_cape = false;
                let bytes = serde_json::to_vec(&skin).map_err(|err| err.to_string())?;
                let _ = tokio::fs::write(dir.join(META_FILE), bytes).await;
            }
            Ok(())
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.to_string()),
    }
}

fn texture_property(
    account: &MinecraftAccount,
    base_url: &str,
    has_cape: bool,
    model: SkinModel,
) -> String {
    let id = account.id.simple().to_string();
    let mut textures = serde_json::Map::new();
    textures.insert(
        "SKIN".to_string(),
        json!({
            "url": format!("{base_url}/skin/{id}.png"),
            "metadata": { "model": match model { SkinModel::Classic => "classic", SkinModel::Slim => "slim" } },
        }),
    );
    if has_cape {
        textures.insert(
            "CAPE".to_string(),
            json!({ "url": format!("{base_url}/cape/{id}.png") }),
        );
    }

    let payload = json!({
        "timestamp": 0,
        "profileId": id,
        "profileName": account.username,
        "signatureRequired": false,
        "textures": textures,
    });
    let encoded = BASE64_STANDARD.encode(payload.to_string());
    // `--userProperties` is parsed by authlib's `PropertyMap` deserializer, which
    // expects array entries to be property objects with a `name` and a `value`.
    // A bare base64 string is silently dropped there, so the skin never loads.
    json!({ "textures": [{ "name": "textures", "value": encoded }] }).to_string()
}

/// Where the launcher UI can preview an offline account's local skin and cape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflinePreview {
    pub skin_url: Option<String>,
    pub cape_url: Option<String>,
    pub is_slim: bool,
}

/// `None` when the account has no local skin or the texture server is down.
pub async fn local_skin_preview(account_id: Uuid) -> Option<OfflinePreview> {
    let skin = load_offline_skin(account_id).await?;
    let base = texture_server_url().await?;
    let id = account_id.simple().to_string();
    Some(OfflinePreview {
        skin_url: Some(format!("{base}/skin/{id}.png")),
        cape_url: skin.has_cape.then(|| format!("{base}/cape/{id}.png")),
        is_slim: skin.model == SkinModel::Slim,
    })
}

/// `--userProperties` value for an offline account with a local skin.
/// `None` means the caller should keep the default `{}`.
pub async fn user_properties(account: &MinecraftAccount) -> Option<String> {
    if !account.is_offline() {
        return None;
    }
    let skin = load_offline_skin(account.id).await?;
    let base = texture_server_url().await?;
    Some(texture_property(account, &base, skin.has_cape, skin.model))
}

async fn texture_server_url() -> Option<String> {
    if let Some(url) = TEXTURE_SERVER.get() {
        return Some(url.clone());
    }
    // Bind outside the cell so a transient failure is not cached forever: a
    // failed bind used to disable offline skins until the launcher restarted.
    let listener = TcpListener::bind("127.0.0.1:0").await.ok()?;
    let port = listener.local_addr().ok()?.port();
    tokio::spawn(serve(listener));
    tracing::info!(port, "offline skin texture server listening");
    let url = format!("http://127.0.0.1:{port}");
    Some(TEXTURE_SERVER.get_or_init(|| async { url }).await.clone())
}

async fn serve(listener: TcpListener) {
    loop {
        let Ok((mut stream, _)) = listener.accept().await else {
            continue;
        };
        tokio::spawn(async move {
            let mut buffer = vec![0u8; MAX_REQUEST_BYTES];
            let Ok(read) = stream.read(&mut buffer).await else {
                return;
            };
            let request = String::from_utf8_lossy(&buffer[..read]);
            let path = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or_default();

            let Some((kind, id)) = parse_texture_path(path) else {
                let _ = write_response(&mut stream, 404, &[]).await;
                return;
            };
            let file_name = match kind {
                "skin" => SKIN_FILE,
                "cape" => CAPE_FILE,
                _ => {
                    let _ = write_response(&mut stream, 404, &[]).await;
                    return;
                }
            };
            let Ok(dir) = paths::skins_dir() else {
                let _ = write_response(&mut stream, 500, &[]).await;
                return;
            };
            let data = tokio::fs::read(dir.join(id).join(file_name)).await;
            match data {
                Ok(bytes) => {
                    let _ = write_response(&mut stream, 200, &bytes).await;
                }
                Err(_) => {
                    let _ = write_response(&mut stream, 404, &[]).await;
                }
            }
        });
    }
}

fn parse_texture_path(path: &str) -> Option<(&str, String)> {
    let mut parts = path.trim_start_matches('/').split('/');
    let kind = parts.next()?;
    if !matches!(kind, "skin" | "cape") {
        return None;
    }
    let file = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let stem = file.strip_suffix(".png")?;
    let id = Uuid::parse_str(stem).ok()?;
    Some((kind, id.simple().to_string()))
}

async fn write_response(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    body: &[u8],
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        _ => "Internal Server Error",
    };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account() -> MinecraftAccount {
        oneclient_auth::offline_account("Steve".to_string())
    }

    #[test]
    fn the_texture_property_is_base64_json_with_the_local_urls() {
        let account = account();
        let id = account.id.simple().to_string();
        let outer = texture_property(&account, "http://127.0.0.1:1234", true, SkinModel::Slim);
        let parsed: serde_json::Value = serde_json::from_str(&outer).unwrap();
        assert_eq!(parsed["textures"][0]["name"], "textures");
        let encoded = parsed["textures"][0]["value"].as_str().unwrap();
        let decoded = BASE64_STANDARD.decode(encoded).unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&decoded).unwrap();

        assert_eq!(payload["profileId"], id);
        assert_eq!(payload["profileName"], "Steve");
        assert_eq!(
            payload["textures"]["SKIN"]["url"],
            format!("http://127.0.0.1:1234/skin/{id}.png")
        );
        assert_eq!(payload["textures"]["SKIN"]["metadata"]["model"], "slim");
        assert_eq!(
            payload["textures"]["CAPE"]["url"],
            format!("http://127.0.0.1:1234/cape/{id}.png")
        );
    }

    #[test]
    fn a_cape_is_optional() {
        let account = account();
        let outer = texture_property(&account, "http://127.0.0.1:1", false, SkinModel::Classic);
        let parsed: serde_json::Value = serde_json::from_str(&outer).unwrap();
        let decoded = BASE64_STANDARD
            .decode(parsed["textures"][0]["value"].as_str().unwrap())
            .unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&decoded).unwrap();
        assert!(payload["textures"].get("CAPE").is_none());
    }

    #[test]
    fn only_uuid_skin_and_cape_paths_are_served() {
        let id = Uuid::new_v4().simple().to_string();
        assert_eq!(
            parse_texture_path(&format!("/skin/{id}.png")),
            Some(("skin", id.clone()))
        );
        assert_eq!(
            parse_texture_path(&format!("/cape/{id}.png")),
            Some(("cape", id.clone()))
        );
        assert!(parse_texture_path("/skin/../secret.png").is_none());
        assert!(parse_texture_path("/other/skin.png").is_none());
        assert!(parse_texture_path("/skin/not-a-uuid.png").is_none());
    }
}
