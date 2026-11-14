use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::LauncherResult;

pub const WHITELIST: &str = "whitelist.json";
pub const OPS: &str = "ops.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListedPlayer {
    pub uuid: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
    #[serde(
        default,
        rename = "bypassesPlayerLimit",
        skip_serializing_if = "Option::is_none"
    )]
    pub bypasses_player_limit: Option<bool>,
}

impl ListedPlayer {
    /// Mojang hands back the uuid without dashes, the server files want them
    #[must_use]
    pub fn new(uuid: &str, name: String, op: bool) -> Self {
        let uuid = uuid::Uuid::parse_str(uuid)
            .map_or_else(|_| uuid.to_string(), |u| u.hyphenated().to_string());
        Self {
            uuid,
            name,
            level: op.then_some(4),
            bypasses_player_limit: op.then_some(false),
        }
    }
}

pub async fn read_list(dir: &Path, file: &str) -> Vec<ListedPlayer> {
    load_list(dir, file).await.unwrap_or_default()
}

async fn load_list(dir: &Path, file: &str) -> LauncherResult<Vec<ListedPlayer>> {
    match tokio::fs::read(dir.join(file)).await {
        Ok(bytes) if bytes.iter().all(u8::is_ascii_whitespace) => Ok(Vec::new()),
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(err.into()),
    }
}

pub async fn write_list(dir: &Path, file: &str, players: &[ListedPlayer]) -> LauncherResult<()> {
    let tmp = dir.join(format!("{file}.tmp"));
    tokio::fs::write(&tmp, serde_json::to_vec_pretty(players)?).await?;
    tokio::fs::rename(&tmp, dir.join(file)).await?;
    Ok(())
}

pub async fn add_to_list(dir: &Path, file: &str, player: ListedPlayer) -> LauncherResult<()> {
    let mut players = load_list(dir, file).await?;
    if !players.iter().any(|p| p.uuid == player.uuid) {
        players.push(player);
    }
    write_list(dir, file, &players).await
}

pub async fn remove_from_list(dir: &Path, file: &str, name: &str) -> LauncherResult<()> {
    let mut players = load_list(dir, file).await?;
    players.retain(|p| !p.name.eq_ignore_ascii_case(name));
    write_list(dir, file, &players).await
}
