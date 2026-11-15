use std::io::{Error, ErrorKind, Result};
use std::path::{Component, Path, PathBuf};

use oneclient_common::domain::ContentType;

pub(super) const STASH_DIR: &str = "resourcepacks-disabled";
const PACK_META: &str = "pack.mcmeta";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderPack {
    pub name: String,
    pub enabled: bool,
    pub description: Option<String>,
}

fn packs_dir(game_dir: &Path) -> PathBuf {
    game_dir.join(ContentType::ResourcePack.folder_name())
}

fn stash_dir(game_dir: &Path) -> PathBuf {
    let packs = packs_dir(game_dir);
    polyio::canonicalize(&packs)
        .unwrap_or(packs)
        .with_file_name(STASH_DIR)
}

pub async fn list_folder_packs(game_dir: &Path) -> Vec<FolderPack> {
    let mut packs = scan(&packs_dir(game_dir), true).await;
    packs.extend(scan(&stash_dir(game_dir), false).await);
    packs.sort_by_key(|pack| pack.name.to_lowercase());
    packs
}

async fn scan(dir: &Path, enabled: bool) -> Vec<FolderPack> {
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return Vec::new();
    };
    let mut packs = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if !is_folder_pack(&path).await {
            continue;
        }
        packs.push(FolderPack {
            name: entry.file_name().to_string_lossy().into_owned(),
            enabled,
            description: description(&path).await,
        });
    }
    packs
}

async fn is_folder_pack(path: &Path) -> bool {
    tokio::fs::metadata(path.join(PACK_META))
        .await
        .is_ok_and(|meta| meta.is_file())
}

async fn description(dir: &Path) -> Option<String> {
    let raw = tokio::fs::read(dir.join(PACK_META)).await.ok()?;
    let meta: serde_json::Value = serde_json::from_slice(&raw).ok()?;
    let text = meta.get("pack")?.get("description")?.as_str()?;
    Some(strip_formatting(text)).filter(|text| !text.is_empty())
}

fn strip_formatting(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out.trim().to_string()
}

fn checked_name(name: &str) -> Result<&str> {
    let mut parts = Path::new(name).components();
    match (parts.next(), parts.next()) {
        (Some(Component::Normal(_)), None) => Ok(name),
        _ => Err(Error::new(
            ErrorKind::InvalidInput,
            format!("not a pack folder name: {name}"),
        )),
    }
}

pub async fn set_folder_pack_enabled(game_dir: &Path, name: &str, enabled: bool) -> Result<()> {
    let name = checked_name(name)?;
    let (from, to) = if enabled {
        (stash_dir(game_dir), packs_dir(game_dir))
    } else {
        (packs_dir(game_dir), stash_dir(game_dir))
    };
    let source = from.join(name);
    if !is_folder_pack(&source).await {
        return Err(Error::new(
            ErrorKind::NotFound,
            format!("{name} is no longer there"),
        ));
    }
    let target = to.join(name);
    if tokio::fs::symlink_metadata(&target).await.is_ok() {
        return Err(Error::new(
            ErrorKind::AlreadyExists,
            format!("{} already has a {name}", to.display()),
        ));
    }
    tokio::fs::create_dir_all(&to).await?;
    tokio::fs::rename(&source, &target).await
}

pub async fn remove_folder_pack(game_dir: &Path, name: &str, enabled: bool) -> Result<()> {
    let name = checked_name(name)?;
    let dir = if enabled {
        packs_dir(game_dir)
    } else {
        stash_dir(game_dir)
    };
    let path = dir.join(name);
    if !is_folder_pack(&path).await {
        return Err(Error::new(
            ErrorKind::NotFound,
            format!("{name} is no longer there"),
        ));
    }
    tokio::task::spawn_blocking(move || trash::delete(&path))
        .await
        .map_err(Error::other)?
        .map_err(Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack(dir: &Path, name: &str, meta: &str) {
        let path = dir.join(name);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join(PACK_META), meta).unwrap();
    }

    #[tokio::test]
    async fn folder_packs_toggle_between_the_packs_folder_and_the_stash() {
        let game = polyio::tempdir().await.unwrap();
        let game = game.dir_path();
        let packs = packs_dir(game);
        pack(
            &packs,
            "Faithful",
            r#"{"pack":{"pack_format":34,"description":"§aSmooth §rtextures"}}"#,
        );
        pack(
            &packs,
            "Raw",
            r#"{"pack":{"description":{"text":"component"}}}"#,
        );
        std::fs::create_dir_all(packs.join("NotAPack")).unwrap();
        std::fs::write(packs.join("Zipped.zip"), b"zip").unwrap();

        let listed = list_folder_packs(game).await;
        assert_eq!(
            listed,
            vec![
                FolderPack {
                    name: "Faithful".into(),
                    enabled: true,
                    description: Some("Smooth textures".into())
                },
                FolderPack {
                    name: "Raw".into(),
                    enabled: true,
                    description: None
                },
            ]
        );

        set_folder_pack_enabled(game, "Faithful", false)
            .await
            .unwrap();
        assert!(stash_dir(game).join("Faithful").join(PACK_META).is_file());
        assert!(!packs.join("Faithful").exists());
        assert!(!list_folder_packs(game).await[0].enabled);

        assert!(
            set_folder_pack_enabled(game, "Faithful", false)
                .await
                .is_err()
        );
        set_folder_pack_enabled(game, "Faithful", true)
            .await
            .unwrap();
        assert!(packs.join("Faithful").join(PACK_META).is_file());

        assert!(
            set_folder_pack_enabled(game, "../Faithful", false)
                .await
                .is_err()
        );
        assert!(remove_folder_pack(game, "NotAPack", true).await.is_err());

        set_folder_pack_enabled(game, "Raw", false).await.unwrap();
        pack(&packs, "Raw", r#"{"pack":{"description":"new"}}"#);
        let raw: Vec<bool> = list_folder_packs(game)
            .await
            .into_iter()
            .filter(|pack| pack.name == "Raw")
            .map(|pack| pack.enabled)
            .collect();
        assert_eq!(raw.len(), 2);
        assert!(set_folder_pack_enabled(game, "Raw", true).await.is_err());
        std::fs::remove_dir_all(packs.join("Raw")).unwrap();
        assert!(remove_folder_pack(game, "Raw", true).await.is_err());
        assert!(stash_dir(game).join("Raw").join(PACK_META).is_file());
    }

    #[tokio::test]
    async fn instances_sharing_a_pack_folder_share_the_off_state() {
        let root = polyio::tempdir().await.unwrap();
        let root = root.dir_path();
        let (first, second, shared) = (root.join("a"), root.join("b"), root.join("shared"));
        pack(&packs_dir(&shared), "Faithful", r#"{"pack":{}}"#);
        for game in [&first, &second] {
            std::fs::create_dir_all(game).unwrap();
            polyio::symlink_dir(packs_dir(&shared), packs_dir(game))
                .await
                .unwrap();
        }

        set_folder_pack_enabled(&first, "Faithful", false)
            .await
            .unwrap();
        assert!(shared.join(STASH_DIR).join("Faithful").is_dir());
        assert!(!list_folder_packs(&second).await[0].enabled);

        set_folder_pack_enabled(&second, "Faithful", true)
            .await
            .unwrap();
        assert!(list_folder_packs(&first).await[0].enabled);
    }
}
