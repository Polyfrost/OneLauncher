use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use oneclient_common::{domain::ContentType, paths};
use oneclient_db::dao::artifact as artifacts;
use oneclient_db::models::{ClusterArtifactRow, ClusterRow};

use super::{PackageStore, artifact_absolute_path, manifest};
use crate::ctx::ContentCtx;
use crate::error::{ContentError, ContentResult};
use crate::packages::JarDependencies;
use crate::packages::local_manifest::verified_jar_dependencies;

/// A mod's jar may declare requirements absent from its provider records.
/// Check surviving mods and keep the candidates if their safety is unverified.
pub async fn local_mods_may_need(
    cluster_id: i64,
    candidate_mods: &HashSet<&str>,
    ctx: &ContentCtx,
) -> ContentResult<bool> {
    let mut check = LocalDependencyCheck {
        supplied: HashSet::new(),
        candidates_identified: true,
    };
    for hash in candidate_mods {
        let metadata = cached_jar_dependencies(hash, ctx).await?;
        if let Some(metadata) = metadata.filter(|metadata| !metadata.provides.is_empty()) {
            check
                .supplied
                .extend(metadata.provides.into_iter().chain(metadata.bundled));
        } else {
            check.candidates_identified = false;
        }
    }
    let mut checked: HashSet<_> = candidate_mods
        .iter()
        .map(|hash| (*hash).to_owned())
        .collect();
    for link in artifacts::list_cluster_artifacts_detailed(&ctx.db, cluster_id).await? {
        if link.content_type == ContentType::Mod as i64
            && checked.insert(link.hash.clone())
            && cached_jar_dependencies(&link.hash, ctx)
                .await?
                .is_none_or(|metadata| check.could_need(&metadata))
        {
            return Ok(true);
        }
    }

    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    let personal = paths::cluster_mods_dir(&cluster.folder_name)?;
    let game = paths::cluster_game_dir(&cluster.folder_name, cluster.is_isolated())?;
    if has_unknown_jar(&personal, &checked, &check, None).await? {
        return Ok(true);
    }
    let shared = game.join(ContentType::Mod.folder_name());
    if shared == personal {
        return Ok(false);
    }

    let other_session = manifest::load(&game, manifest::MANIFEST_NAME)
        .await
        .filter(|session| session.cluster_id != cluster_id);
    has_unknown_jar(&shared, &checked, &check, other_session.as_ref()).await
}

async fn cached_jar_dependencies(
    hash: &str,
    ctx: &ContentCtx,
) -> ContentResult<Option<JarDependencies>> {
    let Some(row) = artifacts::get_artifact_by_hash(&ctx.db, hash).await? else {
        return Ok(None);
    };
    let path = artifact_absolute_path(&row.path)?;
    if !polyio::sha1_file(&path)
        .await
        .is_ok_and(|actual| polyio::normalize_hash(&actual) == hash)
    {
        return Ok(None);
    }
    Ok(verified_jar_dependencies(&path).await)
}

struct LocalDependencyCheck {
    supplied: HashSet<String>,
    candidates_identified: bool,
}

