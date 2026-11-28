use std::collections::HashSet;
use std::io::{Error, ErrorKind, Result};
use std::path::{Component, Path, PathBuf};

use oneclient_common::domain::ContentType;

use crate::clusters::Cluster;
use crate::state::LauncherServices;

pub(super) const STASH_DIR: &str = "resourcepacks-disabled";
const SHADER_STASH_DIR: &str = "shaderpacks-disabled";
pub(super) const STASH_DIRS: [&str; 2] = [STASH_DIR, SHADER_STASH_DIR];
const PACK_META: &str = "pack.mcmeta";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderPack {
    pub name: String,
    pub enabled: bool,
    pub description: Option<String>,
}

fn packs_dir(root: &Path, content_type: ContentType) -> PathBuf {
    root.join(content_type.folder_name())
}

fn stash_dir(root: &Path, content_type: ContentType) -> PathBuf {
    let packs = packs_dir(root, content_type);
    let stash = match content_type {
        ContentType::Shader => SHADER_STASH_DIR,
        _ => STASH_DIR,
    };
    polyio::canonicalize(&packs)
        .unwrap_or(packs)
        .with_file_name(stash)
}

fn pack_roots(cluster: &Cluster, content_type: ContentType) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = cluster.game_dir().ok().into_iter().collect();
    if cluster.shares_content(content_type)
        && let Ok(own) = cluster.dir()
    {
        roots.push(own);
    }
    roots
}

pub async fn list_folder_packs(
    services: &LauncherServices,
    cluster: &Cluster,
    content_type: ContentType,
) -> Vec<FolderPack> {
    let managed = super::shared_dir::managed_pack_names(services, cluster, content_type).await;
    list_in(
        &pack_roots(cluster, content_type),
        content_type,
        managed.as_ref(),
    )
    .await
}

async fn list_in(
    roots: &[PathBuf],
    content_type: ContentType,
    managed: Option<&HashSet<String>>,
) -> Vec<FolderPack> {
    let mut seen = HashSet::new();
    let mut packs = Vec::new();
    let none_managed = HashSet::new();
    for (i, root) in roots.iter().enumerate() {
        let dir = packs_dir(root, content_type);
        if !seen.insert(polyio::canonicalize(&dir).unwrap_or(dir.clone())) {
            continue;
        }
        let managed = managed_in(managed, i, &none_managed);
        packs.extend(scan(&dir, content_type, true, managed).await);
        packs.extend(scan(&stash_dir(root, content_type), content_type, false, None).await);
    }
    let mut names = HashSet::new();
    packs.retain(|pack| names.insert((pack.name.clone(), pack.enabled)));
    packs.sort_by_key(|pack| pack.name.to_lowercase());
    packs
}

async fn scan(
    dir: &Path,
    content_type: ContentType,
    enabled: bool,
    managed: Option<&HashSet<String>>,
) -> Vec<FolderPack> {
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return Vec::new();
    };
    let mut packs = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if !is_listed(&path, &name, content_type, enabled, managed).await {
            continue;
        }
        packs.push(FolderPack {
            name,
            enabled,
            description: description(&path).await,
        });
    }
    packs
}

fn managed_in<'a>(
    managed: Option<&'a HashSet<String>>,
    root: usize,
    none_managed: &'a HashSet<String>,
) -> Option<&'a HashSet<String>> {
    managed.map(|names| if root == 0 { names } else { none_managed })
}

async fn is_listed(
    path: &Path,
    name: &str,
    content_type: ContentType,
    enabled: bool,
    managed: Option<&HashSet<String>>,
) -> bool {
    if name.starts_with('.') {
        return false;
    }
    if is_zip_pack(path).await {
        !enabled || managed.is_some_and(|managed| !managed.contains(name))
    } else {
        is_folder_pack(path, content_type).await
    }
}

async fn is_zip_pack(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.to_ascii_lowercase().ends_with(".zip"))
        && tokio::fs::metadata(path)
            .await
            .is_ok_and(|meta| meta.is_file())
}

