use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use serde::Deserialize;

use super::{BlockedFile, LooseFile, ModpackContents, ModpackManifest, is_safe_relative_path};
use crate::bundles::{BundleFile, BundleFileKind, BundleFileType};
use crate::ctx::ContentCtx;
use crate::error::ContentResult;
use crate::packages::PackageError;
use crate::packages::types::{ExternalFile, ProjectDetail, VersionDetail};
use oneclient_common::domain::{ContentType, GameLoader, ProviderId};

pub(super) const MANIFEST_ENTRY: &str = "manifest.json";

const MODPACK_MANIFEST_TYPE: &str = "minecraftModpack";
const LOOKUP_CHUNK: usize = 500;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfManifest {
    minecraft: CfMinecraft,
    #[serde(default)]
    manifest_type: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    files: Vec<CfManifestFile>,
    #[serde(default)]
    overrides: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfMinecraft {
    version: String,
    #[serde(default)]
    mod_loaders: Vec<CfModLoader>,
}

#[derive(Deserialize)]
struct CfModLoader {
    id: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Deserialize)]
struct CfManifestFile {
    #[serde(rename = "projectID")]
    project_id: u32,
    #[serde(rename = "fileID")]
    file_id: u32,
    #[serde(default = "required_by_default")]
    required: bool,
}

fn required_by_default() -> bool {
    true
}

pub(super) fn is_modpack_manifest(bytes: &[u8]) -> bool {
    serde_json::from_slice::<CfManifest>(bytes)
        .is_ok_and(|manifest| manifest.manifest_type == MODPACK_MANIFEST_TYPE)
}

pub(super) async fn resolve(bytes: &[u8], ctx: &ContentCtx) -> ContentResult<ModpackManifest> {
    let manifest: CfManifest =
        serde_json::from_slice(bytes).map_err(|_| PackageError::UnsupportedModpackFormat)?;
    if manifest.manifest_type != MODPACK_MANIFEST_TYPE {
        return Err(PackageError::UnsupportedModpackFormat.into());
    }

    let (loader, loader_version) = parse_loader(&manifest.minecraft.mod_loaders)?;

    let requests: Vec<FileRequest> = manifest
        .files
        .iter()
        .map(|file| FileRequest {
            file_id: file.file_id,
            project_id: Some(file.project_id),
            required: file.required,
            label: None,
            destination: None,
        })
        .collect();
    let contents = resolve_requests(&requests, ctx).await?;

    let overrides = manifest
        .overrides
        .map(|folder| folder.trim_matches('/').to_string())
        .filter(|folder| !folder.is_empty() && is_safe_relative_path(folder))
        .unwrap_or_else(|| "overrides".to_string());

    Ok(ModpackManifest {
        name: manifest.name,
        version: manifest.version,
        summary: None,
        mc_version: manifest.minecraft.version,
        loader,
        loader_version,
        contents,
        override_prefixes: vec![format!("{overrides}/")],
    })
}

pub(super) struct FileRequest {
    pub(super) file_id: u32,
    pub(super) project_id: Option<u32>,
    pub(super) required: bool,
    pub(super) label: Option<String>,
    pub(super) destination: Option<ContentType>,
}

pub(super) async fn resolve_requests(
    requests: &[FileRequest],
    ctx: &ContentCtx,
) -> ContentResult<ModpackContents> {
    let provider = ctx.providers.get(ProviderId::CurseForge)?;

    let file_ids: Vec<String> = requests
        .iter()
        .map(|request| request.file_id.to_string())
        .collect();
    let mut versions: HashMap<String, VersionDetail> = HashMap::new();
    for chunk in file_ids.chunks(LOOKUP_CHUNK) {
        for version in provider.get_versions(chunk, ctx).await? {
            versions.insert(version.version_id.clone(), version);
        }
    }

    let project_of = |request: &FileRequest| {
        request.project_id.map(|id| id.to_string()).or_else(|| {
            versions
                .get(&request.file_id.to_string())
                .map(|version| version.project_id.clone())
        })
    };
    let project_ids: Vec<String> = requests
        .iter()
        .filter_map(project_of)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let mut projects: HashMap<String, ProjectDetail> = HashMap::new();
    for chunk in project_ids.chunks(LOOKUP_CHUNK) {
        for project in provider.get_projects(chunk, ctx).await? {
            projects.insert(project.id.clone(), project);
        }
    }

    let mut contents = ModpackContents::default();
    for request in requests {
        contents.add(
            request,
            versions.get(&request.file_id.to_string()),
            project_of(request).and_then(|id| projects.get(&id)),
        );
    }
    Ok(contents)
}