impl LocalDependencyCheck {
    fn could_need(&self, metadata: &JarDependencies) -> bool {
        metadata.provides.is_empty()
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
    checked: &HashSet<String>,
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
            if checked.contains(&hash) {
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

const UNDO_SUFFIX: &str = ".oneclient-undo";

/// `.<name>.oneclient-undo` so a forced exit leaves a name recovery can reverse
fn staging_path(original: &Path) -> PathBuf {
    let mut name = std::ffi::OsString::from(".");
    name.push(original.file_name().unwrap_or_default());
    name.push(UNDO_SUFFIX);
    original.with_file_name(name)
}

/// An exit between staging and cleanup strands staged files. A file whose link
/// survived goes back, the rest belong to an Undo the database already committed.
pub async fn recover_staged_undo_files(ctx: &ContentCtx) -> ContentResult<()> {
    let _mutation = crate::packages::exclusive_package_mutation_guard().await;
    let _manifest = manifest::lock().await;
    // Folders can be shared, so each one knows the names linked by any cluster using it
    let mut dirs: HashMap<PathBuf, HashSet<String>> = HashMap::new();
    for cluster in oneclient_db::dao::cluster::list_all(&ctx.db).await? {
        let linked: Vec<String> = artifacts::list_cluster_artifacts_detailed(&ctx.db, cluster.id)
            .await?
            .into_iter()
            .map(|link| link.cluster_file_name)
            .collect();
        let name = &cluster.folder_name;
        let locations = [
            paths::cluster_dir(name).ok(),
            paths::cluster_game_dir(name, cluster.is_isolated()).ok(),
            paths::cluster_shared_dir(name).ok(),
        ];
        for location in locations.into_iter().flatten() {
            for content_type in [
                ContentType::Mod,
                ContentType::ResourcePack,
                ContentType::Shader,
                ContentType::DataPack,
            ] {
                dirs.entry(location.join(content_type.folder_name()))
                    .or_default()
                    .extend(linked.iter().cloned());
            }
        }
    }
    for (dir, linked) in dirs {
        recover_staged_in(&dir, &linked).await;
    }
    Ok(())
}

async fn recover_staged_in(dir: &Path, linked: &HashSet<String>) {
    let Ok(mut entries) = polyio::read_dir(dir).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let file_name = entry.file_name();
        let Some(name) = file_name
            .to_str()
            .and_then(|name| name.strip_prefix('.')?.strip_suffix(UNDO_SUFFIX))
        else {
            continue;
        };
        let staged = entry.path();
        let original = dir.join(name);
        let result = match (linked.contains(name), polyio::try_exists(&original).await) {
            (true, Ok(false)) => polyio::rename(&staged, &original).await,
            // Rollback keeps these on purpose when the original path was taken
            (true, _) | (_, Err(_)) => continue,
            // The cache keeps its own copy, so a committed Undo's leftover is disposable
            (false, Ok(_)) => polyio::remove_file(&staged).await,
        };
        if let Err(err) = result {
            tracing::warn!(path = %staged.display(), %err, "could not recover a staged Undo file");
        }
    }
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
        let staged = staging_path(&original);
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

    #[tokio::test]
    async fn remote_mods_can_require_an_undo_candidate_in_their_jar() {
        check_linked_mod_requirements(false, true).await;
    }

    #[tokio::test]
    async fn unrelated_nested_library_allows_undo() {
        check_linked_mod_requirements(true, false).await;
    }

    #[tokio::test]
    async fn nested_library_requirement_blocks_undo() {
        check_linked_mod_requirements(true, true).await;
    }

    async fn jar_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zip = async_zip::base::write::ZipFileWriter::new(Vec::new());
        for &(name, contents) in entries {
            let entry =
                async_zip::ZipEntryBuilder::new(name.into(), async_zip::Compression::Stored);
            zip.write_entry_whole(entry, contents).await.unwrap();
        }
        zip.close().await.unwrap()
    }

    async fn check_linked_mod_requirements(nested: bool, requires_candidate: bool) {
        use oneclient_common::domain::{GameLoader, ProviderId};
        use oneclient_db::models::{ClusterKind, NewCluster};

        let dir = polyio::tempdir().await.unwrap();
        let db = oneclient_db::connect(dir.dir_path().join("test.db"))
            .await
            .unwrap();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let ctx = ContentCtx::new(
            db,
            oneclient_net::RequestClient::new(oneclient_net::NetConfig::default()).unwrap(),
            oneclient_events::EventBus::new(tx),
            crate::packages::PackageProviderRegistry::new(),
            Default::default(),
        );
        let cluster = oneclient_db::dao::cluster::insert(
            &ctx.db,
            &NewCluster {
                name: "Undo manifest test",
                folder_name: dir.dir_path().file_name().unwrap().to_str().unwrap(),
                mc_version: "1.21.1",
                mc_loader: GameLoader::Fabric as i64,
                mc_loader_version: None,
                setting_profile_name: None,
                stage: 0,
                kind: ClusterKind::Modded as i64,
                user_created: 1,
                description: None,
                tags: "[]",
                cover_path: None,
            },
        )
        .await
        .unwrap();
        let dependent_manifest: &[u8] = if requires_candidate {
            br#"{"id":"dependent","depends":{"library":"*"}}"#
        } else {
            br#"{"id":"dependent","depends":{"minecraft":"*"}}"#
        };
        let dependent = jar_bytes(&[("fabric.mod.json", dependent_manifest)]).await;
        let dependent = if nested {
            let wrapper = jar_bytes(&[
                (
                    "fabric.mod.json",
                    br#"{"id":"wrapper","jars":[{"file":"META-INF/jars/dependent.jar"}]}"#,
                ),
                ("META-INF/jars/dependent.jar", &dependent),
            ])
            .await;
            jar_bytes(&[
                (
                    "fabric.mod.json",
                    br#"{"id":"outer","jars":[{"file":"vendor/wrapper.jar"}]}"#,
                ),
                ("vendor/wrapper.jar", &wrapper),
            ])
            .await
        } else {
            dependent
        };
        let mut hashes = Vec::new();
        for (id, bytes) in [
            (
                "library",
                jar_bytes(&[("fabric.mod.json", br#"{"id":"library"}"#)]).await,
            ),
            ("dependent", dependent),
        ] {
            let file_name = format!("{id}.jar");
            let path = dir.dir_path().join(&file_name);
            tokio::fs::write(&path, bytes).await.unwrap();
            let hash = polyio::normalize_hash(&polyio::sha1_file(&path).await.unwrap());
            artifacts::insert_artifact(
                &ctx.db,
                &hash,
                ContentType::Mod as i64,
                path.to_str().unwrap(),
                &file_name,
                None,
            )
            .await
            .unwrap();
            artifacts::upsert_provider_release(
                &ctx.db,
                ProviderId::Modrinth as i64,
                id,
                "v1",
                &hash,
                "",
                "",
                None,
                "[]",
                "[]",
            )
            .await
            .unwrap();
            artifacts::link_cluster_artifact(&ctx.db, cluster.id, &hash, &file_name)
                .await
                .unwrap();
            hashes.push(hash);
        }

        let candidates = HashSet::from([hashes[0].as_str()]);
        assert!(
            crate::packages::dependents::removal_blockers(cluster.id, &candidates, &ctx)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            local_mods_may_need(cluster.id, &candidates, &ctx)
                .await
                .unwrap(),
            requires_candidate
        );

        // If the cached dependency manifest can't be read, Undo must also keep the library.
        tokio::fs::remove_file(dir.dir_path().join("dependent.jar"))
            .await
            .unwrap();
        assert!(
            local_mods_may_need(cluster.id, &candidates, &ctx)
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

    #[tokio::test]
    async fn recovery_restores_linked_leftovers_and_drops_committed_ones() {
        let dir = polyio::testing::ScratchDir::new("undo_recovery");
        polyio::create_dir_all(dir.path()).await.unwrap();
        let kept = dir.path().join("kept.jar");
        let undone = dir.path().join("undone.jar");
        let taken = dir.path().join("taken.jar");
        for path in [&kept, &undone, &taken] {
            polyio::write(staging_path(path), b"package").await.unwrap();
        }
        polyio::write(&taken, b"other").await.unwrap();

        let linked = HashSet::from(["kept.jar".into(), "taken.jar".into()]);
        recover_staged_in(dir.path(), &linked).await;

        assert!(polyio::try_exists(staging_path(&taken)).await.unwrap());
        assert!(polyio::try_exists(&kept).await.unwrap());
        assert!(!polyio::try_exists(&undone).await.unwrap());
        assert!(!polyio::try_exists(staging_path(&kept)).await.unwrap());
        assert!(!polyio::try_exists(staging_path(&undone)).await.unwrap());
    }
}