async fn is_folder_pack(path: &Path, content_type: ContentType) -> bool {
    match content_type {
        ContentType::ResourcePack => tokio::fs::metadata(path.join(PACK_META))
            .await
            .is_ok_and(|meta| meta.is_file()),
        ContentType::Shader => tokio::fs::metadata(path.join("shaders"))
            .await
            .is_ok_and(|meta| meta.is_dir()),
        _ => false,
    }
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
            format!("not a pack name: {name}"),
        )),
    }
}

async fn find(
    roots: &[PathBuf],
    content_type: ContentType,
    name: &str,
    enabled: bool,
    managed: Option<&HashSet<String>>,
) -> Result<(PathBuf, PathBuf)> {
    let name = checked_name(name)?;
    let none_managed = HashSet::new();
    for (i, root) in roots.iter().enumerate() {
        let dir = if enabled {
            packs_dir(root, content_type)
        } else {
            stash_dir(root, content_type)
        };
        let path = dir.join(name);
        let managed = managed_in(managed, i, &none_managed);
        if is_listed(&path, name, content_type, enabled, managed).await {
            return Ok((root.clone(), path));
        }
    }
    Err(Error::new(
        ErrorKind::NotFound,
        format!("{name} is no longer there"),
    ))
}

pub async fn set_folder_pack_enabled(
    services: &LauncherServices,
    cluster: &Cluster,
    content_type: ContentType,
    name: &str,
    enabled: bool,
) -> Result<()> {
    let managed = super::shared_dir::managed_pack_names(services, cluster, content_type).await;
    set_enabled_in(
        &pack_roots(cluster, content_type),
        content_type,
        name,
        enabled,
        managed.as_ref(),
    )
    .await
}

