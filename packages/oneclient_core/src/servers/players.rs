use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::LauncherResult;

const JSON_LISTS: [&str; 4] = [
    "whitelist.json",
    "ops.json",
    "banned-players.json",
    "banned-ips.json",
];
const TXT_LISTS: [&str; 4] = [
    "white-list.txt",
    "ops.txt",
    "banned-players.txt",
    "banned-ips.txt",
];

#[must_use]
pub fn list_file(mc_version: &str, op: bool) -> &'static str {
    let lists = if super::json_lists(mc_version) {
        JSON_LISTS
    } else {
        TXT_LISTS
    };
    lists[usize::from(op)]
}

pub async fn ensure_lists(dir: &Path, mc_version: &str) -> LauncherResult<()> {
    let (lists, empty) = if super::json_lists(mc_version) {
        (JSON_LISTS, "[]")
    } else {
        (TXT_LISTS, "")
    };
    for name in lists {
        let path = dir.join(name);
        if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
            tokio::fs::write(path, empty).await?;
        }
    }
    Ok(())
}

fn is_txt(file: &str) -> bool {
    file.ends_with(".txt")
}

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
        Ok(bytes) if is_txt(file) => Ok(String::from_utf8_lossy(&bytes)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|name| ListedPlayer {
                uuid: String::new(),
                name: name.to_string(),
                level: None,
                bypasses_player_limit: None,
            })
            .collect()),
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(err.into()),
    }
}

pub async fn write_list(dir: &Path, file: &str, players: &[ListedPlayer]) -> LauncherResult<()> {
    let bytes = if is_txt(file) {
        players
            .iter()
            .map(|p| {
                format!(
                    "{}
",
                    p.name
                )
            })
            .collect::<String>()
            .into_bytes()
    } else {
        serde_json::to_vec_pretty(players)?
    };
    polyio::write_atomic(dir.join(file), bytes).await?;
    Ok(())
}

pub async fn add_to_list(dir: &Path, file: &str, player: ListedPlayer) -> LauncherResult<()> {
    let mut players = load_list(dir, file).await?;
    if !players.iter().any(|p| {
        (!p.uuid.is_empty() && p.uuid == player.uuid) || p.name.eq_ignore_ascii_case(&player.name)
    }) {
        players.push(player);
    }
    write_list(dir, file, &players).await
}

pub async fn remove_from_list(dir: &Path, file: &str, name: &str) -> LauncherResult<()> {
    let mut players = load_list(dir, file).await?;
    players.retain(|p| !p.name.eq_ignore_ascii_case(name));
    write_list(dir, file, &players).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn old_servers_get_name_per_line_lists() {
        let tmp = polyio::tempdir().await.expect("dir");
        let dir = tmp.dir_path();
        ensure_lists(dir, "1.7.2").await.unwrap();
        let file = list_file("1.7.2", true);
        assert_eq!(file, "ops.txt");
        add_to_list(
            dir,
            file,
            ListedPlayer::new("069a79f444e94726a5befca90e38aaf5", "Notch".into(), true),
        )
        .await
        .unwrap();
        add_to_list(dir, file, ListedPlayer::new("x", "notch".into(), true))
            .await
            .unwrap();
        assert_eq!(
            tokio::fs::read_to_string(dir.join(file)).await.unwrap(),
            "Notch
"
        );
        assert_eq!(read_list(dir, file).await[0].name, "Notch");
    }

    #[tokio::test]
    async fn missing_json_lists_are_created_empty() {
        let tmp = polyio::tempdir().await.expect("dir");
        let dir = tmp.dir_path();
        ensure_lists(dir, "1.8.9").await.unwrap();
        for name in JSON_LISTS {
            assert_eq!(
                tokio::fs::read_to_string(dir.join(name)).await.unwrap(),
                "[]"
            );
        }
        assert!(read_list(dir, list_file("1.8.9", false)).await.is_empty());
    }
}
