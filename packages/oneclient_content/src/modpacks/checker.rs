use std::collections::HashSet;
use std::path::Path;

use serde::Deserialize;

use super::curseforge::{FileRequest, resolve_requests};
use super::{
    ModpackContents, ModpackManifest, file_name_of, file_sha1, tracked_content_type, tracked_folder,
};
use crate::bundles::overrides::OverrideLayers;
use crate::ctx::ContentCtx;
use oneclient_common::domain::ContentType;

const CHECKER_CONFIG: &str = "config/missing_mods_checker.json";
const CURSEFORGE_HOST: &str = "curseforge.com";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckerEntry {
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    destination: String,
}

pub(super) async fn add_checker_mods(
    archive_path: &Path,
    manifest: &mut ModpackManifest,
    ctx: &ContentCtx,
) {
    let Some(bytes) = checker_config(archive_path, &manifest.override_prefixes).await else {
        return;
    };

    let requests = checker_requests(&bytes);
    if requests.is_empty() {
        return;
    }

    match resolve_requests(&requests, ctx).await {
        Ok(resolved) => {
            tracing::info!(
                listed = requests.len(),
                "adding the curseforge mods the pack's missing mods checker asks for"
            );
            let bundled = bundled_file_names(archive_path, &manifest.override_prefixes).await;
            merge(manifest, resolved, &bundled);
        }
        Err(err) => {
            tracing::warn!(
                error = %err,
                "could not resolve the missing mods checker list; the game will ask for them instead"
            );
        }
    }
}

async fn checker_config(archive_path: &Path, prefixes: &[String]) -> Option<Vec<u8>> {
    for prefix in prefixes.iter().rev() {
        match super::zip_entry(archive_path, &format!("{prefix}{CHECKER_CONFIG}")).await {
            Ok(Some(bytes)) => return Some(bytes),
            Ok(None) => {}
            Err(err) => {
                tracing::debug!(error = %err, "could not read the missing mods checker config");
                return None;
            }
        }
    }
    None
}

fn checker_requests(bytes: &[u8]) -> Vec<FileRequest> {
    let entries: Vec<CheckerEntry> = match serde_json::from_slice(bytes) {
        Ok(entries) => entries,
        Err(err) => {
            tracing::warn!(error = %err, "the missing mods checker config is not a list of mods");
            return Vec::new();
        }
    };

    let mut seen = HashSet::new();
    entries
        .into_iter()
        .filter_map(|entry| {
            let file_id = curseforge_file_id(&entry.url)?;
            seen.insert(file_id).then(|| FileRequest {
                file_id,
                project_id: None,
                required: true,
                label: Some(entry.display_name).filter(|name| !name.trim().is_empty()),
                destination: destination(&entry.destination),
            })
        })
        .collect()
}

fn destination(folder: &str) -> Option<ContentType> {
    tracked_folder(folder.trim_matches('/'))
}

fn curseforge_file_id(url: &str) -> Option<u32> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if host != CURSEFORGE_HOST && !host.ends_with(&format!(".{CURSEFORGE_HOST}")) {
        return None;
    }

    let segments: Vec<&str> = parsed.path_segments()?.collect();
    segments
        .windows(2)
        .find(|pair| matches!(pair[0], "download" | "files"))
        .and_then(|pair| pair[1].parse().ok())
}

fn merge(
    manifest: &mut ModpackManifest,
    resolved: ModpackContents,
    bundled_names: &HashSet<String>,
) {
    let contents = &mut manifest.contents;
    let mut ids = HashSet::new();
    let mut hashes = HashSet::new();
    let mut names: HashSet<String> = bundled_names.clone();

    let mut remember = |id: String, sha1: &str, path: &str| {
        let name = file_name_of(path);
        let known = ids.contains(&id) || hashes.contains(sha1) || names.contains(&name);
        ids.insert(id);
        hashes.insert(sha1.to_string());
        names.insert(name);
        !known
    };

    for file in &contents.files {
        remember(file.kind.package_id(), file_sha1(file), &file.path);
    }
    for file in &contents.blocked {
        remember(file.project_id.clone(), &file.sha1, &file.file_name);
    }

    for file in resolved.files {
        if remember(file.kind.package_id(), file_sha1(&file), &file.path) {
            contents.files.push(file);
        } else {
            tracing::debug!(file = %file.path, "the pack already has this checker mod");
        }
    }
    for file in resolved.blocked {
        if remember(file.project_id.clone(), &file.sha1, &file.path) {
            contents.blocked.push(file);
        } else {
            tracing::debug!(file = %file.path, "the pack already has this checker mod");
        }
    }
    for (sha1, file) in resolved.direct {
        contents.direct.entry(sha1).or_insert(file);
    }
    contents.optional.extend(resolved.optional);
    contents.loose.extend(resolved.loose);
    contents.unresolved.extend(resolved.unresolved);
}

