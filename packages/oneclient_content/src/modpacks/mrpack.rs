use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use super::{
    LooseFile, ModpackContents, ModpackManifest, is_safe_relative_path, tracked_content_type,
};
use crate::bundles::polymrpack::{mrpack_file_kind, scan_dependencies};
use crate::bundles::{BundleFile, BundleFileKind, BundleFileType};
use crate::error::ContentResult;
use crate::packages::PackageError;
use crate::packages::types::ExternalFile;
use oneclient_common::domain::GameLoader;

pub(super) const INDEX_ENTRY: &str = "modrinth.index.json";

const OVERRIDES: &str = "overrides/";
const CLIENT_OVERRIDES: &str = "client-overrides/";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrpackManifest {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version_id: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    files: Vec<MrpackFileEntry>,
    #[serde(default)]
    dependencies: HashMap<String, String>,
}

#[derive(Deserialize)]
struct MrpackFileEntry {
    path: String,
    hashes: MrpackHashes,
    #[serde(default)]
    env: Option<MrpackEnv>,
    #[serde(default)]
    downloads: Vec<String>,
    #[serde(rename = "fileSize", default)]
    file_size: u64,
}

#[derive(Deserialize)]
struct MrpackHashes {
    sha1: String,
}

#[derive(Deserialize)]
struct MrpackEnv {
    #[serde(default)]
    client: Option<String>,
}

impl MrpackFileEntry {
    fn client_env(&self) -> Option<&str> {
        self.env.as_ref().and_then(|env| env.client.as_deref())
    }

    fn client_side(&self) -> bool {
        self.client_env() != Some("unsupported")
    }

    fn optional(&self) -> bool {
        self.client_env() == Some("optional")
    }
}

pub(super) fn parse(bytes: &[u8]) -> ContentResult<ModpackManifest> {
    let manifest: MrpackManifest = serde_json::from_slice(bytes)?;
    let (mc_version, loader, loader_version) = parse_dependencies(&manifest.dependencies)?;

    let mut contents = ModpackContents::default();

    for entry in manifest.files {
        if !entry.client_side() {
            continue;
        }

        let path = entry.path.replace('\\', "/");
        if !is_safe_relative_path(&path) {
            tracing::warn!(path = %entry.path, "skipping modpack file with an unsafe path");
            contents.unresolved.push(entry.path);
            continue;
        }

        let sha1 = entry.hashes.sha1.to_ascii_lowercase();

        if let Some(content_type) = tracked_content_type(&path)
            && let Some(kind) = mrpack_file_kind(&path, &entry.downloads, &sha1, entry.file_size)
        {
            if let (BundleFileKind::Managed { .. }, Some(url)) = (&kind, entry.downloads.first()) {
                contents.direct.insert(
                    sha1.clone(),
                    ExternalFile {
                        name: file_name(&path),
                        url: url.clone(),
                        sha1: sha1.clone(),
                        size: entry.file_size,
                        content_type,
                    },
                );
            }
            if entry.optional() {
                contents.optional.insert(kind.package_id());
            }
            contents.files.push(BundleFile {
                enabled: true,
                hidden: false,
                path,
                size: entry.file_size,
                file_type: BundleFileType::Normal,
                kind,
            });
            continue;
        }

        match entry.downloads.into_iter().next() {
            Some(url) => contents.loose.push(LooseFile {
                path,
                url,
                sha1,
                size: entry.file_size,
            }),
            None => {
                tracing::warn!(path = %path, "modpack file has no download");
                contents.unresolved.push(path);
            }
        }
    }

    Ok(ModpackManifest {
        name: manifest.name,
        version: manifest.version_id,
        summary: manifest
            .summary
            .filter(|summary| !summary.trim().is_empty()),
        mc_version,
        loader,
        loader_version,
        contents,
        override_prefixes: vec![OVERRIDES.to_string(), CLIENT_OVERRIDES.to_string()],
    })
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_string()
}

