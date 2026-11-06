#![recursion_limit = "256"]

use std::collections::HashMap;
use std::sync::Arc;

use oneclient_common::domain::{ContentType, GameLoader, ProviderId};
use oneclient_content::bundles::{
    BundleFile, BundleFileKind, BundleFileType, BundleManifest, check_bundle_updates,
    get_bundles_with_update_status, install_cluster_bundles, on_user_disable_artifact,
    on_user_remove_artifact, set_bundle_choices, set_bundle_package_enabled,
};
use oneclient_content::packages::store::artifact_absolute_path;
use oneclient_content::packages::{
    BrowserPackageUpdate, DependencyKind, FileIdentity, InstalledPackage, PackageBody,
    PackageProvider, PackageStore, Page, ProjectDetail, ProjectSummary, ReleaseType, SearchFilters,
    VersionDependency, VersionDetail, VersionFile, VersionLookup, VersionSummary,
    apply_browser_package_update, apply_staged_package_update, bundle_changes_left,
    cluster_wants_bundle_catalog, get_version_cached, refresh_all_browser_package_updates, refresh_browser_package_updates,
    resolve_required, stage_browser_package_update,
};
use oneclient_content::{ContentCtx, ContentError, ContentResult};
use oneclient_core::LauncherState;
use oneclient_core::clusters::CreateClusterOptions;
use oneclient_db::dao::cluster_flag::{self as flag_dao, ClusterFlag};
use oneclient_db::dao::{
    artifact as artifact_dao, cluster_bisect as bisect_dao,
    cluster_bundle as bundle_dao,
};
use oneclient_db::models::OverrideType;

const BUNDLE: &str = "Test Bundle";
const MC_VERSION: &str = "1.21.1";
const PROJECT_ID: &str = "sodium";
const VERSION_ID: &str = "v1";
const RESOLVED_VERSION_ID: &str = "AbC123";
const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SHARED_BUNDLE: &str = "Shared Bundle";
const FABRIC_API_HASH: &str = "dddddddddddddddddddddddddddddddddddddddd";
const HIDDEN_DEP_HASH: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const OTHER_PROJECT_ID: &str = "lithium";
const OTHER_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const OFFLINE_VERSION_ID: &str = "offline";
const NEEDS_MISSING_DEP_VERSION_ID: &str = "needsmissingdep";
const NEEDS_PINNED_DEP_VERSION_ID: &str = "needspinneddep";
const PINNED_DEP_VERSION_ID: &str = "pinneddep";
const NEEDS_LISTED_DEP_VERSION_ID: &str = "needslisteddep";
const LISTED_PROJECT_ID: &str = "listed";
const LISTED_VERSION_ID: &str = "listedversion";
const NEEDS_EXACT_DEP_VERSION_ID: &str = "needsexactdep";
const EXACT_DEP_VERSION_ID: &str = "exactdep";
const HANGING_VERSION_ID: &str = "hanging";
const NEEDS_INSTALLED_VERSION_ID: &str = "needsinstalled";
const NEEDS_VERSION_ONLY_DEP_VERSION_ID: &str = "needsversiononlydep";
const GONE_VERSION_ID: &str = "gone";
const SLOW_VERSION_ID: &str = "slow";
const HANGING_PROJECT_ID: &str = "hanging-project";
const OFFLINE_VERSION_NUMBER: &str = "offline.1";

fn managed_file(enabled: bool) -> BundleFile {
    BundleFile {
        enabled,
        hidden: false,
        path: "mods/sodium.jar".to_string(),
        size: 1,
        file_type: BundleFileType::Normal,
        kind: BundleFileKind::Managed {
            provider: ProviderId::Modrinth,
            project_id: PROJECT_ID.to_string(),
            version_id: VERSION_ID.to_string(),
            sha1: HASH.to_string(),
        },
    }
}

/// Added by the catalog after the last sync untracked so it has no override
fn newly_shipped_file() -> BundleFile {
    BundleFile {
        enabled: true,
        hidden: false,
        path: "mods/newcomer.jar".to_string(),
        size: 1,
        file_type: BundleFileType::Normal,
        kind: BundleFileKind::Managed {
            provider: ProviderId::Modrinth,
            project_id: "newcomer".to_string(),
            version_id: "v1".to_string(),
            sha1: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        },
    }
}

fn manifest(files: Vec<BundleFile>) -> BundleManifest {
    BundleManifest {
        name: BUNDLE.to_string(),
        version_id: "1".to_string(),
        category: "test".to_string(),
        mc_version: MC_VERSION.to_string(),
        loader: GameLoader::Fabric,
        loader_version: "0.16.0".to_string(),
        enabled: true,
        java_version_override: None,
        files,
    }
}

struct PinProvider(Option<String>, bool, Option<String>);

fn pin_version(project_id: &str, version_id: &str, file: Option<&str>) -> VersionDetail {
    VersionDetail {
        version_id: version_id.to_string(),
        project_id: project_id.to_string(),
        name: version_id.to_string(),
        version_number: version_id.to_string(),
        changelog: None,
        game_versions: vec![MC_VERSION.to_string()],
        loaders: vec![GameLoader::Fabric],
        published: "2026-01-01T00:00:00Z".parse().unwrap(),
        downloads: 0,
        files: file
            .into_iter()
            .map(|sha1| VersionFile {
                sha1: sha1.to_string(),
                url: "https://example.invalid/mod.jar".to_string(),
                file_name: "mod.jar".to_string(),
                primary: true,
                size: 1,
                fingerprint: None,
            })
            .collect(),
        dependencies: Vec::new(),
    }
}

#[async_trait::async_trait]
impl PackageProvider for PinProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Modrinth
    }

    async fn get_version(
        &self,
        project_id: &str,
        version_id: &str,
        _ctx: &ContentCtx,
    ) -> ContentResult<VersionDetail> {
        if version_id == OFFLINE_VERSION_ID || version_id == OFFLINE_VERSION_NUMBER {
            return Err(ContentError::InvalidData {
                reason: "provider unreachable".to_string(),
            });
        }

        if version_id == HANGING_VERSION_ID {
            std::future::pending::<()>().await;
        }

        if version_id == SLOW_VERSION_ID {
            tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
        }

        if version_id == GONE_VERSION_ID {
            return Err(oneclient_net::RequestError::HttpStatus {
                status: 404,
                url: "https://example.invalid/version".to_string(),
                snippet: String::new(),
            }
            .into());
        }

        let file = if version_id == PINNED_DEP_VERSION_ID {
            self.2.as_deref()
        } else {
            self.0.as_deref()
        };
        let mut version = pin_version(project_id, version_id, file);
        if version_id == NEEDS_MISSING_DEP_VERSION_ID {
            version.dependencies.push(VersionDependency {
                project_id: Some("missing".to_string()),
                version_id: Some(OFFLINE_VERSION_ID.to_string()),
                kind: DependencyKind::Required,
            });
        }
        if version_id == NEEDS_PINNED_DEP_VERSION_ID {
            version.dependencies.push(VersionDependency {
                project_id: Some(OTHER_PROJECT_ID.to_string()),
                version_id: None,
                kind: DependencyKind::Required,
            });
        }
        if version_id == NEEDS_LISTED_DEP_VERSION_ID {
            version.dependencies.push(VersionDependency {
                project_id: Some(LISTED_PROJECT_ID.to_string()),
                version_id: None,
                kind: DependencyKind::Required,
            });
        }
        if version_id == NEEDS_INSTALLED_VERSION_ID {
            version.dependencies.push(VersionDependency {
                project_id: Some(PROJECT_ID.to_string()),
                version_id: Some(VERSION_ID.to_string()),
                kind: DependencyKind::Required,
            });
        }
        if version_id == NEEDS_EXACT_DEP_VERSION_ID {
            version.dependencies.push(VersionDependency {
                project_id: Some(OTHER_PROJECT_ID.to_string()),
                version_id: Some(EXACT_DEP_VERSION_ID.to_string()),
                kind: DependencyKind::Required,
            });
        }
        if version_id == NEEDS_VERSION_ONLY_DEP_VERSION_ID {
            version.dependencies.push(VersionDependency {
                project_id: None,
                version_id: Some(EXACT_DEP_VERSION_ID.to_string()),
                kind: DependencyKind::Required,
            });
        }
        Ok(version)
    }

    async fn search(
        &self,
        _filters: &SearchFilters,
        _ctx: &ContentCtx,
    ) -> ContentResult<Page<ProjectSummary>> {
        unimplemented!()
    }

    async fn get_project(
        &self,
        project_id: &str,
        _ctx: &ContentCtx,
    ) -> ContentResult<ProjectDetail> {
        if self.1 {
            return Err(ContentError::InvalidData {
                reason: "provider unreachable".to_string(),
            });
        }

        let published = "2026-01-01T00:00:00Z".parse().unwrap();
        Ok(ProjectDetail {
            id: project_id.to_string(),
            slug: project_id.to_string(),
            provider: ProviderId::Modrinth,
            content_type: ContentType::Mod,
            name: project_id.to_string(),
            summary: String::new(),
            author: String::new(),
            members: Vec::new(),
            gallery: Vec::new(),
            body: PackageBody::raw(""),
            license: None,
            links: Vec::new(),
            version_ids: Vec::new(),
            game_versions: Vec::new(),
            loaders: Vec::new(),
            icon_url: None,
            created: published,
            updated: published,
            downloads: 0,
        })
    }

    async fn get_projects(
        &self,
        _project_ids: &[String],
        _ctx: &ContentCtx,
    ) -> ContentResult<Vec<ProjectDetail>> {
        unimplemented!()
    }

    async fn list_versions(
        &self,
        project_id: &str,
        _game_version: Option<&str>,
        _loader: Option<GameLoader>,
        _offset: usize,
        _limit: usize,
        _ctx: &ContentCtx,
    ) -> ContentResult<Page<VersionSummary>> {
        if project_id == HANGING_PROJECT_ID {
            std::future::pending::<()>().await;
        }

        let items = (project_id == LISTED_PROJECT_ID)
            .then(|| VersionSummary {
                version_id: LISTED_VERSION_ID.to_string(),
                project_id: project_id.to_string(),
                name: LISTED_VERSION_ID.to_string(),
                version_number: "1.0".to_string(),
                published: "2026-01-01T00:00:00Z".parse().unwrap(),
                release_type: ReleaseType::Release,
                game_versions: vec![MC_VERSION.to_string()],
                loaders: vec![GameLoader::Fabric],
                downloads: 0,
                file_size: 0,
                dependencies: Vec::new(),
            })
            .into_iter()
            .collect();

        Ok(Page {
            offset: 0,
            limit: 0,
            total: 0,
            items,
        })
    }

    async fn get_versions(
        &self,
        _version_ids: &[String],
        _ctx: &ContentCtx,
    ) -> ContentResult<Vec<VersionDetail>> {
        unimplemented!()
    }

    async fn lookup_versions(
        &self,
        identities: &[FileIdentity],
        _ctx: &ContentCtx,
    ) -> ContentResult<VersionLookup> {
        if self.1 {
            return Err(ContentError::InvalidData {
                reason: "provider unreachable".to_string(),
            });
        }

        Ok(identities
            .iter()
            .map(|identity| {
                let version = pin_version(PROJECT_ID, RESOLVED_VERSION_ID, None);
                (identity.sha1.clone(), version)
            })
            .collect())
    }

    async fn latest_for_game_version(
        &self,
        _packages: &[InstalledPackage],
        _mc_version: &str,
        _loader: GameLoader,
        _ctx: &ContentCtx,
    ) -> ContentResult<HashMap<String, VersionDetail>> {
        unimplemented!()
    }
}

const CURSEFORGE_VERSION_ID: &str = "12345";

struct CurseForgePin;

#[async_trait::async_trait]
impl PackageProvider for CurseForgePin {
    fn id(&self) -> ProviderId {
        ProviderId::CurseForge
    }

    async fn get_version(
        &self,
        project_id: &str,
        version_id: &str,
        _ctx: &ContentCtx,
    ) -> ContentResult<VersionDetail> {
        Ok(pin_version(project_id, version_id, None))
    }

    async fn search(
        &self,
        _filters: &SearchFilters,
        _ctx: &ContentCtx,
    ) -> ContentResult<Page<ProjectSummary>> {
        unimplemented!()
    }

    async fn get_project(
        &self,
        _project_id: &str,
        _ctx: &ContentCtx,
    ) -> ContentResult<ProjectDetail> {
        unimplemented!()
    }

    async fn get_projects(
        &self,
        _project_ids: &[String],
        _ctx: &ContentCtx,
    ) -> ContentResult<Vec<ProjectDetail>> {
        unimplemented!()
    }

    async fn list_versions(
        &self,
        _project_id: &str,
        _mc_version: Option<&str>,
        _loader: Option<GameLoader>,
        _offset: usize,
        _limit: usize,
        _ctx: &ContentCtx,
    ) -> ContentResult<Page<VersionSummary>> {
        unimplemented!()
    }

    async fn get_versions(
        &self,
        _version_ids: &[String],
        _ctx: &ContentCtx,
    ) -> ContentResult<Vec<VersionDetail>> {
        unimplemented!()
    }

    async fn lookup_versions(
        &self,
        identities: &[FileIdentity],
        _ctx: &ContentCtx,
    ) -> ContentResult<VersionLookup> {
        Ok(identities
            .iter()
            .map(|identity| {
                let version = pin_version(PROJECT_ID, CURSEFORGE_VERSION_ID, None);
                (identity.sha1.clone(), version)
            })
            .collect())
    }

    async fn latest_for_game_version(
        &self,
        _packages: &[InstalledPackage],
        _mc_version: &str,
        _loader: GameLoader,
        _ctx: &ContentCtx,
    ) -> ContentResult<HashMap<String, VersionDetail>> {
        unimplemented!()
    }
}

fn pinned_to(version_id: &str) -> BundleFile {
    BundleFile {
        kind: BundleFileKind::Managed {
            provider: ProviderId::Modrinth,
            project_id: PROJECT_ID.to_string(),
            version_id: version_id.to_string(),
            sha1: "cccccccccccccccccccccccccccccccccccccccc".to_string(),
        },
        ..managed_file(true)
    }
}