async fn set_enabled_in(
    roots: &[PathBuf],
    content_type: ContentType,
    name: &str,
    enabled: bool,
    managed: Option<&HashSet<String>>,
) -> Result<()> {
    let (root, source) = find(roots, content_type, name, !enabled, managed).await?;
    let to = if enabled {
        packs_dir(&root, content_type)
    } else {
        stash_dir(&root, content_type)
    };
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

pub async fn remove_folder_pack(
    services: &LauncherServices,
    cluster: &Cluster,
    content_type: ContentType,
    name: &str,
    enabled: bool,
) -> Result<()> {
    let managed = super::shared_dir::managed_pack_names(services, cluster, content_type).await;
    remove_in(
        &pack_roots(cluster, content_type),
        content_type,
        name,
        enabled,
        managed.as_ref(),
    )
    .await
}

async fn remove_in(
    roots: &[PathBuf],
    content_type: ContentType,
    name: &str,
    enabled: bool,
    managed: Option<&HashSet<String>>,
) -> Result<()> {
    let (_, path) = find(roots, content_type, name, enabled, managed).await?;
    tokio::task::spawn_blocking(move || trash::delete(&path))
        .await
        .map_err(Error::other)?
        .map_err(Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RP: ContentType = ContentType::ResourcePack;

    fn pack(dir: &Path, name: &str, meta: &str) {
        let path = dir.join(name);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join(PACK_META), meta).unwrap();
    }

    #[tokio::test]
    async fn folder_packs_toggle_between_the_packs_folder_and_the_stash() {
        let game = polyio::tempdir().await.unwrap();
        let game = game.dir_path();
        let packs = packs_dir(game, RP);
        let roots = [game.to_path_buf()];
        let managed = HashSet::from(["Managed.zip".to_owned()]);
        let list = || list_in(&roots, RP, Some(&managed));
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
        std::fs::write(packs.join("Managed.zip"), b"zip").unwrap();
        std::fs::write(packs.join("notes.txt"), b"txt").unwrap();

        let listed = list().await;
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
                FolderPack {
                    name: "Zipped.zip".into(),
                    enabled: true,
                    description: None
                },
            ]
        );

        set_enabled_in(&roots, RP, "Faithful", false, Some(&managed))
            .await
            .unwrap();
        assert!(
            stash_dir(game, RP)
                .join("Faithful")
                .join(PACK_META)
                .is_file()
        );
        assert!(!packs.join("Faithful").exists());
        assert!(!list().await[0].enabled);

        assert!(
            set_enabled_in(&roots, RP, "Faithful", false, Some(&managed))
                .await
                .is_err()
        );
        set_enabled_in(&roots, RP, "Faithful", true, Some(&managed))
            .await
            .unwrap();
        assert!(packs.join("Faithful").join(PACK_META).is_file());

        assert!(
            set_enabled_in(&roots, RP, "../Faithful", false, Some(&managed))
                .await
                .is_err()
        );
        assert!(
            remove_in(&roots, RP, "NotAPack", true, Some(&managed))
                .await
                .is_err()
        );
        assert!(
            remove_in(&roots, RP, "Managed.zip", true, Some(&managed))
                .await
                .is_err()
        );

        set_enabled_in(&roots, RP, "Raw", false, Some(&managed))
            .await
            .unwrap();
        pack(&packs, "Raw", r#"{"pack":{"description":"new"}}"#);
        let raw: Vec<bool> = list()
            .await
            .into_iter()
            .filter(|pack| pack.name == "Raw")
            .map(|pack| pack.enabled)
            .collect();
        assert_eq!(raw.len(), 2);
        assert!(
            set_enabled_in(&roots, RP, "Raw", true, Some(&managed))
                .await
                .is_err()
        );
        std::fs::remove_dir_all(packs.join("Raw")).unwrap();
        assert!(
            remove_in(&roots, RP, "Raw", true, Some(&managed))
                .await
                .is_err()
        );
        assert!(stash_dir(game, RP).join("Raw").join(PACK_META).is_file());

        set_enabled_in(&roots, RP, "Zipped.zip", false, Some(&managed))
            .await
            .unwrap();
        assert!(stash_dir(game, RP).join("Zipped.zip").is_file());
    }

    #[tokio::test]
    async fn instances_sharing_a_pack_folder_share_the_off_state() {
        let root = polyio::tempdir().await.unwrap();
        let root = root.dir_path();
        let (first, second, shared) = (root.join("a"), root.join("b"), root.join("shared"));
        pack(&packs_dir(&shared, RP), "Faithful", r#"{"pack":{}}"#);
        for game in [&first, &second] {
            std::fs::create_dir_all(game).unwrap();
            polyio::symlink_dir(packs_dir(&shared, RP), packs_dir(game, RP))
                .await
                .unwrap();
        }

        set_enabled_in(&[first.clone()], RP, "Faithful", false, None)
            .await
            .unwrap();
        assert!(shared.join(STASH_DIR).join("Faithful").is_dir());
        assert!(!list_in(&[second.clone()], RP, None).await[0].enabled);

        set_enabled_in(&[second.clone()], RP, "Faithful", true, None)
            .await
            .unwrap();
        assert!(list_in(&[first.clone()], RP, None).await[0].enabled);
    }

    #[tokio::test]
    async fn shader_folders_list_and_managed_names_stay_in_their_root() {
        let root = polyio::tempdir().await.unwrap();
        let root = root.dir_path();
        let (shared, own) = (root.join("shared"), root.join("own"));
        let sh = ContentType::Shader;
        std::fs::create_dir_all(packs_dir(&shared, sh).join("Folder").join("shaders")).unwrap();
        std::fs::create_dir_all(packs_dir(&shared, sh).join("Junk")).unwrap();
        std::fs::write(packs_dir(&shared, sh).join("Same.zip"), b"zip").unwrap();
        std::fs::create_dir_all(packs_dir(&own, sh)).unwrap();
        std::fs::write(packs_dir(&own, sh).join("Same.zip"), b"zip").unwrap();

        let managed = HashSet::from(["Same.zip".to_owned()]);
        let roots = [shared.clone(), own.clone()];
        let names: Vec<String> = list_in(&roots, sh, Some(&managed))
            .await
            .into_iter()
            .map(|pack| pack.name)
            .collect();
        assert_eq!(names, ["Folder", "Same.zip"]);

        set_enabled_in(&roots, sh, "Same.zip", false, Some(&managed))
            .await
            .unwrap();
        assert!(packs_dir(&shared, sh).join("Same.zip").is_file());
        assert!(stash_dir(&own, sh).join("Same.zip").is_file());
    }
}