async fn bundled_file_names(archive_path: &Path, prefixes: &[String]) -> HashSet<String> {
    let prefixes: Vec<&str> = prefixes.iter().map(String::as_str).collect();
    match OverrideLayers::open(archive_path, &prefixes).await {
        Ok(layers) => layers
            .entries()
            .iter()
            .filter(|(rel, _)| tracked_content_type(rel).is_some())
            .map(|(rel, _)| file_name_of(rel))
            .collect(),
        Err(err) => {
            tracing::debug!(error = %err, "could not list the pack's bundled files");
            HashSet::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_ids_come_from_curseforge_download_links() {
        assert_eq!(
            curseforge_file_id(
                "https://www.curseforge.com/minecraft/mc-mods/balm-fabric/download/7420614"
            ),
            Some(7_420_614)
        );
        assert_eq!(
            curseforge_file_id("https://www.curseforge.com/minecraft/mc-mods/thing/files/5010620"),
            Some(5_010_620)
        );
        assert_eq!(
            curseforge_file_id("https://example.com/minecraft/mc-mods/thing/download/5010620"),
            None
        );
        assert_eq!(
            curseforge_file_id("https://www.curseforge.com/minecraft/mc-mods/thing"),
            None
        );
    }

    #[test]
    fn the_checker_list_becomes_required_file_requests() {
        let config = serde_json::json!([
            {
                "displayName": "Balm (Fabric Edition)",
                "pattern": "balm-fabric-1.20.1-7.3.38.jar",
                "url": "https://www.curseforge.com/minecraft/mc-mods/balm-fabric/download/7420614",
                "destination": "mods"
            },
            {
                "displayName": "Stay Clear",
                "url": "https://www.curseforge.com/minecraft/texture-packs/stay-clear/download/6000001",
                "destination": "resourcepacks"
            },
            {
                "displayName": "Duplicate",
                "url": "https://www.curseforge.com/minecraft/mc-mods/balm-fabric/download/7420614"
            },
            {
                "displayName": "Elsewhere",
                "url": "https://github.com/someone/thing/releases/download/1/thing.jar"
            }
        ]);

        let requests = checker_requests(&serde_json::to_vec(&config).unwrap());

        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].file_id, 7_420_614);
        assert!(requests[0].required);
        assert_eq!(requests[0].label.as_deref(), Some("Balm (Fabric Edition)"));
        assert_eq!(requests[0].destination, Some(ContentType::Mod));
        assert_eq!(requests[1].destination, Some(ContentType::ResourcePack));
    }

    #[test]
    fn a_checker_mod_the_pack_already_has_is_not_added_twice() {
        use crate::bundles::{BundleFile, BundleFileKind, BundleFileType};
        use oneclient_common::domain::{GameLoader, ProviderId};

        let managed = |project: &str, sha1: &str, path: &str| BundleFile {
            enabled: true,
            hidden: false,
            path: path.into(),
            size: 1,
            file_type: BundleFileType::Normal,
            kind: BundleFileKind::Managed {
                provider: ProviderId::CurseForge,
                project_id: project.into(),
                version_id: "1".into(),
                sha1: sha1.into(),
            },
        };

        let mut manifest = ModpackManifest {
            name: "Pack".into(),
            version: "1".into(),
            summary: None,
            mc_version: "1.20.1".into(),
            loader: GameLoader::Fabric,
            loader_version: None,
            contents: ModpackContents {
                files: vec![managed(
                    "modrinth-balm",
                    "aaaa",
                    "mods/balm-fabric-1.20.1-7.3.38.jar",
                )],
                ..Default::default()
            },
            override_prefixes: vec!["overrides/".into()],
        };

        let resolved = ModpackContents {
            files: vec![
                managed("111", "bbbb", "mods/balm-fabric-1.20.1-7.3.38.jar"),
                managed("222", "aaaa", "mods/renamed.jar"),
                managed("333", "cccc", "mods/bundled.jar"),
                managed("444", "dddd", "mods/new.jar"),
            ],
            ..Default::default()
        };
        let bundled: HashSet<String> = ["bundled.jar".to_string()].into_iter().collect();

        merge(&mut manifest, resolved, &bundled);

        let paths: Vec<&str> = manifest
            .contents
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect();
        assert_eq!(
            paths,
            vec!["mods/balm-fabric-1.20.1-7.3.38.jar", "mods/new.jar"]
        );
    }

    #[test]
    fn a_config_that_is_not_a_list_is_ignored() {
        assert!(checker_requests(br#"{"mods": []}"#).is_empty());
    }
}