fn pin_content(state: &LauncherState) -> ContentCtx {
    let mut content = state.services.content();
    content
        .providers
        .register(Arc::new(PinProvider(None, false, None)));
    content
}

fn offline_pin_content(state: &LauncherState) -> ContentCtx {
    let mut content = state.services.content();
    content
        .providers
        .register(Arc::new(PinProvider(None, true, None)));
    content
}

async fn cluster_with_tracked_mod(state: &LauncherState) -> i64 {
    let cluster_id = cluster_with_plain_mod(state).await;

    bundle_dao::track_bundle_artifact(
        &state.services.db,
        cluster_id,
        HASH,
        BUNDLE,
        VERSION_ID,
        PROJECT_ID,
    )
    .await
    .unwrap();

    cluster_id
}

async fn empty_cluster(state: &LauncherState) -> i64 {
    let global = state.settings.read().global_game_settings.clone();
    state
        .clusters
        .create(
            &global,
            CreateClusterOptions::new("Bundle Cluster", MC_VERSION, GameLoader::Fabric),
        )
        .await
        .unwrap()
        .id
}

async fn cluster_with_plain_mod(state: &LauncherState) -> i64 {
    let cluster_id = empty_cluster(state).await;

    artifact_dao::insert_artifact(
        &state.services.db,
        HASH,
        ContentType::Mod as i64,
        "artifacts/sodium.jar",
        "sodium.jar",
        Some(1),
    )
    .await
    .unwrap();

    artifact_dao::link_cluster_artifact(&state.services.db, cluster_id, HASH, "sodium.jar")
        .await
        .unwrap();

    artifact_dao::upsert_provider_release(
        &state.services.db,
        ProviderId::Modrinth as i64,
        PROJECT_ID,
        VERSION_ID,
        HASH,
        "Sodium",
        "1.0.0",
        None,
        MC_VERSION,
        "fabric",
    )
    .await
    .unwrap();

    cluster_id
}

async fn check_with_installed_date(published_at: &str) -> oneclient_core::BrowserUpdateCheck {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(&state, manifest(vec![pinned_to("v0")]))
        .await
        .unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    artifact_dao::upsert_provider_release(
        &state.services.db,
        ProviderId::Modrinth as i64,
        PROJECT_ID,
        VERSION_ID,
        HASH,
        "Sodium",
        "1.0.0",
        Some(published_at),
        MC_VERSION,
        "fabric",
    )
    .await
    .unwrap();

    refresh_browser_package_updates(cluster_id, state.bundles.as_ref(), &pin_content(&state))
        .await
        .unwrap()
}

#[tokio::test]
async fn a_copy_newer_than_its_pin_stays() {
    let check = check_with_installed_date("2026-06-01T00:00:00Z").await;

    assert!(check.updates.is_empty(), "{:?}", check.updates);
}

#[tokio::test]
async fn a_copy_older_than_its_pin_moves_up() {
    let check = check_with_installed_date("2025-06-01T00:00:00Z").await;

    let bundled = check.bundled_pending();
    assert_eq!(bundled.len(), 1);
    assert_eq!(bundled[0].latest_version_id, "v0");
}

#[tokio::test]
async fn a_pin_reaches_a_copy_through_its_other_release() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_a_second_release(&state).await;
    let content = pin_content(&state);

    let check = refresh_browser_package_updates(cluster_id, state.bundles.as_ref(), &content)
        .await
        .unwrap();

    let bundled = check.bundled_pending();
    assert_eq!(bundled.len(), 1, "{:?}", check.updates);
    assert_eq!(bundled[0].project_id, PROJECT_ID);
    assert_eq!(bundled[0].installed_version_id, VERSION_ID);
}

async fn seed_unreadable_archive(state: &LauncherState, hidden: bool) {
    oneclient_db::dao::bundle::upsert_bundle(
        &state.services.db,
        oneclient_db::models::NewBundle {
            remote_path: "bundles/missing.mrpack",
            mc_version: MC_VERSION,
            mc_loader: GameLoader::Fabric as i64,
            file_name: "missing.mrpack",
            name: Some("Missing"),
            version_id: Some("1"),
            category: Some("test"),
            loader_version: Some("0.16.0"),
            disk_path: "bundles/missing.mrpack",
            hidden,
            etag: None,
            synced_at: None,
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn a_pin_by_version_number_resolves_through_its_hash() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(&state, manifest(vec![pinned_to("1.2.3")]))
        .await
        .unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;

    let check =
        refresh_browser_package_updates(cluster_id, state.bundles.as_ref(), &pin_content(&state))
            .await
            .unwrap();

    assert!(check.unchecked.is_empty(), "{:?}", check.unchecked);
    let bundled = check.bundled_pending();
    assert_eq!(bundled.len(), 1);
    assert_eq!(bundled[0].latest_version_id, RESOLVED_VERSION_ID);
}

#[tokio::test]
async fn a_copy_of_the_pinned_file_is_already_on_the_pin() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let pinned = BundleFile {
        kind: BundleFileKind::Managed {
            provider: ProviderId::Modrinth,
            project_id: PROJECT_ID.to_string(),
            version_id: "1.2.3".to_string(),
            sha1: HASH.to_string(),
        },
        ..managed_file(true)
    };
    oneclient_core::dev::seed_bundle_archive(&state, manifest(vec![pinned]))
        .await
        .unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;

    let check =
        refresh_browser_package_updates(cluster_id, state.bundles.as_ref(), &pin_content(&state))
            .await
            .unwrap();

    assert!(check.updates.is_empty(), "{:?}", check.updates);
}

async fn cached_file(
    state: &LauncherState,
    cluster_id: i64,
    bytes: &[u8],
) -> oneclient_db::models::ArtifactRow {
    let source = std::env::temp_dir().join(format!("{}.jar", uuid::Uuid::new_v4()));
    std::fs::write(&source, bytes).unwrap();
    let cached = PackageStore::import_local_file(
        &source,
        ContentType::Mod,
        cluster_id,
        &state.services.content(),
    )
    .await
    .unwrap();
    artifact_dao::unlink_cluster_artifact(&state.services.db, cluster_id, &cached.hash)
        .await
        .unwrap();
    cached
}

async fn content_with_dependency_files(
    state: &LauncherState,
    cluster_id: i64,
) -> (ContentCtx, oneclient_db::models::ArtifactRow) {
    let package = cached_file(state, cluster_id, b"sodium needs lithium").await;
    let dependency = cached_file(state, cluster_id, b"lithium on its pin").await;

    let mut content = state.services.content();
    content.providers.register(Arc::new(PinProvider(
        Some(package.hash),
        false,
        Some(dependency.hash.clone()),
    )));
    (content, dependency)
}

fn stage_update(cluster_id: i64) -> BrowserPackageUpdate {
    BrowserPackageUpdate {
        cluster_id,
        hash: HASH.to_string(),
        provider: ProviderId::Modrinth,
        project_id: PROJECT_ID.to_string(),
        installed_version_id: VERSION_ID.to_string(),
        installed_version_name: "1.0.0".to_string(),
        latest_version_id: "v2".to_string(),
        latest_version_name: "v2".to_string(),
        display_name: "Sodium".to_string(),
        skipped: false,
    }
}

#[tokio::test]
async fn a_failed_stage_leaves_the_copy_in_place() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    let content = pin_content(&state);

    stage_browser_package_update(
        &stage_update(cluster_id),
        state.bundles.as_ref(),
        None,
        &content,
    )
    .await
    .err()
    .unwrap();

    assert!(
        artifact_dao::get_cluster_artifact(&state.services.db, cluster_id, HASH)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn a_plain_update_waits_for_a_catalog_it_can_read() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    let (content, _) = content_with_dependency_files(&state, cluster_id).await;
    let update = stage_update(cluster_id);

    seed_unreadable_archive(&state, false).await;
    apply_browser_package_update(&update, state.bundles.as_ref(), None, &content)
        .await
        .unwrap_err();
    assert!(
        artifact_dao::get_cluster_artifact(&state.services.db, cluster_id, HASH)
            .await
            .unwrap()
            .is_some()
    );

    seed_unreadable_archive(&state, true).await;
    apply_browser_package_update(&update, state.bundles.as_ref(), None, &content)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_staged_update_replaces_the_copy_only_when_applied() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;

    let source = std::env::temp_dir().join(format!("{}.jar", uuid::Uuid::new_v4()));
    std::fs::write(&source, b"sodium v2").unwrap();
    let cached = PackageStore::import_local_file(
        &source,
        ContentType::Mod,
        cluster_id,
        &state.services.content(),
    )
    .await
    .unwrap();
    artifact_dao::unlink_cluster_artifact(&state.services.db, cluster_id, &cached.hash)
        .await
        .unwrap();

    let mut content = state.services.content();
    content.providers.register(Arc::new(PinProvider(
        Some(cached.hash.clone()),
        false,
        None,
    )));

    let staged = stage_browser_package_update(
        &stage_update(cluster_id),
        state.bundles.as_ref(),
        None,
        &content,
    )
    .await
    .unwrap();
    assert!(
        artifact_dao::get_cluster_artifact(&state.services.db, cluster_id, HASH)
            .await
            .unwrap()
            .is_some(),
        "staging must not touch the cluster"
    );

    let installed = apply_staged_package_update(&staged, &content)
        .await
        .unwrap();

    assert_eq!(installed, cached.hash);
    assert!(
        artifact_dao::get_cluster_artifact(&state.services.db, cluster_id, HASH)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        flag_dao::is_set(&state.services.db, cluster_id, ClusterFlag::PinsUnsettled)
            .await
            .unwrap(),
        "only the launch that saw every pin land can settle them"
    );
}

async fn flag_pins_unsettled(state: &LauncherState, cluster_id: i64) {
    flag_dao::set(
        &state.services.db,
        cluster_id,
        ClusterFlag::PinsUnsettled,
        None,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn unsettled_pins_wait_for_tracked_mods_behind_the_catalog() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(&state, manifest(vec![pinned_to("v2")]))
        .await
        .unwrap();
    let cluster_id = cluster_with_tracked_mod(&state).await;
    let content = pin_content(&state);
    let left = || bundle_changes_left(cluster_id, state.bundles.as_ref(), &content);

    assert!(left().await.unwrap().is_empty(), "no update has begun");

    flag_pins_unsettled(&state, cluster_id).await;
    assert_eq!(left().await.unwrap().len(), 1);

    bisect_dao::start(&state.services.db, cluster_id, &[])
        .await
        .unwrap();
    assert!(
        left().await.unwrap().is_empty(),
        "a bisect freezes the mods"
    );
}

#[tokio::test]
async fn unsettled_pins_ignore_a_bundle_the_cluster_never_took() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(&state, manifest(vec![managed_file(true)]))
        .await
        .unwrap();
    let cluster_id = empty_cluster(&state).await;
    flag_pins_unsettled(&state, cluster_id).await;

    assert!(
        bundle_changes_left(
            cluster_id,
            state.bundles.as_ref(),
            &state.services.content()
        )
        .await
        .unwrap()
        .is_empty()
    );
}

#[tokio::test]
async fn a_bundle_change_leaves_the_pins_unsettled() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(&state, manifest(vec![]))
        .await
        .unwrap();
    let cluster_id = cluster_with_tracked_mod(&state).await;
    let content = state.services.content();
    let unsettled = || flag_dao::is_set(&content.db, cluster_id, ClusterFlag::PinsUnsettled);

    let result = oneclient_content::bundles::apply_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &content,
        None,
    )
    .await
    .unwrap();
    assert_eq!(result.removals_applied.len(), 1);
    assert!(unsettled().await.unwrap());
}

#[tokio::test]
async fn a_plain_copy_wants_the_catalog_before_any_bundle_ships_it() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    let content = state.services.content();

    assert!(
        cluster_wants_bundle_catalog(cluster_id, &content)
            .await
            .unwrap(),
        "only a refreshed catalog can show a newly added pin"
    );

    artifact_dao::unlink_cluster_artifact(&state.services.db, cluster_id, HASH)
        .await
        .unwrap();
    assert!(
        !cluster_wants_bundle_catalog(cluster_id, &content)
            .await
            .unwrap()
    );
}

fn other_mod_pinned_to(version_id: &str) -> BundleFile {
    BundleFile {
        path: "mods/lithium.jar".to_string(),
        kind: BundleFileKind::Managed {
            provider: ProviderId::Modrinth,
            project_id: OTHER_PROJECT_ID.to_string(),
            version_id: version_id.to_string(),
            sha1: "ffffffffffffffffffffffffffffffffffffffff".to_string(),
        },
        ..managed_file(true)
    }
}

async fn add_other_plain_copy(state: &LauncherState, cluster_id: i64) {
    artifact_dao::insert_artifact(
        &state.services.db,
        OTHER_HASH,
        ContentType::Mod as i64,
        "artifacts/lithium.jar",
        "lithium.jar",
        Some(1),
    )
    .await
    .unwrap();

    artifact_dao::link_cluster_artifact(&state.services.db, cluster_id, OTHER_HASH, "lithium.jar")
        .await
        .unwrap();

    artifact_dao::upsert_provider_release(
        &state.services.db,
        ProviderId::Modrinth as i64,
        OTHER_PROJECT_ID,
        VERSION_ID,
        OTHER_HASH,
        "Lithium",
        "1.0.0",
        None,
        MC_VERSION,
        "fabric",
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn a_failed_pin_lookup_holds_the_other_pins_back() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![
            pinned_to("v2"),
            other_mod_pinned_to(OFFLINE_VERSION_ID),
        ]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    add_other_plain_copy(&state, cluster_id).await;

    let check =
        refresh_browser_package_updates(cluster_id, state.bundles.as_ref(), &pin_content(&state))
            .await
            .unwrap();

    assert_eq!(check.unchecked_pins, ["Lithium"]);
    assert!(check.bundled_pending().is_empty(), "{:?}", check.updates);
    assert!(check.pending().is_empty(), "{:?}", check.updates);
}

#[tokio::test]
async fn a_required_dependency_that_cannot_resolve_fails_the_stage() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    let content = pin_content(&state);
    let update = BrowserPackageUpdate {
        latest_version_id: NEEDS_MISSING_DEP_VERSION_ID.to_string(),
        ..stage_update(cluster_id)
    };

    let err = stage_browser_package_update(&update, state.bundles.as_ref(), None, &content)
        .await
        .err()
        .unwrap();

    assert!(err.to_string().contains("missing"), "{err}");
}

