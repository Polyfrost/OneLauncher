use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use oneclient_common::{
    domain::{ContentType, ProviderId},
    paths,
};
use oneclient_db::dao::artifact as artifacts;
use oneclient_db::models::{ClusterArtifactRow, ClusterRow};

use super::{PackageStore, artifact_absolute_path, manifest};
use crate::ctx::ContentCtx;
use crate::error::{ContentError, ContentResult};
use crate::packages::JarDependencies;
use crate::packages::local_manifest::verified_jar_dependencies;

/// A manually installed or unsynced jar may declare requirements that have no
/// provider records. Use the jar manifests if they can prove it is unrelated.
pub async fn local_mods_may_need(
    cluster_id: i64,
    candidate_mods: &HashSet<&str>,
    ctx: &ContentCtx,
) -> ContentResult<bool> {
    let known: HashSet<_> = artifacts::list_cluster_release_identities(&ctx.db, cluster_id)
        .await?
        .into_iter()
        .filter(|id| {
            ProviderId::REMOTE_PROVIDERS
                .iter()
                .any(|provider| *provider as i64 == id.provider)
        })
        .map(|id| id.hash)
        .collect();

    let mut check = LocalDependencyCheck {
        supplied: HashSet::new(),
        candidates_identified: true,
    };
    for hash in candidate_mods {
        let metadata = match artifacts::get_artifact_by_hash(&ctx.db, hash).await? {
            Some(row) => verified_jar_dependencies(&artifact_absolute_path(&row.path)?).await,
            None => None,
        };
        if let Some(metadata) = metadata.filter(|metadata| !metadata.provides.is_empty()) {
            check
                .supplied
                .extend(metadata.provides.into_iter().chain(metadata.bundled));
        } else {
            check.candidates_identified = false;
        }
    }
    for link in PackageStore::list_linked_artifacts(cluster_id, ctx).await? {
        if link.content_type == ContentType::Mod && !known.contains(&link.hash) {
            let Some(row) = artifacts::get_artifact_by_hash(&ctx.db, &link.hash).await? else {
                return Ok(true);
            };
            let path = artifact_absolute_path(&row.path)?;
            if verified_jar_dependencies(&path)
                .await
                .is_none_or(|metadata| check.could_need(&metadata))
            {
                return Ok(true);
            }
        }
    }

    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    let personal = paths::cluster_mods_dir(&cluster.folder_name)?;
    let game = paths::cluster_game_dir(&cluster.folder_name, cluster.is_isolated())?;
    if has_unknown_jar(&personal, &known, &check, None).await? {
        return Ok(true);
    }
    let shared = game.join(ContentType::Mod.folder_name());
    if shared == personal {
        return Ok(false);
    }

    let other_session = manifest::load(&game, manifest::MANIFEST_NAME)
        .await
        .filter(|session| session.cluster_id != cluster_id);
    has_unknown_jar(&shared, &known, &check, other_session.as_ref()).await
}

struct LocalDependencyCheck {
    supplied: HashSet<String>,
    candidates_identified: bool,
}

impl LocalDependencyCheck {
    fn could_need(&self, metadata: &JarDependencies) -> bool {
        // Nested jars' requirements aren't all represented by the parent metadata.
        metadata.provides.is_empty()
            || !metadata.bundled.is_empty()
            || metadata.requires.iter().any(|id| {
                self.supplied.contains(id)
                    || (!self.candidates_identified
                        && !matches!(
                            id.as_str(),
                            "java"
                                | "minecraft"
                                | "fabricloader"
                                | "quilt_loader"
                                | "forge"
                                | "neoforge"
                        ))
            })
    }
}