fn parse_dependencies(
    deps: &HashMap<String, String>,
) -> ContentResult<(String, GameLoader, Option<String>)> {
    let scanned = scan_dependencies(deps, GameLoader::is_modded);
    let mc_version = scanned
        .mc_version
        .ok_or(PackageError::UnsupportedModpackFormat)?;
    Ok(match scanned.loader {
        Some((loader, version)) => (mc_version, loader, Some(version)),
        None => (mc_version, GameLoader::Vanilla, None),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundles::BundleFileKind;
    use oneclient_common::domain::ProviderId;

    fn index(files: serde_json::Value, dependencies: serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "formatVersion": 1,
            "game": "minecraft",
            "versionId": "1.2.0",
            "name": "Example Pack",
            "files": files,
            "dependencies": dependencies,
        }))
        .unwrap()
    }

    #[test]
    fn a_plain_mrpack_parses_into_tracked_and_loose_files() {
        let bytes = index(
            serde_json::json!([
                {
                    "path": "mods/sodium.jar",
                    "hashes": {"sha1": "AAAA", "sha512": "x"},
                    "downloads": ["https://cdn.modrinth.com/data/AANobbMI/versions/tFw0iWAk/sodium.jar"],
                    "fileSize": 10
                },
                {
                    "path": "mods/extra.jar",
                    "hashes": {"sha1": "bbbb"},
                    "downloads": ["https://github.com/a/b/releases/download/1/extra.jar"],
                    "fileSize": 20
                },
                {
                    "path": "config/thing/defaults.json",
                    "hashes": {"sha1": "cccc"},
                    "downloads": ["https://example.invalid/defaults.json"],
                    "fileSize": 30
                },
                {
                    "path": "mods/pick-me.jar",
                    "hashes": {"sha1": "ffff"},
                    "env": {"client": "optional", "server": "optional"},
                    "downloads": ["https://example.invalid/pick-me.jar"],
                    "fileSize": 60
                },
                {
                    "path": "mods/server-only.jar",
                    "hashes": {"sha1": "dddd"},
                    "env": {"client": "unsupported", "server": "required"},
                    "downloads": ["https://example.invalid/server-only.jar"],
                    "fileSize": 40
                },
                {
                    "path": "../escape.jar",
                    "hashes": {"sha1": "eeee"},
                    "downloads": ["https://example.invalid/escape.jar"],
                    "fileSize": 50
                }
            ]),
            serde_json::json!({"minecraft": "1.21.1", "fabric-loader": "0.16.5"}),
        );

        let manifest = parse(&bytes).unwrap();

        assert_eq!(manifest.name, "Example Pack");
        assert_eq!(manifest.version, "1.2.0");
        assert_eq!(manifest.mc_version, "1.21.1");
        assert_eq!(manifest.loader, GameLoader::Fabric);
        assert_eq!(manifest.loader_version.as_deref(), Some("0.16.5"));

        assert_eq!(manifest.contents.files.len(), 3);
        assert!(manifest.contents.optional.contains("ffff"));
        assert_eq!(manifest.contents.optional.len(), 1);
        assert_eq!(
            manifest.contents.files[0].kind,
            BundleFileKind::Managed {
                provider: ProviderId::Modrinth,
                project_id: "AANobbMI".into(),
                version_id: "tFw0iWAk".into(),
                sha1: "aaaa".into(),
            }
        );
        assert!(matches!(
            manifest.contents.files[1].kind,
            BundleFileKind::External { .. }
        ));
        assert_eq!(
            manifest
                .contents
                .direct
                .get("aaaa")
                .map(|file| file.url.as_str()),
            Some("https://cdn.modrinth.com/data/AANobbMI/versions/tFw0iWAk/sodium.jar")
        );
        assert_eq!(
            manifest
                .contents
                .direct
                .get("aaaa")
                .map(|file| file.name.as_str()),
            Some("sodium.jar")
        );
        assert!(!manifest.contents.direct.contains_key("bbbb"));

        assert_eq!(manifest.contents.loose.len(), 1);
        assert_eq!(
            manifest.contents.loose[0].path,
            "config/thing/defaults.json"
        );
        assert_eq!(
            manifest.contents.unresolved,
            vec!["../escape.jar".to_string()]
        );
        assert_eq!(
            manifest.override_prefixes,
            vec!["overrides/".to_string(), "client-overrides/".to_string()]
        );
    }

    #[test]
    fn a_pack_without_a_loader_is_vanilla() {
        let bytes = index(
            serde_json::json!([]),
            serde_json::json!({"minecraft": "1.21.1"}),
        );

        let manifest = parse(&bytes).unwrap();

        assert_eq!(manifest.loader, GameLoader::Vanilla);
        assert_eq!(manifest.loader_version, None);
    }

    #[test]
    fn neoforge_and_quilt_keys_are_recognised() {
        for (key, loader) in [
            ("neoforge", GameLoader::NeoForge),
            ("forge", GameLoader::Forge),
            ("quilt-loader", GameLoader::Quilt),
        ] {
            let bytes = index(
                serde_json::json!([]),
                serde_json::json!({"minecraft": "1.21.1", key: "1.0"}),
            );
            assert_eq!(parse(&bytes).unwrap().loader, loader, "{key}");
        }
    }

    #[test]
    fn a_pack_without_a_minecraft_version_is_rejected() {
        let bytes = index(serde_json::json!([]), serde_json::json!({}));
        assert!(parse(&bytes).is_err());
    }
}