#[tokio::test]
async fn a_new_dependency_lands_on_the_bundle_pin() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![other_mod_pinned_to(PINNED_DEP_VERSION_ID)]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    let (content, dependency) = content_with_dependency_files(&state, cluster_id).await;
    let update = BrowserPackageUpdate {
        latest_version_id: NEEDS_PINNED_DEP_VERSION_ID.to_string(),
        ..stage_update(cluster_id)
    };

    let staged = stage_browser_package_update(&update, state.bundles.as_ref(), None, &content)
        .await
        .unwrap();
    apply_staged_package_update(&staged, &content)
        .await
        .unwrap();

    let release = artifact_dao::get_provider_release(
        &state.services.db,
        ProviderId::Modrinth as i64,
        OTHER_PROJECT_ID,
        PINNED_DEP_VERSION_ID,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(release.hash, dependency.hash);
    assert!(
        artifact_dao::get_cluster_artifact(&state.services.db, cluster_id, &dependency.hash)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn a_dependency_that_cannot_link_fails_the_staged_update() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![other_mod_pinned_to(PINNED_DEP_VERSION_ID)]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    let (content, dependency) = content_with_dependency_files(&state, cluster_id).await;
    let update = BrowserPackageUpdate {
        latest_version_id: NEEDS_PINNED_DEP_VERSION_ID.to_string(),
        ..stage_update(cluster_id)
    };
    let staged = stage_browser_package_update(&update, state.bundles.as_ref(), None, &content)
        .await
        .unwrap();

    std::fs::remove_file(artifact_absolute_path(&dependency.path).unwrap()).unwrap();

    apply_staged_package_update(&staged, &content)
        .await
        .err()
        .unwrap();
    assert!(
        artifact_dao::get_cluster_artifact(&state.services.db, cluster_id, HASH)
            .await
            .unwrap()
            .is_some(),
        "the package must not move without what it needs"
    );
}

#[tokio::test]
async fn staging_reaches_a_cached_pin_with_no_provider() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    let cached = cached_file(&state, cluster_id, b"sodium offline").await;
    artifact_dao::upsert_provider_release(
        &state.services.db,
        ProviderId::Modrinth as i64,
        PROJECT_ID,
        OFFLINE_VERSION_ID,
        &cached.hash,
        "Sodium",
        "2.0.0",
        None,
        "[]",
        "[]",
    )
    .await
    .unwrap();
    let content = pin_content(&state);
    let update = BrowserPackageUpdate {
        latest_version_id: OFFLINE_VERSION_ID.to_string(),
        ..stage_update(cluster_id)
    };

    artifact_dao::replace_release_dependencies(
        &state.services.db,
        ProviderId::Modrinth as i64,
        PROJECT_ID,
        OFFLINE_VERSION_ID,
        &[],
    )
    .await
    .unwrap();

    let staged = stage_browser_package_update(&update, state.bundles.as_ref(), None, &content)
        .await
        .unwrap();
    let installed = apply_staged_package_update(&staged, &content)
        .await
        .unwrap();

    assert_eq!(installed, cached.hash);
}

async fn record_release(
    state: &LauncherState,
    project_id: &str,
    version_id: &str,
    hash: &str,
    dependencies: Option<&[(String, String, String)]>,
) {
    artifact_dao::upsert_provider_release(
        &state.services.db,
        ProviderId::Modrinth as i64,
        project_id,
        version_id,
        hash,
        version_id,
        version_id,
        None,
        "[]",
        "[]",
    )
    .await
    .unwrap();

    if let Some(dependencies) = dependencies {
        artifact_dao::replace_release_dependencies(
            &state.services.db,
            ProviderId::Modrinth as i64,
            project_id,
            version_id,
            dependencies,
        )
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn staging_refuses_a_cached_release_with_no_recorded_dependencies_offline() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    let cached = cached_file(&state, cluster_id, b"sodium never synced").await;
    record_release(&state, PROJECT_ID, OFFLINE_VERSION_ID, &cached.hash, None).await;
    let update = BrowserPackageUpdate {
        latest_version_id: OFFLINE_VERSION_ID.to_string(),
        ..stage_update(cluster_id)
    };

    stage_browser_package_update(&update, state.bundles.as_ref(), None, &pin_content(&state))
        .await
        .err()
        .expect("an unknown dependency list must not read as none");
}

#[tokio::test]
async fn staging_asks_the_provider_about_a_cached_release_with_no_recorded_dependencies() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![other_mod_pinned_to(PINNED_DEP_VERSION_ID)]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    let (content, dependency) = content_with_dependency_files(&state, cluster_id).await;
    let package = artifact_dao::get_artifact_by_hash(
        &state.services.db,
        &content
            .providers
            .get(ProviderId::Modrinth)
            .unwrap()
            .get_version(PROJECT_ID, NEEDS_PINNED_DEP_VERSION_ID, &content)
            .await
            .unwrap()
            .files[0]
            .sha1,
    )
    .await
    .unwrap()
    .unwrap();
    record_release(
        &state,
        PROJECT_ID,
        NEEDS_PINNED_DEP_VERSION_ID,
        &package.hash,
        None,
    )
    .await;
    let update = BrowserPackageUpdate {
        latest_version_id: NEEDS_PINNED_DEP_VERSION_ID.to_string(),
        ..stage_update(cluster_id)
    };

    let staged = stage_browser_package_update(&update, state.bundles.as_ref(), None, &content)
        .await
        .unwrap();
    apply_staged_package_update(&staged, &content)
        .await
        .unwrap();

    assert!(
        artifact_dao::get_cluster_artifact(&state.services.db, cluster_id, &dependency.hash)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn a_corrupt_cached_file_falls_back_to_the_provider() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    let cached = cached_file(&state, cluster_id, b"sodium before the bit rot").await;
    record_release(&state, PROJECT_ID, "v2", &cached.hash, Some(&[])).await;
    std::fs::write(artifact_absolute_path(&cached.path).unwrap(), b"rotten").unwrap();

    let mut content = state.services.content();
    content.providers.register(Arc::new(PinProvider(
        Some(cached.hash.clone()),
        false,
        None,
    )));

    let version = get_version_cached(&content, ProviderId::Modrinth, PROJECT_ID, "v2")
        .await
        .unwrap();

    assert!(
        !version.files[0].url.is_empty(),
        "nothing could redownload it"
    );
}

#[tokio::test]
async fn a_pin_by_version_number_resolves_offline_through_its_cached_file() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    let cached = cached_file(&state, cluster_id, b"sodium by number").await;
    record_release(
        &state,
        PROJECT_ID,
        RESOLVED_VERSION_ID,
        &cached.hash,
        Some(&[]),
    )
    .await;
    let pinned = BundleFile {
        kind: BundleFileKind::Managed {
            provider: ProviderId::Modrinth,
            project_id: PROJECT_ID.to_string(),
            version_id: OFFLINE_VERSION_NUMBER.to_string(),
            sha1: cached.hash.clone(),
        },
        ..managed_file(true)
    };
    oneclient_core::dev::seed_bundle_archive(&state, manifest(vec![pinned]))
        .await
        .unwrap();

    let check = refresh_browser_package_updates(
        cluster_id,
        state.bundles.as_ref(),
        &offline_pin_content(&state),
    )
    .await
    .unwrap();

    assert!(
        check.unchecked_pins.is_empty(),
        "{:?}",
        check.unchecked_pins
    );
    let bundled = check.bundled_pending();
    assert_eq!(bundled.len(), 1);
    assert_eq!(bundled[0].latest_version_id, RESOLVED_VERSION_ID);
}

#[tokio::test]
async fn a_cached_pinned_dependency_needs_no_provider() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![other_mod_pinned_to(PINNED_DEP_VERSION_ID)]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    let package = cached_file(&state, cluster_id, b"sodium needs lithium offline").await;
    let dependency = cached_file(&state, cluster_id, b"lithium cached offline").await;
    record_release(
        &state,
        OTHER_PROJECT_ID,
        PINNED_DEP_VERSION_ID,
        &dependency.hash,
        Some(&[]),
    )
    .await;

    let mut content = state.services.content();
    content.providers.register(Arc::new(PinProvider(
        Some(package.hash),
        true,
        Some(dependency.hash.clone()),
    )));
    let update = BrowserPackageUpdate {
        latest_version_id: NEEDS_PINNED_DEP_VERSION_ID.to_string(),
        ..stage_update(cluster_id)
    };

    stage_browser_package_update(&update, state.bundles.as_ref(), None, &content)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_dependency_on_an_exact_version_outranks_the_pin() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![other_mod_pinned_to(PINNED_DEP_VERSION_ID)]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    let (content, _) = content_with_dependency_files(&state, cluster_id).await;
    let update = BrowserPackageUpdate {
        latest_version_id: NEEDS_EXACT_DEP_VERSION_ID.to_string(),
        ..stage_update(cluster_id)
    };

    stage_browser_package_update(&update, state.bundles.as_ref(), None, &content)
        .await
        .unwrap();

    let recorded = |version_id| {
        artifact_dao::get_provider_release(
            &state.services.db,
            ProviderId::Modrinth as i64,
            OTHER_PROJECT_ID,
            version_id,
        )
    };
    assert!(recorded(EXACT_DEP_VERSION_ID).await.unwrap().is_some());
    assert!(recorded(PINNED_DEP_VERSION_ID).await.unwrap().is_none());
}

fn requires_installed_version() -> Vec<(String, String, String)> {
    vec![(
        PROJECT_ID.to_string(),
        VERSION_ID.to_string(),
        DependencyKind::Required.as_str().to_string(),
    )]
}

#[tokio::test]
async fn a_requirement_moves_with_a_dependent_on_its_way_off_that_version() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![pinned_to("v2"), other_mod_pinned_to("v2")]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    add_other_plain_copy(&state, cluster_id).await;
    artifact_dao::replace_release_dependencies(
        &state.services.db,
        ProviderId::Modrinth as i64,
        OTHER_PROJECT_ID,
        VERSION_ID,
        &requires_installed_version(),
    )
    .await
    .unwrap();

    let check =
        refresh_browser_package_updates(cluster_id, state.bundles.as_ref(), &pin_content(&state))
            .await
            .unwrap();

    assert_eq!(check.bundled_pending().len(), 2, "{:?}", check.updates);
}

#[tokio::test]
async fn a_copy_of_the_pinned_file_from_another_project_is_on_the_pin() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let file = BundleFile {
        kind: BundleFileKind::Managed {
            provider: ProviderId::Modrinth,
            project_id: OTHER_PROJECT_ID.to_string(),
            version_id: "v9".to_string(),
            sha1: HASH.to_string(),
        },
        ..managed_file(true)
    };
    oneclient_core::dev::seed_bundle_archive(&state, manifest(vec![file]))
        .await
        .unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    let content = pin_content(&state);

    let check = refresh_browser_package_updates(cluster_id, state.bundles.as_ref(), &content)
        .await
        .unwrap();

    assert!(check.updates.is_empty(), "{:?}", check.updates);
}

#[tokio::test]
async fn a_file_shipped_on_decides_a_pin_over_one_shipped_off() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let off = BundleFile {
        enabled: false,
        ..pinned_to("v2")
    };
    oneclient_core::dev::seed_bundle_archive(&state, named_manifest("A Bundle", vec![off]))
        .await
        .unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        named_manifest("B Bundle", vec![pinned_to("v3")]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    let content = pin_content(&state);

    let check = refresh_browser_package_updates(cluster_id, state.bundles.as_ref(), &content)
        .await
        .unwrap();

    assert_eq!(check.updates.len(), 1);
    assert_eq!(check.updates[0].latest_version_id, "v3");
}

async fn check_past_a_staying_release(version_id: &str) -> oneclient_core::BrowserUpdateCheck {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let other_on_its_pin = BundleFile {
        kind: BundleFileKind::Managed {
            provider: ProviderId::Modrinth,
            project_id: OTHER_PROJECT_ID.to_string(),
            version_id: VERSION_ID.to_string(),
            sha1: OTHER_HASH.to_string(),
        },
        ..managed_file(true)
    };
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![pinned_to("v2"), other_on_its_pin]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    add_other_plain_copy(&state, cluster_id).await;
    record_release(&state, OTHER_PROJECT_ID, version_id, OTHER_HASH, None).await;

    refresh_browser_package_updates(cluster_id, state.bundles.as_ref(), &pin_content(&state))
        .await
        .unwrap()
}

#[tokio::test]
async fn a_deleted_release_does_not_hold_a_pin_back() {
    let check = check_past_a_staying_release(GONE_VERSION_ID).await;

    assert_eq!(check.unchecked_pins, Vec::<String>::new());
    assert_eq!(check.bundled_pending().len(), 1, "{:?}", check.updates);
}

async fn zipped(entries: Vec<(String, Vec<u8>)>) -> Vec<u8> {
    let source = std::env::temp_dir().join(format!("{}.jar", uuid::Uuid::new_v4()));
    let file = tokio::fs::File::create(&source).await.unwrap();
    let mut writer = async_zip::tokio::write::ZipFileWriter::with_tokio(file);
    for (name, bytes) in entries {
        let entry = async_zip::ZipEntryBuilder::new(name.into(), async_zip::Compression::Stored);
        writer.write_entry_whole(entry, &bytes).await.unwrap();
    }
    let mut file = writer.close().await.unwrap().into_inner();
    tokio::io::AsyncWriteExt::shutdown(&mut file).await.unwrap();

    std::fs::read(&source).unwrap()
}

async fn fabric_jar(mut manifest: serde_json::Value, nested: &[Vec<u8>]) -> Vec<u8> {
    let paths: Vec<String> = (0..nested.len())
        .map(|index| format!("META-INF/jars/nested{index}.jar"))
        .collect();
    if !nested.is_empty() {
        let declared = paths.iter().map(|file| serde_json::json!({ "file": file }));
        manifest["jars"] = declared.collect();
    }

    let mut entries = vec![(
        "fabric.mod.json".to_string(),
        manifest.to_string().into_bytes(),
    )];
    entries.extend(paths.into_iter().zip(nested.iter().cloned()));
    zipped(entries).await
}