async fn has_unknown_jar(
    root: &Path,
    known: &HashSet<String>,
    check: &LocalDependencyCheck,
    other_session: Option<&manifest::MaterializedManifest>,
) -> ContentResult<bool> {
    let mut directories = vec![root.to_path_buf()];
    while let Some(dir) = directories.pop() {
        if !polyio::try_exists(&dir).await? {
            continue;
        }
        let mut entries = polyio::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await.map_err(polyio::IOError::from)? {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }

            let path = entry.path();
            let kind = entry.file_type().await.map_err(polyio::IOError::from)?;
            if kind.is_dir() {
                directories.push(path);
                continue;
            }
            let name = name.to_ascii_lowercase();
            if !name.ends_with(".jar") && !name.ends_with(".jar.disabled") {
                continue;
            }

            let hash = polyio::normalize_hash(&polyio::sha1_file(&path).await?);
            if known.contains(&hash) {
                continue;
            }
            let relative = path.strip_prefix(root).ok().map(|path| {
                manifest::entry_path(
                    ContentType::Mod.folder_name(),
                    &path.to_string_lossy().replace('\\', "/"),
                )
            });
            if other_session.is_some_and(|session| {
                session.entries.iter().any(|entry| {
                    Some(entry.path.as_str()) == relative.as_deref() && entry.hash == hash
                })
            }) {
                continue;
            }
            if verified_jar_dependencies(&path)
                .await
                .is_none_or(|metadata| check.could_need(&metadata))
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

type DeployedPackageHashes = HashMap<PathBuf, String>;

/// Checks every filename the relevant content scanners or game may load.
async fn deployed_package_hashes(
    location: &Path,
    content_type: ContentType,
) -> ContentResult<DeployedPackageHashes> {
    let mut files = HashMap::new();
    let mut directories = vec![location.join(content_type.folder_name())];
    while let Some(dir) = directories.pop() {
        if !polyio::try_exists(&dir).await? {
            continue;
        }
        let mut entries = polyio::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await.map_err(polyio::IOError::from)? {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            let kind = entry.file_type().await.map_err(polyio::IOError::from)?;
            if kind.is_dir() {
                if content_type == ContentType::Mod {
                    directories.push(path);
                }
                continue;
            }
            let name = name.to_ascii_lowercase();
            let recognized = match content_type {
                ContentType::Mod => name.ends_with(".jar") || name.ends_with(".jar.disabled"),
                ContentType::ResourcePack | ContentType::Shader => name.ends_with(".zip"),
                _ => false,
            };
            if recognized {
                let hash = polyio::normalize_hash(&polyio::sha1_file(&path).await?);
                files.insert(path, hash);
            }
        }
    }
    Ok(files)
}

fn recorded_link(
    value: Option<&manifest::MaterializedManifest>,
    relative: &str,
    hash: &str,
) -> Option<i64> {
    value
        .filter(|value| {
            value
                .entries
                .iter()
                .any(|entry| entry.path == relative && entry.hash == hash)
        })
        .map(|value| value.cluster_id)
}

fn has_unexpected_copy(
    files: &DeployedPackageHashes,
    original: &Path,
    hash: &str,
    location: &Path,
    another_session: Option<&manifest::MaterializedManifest>,
) -> bool {
    files.iter().any(|(path, found_hash)| {
        if found_hash != hash || path == original {
            return false;
        }
        let relative = path
            .strip_prefix(location)
            .ok()
            .map(|path| path.to_string_lossy().replace('\\', "/"));
        relative.is_none_or(|relative| recorded_link(another_session, &relative, hash).is_none())
    })
}

struct StagedFile {
    original: PathBuf,
    staged: PathBuf,
    hash: String,
    location: MaterializedFile,
}

struct MaterializedFile {
    root: PathBuf,
    manifest: &'static str,
    relative: String,
}

/// Holds moves until the database has removed the entire group. Staging names
/// are hidden and have no package extension, so a retained file can't be
/// imported by a folder scan after a commit.
#[derive(Default)]
pub(crate) struct StagedUndoFiles {
    files: Vec<StagedFile>,
    deployed: HashMap<(PathBuf, ContentType), DeployedPackageHashes>,
}

impl StagedUndoFiles {
    pub(crate) async fn stage(
        &mut self,
        cluster: &ClusterRow,
        link: &ClusterArtifactRow,
        content_type: ContentType,
        ctx: &ContentCtx,
    ) -> ContentResult<()> {
        use super::link::{materialized_root, session_owns, shares_content};

        if !oneclient_common::paths::is_shared_group_name(&link.cluster_file_name) {
            return Err(ContentError::InvalidData {
                reason: "The package file name is not a single path component".into(),
            });
        }
        let personal = paths::cluster_dir(&cluster.folder_name)?;
        let game = paths::cluster_game_dir(&cluster.folder_name, cluster.is_isolated())?;
        let (root, manifest_name) =
            materialized_root(cluster, content_type)
                .await
                .ok_or_else(|| ContentError::InvalidData {
                    reason: "Could not locate the instance's content folder".into(),
                })?;
        let relative = manifest::entry_path(content_type.folder_name(), &link.cluster_file_name);
        let active = !content_type.reloads_in_game() && session_owns(cluster, &ctx.db).await;

        let shared_game = (personal != game).then(|| game.clone());
        let globally_shared = shares_content(cluster, content_type);
        let mut seen_locations = HashSet::new();
        for (location, manifest_name) in [
            (root.clone(), manifest_name),
            (personal, manifest::MODS_MANIFEST_NAME),
            (game, manifest::MANIFEST_NAME),
        ] {
            if !seen_locations.insert(location.clone()) {
                continue;
            }
            let original = location.join(&relative);
            let owner = manifest::load(&location, manifest_name).await;
            let another_session = owner.as_ref().filter(|value| {
                !globally_shared
                    && manifest_name == manifest::MANIFEST_NAME
                    && shared_game.as_deref() == Some(location.as_path())
                    && value.cluster_id != cluster.id
            });

            let key = (location.clone(), content_type);
            if let std::collections::hash_map::Entry::Vacant(entry) =
                self.deployed.entry(key.clone())
            {
                entry.insert(deployed_package_hashes(&location, content_type).await?);
            }
            if has_unexpected_copy(
                &self.deployed[&key],
                &original,
                &link.hash,
                &location,
                another_session,
            ) {
                return Err(ContentError::InvalidData {
                    reason: format!(
                        "A copy of {} was found at another file name",
                        link.cluster_file_name
                    ),
                });
            }

            if polyio::symlink_metadata(&original).await.is_err() {
                // A permission error must fail the Undo. Only an absent entry is safe to skip.
                if !polyio::try_exists(&original).await? {
                    continue;
                }
            }
            let file = polyio::stat(&original).await?;
            if !file.is_file()
                || polyio::normalize_hash(&polyio::sha1_file(&original).await?) != link.hash
            {
                continue;
            }
            if recorded_link(another_session, &relative, &link.hash).is_some() {
                continue;
            }

            let owned = recorded_link(owner.as_ref(), &relative, &link.hash)
                .is_some_and(|id| id == cluster.id || (location == root && globally_shared));
            if active || !owned {
                return Err(ContentError::InvalidData {
                    reason: format!(
                        "{} is in use or its file ownership could not be verified",
                        link.cluster_file_name
                    ),
                });
            }

            self.stage_file(
                original,
                link.hash.clone(),
                MaterializedFile {
                    root: location,
                    manifest: manifest_name,
                    relative: relative.clone(),
                },
            )
            .await?;
        }

        Ok(())
    }

    async fn stage_file(
        &mut self,
        original: PathBuf,
        hash: String,
        location: MaterializedFile,
    ) -> ContentResult<()> {
        let staging_name = format!(".oneclient-undo-{}", uuid::Uuid::new_v4().simple());
        let staged = original.with_file_name(staging_name);
        polyio::rename(&original, &staged).await?;
        self.files.push(StagedFile {
            original,
            staged,
            hash,
            location,
        });
        Ok(())
    }

    /// The database is still unchanged. Keep the cached copy and try to put
    /// every file back even if one location has since been taken externally.
    pub(crate) async fn rollback(self) {
        for file in self.files.into_iter().rev() {
            match polyio::try_exists(&file.original).await {
                Ok(false) => {
                    if let Err(err) = polyio::rename(&file.staged, &file.original).await {
                        tracing::warn!(path = %file.staged.display(), %err, "could not restore a staged Undo file");
                    }
                }
                _ => {
                    tracing::warn!(path = %file.staged.display(), "kept the staged Undo file because its original path is unavailable")
                }
            }
        }
    }

    /// The original package paths are already gone. This only clears stale
    /// manifest records and disposable copies, neither of which can reimport.
    pub(crate) async fn commit(self) {
        for file in self.files {
            let location = file.location;
            if let Some(mut current) = manifest::load(&location.root, location.manifest).await {
                current
                    .entries
                    .retain(|entry| entry.path != location.relative || entry.hash != file.hash);
                manifest::save(&location.root, location.manifest, &current).await;
            }
            if let Err(err) = polyio::remove_file(&file.staged).await {
                tracing::warn!(path = %file.staged.display(), %err, "could not clean up a staged Undo file");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn undo_rejects_an_untracked_nested_jar_but_accepts_known_content() {
        let dir = polyio::testing::ScratchDir::new("undo_local_mods");
        let version = dir.path().join("1.21.1");
        polyio::create_dir_all(&version).await.unwrap();
        let jar = version.join("local.jar");
        polyio::write(&jar, b"local").await.unwrap();

        let check = LocalDependencyCheck {
            supplied: HashSet::from(["sodium".into()]),
            candidates_identified: true,
        };
        assert!(
            has_unknown_jar(dir.path(), &HashSet::new(), &check, None)
                .await
                .unwrap()
        );
        let hash = polyio::normalize_hash(&polyio::sha1_file(&jar).await.unwrap());
        assert!(
            !has_unknown_jar(dir.path(), &HashSet::from([hash]), &check, None)
                .await
                .unwrap()
        );
    }

    #[test]
    fn local_manifests_allow_undo_of_an_unrelated_known_mod() {
        let check = LocalDependencyCheck {
            supplied: HashSet::from(["sodium".into()]),
            candidates_identified: true,
        };
        let mut mod_info = JarDependencies {
            provides: vec!["local".into()],
            requires: vec!["fabric-api".into()],
            ..Default::default()
        };
        assert!(!check.could_need(&mod_info));
        mod_info.requires.push("sodium".into());
        assert!(check.could_need(&mod_info));
        assert!(check.could_need(&JarDependencies::default()));
    }

    #[tokio::test]
    async fn a_renamed_copy_blocks_staging_unless_owned_by_the_other_session() {
        let dir = polyio::testing::ScratchDir::new("undo_renamed_copy");
        let mods = dir.path().join("mods");
        polyio::create_dir_all(&mods).await.unwrap();
        let renamed = mods.join("renamed.jar");
        polyio::write(&renamed, b"same package").await.unwrap();
        let original = mods.join("original.jar");
        let hash = polyio::normalize_hash(&polyio::sha1_file(&renamed).await.unwrap());
        let files = deployed_package_hashes(dir.path(), ContentType::Mod)
            .await
            .unwrap();

        assert!(has_unexpected_copy(
            &files,
            &original,
            &hash,
            dir.path(),
            None
        ));
        assert!(!has_unexpected_copy(
            &files,
            &renamed,
            &hash,
            dir.path(),
            None
        ));

        let other_session = manifest::MaterializedManifest::new(
            2,
            vec![manifest::ManifestEntry {
                path: "mods/renamed.jar".into(),
                hash: hash.clone(),
            }],
        );
        assert!(!has_unexpected_copy(
            &files,
            &original,
            &hash,
            dir.path(),
            Some(&other_session),
        ));
        assert_eq!(
            recorded_link(Some(&other_session), "mods/renamed.jar", &hash),
            Some(2)
        );
        assert_eq!(
            recorded_link(Some(&other_session), "mods/renamed.jar", "different"),
            None
        );
        assert!(polyio::try_exists(&renamed).await.unwrap());
    }

    #[tokio::test]
    async fn staging_hides_the_package_and_rollback_restores_the_original() {
        let dir = polyio::testing::ScratchDir::new("undo_staged_files");
        let original = dir.path().join("original.jar");
        polyio::create_dir_all(dir.path()).await.unwrap();
        polyio::write(&original, b"installed package")
            .await
            .unwrap();
        let mut files = StagedUndoFiles::default();
        files
            .stage_file(
                original.clone(),
                "hash".into(),
                MaterializedFile {
                    root: dir.path().to_path_buf(),
                    manifest: manifest::MODS_MANIFEST_NAME,
                    relative: "original.jar".into(),
                },
            )
            .await
            .unwrap();

        assert!(!polyio::try_exists(&original).await.unwrap());
        let staged = files.files[0].staged.clone();
        let staging_name = staged.file_name().unwrap().to_string_lossy();
        assert!(staging_name.starts_with('.'));
        assert!(!staging_name.ends_with(".jar"));

        files.rollback().await;
        assert!(polyio::try_exists(&original).await.unwrap());
        assert!(!polyio::try_exists(&staged).await.unwrap());
    }
}