impl ModpackContents {
    fn add(
        &mut self,
        entry: &FileRequest,
        version: Option<&VersionDetail>,
        project: Option<&ProjectDetail>,
    ) {
        let label = || {
            project
                .map(|project| project.name.clone())
                .or_else(|| entry.label.clone())
                .unwrap_or_else(|| format!("CurseForge file {}", entry.file_id))
        };

        let (Some(version), Some(file)) = (version, version.and_then(VersionDetail::primary_file))
        else {
            tracing::warn!(file_id = entry.file_id, "curseforge file not found");
            self.unresolved.push(label());
            return;
        };

        let content_type = entry
            .destination
            .or_else(|| project.map(|project| project.content_type))
            .unwrap_or(ContentType::Mod);
        let path = format!("{}/{}", content_type.folder_name(), file.file_name);
        if !is_safe_relative_path(&path) || file.sha1.is_empty() {
            tracing::warn!(path = %path, "curseforge file has an unusable name or no sha1");
            self.unresolved.push(label());
            return;
        }

        let sha1 = file.sha1.to_ascii_lowercase();
        let project_id = version.project_id.clone();
        let version_id = entry.file_id.to_string();

        if file.url.is_empty() {
            self.blocked.push(BlockedFile {
                project_id,
                version_id: version_id.clone(),
                project_name: label(),
                file_name: file.file_name.clone(),
                path,
                sha1,
                size: file.size,
                content_type,
                page_url: project.and_then(|project| download_page(project, &version_id)),
            });
            return;
        }

        match content_type {
            ContentType::Mod | ContentType::ResourcePack | ContentType::Shader => {
                self.direct.insert(
                    sha1.clone(),
                    ExternalFile {
                        name: file.file_name.clone(),
                        url: file.url.clone(),
                        sha1: sha1.clone(),
                        size: file.size,
                        content_type,
                    },
                );
                if !entry.required {
                    self.optional.insert(project_id.clone());
                }
                self.files.push(BundleFile {
                    enabled: true,
                    hidden: false,
                    path,
                    size: file.size,
                    file_type: BundleFileType::Normal,
                    kind: BundleFileKind::Managed {
                        provider: ProviderId::CurseForge,
                        project_id,
                        version_id,
                        sha1,
                    },
                });
            }
            _ => self.loose.push(LooseFile {
                path,
                url: file.url.clone(),
                sha1,
                size: file.size,
            }),
        }
    }
}

fn download_page(project: &ProjectDetail, file_id: &str) -> Option<String> {
    let website = project
        .links
        .iter()
        .find(|(label, _)| label == "Website")
        .map(|(_, url)| url.trim_end_matches('/').to_string())?;
    Some(format!("{website}/download/{file_id}"))
}