async fn link_jar(state: &LauncherState, cluster_id: i64, bytes: &[u8]) -> String {
    let cached = cached_file(state, cluster_id, bytes).await;
    artifact_dao::link_cluster_artifact(
        &state.services.db,
        cluster_id,
        &cached.hash,
        &cached.file_name,
    )
    .await
    .unwrap();
    cached.hash
}

async fn local_fabric_jar(
    state: &LauncherState,
    cluster_id: i64,
    manifest: serde_json::Value,
) -> String {
    link_jar(state, cluster_id, &fabric_jar(manifest, &[]).await).await
}

async fn linked_fabric_jar(
    state: &LauncherState,
    cluster_id: i64,
    manifest: serde_json::Value,
    project_id: &str,
    version_id: &str,
) -> String {
    let hash = local_fabric_jar(state, cluster_id, manifest).await;
    record_release(state, project_id, version_id, &hash, None).await;
    hash
}

fn needs_sodium(range: &str) -> serde_json::Value {
    serde_json::json!({ "id": "lithium", "depends": { "sodium": range } })
}

async fn cluster_with_pinned_sodium(state: &LauncherState) -> i64 {
    let cluster_id = empty_cluster(state).await;
    linked_fabric_jar(
        state,
        cluster_id,
        serde_json::json!({ "id": "sodium" }),
        PROJECT_ID,
        VERSION_ID,
    )
    .await;
    oneclient_core::dev::seed_bundle_archive(state, manifest(vec![pinned_to("v2")]))
        .await
        .unwrap();
    cluster_id
}

async fn check_past_a_range(
    range: &str,
    deleted: bool,
    local: bool,
) -> oneclient_core::BrowserUpdateCheck {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_pinned_sodium(&state).await;
    let version_id = if deleted { GONE_VERSION_ID } else { VERSION_ID };
    if local {
        local_fabric_jar(&state, cluster_id, needs_sodium(range)).await;
    } else {
        linked_fabric_jar(
            &state,
            cluster_id,
            needs_sodium(range),
            OTHER_PROJECT_ID,
            version_id,
        )
        .await;
    }

    refresh_all_browser_package_updates(cluster_id, state.bundles.as_ref(), &pin_content(&state))
        .await
        .unwrap()
}

#[tokio::test]
async fn a_pin_lands_outside_the_range_a_jar_asks_for() {
    let check = check_past_a_range(">=3", false, false).await;

    assert_eq!(check.unchecked_pins, Vec::<String>::new());
    assert_eq!(check.bundled_pending().len(), 1, "{:?}", check.updates);
}

#[tokio::test]
async fn a_pin_inside_a_jars_range_moves() {
    let check = check_past_a_range(">=2", false, false).await;

    assert_eq!(check.bundled_pending().len(), 1, "{:?}", check.updates);
}

#[tokio::test]
async fn a_local_mod_lets_a_pin_inside_its_range_move() {
    let check = check_past_a_range("*", false, true).await;

    assert_eq!(check.bundled_pending().len(), 1, "{:?}", check.updates);
}

async fn plain_update_onto_a_pinned_dependency(
    dependency_version: &str,
) -> (ContentResult<String>, Arc<LauncherState>, i64, String) {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![other_mod_pinned_to(PINNED_DEP_VERSION_ID)]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    let package = fabric_jar(
        serde_json::json!({ "id": "sodium", "depends": { "lithium": ">=2" } }),
        &[],
    )
    .await;
    let package = cached_file(&state, cluster_id, &package).await;
    let dependency = fabric_jar(
        serde_json::json!({ "id": "lithium", "version": dependency_version }),
        &[],
    )
    .await;
    let dependency = cached_file(&state, cluster_id, &dependency).await;
    let mut content = state.services.content();
    content.providers.register(Arc::new(PinProvider(
        Some(package.hash),
        false,
        Some(dependency.hash.clone()),
    )));

    let update = BrowserPackageUpdate {
        latest_version_id: NEEDS_PINNED_DEP_VERSION_ID.to_string(),
        ..stage_update(cluster_id)
    };
    let result =
        apply_browser_package_update(&update, state.bundles.as_ref(), None, &content).await;
    (result, state, cluster_id, dependency.hash)
}

#[tokio::test]
async fn a_plain_update_lands_when_its_pinned_dependency_is_too_old() {
    plain_update_onto_a_pinned_dependency("1.0")
        .await
        .0
        .unwrap();
}

async fn plain_update_beside_an_installed_library(
    library_version: &str,
) -> (ContentResult<String>, Arc<LauncherState>, i64) {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    linked_fabric_jar(
        &state,
        cluster_id,
        serde_json::json!({ "id": "lithium", "version": library_version }),
        OTHER_PROJECT_ID,
        VERSION_ID,
    )
    .await;
    let package = fabric_jar(
        serde_json::json!({ "id": "sodium", "depends": { "lithium": ">=2" } }),
        &[],
    )
    .await;
    let package = cached_file(&state, cluster_id, &package).await;
    let mut content = state.services.content();
    content
        .providers
        .register(Arc::new(PinProvider(Some(package.hash), false, None)));

    let update = BrowserPackageUpdate {
        latest_version_id: NEEDS_PINNED_DEP_VERSION_ID.to_string(),
        ..stage_update(cluster_id)
    };
    let result =
        apply_browser_package_update(&update, state.bundles.as_ref(), None, &content).await;
    (result, state, cluster_id)
}

#[tokio::test]
async fn a_plain_update_lands_beside_a_library_too_old_for_it() {
    plain_update_beside_an_installed_library("1.0")
        .await
        .0
        .unwrap();
}

#[tokio::test]
async fn a_plain_update_lands_beside_a_library_that_fits() {
    plain_update_beside_an_installed_library("2.0")
        .await
        .0
        .unwrap();
}

async fn plain_update_beside_a_mod_on_version(
    version_id: &str,
    exact: Option<&[(String, String, String)]>,
) -> ContentResult<String> {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![other_mod_pinned_to(PINNED_DEP_VERSION_ID)]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    let hash = local_fabric_jar(&state, cluster_id, needs_sodium("<2")).await;
    record_release(&state, OTHER_PROJECT_ID, version_id, &hash, exact).await;
    let package = fabric_jar(serde_json::json!({ "id": "sodium", "version": "2.0" }), &[]).await;
    let package = cached_file(&state, cluster_id, &package).await;
    let mut content = state.services.content();
    content
        .providers
        .register(Arc::new(PinProvider(Some(package.hash), false, None)));

    apply_browser_package_update(
        &stage_update(cluster_id),
        state.bundles.as_ref(),
        None,
        &content,
    )
    .await
}

#[tokio::test]
async fn a_plain_update_lands_when_a_mod_on_its_pin_needs_the_old_version() {
    plain_update_beside_a_mod_on_version(PINNED_DEP_VERSION_ID, None)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_plain_update_passes_a_mod_behind_its_pin() {
    plain_update_beside_a_mod_on_version(VERSION_ID, None)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_plain_update_lands_when_a_mod_on_its_pin_names_the_old_version() {
    let exact = requires_installed_version();
    plain_update_beside_a_mod_on_version(PINNED_DEP_VERSION_ID, Some(&exact))
        .await
        .unwrap();
}

async fn resolve_past_an_unavailable_pin(
    state: &LauncherState,
    cluster_id: i64,
    content: &ContentCtx,
) -> oneclient_content::packages::DependencyResolution {
    oneclient_core::dev::seed_bundle_archive(
        state,
        manifest(vec![BundleFile {
            kind: BundleFileKind::Managed {
                provider: ProviderId::Modrinth,
                project_id: LISTED_PROJECT_ID.to_string(),
                version_id: OFFLINE_VERSION_ID.to_string(),
                sha1: "ffffffffffffffffffffffffffffffffffffffff".to_string(),
            },
            ..managed_file(true)
        }]),
    )
    .await
    .unwrap();
    let root = get_version_cached(
        content,
        ProviderId::Modrinth,
        PROJECT_ID,
        NEEDS_LISTED_DEP_VERSION_ID,
    )
    .await
    .unwrap();

    resolve_required(
        ProviderId::Modrinth,
        &root,
        cluster_id,
        state.bundles.as_ref(),
        content,
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn a_fresh_install_falls_back_past_a_pin_it_cannot_fetch() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = empty_cluster(&state).await;
    let content = pin_content(&state);

    let resolution = resolve_past_an_unavailable_pin(&state, cluster_id, &content).await;

    assert_eq!(resolution.unresolved, Vec::<String>::new());
    assert_eq!(resolution.install[0].version.version_id, LISTED_VERSION_ID);
}

#[tokio::test]
async fn a_stage_does_not_fall_back_past_a_pin_it_cannot_fetch() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    let content = pin_content(&state);
    resolve_past_an_unavailable_pin(&state, cluster_id, &content).await;

    let update = BrowserPackageUpdate {
        latest_version_id: NEEDS_LISTED_DEP_VERSION_ID.to_string(),
        ..stage_update(cluster_id)
    };
    let err = stage_browser_package_update(&update, state.bundles.as_ref(), None, &content)
        .await
        .err()
        .unwrap();
    assert!(format!("{err}").contains("could not be resolved"), "{err}");
}

#[tokio::test]
async fn a_fresh_install_takes_a_dependency_on_its_pin() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![other_mod_pinned_to(PINNED_DEP_VERSION_ID)]),
    )
    .await
    .unwrap();
    let cluster_id = empty_cluster(&state).await;
    let content = pin_content(&state);
    let root = get_version_cached(
        &content,
        ProviderId::Modrinth,
        PROJECT_ID,
        NEEDS_PINNED_DEP_VERSION_ID,
    )
    .await
    .unwrap();

    let resolution = resolve_required(
        ProviderId::Modrinth,
        &root,
        cluster_id,
        state.bundles.as_ref(),
        &content,
    )
    .await
    .unwrap();

    assert_eq!(resolution.unresolved, Vec::<String>::new());
    assert_eq!(resolution.install.len(), 1);
    assert_eq!(
        resolution.install[0].version.version_id,
        PINNED_DEP_VERSION_ID
    );
}

async fn fresh_install_beside_a_pinned_dependency(
    dependency_version: &str,
) -> oneclient_content::packages::DependencyResolution {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![other_mod_pinned_to(PINNED_DEP_VERSION_ID)]),
    )
    .await
    .unwrap();
    let cluster_id = empty_cluster(&state).await;
    let package = fabric_jar(
        serde_json::json!({ "id": "sodium", "depends": { "lithium": ">=2" } }),
        &[],
    )
    .await;
    let package = cached_file(&state, cluster_id, &package).await;
    let dependency = fabric_jar(
        serde_json::json!({ "id": "lithium", "version": dependency_version }),
        &[],
    )
    .await;
    let dependency = cached_file(&state, cluster_id, &dependency).await;
    let mut content = state.services.content();
    content.providers.register(Arc::new(PinProvider(
        Some(package.hash),
        false,
        Some(dependency.hash),
    )));
    let root = get_version_cached(
        &content,
        ProviderId::Modrinth,
        PROJECT_ID,
        NEEDS_PINNED_DEP_VERSION_ID,
    )
    .await
    .unwrap();

    resolve_required(
        ProviderId::Modrinth,
        &root,
        cluster_id,
        state.bundles.as_ref(),
        &content,
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn a_fresh_install_keeps_a_pin_it_can_run_on() {
    let resolution = fresh_install_beside_a_pinned_dependency("2.0").await;
    assert_eq!(
        resolution.install[0].version.version_id,
        PINNED_DEP_VERSION_ID
    );
}

#[tokio::test]
async fn an_unreadable_catalog_keeps_a_fresh_installs_dependencies() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    seed_unreadable_archive(&state, false).await;
    let cluster_id = empty_cluster(&state).await;
    let content = pin_content(&state);
    let root = get_version_cached(
        &content,
        ProviderId::Modrinth,
        PROJECT_ID,
        NEEDS_PINNED_DEP_VERSION_ID,
    )
    .await
    .unwrap();

    resolve_required(
        ProviderId::Modrinth,
        &root,
        cluster_id,
        state.bundles.as_ref(),
        &content,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn a_plain_update_lands_on_a_pinned_dependency_that_fits() {
    let (result, state, cluster_id, dependency) =
        plain_update_onto_a_pinned_dependency("2.0").await;

    result.unwrap();
    assert!(
        artifact_dao::get_cluster_artifact(&state.services.db, cluster_id, &dependency)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn a_manual_check_waits_out_a_slow_backfill() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_pinned_sodium(&state).await;
    linked_fabric_jar(
        &state,
        cluster_id,
        serde_json::json!({ "id": "lithium" }),
        OTHER_PROJECT_ID,
        SLOW_VERSION_ID,
    )
    .await;

    let check = refresh_all_browser_package_updates(
        cluster_id,
        state.bundles.as_ref(),
        &pin_content(&state),
    )
    .await
    .unwrap();

    assert_eq!(check.unchecked_pins, Vec::<String>::new());
    assert_eq!(check.bundled_pending().len(), 1, "{:?}", check.updates);
}

#[tokio::test]
async fn a_hash_lookup_stays_on_the_pinned_provider() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    artifact_dao::upsert_provider_release(
        &state.services.db,
        ProviderId::CurseForge as i64,
        PROJECT_ID,
        "cf1",
        HASH,
        "Sodium",
        "1.0.0",
        None,
        MC_VERSION,
        "fabric",
    )
    .await
    .unwrap();
    let curseforge_pin = BundleFile {
        kind: BundleFileKind::Managed {
            provider: ProviderId::CurseForge,
            project_id: PROJECT_ID.to_string(),
            version_id: "2.0.0".to_string(),
            sha1: "c".repeat(40),
        },
        ..managed_file(true)
    };
    oneclient_core::dev::seed_bundle_archive(&state, manifest(vec![curseforge_pin]))
        .await
        .unwrap();
    let mut content = pin_content(&state);
    content.providers.register(Arc::new(CurseForgePin));

    let check = refresh_browser_package_updates(cluster_id, state.bundles.as_ref(), &content)
        .await
        .unwrap();

    let bundled = check.bundled_pending();
    assert_eq!(bundled.len(), 1, "{:?}", check.updates);
    assert_eq!(bundled[0].latest_version_id, CURSEFORGE_VERSION_ID);
}

#[tokio::test]
async fn a_switched_off_dependency_stops_the_stage_until_it_is_back_on() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    add_other_plain_copy(&state, cluster_id).await;
    let (content, _) = content_with_dependency_files(&state, cluster_id).await;
    let update = BrowserPackageUpdate {
        latest_version_id: NEEDS_PINNED_DEP_VERSION_ID.to_string(),
        ..stage_update(cluster_id)
    };

    PackageStore::set_artifact_enabled_to(cluster_id, OTHER_HASH, false, &content)
        .await
        .unwrap();
    let err = stage_browser_package_update(&update, state.bundles.as_ref(), None, &content)
        .await
        .err()
        .expect("a disabled copy does not meet a requirement");
    assert!(err.to_string().contains("Lithium"), "{err}");

    PackageStore::set_artifact_enabled_to(cluster_id, OTHER_HASH, true, &content)
        .await
        .unwrap();
    stage_browser_package_update(&update, state.bundles.as_ref(), None, &content)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_dependency_listed_under_another_provider_stays_off_the_pin() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![other_mod_pinned_to(PINNED_DEP_VERSION_ID)]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    add_other_plain_copy(&state, cluster_id).await;
    artifact_dao::upsert_provider_release(
        &state.services.db,
        ProviderId::CurseForge as i64,
        "lithium-mirror",
        "mirror",
        OTHER_HASH,
        "Lithium",
        "1.0.0",
        Some("2026-06-01T00:00:00+00:00"),
        MC_VERSION,
        "fabric",
    )
    .await
    .unwrap();
    let (content, dependency) = content_with_dependency_files(&state, cluster_id).await;
    let update = BrowserPackageUpdate {
        latest_version_id: NEEDS_PINNED_DEP_VERSION_ID.to_string(),
        ..stage_update(cluster_id)
    };

    let staged = stage_browser_package_update(&update, state.bundles.as_ref(), None, &content)
        .await
        .unwrap();
    apply_staged_package_update(&staged, &content)
        .await
        .unwrap();

    let linked = |hash: String| {
        let db = state.services.db.clone();
        async move {
            artifact_dao::get_cluster_artifact(&db, cluster_id, &hash)
                .await
                .unwrap()
                .is_some()
        }
    };
    assert!(linked(OTHER_HASH.to_string()).await);
    assert!(!linked(dependency.hash).await);
}

#[tokio::test]
async fn a_hanging_unpinned_lookup_leaves_the_pins_to_settle() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(&state, manifest(vec![pinned_to("v2")]))
        .await
        .unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    add_other_plain_copy(&state, cluster_id).await;
    record_release(
        &state,
        HANGING_PROJECT_ID,
        VERSION_ID,
        OTHER_HASH,
        Some(&[]),
    )
    .await;
    let content = pin_content(&state);

    let check = tokio::time::timeout(
        std::time::Duration::from_secs(7),
        refresh_browser_package_updates(cluster_id, state.bundles.as_ref(), &content),
    )
    .await
    .expect("the launch check must outlive an unpinned lookup")
    .unwrap();

    assert_eq!(check.unchecked, [OTHER_HASH]);
    assert_eq!(check.bundled_pending().len(), 1, "{:?}", check.updates);
}

