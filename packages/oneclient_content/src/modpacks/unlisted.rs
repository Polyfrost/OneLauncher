use std::collections::HashSet;
use std::path::Path;

use super::{ModpackManifest, tracked_content_type};
use crate::bundles::BundleFileKind;
use crate::bundles::overrides::OverrideLayers;
use crate::ctx::ContentCtx;
use crate::error::ContentResult;
use crate::packages::{FileIdentity, curseforge_fingerprint};
use oneclient_common::domain::{ContentType, ProviderId};

const CURSEFORGE_HOSTS: [&str; 2] = ["forgecdn.net", "curseforge.com"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlistedMod {
    pub name: String,
    pub sha1: String,
    pub bundled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnlistedReport {
    pub unlisted: Vec<UnlistedMod>,
    pub unconfirmed: usize,
}

struct Candidate {
    name: String,
    identity: FileIdentity,
    bundled: bool,
}

#[tracing::instrument(level = "debug", skip_all, fields(pack = %manifest.name))]
pub async fn find_unlisted_mods(
    archive_path: &Path,
    manifest: &ModpackManifest,
    ctx: &ContentCtx,
) -> ContentResult<UnlistedReport> {
    let mut candidates = downloaded_jars(manifest);
    candidates.extend(bundled_jars(archive_path, manifest).await?);

    let mut seen = HashSet::new();
    candidates.retain(|candidate| seen.insert(candidate.identity.sha1.clone()));
    if candidates.is_empty() {
        return Ok(UnlistedReport::default());
    }

    let identities: Vec<FileIdentity> = candidates
        .iter()
        .map(|candidate| candidate.identity.clone())
        .collect();
    let mut known = HashSet::new();
    let mut complete = true;
    for provider in ctx.providers.remote() {
        match provider.lookup_versions(&identities, ctx).await {
            Ok(found) => known.extend(found.into_keys()),
            Err(err) => {
                tracing::warn!(provider = ?provider.id(), error = %err, "could not look up the modpack's mods");
                complete = false;
            }
        }
    }

    let report = report(candidates, &known, complete);
    if !report.unlisted.is_empty() {
        tracing::warn!(
            count = report.unlisted.len(),
            "modpack has mods that are not on modrinth or curseforge"
        );
    }
    Ok(report)
}

fn report(candidates: Vec<Candidate>, known: &HashSet<String>, complete: bool) -> UnlistedReport {
    let unknown = candidates
        .into_iter()
        .filter(|candidate| !known.contains(&candidate.identity.sha1));
    if !complete {
        return UnlistedReport {
            unlisted: Vec::new(),
            unconfirmed: unknown.count(),
        };
    }
    UnlistedReport {
        unlisted: unknown
            .map(|candidate| UnlistedMod {
                name: candidate.name,
                sha1: candidate.identity.sha1,
                bundled: candidate.bundled,
            })
            .collect(),
        unconfirmed: 0,
    }
}

fn downloaded_jars(manifest: &ModpackManifest) -> Vec<Candidate> {
    let contents = &manifest.contents;
    let mods = contents
        .files
        .iter()
        .filter(|file| file.content_type() == ContentType::Mod)
        .filter_map(|file| match &file.kind {
            BundleFileKind::External { file: external, .. }
                if !is_curseforge_url(&external.url) =>
            {
                Some(Candidate {
                    name: file.display_name(),
                    identity: FileIdentity::from_sha1(&external.sha1),
                    bundled: false,
                })
            }
            BundleFileKind::Managed {
                provider: ProviderId::Modrinth,
                sha1,
                ..
            } => Some(Candidate {
                name: file.display_name(),
                identity: FileIdentity::from_sha1(sha1),
                bundled: false,
            }),
            _ => None,
        });
    let elsewhere = contents
        .loose
        .iter()
        .filter(|file| is_jar(&file.path) && !is_curseforge_url(&file.url))
        .map(|file| Candidate {
            name: file.path.clone(),
            identity: FileIdentity::from_sha1(&file.sha1),
            bundled: false,
        });
    mods.chain(elsewhere).collect()
}

async fn bundled_jars(
    archive_path: &Path,
    manifest: &ModpackManifest,
) -> ContentResult<Vec<Candidate>> {
    let prefixes: Vec<&str> = manifest
        .override_prefixes
        .iter()
        .map(String::as_str)
        .collect();
    let mut layers = OverrideLayers::open(archive_path, &prefixes).await?;
    let entries: Vec<(String, String)> = layers
        .entries()
        .iter()
        .filter(|(rel, _)| tracked_content_type(rel) == Some(ContentType::Mod) || is_jar(rel))
        .cloned()
        .collect();

    let mut jars = Vec::new();
    for (rel, entry) in entries {
        let Some(bytes) = layers.read(&entry).await else {
            continue;
        };
        let name = match tracked_content_type(&rel) {
            Some(ContentType::Mod) => rel.rsplit('/').next().unwrap_or(&rel).to_string(),
            _ => rel.clone(),
        };
        let identity = tokio::task::spawn_blocking(move || FileIdentity {
            sha1: polyio::normalize_hash(&polyio::sha1_bytes(&bytes)),
            cf_fingerprint: Some(curseforge_fingerprint(&bytes)),
        })
        .await
        .map_err(std::io::Error::other)?;
        jars.push(Candidate {
            name,
            identity,
            bundled: true,
        });
    }
    Ok(jars)
}

fn is_jar(path: &str) -> bool {
    let path = path.to_ascii_lowercase();
    path.strip_suffix(".disabled")
        .unwrap_or(&path)
        .ends_with(".jar")
}

fn is_curseforge_url(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|url| {
        url.host_str().is_some_and(|host| {
            CURSEFORGE_HOSTS
                .iter()
                .any(|known| host == *known || host.ends_with(&format!(".{known}")))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundles::{BundleFile, BundleFileType};
    use crate::modpacks::ModpackContents;
    use crate::packages::types::ExternalFile;
    use oneclient_common::domain::{GameLoader, ProviderId};

    fn file(path: &str, kind: BundleFileKind) -> BundleFile {
        BundleFile {
            enabled: true,
            hidden: false,
            path: path.to_string(),
            size: 0,
            file_type: BundleFileType::Normal,
            kind,
        }
    }

    fn external(path: &str, url: &str, sha1: &str) -> BundleFile {
        file(
            path,
            BundleFileKind::External {
                file: ExternalFile {
                    name: path.rsplit('/').next().unwrap().to_string(),
                    url: url.to_string(),
                    sha1: sha1.to_string(),
                    size: 0,
                    content_type: if path.starts_with("mods/") {
                        ContentType::Mod
                    } else {
                        ContentType::ResourcePack
                    },
                },
                id: None,
                meta: None,
            },
        )
    }

    #[test]
    fn every_mod_not_from_curseforge_is_checked() {
        let manifest = ModpackManifest {
            name: "Pack".to_string(),
            version: "1.0".to_string(),
            summary: None,
            mc_version: "1.21.1".to_string(),
            loader: GameLoader::Fabric,
            loader_version: None,
            contents: ModpackContents {
                files: vec![
                    file(
                        "mods/sodium.jar",
                        BundleFileKind::Managed {
                            provider: ProviderId::Modrinth,
                            project_id: "AANobbMI".to_string(),
                            version_id: "v1".to_string(),
                            sha1: "aaa".to_string(),
                        },
                    ),
                    file(
                        "mods/jei.jar",
                        BundleFileKind::Managed {
                            provider: ProviderId::CurseForge,
                            project_id: "238222".to_string(),
                            version_id: "1".to_string(),
                            sha1: "eee".to_string(),
                        },
                    ),
                    external(
                        "mods/github.jar",
                        "https://github.com/a/b/releases/x.jar",
                        "bbb",
                    ),
                    external(
                        "mods/forge.jar",
                        "https://edge.forgecdn.net/files/1/2/forge.jar",
                        "ccc",
                    ),
                    external(
                        "resourcepacks/pack.zip",
                        "https://example.invalid/pack.zip",
                        "ddd",
                    ),
                ],
                loose: vec![
                    loose("config/loader/hook.jar", "https://example.invalid/hook.jar"),
                    loose(
                        "config/settings.json",
                        "https://example.invalid/settings.json",
                    ),
                    loose(
                        "config/cf.jar",
                        "https://edge.forgecdn.net/files/1/2/cf.jar",
                    ),
                ],
                ..ModpackContents::default()
            },
            override_prefixes: Vec::new(),
        };

        let found = downloaded_jars(&manifest);
        let names: Vec<&str> = found.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            ["sodium.jar", "github.jar", "config/loader/hook.jar"]
        );
        assert_eq!(found[0].identity.sha1, "aaa");
        assert_eq!(found[1].identity.sha1, "bbb");
        assert!(found.iter().all(|candidate| !candidate.bundled));
    }

    fn loose(path: &str, url: &str) -> crate::modpacks::LooseFile {
        crate::modpacks::LooseFile {
            path: path.to_string(),
            url: url.to_string(),
            sha1: format!("sha-{path}"),
            size: 0,
        }
    }

    #[test]
    fn mods_are_only_called_unlisted_when_every_site_answered() {
        let candidates = || {
            ["known", "packed", "linked"]
                .into_iter()
                .map(|name| Candidate {
                    name: format!("{name}.jar"),
                    identity: FileIdentity::from_sha1(name),
                    bundled: name == "packed",
                })
                .collect::<Vec<_>>()
        };
        let known: HashSet<String> = ["known".to_string()].into_iter().collect();

        let complete = report(candidates(), &known, true);
        assert_eq!(complete.unconfirmed, 0);
        assert_eq!(
            complete.unlisted,
            [
                UnlistedMod {
                    name: "packed.jar".to_string(),
                    sha1: "packed".to_string(),
                    bundled: true,
                },
                UnlistedMod {
                    name: "linked.jar".to_string(),
                    sha1: "linked".to_string(),
                    bundled: false,
                },
            ]
        );

        let partial = report(candidates(), &known, false);
        assert!(partial.unlisted.is_empty());
        assert_eq!(partial.unconfirmed, 2);
    }

    #[test]
    fn jars_are_recognised_even_when_disabled() {
        assert!(is_jar("mods/a.jar"));
        assert!(is_jar("mods/1.20.1/A.JAR"));
        assert!(is_jar("config/a.jar.disabled"));
        assert!(!is_jar("mods/a.zip"));
        assert!(!is_jar("config/jar.json"));
    }

    #[test]
    fn curseforge_hosts_are_recognised() {
        assert!(is_curseforge_url(
            "https://edge.forgecdn.net/files/1/2/a.jar"
        ));
        assert!(is_curseforge_url(
            "https://mediafilez.forgecdn.net/files/1/2/a.jar"
        ));
        assert!(is_curseforge_url("https://www.curseforge.com/api/v1/a.jar"));
        assert!(!is_curseforge_url("https://notforgecdn.net/a.jar"));
        assert!(!is_curseforge_url("https://github.com/a/b/a.jar"));
        assert!(!is_curseforge_url("not a url"));
    }
}