fn parse_loader(loaders: &[CfModLoader]) -> ContentResult<(GameLoader, Option<String>)> {
    let Some(chosen) = loaders
        .iter()
        .find(|loader| loader.primary)
        .or_else(|| loaders.first())
    else {
        return Ok((GameLoader::Vanilla, None));
    };

    let (name, version) = chosen
        .id
        .split_once('-')
        .ok_or(PackageError::UnsupportedModpackFormat)?;
    let loader = GameLoader::from_str(name).map_err(|_| PackageError::UnsupportedModpackFormat)?;
    Ok((loader, Some(version.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packages::types::{PackageBody, VersionFile};
    use chrono::Utc;

    fn loader(id: &str, primary: bool) -> CfModLoader {
        CfModLoader {
            id: id.to_string(),
            primary,
        }
    }

    #[test]
    fn the_primary_loader_is_split_into_name_and_version() {
        assert_eq!(
            parse_loader(&[loader("fabric-0.15.3", false), loader("forge-47.2.0", true)]).unwrap(),
            (GameLoader::Forge, Some("47.2.0".to_string()))
        );
        assert_eq!(
            parse_loader(&[loader("neoforge-21.1.72", true)]).unwrap(),
            (GameLoader::NeoForge, Some("21.1.72".to_string()))
        );
        assert_eq!(parse_loader(&[]).unwrap(), (GameLoader::Vanilla, None));
        assert!(parse_loader(&[loader("rift", true)]).is_err());
    }

    #[test]
    fn only_minecraft_modpack_manifests_are_recognised() {
        let pack = serde_json::json!({
            "minecraft": {"version": "1.20.1", "modLoaders": []},
            "manifestType": "minecraftModpack",
            "files": []
        });
        let other = serde_json::json!({"name": "not a pack"});

        assert!(is_modpack_manifest(&serde_json::to_vec(&pack).unwrap()));
        assert!(!is_modpack_manifest(&serde_json::to_vec(&other).unwrap()));
    }

    fn project(content_type: ContentType) -> ProjectDetail {
        ProjectDetail {
            id: "10".into(),
            slug: "thing".into(),
            provider: ProviderId::CurseForge,
            content_type,
            name: "Thing".into(),
            summary: String::new(),
            author: String::new(),
            members: Vec::new(),
            gallery: Vec::new(),
            license: None,
            links: vec![(
                "Website".into(),
                "https://www.curseforge.com/minecraft/mc-mods/thing/".into(),
            )],
            body: PackageBody::Raw(String::new()),
            version_ids: Vec::new(),
            game_versions: Vec::new(),
            loaders: Vec::new(),
            icon_url: None,
            created: Utc::now(),
            updated: Utc::now(),
            downloads: 0,
        }
    }

    fn version(url: &str) -> VersionDetail {
        VersionDetail {
            version_id: "20".into(),
            project_id: "10".into(),
            name: "Thing 1.0".into(),
            version_number: "thing-1.0.jar".into(),
            changelog: None,
            game_versions: Vec::new(),
            loaders: Vec::new(),
            published: Utc::now(),
            downloads: 0,
            files: vec![VersionFile {
                sha1: "ABCD".into(),
                url: url.into(),
                file_name: "thing-1.0.jar".into(),
                primary: true,
                size: 5,
                fingerprint: None,
            }],
            dependencies: Vec::new(),
        }
    }

    fn entry(required: bool) -> FileRequest {
        FileRequest {
            file_id: 20,
            project_id: Some(10),
            required,
            label: None,
            destination: None,
        }
    }

    #[test]
    fn a_downloadable_mod_becomes_a_managed_file() {
        let mut contents = ModpackContents::default();
        contents.add(
            &entry(false),
            Some(&version("https://edge.forgecdn.net/thing-1.0.jar")),
            Some(&project(ContentType::Mod)),
        );

        assert_eq!(contents.files.len(), 1);
        assert_eq!(contents.files[0].path, "mods/thing-1.0.jar");
        assert!(contents.files[0].enabled);
        assert!(contents.optional.contains("10"));
        assert_eq!(
            contents.files[0].kind,
            BundleFileKind::Managed {
                provider: ProviderId::CurseForge,
                project_id: "10".into(),
                version_id: "20".into(),
                sha1: "abcd".into(),
            }
        );
    }

    #[test]
    fn a_file_without_a_download_url_is_blocked() {
        let mut contents = ModpackContents::default();
        contents.add(
            &entry(true),
            Some(&version("")),
            Some(&project(ContentType::ResourcePack)),
        );

        assert!(contents.files.is_empty());
        assert_eq!(contents.blocked.len(), 1);
        assert_eq!(contents.blocked[0].path, "resourcepacks/thing-1.0.jar");
        assert_eq!(
            contents.blocked[0].page_url.as_deref(),
            Some("https://www.curseforge.com/minecraft/mc-mods/thing/download/20")
        );
    }

    #[test]
    fn a_requested_destination_beats_the_project_category() {
        let mut contents = ModpackContents::default();
        let request = FileRequest {
            destination: Some(ContentType::ResourcePack),
            ..entry(true)
        };
        contents.add(
            &request,
            Some(&version("https://edge.forgecdn.net/thing-1.0.jar")),
            Some(&project(ContentType::Mod)),
        );

        assert_eq!(contents.files[0].path, "resourcepacks/thing-1.0.jar");
    }

    #[test]
    fn a_missing_file_without_a_project_uses_its_label() {
        let mut contents = ModpackContents::default();
        let request = FileRequest {
            file_id: 20,
            project_id: None,
            required: true,
            label: Some("Stay Clear".into()),
            destination: None,
        };
        contents.add(&request, None, None);

        assert_eq!(contents.unresolved, vec!["Stay Clear".to_string()]);
    }

    #[test]
    fn a_missing_file_is_reported_by_project_name() {
        let mut contents = ModpackContents::default();
        contents.add(&entry(true), None, Some(&project(ContentType::Mod)));

        assert_eq!(contents.unresolved, vec!["Thing".to_string()]);
    }
}