async fn stages_a_cached_exact_dependency_offline(needs: &str) {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_plain_mod(&state).await;
    let package = cached_file(&state, cluster_id, b"sodium needs an exact lithium").await;
    let dependency = cached_file(&state, cluster_id, b"exact lithium cached offline").await;
    record_release(
        &state,
        OTHER_PROJECT_ID,
        EXACT_DEP_VERSION_ID,
        &dependency.hash,
        Some(&[]),
    )
    .await;

    let mut content = state.services.content();
    content
        .providers
        .register(Arc::new(PinProvider(Some(package.hash), true, None)));
    let update = BrowserPackageUpdate {
        latest_version_id: needs.to_string(),
        ..stage_update(cluster_id)
    };

    stage_browser_package_update(&update, state.bundles.as_ref(), None, &content)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_cached_exact_dependency_needs_no_provider() {
    stages_a_cached_exact_dependency_offline(NEEDS_EXACT_DEP_VERSION_ID).await;
}

#[tokio::test]
async fn a_cached_exact_dependency_named_by_version_alone_needs_no_provider() {
    stages_a_cached_exact_dependency_offline(NEEDS_VERSION_ONLY_DEP_VERSION_ID).await;
}

async fn cluster_with_a_second_release(state: &LauncherState) -> i64 {
    oneclient_core::dev::seed_bundle_archive(state, manifest(vec![pinned_to("v2")]))
        .await
        .unwrap();
    let cluster_id = cluster_with_plain_mod(state).await;
    artifact_dao::upsert_provider_release(
        &state.services.db,
        ProviderId::Modrinth as i64,
        OTHER_PROJECT_ID,
        "mirror",
        HASH,
        "Lithium",
        "1.0.0",
        Some("2026-06-01T00:00:00+00:00"),
        MC_VERSION,
        "fabric",
    )
    .await
    .unwrap();
    cluster_id
}

const BUNDLE_NAME_ORDERS: [(&str, &str); 2] = [("A Bundle", "Z Bundle"), ("Z Bundle", "A Bundle")];

#[tokio::test]
async fn a_taken_bundle_decides_a_pin_over_a_declined_one() {
    for (declined, taken) in BUNDLE_NAME_ORDERS {
        let state = oneclient_core::dev::ephemeral_state().await.unwrap();
        let mut opt_in = named_manifest(declined, vec![pinned_to("v3")]);
        opt_in.enabled = false;
        oneclient_core::dev::seed_bundle_archive(&state, opt_in)
            .await
            .unwrap();
        oneclient_core::dev::seed_bundle_archive(
            &state,
            named_manifest(taken, vec![pinned_to(VERSION_ID)]),
        )
        .await
        .unwrap();
        let cluster_id = cluster_with_plain_mod(&state).await;
        let content = pin_content(&state);
        set_bundle_choices(cluster_id, &[(declined.to_string(), false)], &content)
            .await
            .unwrap();

        let check = refresh_browser_package_updates(cluster_id, state.bundles.as_ref(), &content)
            .await
            .unwrap();

        assert!(check.updates.is_empty(), "{declined}: {:?}", check.updates);
    }
}

#[tokio::test]
async fn a_live_bundle_decides_a_pin_over_an_emptied_one() {
    for (emptied, live) in BUNDLE_NAME_ORDERS {
        let state = oneclient_core::dev::ephemeral_state().await.unwrap();
        oneclient_core::dev::seed_bundle_archive(
            &state,
            named_manifest(emptied, vec![pinned_to("v3")]),
        )
        .await
        .unwrap();
        oneclient_core::dev::seed_bundle_archive(
            &state,
            named_manifest(
                live,
                vec![pinned_to(VERSION_ID), other_mod_pinned_to(VERSION_ID)],
            ),
        )
        .await
        .unwrap();
        let cluster_id = cluster_with_plain_mod(&state).await;
        add_other_plain_copy(&state, cluster_id).await;
        bundle_dao::track_bundle_artifact(
            &state.services.db,
            cluster_id,
            OTHER_HASH,
            live,
            VERSION_ID,
            OTHER_PROJECT_ID,
        )
        .await
        .unwrap();

        let check = refresh_browser_package_updates(
            cluster_id,
            state.bundles.as_ref(),
            &pin_content(&state),
        )
        .await
        .unwrap();

        assert!(check.updates.is_empty(), "{live}: {:?}", check.updates);
    }
}

#[tokio::test]
async fn mod_still_in_manifest_is_not_removed() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(&state, manifest(vec![managed_file(true)]))
        .await
        .unwrap();
    let cluster_id = cluster_with_tracked_mod(&state).await;

    let check = check_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();

    assert!(
        check.removals_available.is_empty(),
        "an unchanged tracked mod was flagged for removal: {:?}",
        check.removals_available
    );
    assert!(check.updates_available.is_empty());
}

#[tokio::test]
async fn mod_dropped_from_manifest_is_removed() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(&state, manifest(vec![]))
        .await
        .unwrap();
    let cluster_id = cluster_with_tracked_mod(&state).await;

    let check = check_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();

    assert_eq!(
        check.removals_available.len(),
        1,
        "a mod the bundle no longer ships should be removed"
    );
    assert_eq!(check.removals_available[0].package_id, PROJECT_ID);
}

#[tokio::test]
async fn disabled_mod_dropped_from_manifest_is_still_removed() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(&state, manifest(vec![]))
        .await
        .unwrap();
    let cluster_id = cluster_with_tracked_mod(&state).await;

    artifact_dao::update_cluster_artifact(&state.services.db, cluster_id, HASH, "sodium.jar", 0)
        .await
        .unwrap();
    bundle_dao::save_override(
        &state.services.db,
        cluster_id,
        BUNDLE,
        PROJECT_ID,
        OverrideType::Disabled,
    )
    .await
    .unwrap();

    let check = check_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();

    assert_eq!(
        check.removals_available.len(),
        1,
        "a disabled mod the bundle no longer ships should still be removed"
    );
}

#[tokio::test]
async fn user_disabled_mod_is_not_treated_as_a_removal() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(&state, manifest(vec![managed_file(true)]))
        .await
        .unwrap();
    let cluster_id = cluster_with_tracked_mod(&state).await;

    artifact_dao::update_cluster_artifact(&state.services.db, cluster_id, HASH, "sodium.jar", 0)
        .await
        .unwrap();
    bundle_dao::save_override(
        &state.services.db,
        cluster_id,
        BUNDLE,
        PROJECT_ID,
        OverrideType::Disabled,
    )
    .await
    .unwrap();

    let check = check_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();

    assert!(
        check.removals_available.is_empty(),
        "a user-disabled mod is still shipped by the bundle and must not be removed: {:?}",
        check.removals_available
    );
}

#[tokio::test]
async fn deleted_bundled_mod_is_not_treated_as_a_removal() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(&state, manifest(vec![managed_file(true)]))
        .await
        .unwrap();
    let cluster_id = cluster_with_tracked_mod(&state).await;

    artifact_dao::update_cluster_artifact(&state.services.db, cluster_id, HASH, "sodium.jar", 0)
        .await
        .unwrap();
    bundle_dao::save_override(
        &state.services.db,
        cluster_id,
        BUNDLE,
        PROJECT_ID,
        OverrideType::Removed,
    )
    .await
    .unwrap();

    let check = check_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();

    assert!(
        check.removals_available.is_empty(),
        "a deleted mod is still shipped by the bundle and must not be removed: {:?}",
        check.removals_available
    );
    assert!(check.updates_available.is_empty());
}

#[tokio::test]
async fn deleting_a_bundled_mod_leaves_only_the_removal() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_tracked_mod(&state).await;
    let ctx = state.services.content();
    bundle_dao::save_override(
        &state.services.db,
        cluster_id,
        "Old Bundle",
        PROJECT_ID,
        OverrideType::Disabled,
    )
    .await
    .unwrap();

    on_user_disable_artifact(cluster_id, HASH, &ctx)
        .await
        .unwrap();
    on_user_remove_artifact(cluster_id, HASH, &ctx)
        .await
        .unwrap();

    let overrides: Vec<(String, String)> =
        bundle_dao::list_overrides(&state.services.db, cluster_id)
            .await
            .unwrap()
            .into_iter()
            .filter(|o| o.package_id == PROJECT_ID)
            .map(|o| (o.bundle_name, o.override_type))
            .collect();
    assert_eq!(overrides, [(BUNDLE.to_string(), "removed".to_string())]);
}

#[tokio::test]
async fn user_disabled_mod_still_takes_updates() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let mut newer = managed_file(true);
    if let BundleFileKind::Managed {
        version_id, sha1, ..
    } = &mut newer.kind
    {
        *version_id = "v2".to_string();
        *sha1 = "cccccccccccccccccccccccccccccccccccccccc".to_string();
    }
    oneclient_core::dev::seed_bundle_archive(&state, manifest(vec![newer]))
        .await
        .unwrap();
    let cluster_id = cluster_with_tracked_mod(&state).await;

    artifact_dao::update_cluster_artifact(&state.services.db, cluster_id, HASH, "sodium.jar", 0)
        .await
        .unwrap();
    bundle_dao::save_override(
        &state.services.db,
        cluster_id,
        BUNDLE,
        PROJECT_ID,
        OverrideType::Disabled,
    )
    .await
    .unwrap();

    let check = check_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();

    assert_eq!(
        check
            .updates_available
            .iter()
            .map(|u| u.new_version_id.as_str())
            .collect::<Vec<_>>(),
        ["v2"],
        "a user-disabled mod must still be offered the newer version"
    );
    assert!(check.removals_available.is_empty());
    assert!(check.additions_available.is_empty());
}

#[tokio::test]
async fn live_bundle_takes_on_new_catalog_files() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![managed_file(true), newly_shipped_file()]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_tracked_mod(&state).await;

    let check = check_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();

    assert_eq!(
        check.additions_available.len(),
        1,
        "a bundle the user still has enabled should pick up newly shipped files"
    );
    assert_eq!(
        check.additions_available[0].new_file.kind.package_id(),
        "newcomer"
    );
}

#[tokio::test]
async fn emptied_bundle_does_not_take_on_new_catalog_files() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![managed_file(true), newly_shipped_file()]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_tracked_mod(&state).await;

    // Disabling everything is how you opt out
    // The newcomer lacks an override only
    // because it did not exist yet that absence must not read as consent
    artifact_dao::update_cluster_artifact(&state.services.db, cluster_id, HASH, "sodium.jar", 0)
        .await
        .unwrap();
    bundle_dao::save_override(
        &state.services.db,
        cluster_id,
        BUNDLE,
        PROJECT_ID,
        OverrideType::Disabled,
    )
    .await
    .unwrap();

    let check = check_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();

    assert!(
        check.additions_available.is_empty(),
        "a bundle whose content the user disabled must not reinstate itself: {:?}",
        check
            .additions_available
            .iter()
            .map(|a| a.new_file.kind.package_id())
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn removed_bundle_content_does_not_take_on_new_catalog_files() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![managed_file(true), newly_shipped_file()]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_tracked_mod(&state).await;

    oneclient_content::bundles::remove_artifact_from_cluster(
        cluster_id,
        HASH,
        true,
        &state.services.content(),
    )
    .await
    .unwrap();

    let check = check_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();

    assert!(
        check.additions_available.is_empty(),
        "a bundle the user emptied by removal must not reinstate itself: {:?}",
        check
            .additions_available
            .iter()
            .map(|a| a.new_file.kind.package_id())
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn opting_a_single_file_in_keeps_the_bundle_live() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![managed_file(true), newly_shipped_file()]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_tracked_mod(&state).await;

    artifact_dao::update_cluster_artifact(&state.services.db, cluster_id, HASH, "sodium.jar", 0)
        .await
        .unwrap();
    bundle_dao::save_override(
        &state.services.db,
        cluster_id,
        BUNDLE,
        PROJECT_ID,
        OverrideType::Disabled,
    )
    .await
    .unwrap();
    bundle_dao::save_override(
        &state.services.db,
        cluster_id,
        BUNDLE,
        "newcomer",
        OverrideType::Enabled,
    )
    .await
    .unwrap();

    let check = check_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();

    assert_eq!(
        check.additions_available.len(),
        1,
        "an explicit opt-in is consent, even with the rest of the bundle disabled"
    );
}

async fn seed_delisted_bundle(state: &LauncherState) {
    oneclient_db::dao::bundle::upsert_bundle(
        &state.services.db,
        oneclient_db::models::NewBundle {
            remote_path: "bundles/delisted.mrpack",
            mc_version: MC_VERSION,
            mc_loader: GameLoader::Fabric as i64,
            file_name: "delisted.mrpack",
            name: Some(BUNDLE),
            version_id: Some("1"),
            category: Some("test"),
            loader_version: Some("0.16.0"),
            disk_path: "bundles/delisted.mrpack",
            hidden: true,
            etag: None,
            synced_at: None,
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn delisted_bundle_content_is_removed() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    seed_delisted_bundle(&state).await;
    let cluster_id = cluster_with_tracked_mod(&state).await;

    let check = check_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();

    assert_eq!(
        check.removals_available.len(),
        1,
        "content exclusive to a bundle the catalog dropped should be removed"
    );
    assert_eq!(check.removals_available[0].package_id, PROJECT_ID);
}

#[tokio::test]
async fn delisted_bundle_content_another_bundle_still_ships_is_kept() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    seed_delisted_bundle(&state).await;
    let mut successor = manifest(vec![managed_file(true)]);
    successor.name = "Successor Bundle".to_string();
    oneclient_core::dev::seed_bundle_archive(&state, successor)
        .await
        .unwrap();
    let cluster_id = cluster_with_tracked_mod(&state).await;

    let check = check_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();

    assert!(
        check.removals_available.is_empty(),
        "a mod another live bundle still ships is not exclusive: {:?}",
        check.removals_available
    );
}

#[tokio::test]
async fn tracked_bundle_that_never_synced_is_not_removed() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_tracked_mod(&state).await;

    let check = check_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();

    assert!(
        check.removals_available.is_empty(),
        "an absent catalog is not a delisting and must not take content down: {:?}",
        check.removals_available
    );
}

fn resource_pack_file() -> BundleFile {
    BundleFile {
        enabled: true,
        hidden: false,
        file_type: BundleFileType::Normal,
        path: "resourcepacks/looks.zip".to_string(),
        size: 1,
        kind: BundleFileKind::Managed {
            provider: ProviderId::Modrinth,
            project_id: "looks".to_string(),
            version_id: "v1".to_string(),
            sha1: "cccccccccccccccccccccccccccccccccccccccc".to_string(),
        },
    }
}

#[tokio::test]
async fn disabling_every_mod_stops_new_mods_while_a_resource_pack_stays_on() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![
            managed_file(true),
            newly_shipped_file(),
            resource_pack_file(),
        ]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_tracked_mod(&state).await;

    artifact_dao::update_cluster_artifact(&state.services.db, cluster_id, HASH, "sodium.jar", 0)
        .await
        .unwrap();
    bundle_dao::save_override(
        &state.services.db,
        cluster_id,
        BUNDLE,
        PROJECT_ID,
        OverrideType::Disabled,
    )
    .await
    .unwrap();
    bundle_dao::save_override(
        &state.services.db,
        cluster_id,
        BUNDLE,
        "looks",
        OverrideType::Enabled,
    )
    .await
    .unwrap();

    let check = check_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();

    let added: Vec<String> = check
        .additions_available
        .iter()
        .map(|a| a.new_file.kind.package_id())
        .collect();

    assert!(
        !added.iter().any(|id| id == "newcomer"),
        "mods were switched off for this bundle so a new mod must not arrive: {added:?}"
    );
    assert!(
        added.iter().any(|id| id == "looks"),
        "the resource pack side of the bundle is still live: {added:?}"
    );
    assert!(
        !check
            .optional_available
            .iter()
            .any(|o| o.package_id == "newcomer"),
        "mods were switched off for this bundle so a new mod must not be offered either"
    );

    let status = get_bundles_with_update_status(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();
    let types = &status
        .iter()
        .find(|b| b.archive.manifest.name == BUNDLE)
        .unwrap()
        .opted_in_types;
    assert!(
        !types.contains(&ContentType::Mod) && types.contains(&ContentType::ResourcePack),
        "the package list must agree with the update path on which side is opted out: {types:?}"
    );
}

fn fabric_api_file() -> BundleFile {
    BundleFile {
        enabled: true,
        hidden: false,
        file_type: BundleFileType::Normal,
        path: "mods/fabric-api.jar".to_string(),
        size: 1,
        kind: BundleFileKind::Managed {
            provider: ProviderId::Modrinth,
            project_id: "fabric-api".to_string(),
            version_id: "v1".to_string(),
            sha1: FABRIC_API_HASH.to_string(),
        },
    }
}

fn named_manifest(name: &str, files: Vec<BundleFile>) -> BundleManifest {
    let mut m = manifest(files);
    m.name = name.to_string();
    m
}

async fn install_fabric_api(state: &LauncherState, cluster_id: i64) {
    artifact_dao::insert_artifact(
        &state.services.db,
        FABRIC_API_HASH,
        ContentType::Mod as i64,
        "artifacts/fabric-api.jar",
        "fabric-api.jar",
        Some(1),
    )
    .await
    .unwrap();
    artifact_dao::link_cluster_artifact(
        &state.services.db,
        cluster_id,
        FABRIC_API_HASH,
        "fabric-api.jar",
    )
    .await
    .unwrap();
    artifact_dao::upsert_provider_release(
        &state.services.db,
        ProviderId::Modrinth as i64,
        "fabric-api",
        "v1",
        FABRIC_API_HASH,
        "Fabric API",
        "1.0.0",
        None,
        MC_VERSION,
        "fabric",
    )
    .await
    .unwrap();
    bundle_dao::track_bundle_artifact(
        &state.services.db,
        cluster_id,
        FABRIC_API_HASH,
        SHARED_BUNDLE,
        "v1",
        "fabric-api",
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn a_mod_every_bundle_ships_does_not_keep_a_disabled_bundle_taking_new_mods() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        named_manifest(
            BUNDLE,
            vec![managed_file(true), newly_shipped_file(), fabric_api_file()],
        ),
    )
    .await
    .unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        named_manifest(SHARED_BUNDLE, vec![fabric_api_file()]),
    )
    .await
    .unwrap();

    let cluster_id = cluster_with_tracked_mod(&state).await;
    install_fabric_api(&state, cluster_id).await;

    artifact_dao::update_cluster_artifact(&state.services.db, cluster_id, HASH, "sodium.jar", 0)
        .await
        .unwrap();
    bundle_dao::save_override(
        &state.services.db,
        cluster_id,
        BUNDLE,
        PROJECT_ID,
        OverrideType::Disabled,
    )
    .await
    .unwrap();

    let check = check_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();

    let added: Vec<String> = check
        .additions_available
        .iter()
        .map(|a| a.new_file.kind.package_id())
        .collect();

    assert!(
        !added.iter().any(|id| id == "newcomer"),
        "a mod shared with another bundle must not hold this bundle open: {added:?}"
    );
}

fn hidden_dependency_file() -> BundleFile {
    BundleFile {
        enabled: true,
        hidden: true,
        file_type: BundleFileType::Normal,
        path: "mods/sodium-extra-lib.jar".to_string(),
        size: 1,
        kind: BundleFileKind::Managed {
            provider: ProviderId::Modrinth,
            project_id: "sodium-extra-lib".to_string(),
            version_id: "v1".to_string(),
            sha1: HIDDEN_DEP_HASH.to_string(),
        },
    }
}

#[tokio::test]
async fn a_hidden_dependency_only_this_bundle_ships_does_not_hold_it_open() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![
            managed_file(true),
            newly_shipped_file(),
            hidden_dependency_file(),
        ]),
    )
    .await
    .unwrap();

    let cluster_id = cluster_with_tracked_mod(&state).await;

    artifact_dao::insert_artifact(
        &state.services.db,
        HIDDEN_DEP_HASH,
        ContentType::Mod as i64,
        "artifacts/sodium-extra-lib.jar",
        "sodium-extra-lib.jar",
        Some(1),
    )
    .await
    .unwrap();
    artifact_dao::link_cluster_artifact(
        &state.services.db,
        cluster_id,
        HIDDEN_DEP_HASH,
        "sodium-extra-lib.jar",
    )
    .await
    .unwrap();
    artifact_dao::upsert_provider_release(
        &state.services.db,
        ProviderId::Modrinth as i64,
        "sodium-extra-lib",
        "v1",
        HIDDEN_DEP_HASH,
        "Sodium Extra Lib",
        "1.0.0",
        None,
        MC_VERSION,
        "fabric",
    )
    .await
    .unwrap();
    bundle_dao::track_bundle_artifact(
        &state.services.db,
        cluster_id,
        HIDDEN_DEP_HASH,
        BUNDLE,
        "v1",
        "sodium-extra-lib",
    )
    .await
    .unwrap();

    artifact_dao::update_cluster_artifact(&state.services.db, cluster_id, HASH, "sodium.jar", 0)
        .await
        .unwrap();
    bundle_dao::save_override(
        &state.services.db,
        cluster_id,
        BUNDLE,
        PROJECT_ID,
        OverrideType::Disabled,
    )
    .await
    .unwrap();

    let check = check_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();

    let added: Vec<String> = check
        .additions_available
        .iter()
        .map(|a| a.new_file.kind.package_id())
        .collect();

    assert!(
        !added.iter().any(|id| id == "newcomer"),
        "a dependency the user was never shown must not read as wanting the bundle: {added:?}"
    );
}

#[tokio::test]
async fn an_unrelated_catalog_bundle_sharing_a_mod_does_not_undo_the_opt_out() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        named_manifest(
            BUNDLE,
            vec![
                managed_file(true),
                newly_shipped_file(),
                resource_pack_file(),
            ],
        ),
    )
    .await
    .unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        named_manifest("Unrelated Bundle", vec![managed_file(true)]),
    )
    .await
    .unwrap();

    let cluster_id = cluster_with_tracked_mod(&state).await;
    artifact_dao::update_cluster_artifact(&state.services.db, cluster_id, HASH, "sodium.jar", 0)
        .await
        .unwrap();
    bundle_dao::save_override(
        &state.services.db,
        cluster_id,
        BUNDLE,
        PROJECT_ID,
        OverrideType::Disabled,
    )
    .await
    .unwrap();
    bundle_dao::save_override(
        &state.services.db,
        cluster_id,
        BUNDLE,
        "looks",
        OverrideType::Enabled,
    )
    .await
    .unwrap();

    let check = check_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();

    let added: Vec<String> = check
        .additions_available
        .iter()
        .map(|a| a.new_file.kind.package_id())
        .collect();

    assert!(
        !added.iter().any(|id| id == "newcomer"),
        "a bundle the user never installed must not speak for this one: {added:?}"
    );
}

#[tokio::test]
async fn untracked_older_install_counts_as_opted_in() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![managed_file(true), newly_shipped_file()]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_tracked_mod(&state).await;
    bundle_dao::clear_bundle_tracking(&state.services.db, cluster_id, HASH)
        .await
        .unwrap();

    let bundles = get_bundles_with_update_status(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();

    assert!(
        bundles
            .iter()
            .all(|b| b.opted_in_types.contains(&ContentType::Mod)),
        "a bundle the updater infers from its installed mods must not have its files hidden from the All tab"
    );
}

#[tokio::test]
async fn package_list_and_updater_infer_the_same_bundle_through_overrides() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        named_manifest(BUNDLE, vec![managed_file(true), newly_shipped_file()]),
    )
    .await
    .unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        named_manifest(SHARED_BUNDLE, vec![managed_file(true)]),
    )
    .await
    .unwrap();

    let cluster_id = cluster_with_tracked_mod(&state).await;
    bundle_dao::clear_bundle_tracking(&state.services.db, cluster_id, HASH)
        .await
        .unwrap();
    bundle_dao::save_override(
        &state.services.db,
        cluster_id,
        SHARED_BUNDLE,
        PROJECT_ID,
        OverrideType::Disabled,
    )
    .await
    .unwrap();

    let check = check_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();
    assert!(
        check
            .additions_available
            .iter()
            .any(|a| a.new_file.kind.package_id() == "newcomer"),
        "the updater infers this bundle from its uniquely installed mod"
    );

    let status = get_bundles_with_update_status(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();
    let types = &status
        .iter()
        .find(|b| b.archive.manifest.name == BUNDLE)
        .unwrap()
        .opted_in_types;
    assert!(
        types.contains(&ContentType::Mod),
        "the package list must infer the same bundle the updater does: {types:?}"
    );
}

const GITHUB_MOD_ID: &str = "github-mod";
const GITHUB_OLD_HASH: &str = "1111111111111111111111111111111111111111";

/// The catalog has moved on to a newer release than the one installed
fn github_mod_file() -> BundleFile {
    BundleFile {
        enabled: true,
        hidden: false,
        file_type: BundleFileType::Normal,
        path: "mods/github-mod.jar".to_string(),
        size: 1,
        kind: BundleFileKind::External {
            file: oneclient_content::packages::types::ExternalFile {
                name: "github-mod.jar".to_string(),
                url: "https://github.com/example/github-mod/releases/download/v2/github-mod.jar"
                    .to_string(),
                sha1: "2222222222222222222222222222222222222222".to_string(),
                size: 1,
                content_type: ContentType::Mod,
            },
            id: Some(GITHUB_MOD_ID.to_string()),
            meta: None,
        },
    }
}

#[tokio::test]
async fn an_outdated_github_mod_left_on_keeps_the_bundle_taking_mods() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![
            managed_file(true),
            github_mod_file(),
            newly_shipped_file(),
        ]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_tracked_mod(&state).await;

    let db = &state.services.db;
    artifact_dao::update_cluster_artifact(db, cluster_id, HASH, "sodium.jar", 0)
        .await
        .unwrap();
    bundle_dao::save_override(db, cluster_id, BUNDLE, PROJECT_ID, OverrideType::Disabled)
        .await
        .unwrap();
    artifact_dao::insert_artifact(
        db,
        GITHUB_OLD_HASH,
        ContentType::Mod as i64,
        "artifacts/github-mod.jar",
        "github-mod.jar",
        Some(1),
    )
    .await
    .unwrap();
    artifact_dao::link_cluster_artifact(db, cluster_id, GITHUB_OLD_HASH, "github-mod.jar")
        .await
        .unwrap();
    bundle_dao::track_bundle_artifact(
        db,
        cluster_id,
        GITHUB_OLD_HASH,
        BUNDLE,
        GITHUB_OLD_HASH,
        GITHUB_MOD_ID,
    )
    .await
    .unwrap();

    let check = check_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();
    assert!(
        check
            .additions_available
            .iter()
            .any(|a| a.new_file.kind.package_id() == "newcomer"),
        "the GitHub mod is still on even though its installed release is older than the catalog's"
    );

    let status = get_bundles_with_update_status(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();
    let types = &status
        .iter()
        .find(|b| b.archive.manifest.name == BUNDLE)
        .unwrap()
        .opted_in_types;
    assert!(types.contains(&ContentType::Mod), "{types:?}");
}

async fn cluster_with_mods_opted_out(state: &LauncherState, disabled_under: &str) -> i64 {
    let cluster_id = cluster_with_tracked_mod(state).await;
    artifact_dao::update_cluster_artifact(&state.services.db, cluster_id, HASH, "sodium.jar", 0)
        .await
        .unwrap();
    bundle_dao::save_override(
        &state.services.db,
        cluster_id,
        disabled_under,
        PROJECT_ID,
        OverrideType::Disabled,
    )
    .await
    .unwrap();
    bundle_dao::save_override(
        &state.services.db,
        cluster_id,
        BUNDLE,
        "looks",
        OverrideType::Enabled,
    )
    .await
    .unwrap();
    cluster_id
}

async fn added_ids(state: &LauncherState, cluster_id: i64) -> Vec<String> {
    check_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap()
    .additions_available
    .iter()
    .map(|a| a.new_file.kind.package_id())
    .collect()
}

#[tokio::test]
async fn a_disable_filed_under_another_bundle_still_opts_the_mods_out() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![
            managed_file(true),
            newly_shipped_file(),
            resource_pack_file(),
        ]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_mods_opted_out(&state, "Old Bundle").await;

    let added = added_ids(&state, cluster_id).await;
    assert!(
        !added.iter().any(|id| id == "newcomer"),
        "the user switched sodium off while it was filed under another bundle: {added:?}"
    );
}

#[tokio::test]
async fn the_catalog_dropping_every_switched_off_mod_keeps_the_opt_out() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![managed_file(true), resource_pack_file()]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_mods_opted_out(&state, BUNDLE).await;
    added_ids(&state, cluster_id).await;

    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![newly_shipped_file(), resource_pack_file()]),
    )
    .await
    .unwrap();

    let added = added_ids(&state, cluster_id).await;
    assert!(
        !added.iter().any(|id| id == "newcomer"),
        "mods stay opted out after the catalog replaces them: {added:?}"
    );
    let status = get_bundles_with_update_status(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();
    let types = &status
        .iter()
        .find(|b| b.archive.manifest.name == BUNDLE)
        .unwrap()
        .opted_in_types;
    assert!(
        !types.contains(&ContentType::Mod),
        "the package list must remember the opt-out too: {types:?}"
    );

    set_bundle_package_enabled(
        cluster_id,
        BUNDLE,
        "looks",
        true,
        true,
        &state.services.content(),
    )
    .await
    .unwrap();
    let added = added_ids(&state, cluster_id).await;
    assert!(
        !added.iter().any(|id| id == "newcomer"),
        "switching a resource pack on must not opt the mods back in: {added:?}"
    );

    set_bundle_package_enabled(
        cluster_id,
        BUNDLE,
        "newcomer",
        true,
        true,
        &state.services.content(),
    )
    .await
    .unwrap();
    let added = added_ids(&state, cluster_id).await;
    assert!(
        added.iter().any(|id| id == "newcomer"),
        "switching a mod back on opts the mods back in: {added:?}"
    );
}

#[tokio::test]
async fn switching_a_new_mod_on_beside_switched_off_ones_installs_it() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![
            managed_file(true),
            newly_shipped_file(),
            resource_pack_file(),
        ]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_mods_opted_out(&state, BUNDLE).await;
    added_ids(&state, cluster_id).await;

    set_bundle_package_enabled(
        cluster_id,
        BUNDLE,
        "newcomer",
        true,
        true,
        &state.services.content(),
    )
    .await
    .unwrap();
    let added = added_ids(&state, cluster_id).await;
    assert!(
        added.iter().any(|id| id == "newcomer"),
        "sodium staying off must not swallow the user switching newcomer on: {added:?}"
    );
}

#[tokio::test]
async fn the_package_list_does_not_save_the_opt_out() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![managed_file(true), resource_pack_file()]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_mods_opted_out(&state, BUNDLE).await;

    let status = get_bundles_with_update_status(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();
    let types = &status
        .iter()
        .find(|b| b.archive.manifest.name == BUNDLE)
        .unwrap()
        .opted_in_types;
    assert!(!types.contains(&ContentType::Mod), "{types:?}");
    assert!(
        bundle_dao::list_type_opt_outs(&state.services.db, cluster_id)
            .await
            .unwrap()
            .is_empty(),
        "only the updater saves the opt-out"
    );

    added_ids(&state, cluster_id).await;
    assert!(
        !bundle_dao::list_type_opt_outs(&state.services.db, cluster_id)
            .await
            .unwrap()
            .is_empty()
    );
}

async fn seed_opt_in_bundle(state: &LauncherState, name: &str) {
    let mut m = named_manifest(name, vec![managed_file(true)]);
    m.enabled = false;
    seed_bundle_with_pack(state, m).await;
}

async fn seed_bundle_with_pack(state: &LauncherState, m: BundleManifest) {
    let name = &m.name.clone();
    oneclient_core::dev::seed_bundle_archive(state, m)
        .await
        .unwrap();

    let dir = oneclient_common::paths::data_dir().unwrap().join("bundles");
    std::fs::create_dir_all(&dir).unwrap();
    let mut empty_zip = b"PK\x05\x06".to_vec();
    empty_zip.extend([0u8; 18]);
    std::fs::write(dir.join(format!("{name}.mrpack")), empty_zip).unwrap();
}

#[tokio::test]
async fn taking_a_bundle_at_the_prompt_records_the_choice() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    seed_opt_in_bundle(&state, "Opt In").await;
    let cluster_id = cluster_with_tracked_mod(&state).await;
    let ctx = state.services.content();
    let pending = || async {
        oneclient_content::bundles::pending_bundle_choices(cluster_id, state.bundles.as_ref(), &ctx)
            .await
            .unwrap()
            .into_iter()
            .map(|archive| archive.manifest.name)
            .collect::<Vec<_>>()
    };

    assert_eq!(pending().await, ["Opt In".to_string()]);

    oneclient_content::bundles::set_bundle_choices(
        cluster_id,
        &[("Opt In".to_string(), false)],
        &ctx,
    )
    .await
    .unwrap();
    assert!(pending().await.is_empty());
}

#[tokio::test]
async fn turning_a_bundle_down_removes_what_it_installed() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    seed_opt_in_bundle(&state, BUNDLE).await;
    let cluster_id = cluster_with_tracked_mod(&state).await;
    let ctx = state.services.content();

    oneclient_content::bundles::set_bundle_choices(cluster_id, &[(BUNDLE.to_string(), true)], &ctx)
        .await
        .unwrap();
    let tracked = bundle_dao::list_bundle_tracked(&state.services.db, cluster_id)
        .await
        .unwrap();
    assert_eq!(tracked.len(), 1, "taking the bundle keeps its mods");
    bundle_dao::save_override(
        &state.services.db,
        cluster_id,
        BUNDLE,
        PROJECT_ID,
        OverrideType::Disabled,
    )
    .await
    .unwrap();

    oneclient_content::bundles::set_bundle_choices(
        cluster_id,
        &[(BUNDLE.to_string(), false)],
        &ctx,
    )
    .await
    .unwrap();
    let tracked = bundle_dao::list_bundle_tracked(&state.services.db, cluster_id)
        .await
        .unwrap();
    assert!(tracked.is_empty());
    let overrides = bundle_dao::list_overrides(&state.services.db, cluster_id)
        .await
        .unwrap();
    assert_eq!(overrides.len(), 1, "a mod the user switched off stays off");
}

#[tokio::test]
async fn skyblock_nobody_asked_for_is_taken_out_of_an_old_26_3_cluster_once() {
    const SKYBLOCK: &str = "Test [SkyBlock]";
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let db = &state.services.db;
    let mut m = named_manifest(SKYBLOCK, vec![managed_file(true)]);
    m.enabled = false;
    m.category = "SkyBlock".to_string();
    m.mc_version = "26.3".to_string();
    seed_bundle_with_pack(&state, m).await;
    let cluster_id = cluster_with_tracked_mod(&state).await;
    let ctx = state.services.content();
    let track = || {
        bundle_dao::track_bundle_artifact(db, cluster_id, HASH, SKYBLOCK, VERSION_ID, PROJECT_ID)
    };
    let apply = || {
        oneclient_content::bundles::apply_bundle_updates(
            cluster_id,
            state.bundles.as_ref(),
            &ctx,
            None,
        )
    };
    track().await.unwrap();
    bundle_dao::save_bundle_choices(db, cluster_id, &[(SKYBLOCK.to_string(), true)])
        .await
        .unwrap();

    sqlx::query("UPDATE clusters SET mc_version = '26.3' WHERE id = ?")
        .bind(cluster_id)
        .execute(db)
        .await
        .unwrap();
    apply().await.unwrap();
    assert_eq!(
        bundle_dao::list_bundle_tracked(db, cluster_id)
            .await
            .unwrap()
            .len(),
        1,
        "a cluster made after bundles started asking is left alone"
    );

    sqlx::query("DELETE FROM applied_migrations")
        .execute(db)
        .await
        .unwrap();
    sqlx::query("UPDATE clusters SET created_at = '2020-01-01T00:00:00+00:00' WHERE id = ?")
        .bind(cluster_id)
        .execute(db)
        .await
        .unwrap();
    apply().await.unwrap();
    assert!(
        bundle_dao::list_bundle_tracked(db, cluster_id)
            .await
            .unwrap()
            .is_empty()
    );

    artifact_dao::insert_artifact(
        db,
        HASH,
        ContentType::Mod as i64,
        "artifacts/sodium.jar",
        "sodium.jar",
        Some(1),
    )
    .await
    .unwrap();
    artifact_dao::link_cluster_artifact(db, cluster_id, HASH, "sodium.jar")
        .await
        .unwrap();
    track().await.unwrap();
    apply().await.unwrap();
    assert_eq!(
        bundle_dao::list_bundle_tracked(db, cluster_id)
            .await
            .unwrap()
            .len(),
        1,
        "taking SkyBlock again later is not undone"
    );
}

#[tokio::test]
async fn an_opt_in_bundle_switched_off_by_accident_is_repaired_at_install() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    seed_opt_in_bundle(&state, BUNDLE).await;
    let cluster_id = cluster_with_tracked_mod(&state).await;
    artifact_dao::update_cluster_artifact(&state.services.db, cluster_id, HASH, "sodium.jar", 0)
        .await
        .unwrap();

    install_cluster_bundles(
        cluster_id,
        state.bundles.as_ref(),
        None,
        &state.services.content(),
    )
    .await
    .unwrap();

    let tracked = bundle_dao::list_bundle_tracked(&state.services.db, cluster_id)
        .await
        .unwrap();
    assert!(
        tracked.iter().all(|row| row.enabled != 0),
        "nothing recorded the disable so the install should undo it"
    );
}

async fn cluster_mod_enabled(state: &LauncherState, cluster_id: i64) -> bool {
    artifact_dao::get_cluster_artifact(&state.services.db, cluster_id, HASH)
        .await
        .unwrap()
        .unwrap()
        .enabled
        != 0
}

async fn heal(state: &LauncherState, cluster_id: i64) {
    let ctx = state.services.content();
    let archives = state
        .bundles
        .archives_for(&ctx, MC_VERSION, GameLoader::Fabric)
        .await
        .unwrap();
    oneclient_content::bundles::heal_bundle_activity(cluster_id, &archives, &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_mod_left_out_of_a_running_search_is_not_switched_back_on() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    seed_opt_in_bundle(&state, BUNDLE).await;
    let cluster_id = cluster_with_tracked_mod(&state).await;
    let db = &state.services.db;

    artifact_dao::update_cluster_artifact(db, cluster_id, HASH, "sodium.jar", 0)
        .await
        .unwrap();
    bisect_dao::start(
        db,
        cluster_id,
        &[("other".to_string(), "Other".to_string())],
    )
    .await
    .unwrap();

    heal(&state, cluster_id).await;

    assert!(
        !cluster_mod_enabled(&state, cluster_id).await,
        "turning it on would add a mod the search never tested"
    );
}

#[tokio::test]
async fn a_mod_held_from_a_declined_bundle_stays_off_once_the_user_switches_it_off() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    seed_opt_in_bundle(&state, BUNDLE).await;
    let cluster_id = cluster_with_tracked_mod(&state).await;
    let db = &state.services.db;

    artifact_dao::update_cluster_artifact(db, cluster_id, HASH, "sodium.jar", 0)
        .await
        .unwrap();
    bundle_dao::save_override(db, cluster_id, BUNDLE, PROJECT_ID, OverrideType::Disabled)
        .await
        .unwrap();

    for _ in 0..2 {
        oneclient_content::bundles::apply_bundle_updates(
            cluster_id,
            state.bundles.as_ref(),
            &state.services.content(),
            None,
        )
        .await
        .unwrap();
        assert!(
            !cluster_mod_enabled(&state, cluster_id).await,
            "the launch must not switch the mod back on"
        );
    }

    let overrides = bundle_dao::list_overrides(db, cluster_id).await.unwrap();
    assert_eq!(overrides.len(), 1, "the user's choice is kept");
}

#[tokio::test]
async fn a_removal_filed_by_declining_a_bundle_does_not_hold_a_held_mod_off() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    seed_opt_in_bundle(&state, BUNDLE).await;
    let cluster_id = cluster_with_tracked_mod(&state).await;
    let db = &state.services.db;

    artifact_dao::update_cluster_artifact(db, cluster_id, HASH, "sodium.jar", 0)
        .await
        .unwrap();
    bundle_dao::save_override(db, cluster_id, BUNDLE, PROJECT_ID, OverrideType::Removed)
        .await
        .unwrap();

    heal(&state, cluster_id).await;

    assert!(
        cluster_mod_enabled(&state, cluster_id).await,
        "declining the bundle is not a choice about a mod the user holds"
    );
}

#[tokio::test]
async fn a_disable_left_on_a_mod_that_came_back_on_is_cleared_once() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    seed_opt_in_bundle(&state, BUNDLE).await;
    let cluster_id = cluster_with_tracked_mod(&state).await;
    let db = &state.services.db;

    bundle_dao::save_override(db, cluster_id, BUNDLE, PROJECT_ID, OverrideType::Disabled)
        .await
        .unwrap();

    heal(&state, cluster_id).await;
    assert!(
        cluster_mod_enabled(&state, cluster_id).await,
        "the mod stays on the way the user last saw it"
    );
    assert!(
        bundle_dao::list_overrides(db, cluster_id)
            .await
            .unwrap()
            .is_empty(),
        "the leftover choice is cleared so a later update cannot switch it off"
    );

    artifact_dao::update_cluster_artifact(db, cluster_id, HASH, "sodium.jar", 0)
        .await
        .unwrap();
    bundle_dao::save_override(db, cluster_id, BUNDLE, PROJECT_ID, OverrideType::Disabled)
        .await
        .unwrap();

    heal(&state, cluster_id).await;
    assert!(
        !cluster_mod_enabled(&state, cluster_id).await,
        "a choice made after the cleanup is honoured"
    );
    assert_eq!(
        bundle_dao::list_overrides(db, cluster_id)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn an_opt_in_bundle_counts_as_taken_only_on_a_recorded_choice() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    seed_opt_in_bundle(&state, "Opt In").await;
    let cluster_id = cluster_with_tracked_mod(&state).await;
    let ctx = state.services.content();
    let taken = || async {
        let archives = state
            .bundles
            .archives_for(&ctx, MC_VERSION, GameLoader::Fabric)
            .await
            .unwrap();
        oneclient_content::bundles::taken_bundle_names(cluster_id, &archives, &ctx)
            .await
            .unwrap()
    };

    assert!(!taken().await.contains("Opt In"));

    oneclient_content::bundles::set_bundle_choices(
        cluster_id,
        &[("Opt In".to_string(), true)],
        &ctx,
    )
    .await
    .unwrap();
    assert!(taken().await.contains("Opt In"));
}

#[tokio::test]
async fn holding_one_mod_of_an_opt_in_bundle_does_not_bring_the_rest() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let mut m = named_manifest("Opt In", vec![managed_file(true), fabric_api_file()]);
    m.enabled = false;
    seed_bundle_with_pack(&state, m).await;
    let cluster_id = cluster_with_tracked_mod(&state).await;

    let check = check_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();

    assert!(
        check.additions_available.is_empty(),
        "an opt-in bundle nobody took should add nothing"
    );
}

/// Offline, or any other failed download, must leave the cluster exactly as it
/// was so it still launches: applying the removal while the new mod is missing
/// is the half-updated set that stops a game loading
#[tokio::test]
async fn a_failed_download_applies_nothing() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let mut unreachable = github_mod_file();
    if let BundleFileKind::External { file, .. } = &mut unreachable.kind {
        file.url = "unsupported://example.invalid/github-mod.jar".to_string();
    }
    oneclient_core::dev::seed_bundle_archive(&state, manifest(vec![unreachable]))
        .await
        .unwrap();
    let cluster_id = cluster_with_tracked_mod(&state).await;
    let ctx = state.services.content();

    let check = check_bundle_updates(cluster_id, state.bundles.as_ref(), &ctx)
        .await
        .unwrap();
    assert_eq!(check.removals_available.len(), 1);
    assert_eq!(check.additions_available.len(), 1);

    let result = oneclient_content::bundles::apply_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &ctx,
        None,
    )
    .await
    .unwrap();

    assert!(
        result.stopped_early,
        "an unfinished batch must not count as settled"
    );
    assert_eq!(
        result.failures().count(),
        0,
        "nothing was applied so there is nothing to block a launch over"
    );
    assert_eq!(result.downloads_failed.len(), 1);
    assert!(result.removals_applied.is_empty());
    assert!(
        artifact_dao::get_cluster_artifact(&state.services.db, cluster_id, HASH)
            .await
            .unwrap()
            .is_some(),
        "the mod due for removal stays until the whole batch can be applied"
    );
}

async fn mrpack(scratch: &str) -> Vec<u8> {
    let scratch = polyio::testing::ScratchDir::new(scratch);
    let path = scratch.path().join("served.mrpack");
    let index = serde_json::json!({
        "category": "test",
        "enabled": true,
        "versionId": "1",
        "name": "Served",
        "dependencies": { "minecraft": MC_VERSION, "fabric-loader": "0.16.0" },
    });

    let file = tokio::fs::File::create(&path).await.unwrap();
    let mut writer = async_zip::tokio::write::ZipFileWriter::with_tokio(file);
    let entry = async_zip::ZipEntryBuilder::new(
        "modrinth.index.json".to_string().into(),
        async_zip::Compression::Stored,
    );
    writer
        .write_entry_whole(entry, &serde_json::to_vec(&index).unwrap())
        .await
        .unwrap();
    writer.close().await.unwrap();

    tokio::fs::read(&path).await.unwrap()
}

async fn content_serving(
    state: &LauncherState,
    entries: Vec<(&'static str, &'static str, Vec<u8>)>,
) -> ContentCtx {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let mut loaders: HashMap<&str, Vec<serde_json::Value>> = HashMap::new();
    for (loader, path, archive) in &entries {
        loaders.entry(loader).or_default().push(serde_json::json!({
            "path": path,
            "sha1": polyio::sha1_bytes(archive),
        }));
    }
    let catalog = serde_json::json!({ "versions": { MC_VERSION: loaders } })
        .to_string()
        .into_bytes();

    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let mut request = [0u8; 1024];
            let read = stream.read(&mut request).await.unwrap_or(0);
            let request = String::from_utf8_lossy(&request[..read]).to_string();
            let body = entries
                .iter()
                .find(|(_, path, _)| request.starts_with(&format!("GET {path} ")))
                .map_or(&catalog, |(_, _, archive)| archive);
            let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len());
            let _ = stream.write_all(head.as_bytes()).await;
            let _ = stream.write_all(body).await;
        }
    });

    let mut content = state.services.content();
    content.net = oneclient_net::RequestClient::new(oneclient_net::NetConfig {
        meta_url_base: format!("http://127.0.0.1:{port}"),
        ..oneclient_net::NetConfig::default()
    })
    .unwrap();
    content
}

async fn listed_bundles(state: &LauncherState, content: &ContentCtx) -> Vec<String> {
    state
        .bundles
        .list_for(content, MC_VERSION, GameLoader::Fabric)
        .await
        .unwrap()
        .into_iter()
        .map(|bundle| bundle.name)
        .collect()
}

#[tokio::test]
async fn an_archive_that_fails_to_stage_keeps_the_previous_catalog() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(&state, manifest(vec![pinned_to("v2")]))
        .await
        .unwrap();
    let content = content_serving(
        &state,
        vec![
            ("fabric", "/stages-fine.mrpack", mrpack("stages-fine").await),
            ("fabric", "/not-a-zip.mrpack", b"junk".to_vec()),
        ],
    )
    .await;

    let before = state.bundles.catalog_revision();
    assert!(!state.bundles.sync(&content).await.unwrap());

    let root = oneclient_common::paths::bundles_dir().unwrap();
    assert_eq!(state.bundles.catalog_revision(), before);
    assert_eq!(listed_bundles(&state, &content).await, [BUNDLE]);
    assert!(!root.join("bundles/stages-fine.mrpack").exists());
}

#[tokio::test]
async fn an_unknown_loader_does_not_hold_the_catalog_back() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let content = content_serving(
        &state,
        vec![
            ("fabric", "/runnable.mrpack", mrpack("runnable").await),
            ("loader-from-the-future", "/future.mrpack", b"junk".to_vec()),
        ],
    )
    .await;

    assert!(state.bundles.sync(&content).await.unwrap());
    assert_eq!(listed_bundles(&state, &content).await, ["Served"]);
}

#[tokio::test]
async fn entries_named_alike_publish_their_own_archives() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let archive = mrpack("named-alike").await;
    let content = content_serving(
        &state,
        vec![
            ("fabric", "/a/named-alike.mrpack", archive.clone()),
            ("fabric", "/b/named-alike.mrpack", archive),
        ],
    )
    .await;

    assert!(state.bundles.sync(&content).await.unwrap());

    let bundles = state
        .bundles
        .list_for(&content, MC_VERSION, GameLoader::Fabric)
        .await
        .unwrap();
    assert_eq!(bundles.len(), 2);
    assert_ne!(bundles[0].path, bundles[1].path);
    assert!(bundles.iter().all(|bundle| bundle.path.exists()));
}

async fn serve_catalog(
    arrived: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let archive = mrpack("revision").await;
    let catalog = serde_json::json!({ "versions": { MC_VERSION: { "fabric": [{
        "path": "/archive.mrpack",
        "sha1": polyio::sha1_bytes(&archive),
    }]}}})
    .to_string()
    .into_bytes();

    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let (archive, catalog) = (archive.clone(), catalog.clone());
            let (arrived, release) = (arrived.clone(), release.clone());
            tokio::spawn(async move {
                let mut request = [0u8; 1024];
                let read = stream.read(&mut request).await.unwrap_or(0);
                let is_archive =
                    String::from_utf8_lossy(&request[..read]).starts_with("GET /archive.mrpack");
                let body = if is_archive {
                    arrived.notify_one();
                    release.notified().await;
                    archive
                } else {
                    catalog
                };
                let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len());
                let _ = stream.write_all(head.as_bytes()).await;
                let _ = stream.write_all(&body).await;
            });
        }
    });

    format!("http://127.0.0.1:{port}")
}

#[tokio::test]
async fn a_revision_moves_only_once_the_archives_are_in() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let arrived = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let meta_url_base = serve_catalog(arrived.clone(), release.clone()).await;

    let mut content = state.services.content();
    content.net = oneclient_net::RequestClient::new(oneclient_net::NetConfig {
        meta_url_base,
        ..oneclient_net::NetConfig::default()
    })
    .unwrap();

    let before = state.bundles.catalog_revision();
    let (synced, ()) = tokio::join!(state.bundles.sync(&content), async {
        arrived.notified().await;
        assert_eq!(
            state.bundles.catalog_revision(),
            before,
            "a check now would read the old archives"
        );
        release.notify_one();
    });

    assert!(synced.unwrap());
    assert_eq!(state.bundles.catalog_revision(), before + 1);
}

async fn serve_stalling_catalog() -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let archive = b"archive".to_vec();
    let catalog = serde_json::json!({ "versions": { MC_VERSION: { "fabric": [
        { "path": "/first-of-two.mrpack", "sha1": polyio::sha1_bytes(&archive) },
        { "path": "/stalled.mrpack", "sha1": polyio::sha1_bytes(b"never sent") },
    ]}}})
    .to_string()
    .into_bytes();

    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let (archive, catalog) = (archive.clone(), catalog.clone());
            tokio::spawn(async move {
                let mut request = [0u8; 1024];
                let read = stream.read(&mut request).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&request[..read]).to_string();
                if request.starts_with("GET /stalled.mrpack") {
                    std::future::pending::<()>().await;
                }
                let body = if request.starts_with("GET /first-of-two.mrpack") {
                    archive
                } else {
                    catalog
                };
                let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len());
                let _ = stream.write_all(head.as_bytes()).await;
                let _ = stream.write_all(&body).await;
            });
        }
    });

    format!("http://127.0.0.1:{port}")
}

#[tokio::test]
async fn a_timed_out_sync_publishes_no_archive() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let mut content = state.services.content();
    content.net = oneclient_net::RequestClient::new(oneclient_net::NetConfig {
        meta_url_base: serve_stalling_catalog().await,
        ..oneclient_net::NetConfig::default()
    })
    .unwrap();

    let before = state.bundles.catalog_revision();
    let budget = std::time::Duration::from_millis(500);
    tokio::time::timeout(budget, state.bundles.sync(&content))
        .await
        .expect_err("the second archive never arrives");

    let root = oneclient_common::paths::bundles_dir().unwrap();
    assert_eq!(state.bundles.catalog_revision(), before);
    assert!(!root.join("bundles/first-of-two.mrpack").exists());
    assert!(
        root.join("bundles/first-of-two.mrpack.staged").exists(),
        "the next sync resumes from it"
    );
}
