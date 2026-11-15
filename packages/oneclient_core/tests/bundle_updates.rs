use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use async_zip::tokio::write::ZipFileWriter;
use async_zip::{Compression, ZipEntryBuilder};
use oneclient_common::domain::{ContentType, GameLoader, ProviderId};
use oneclient_content::bundles::{
    BundleFile, BundleFileKind, BundleFileType, BundleManifest, check_bundle_updates,
    get_bundles_with_update_status, install_cluster_bundles, on_user_disable_artifact,
    on_user_remove_artifact, set_bundle_package_enabled, wanted_bundle_files,
};
use oneclient_content::modpacks::{ModpackContents, ModpackManifest, install_modpack};
use oneclient_content::packages::release_migration::retarget_cluster_mods;
use oneclient_content::packages::{
    DependencyKind, FileIdentity, InstalledPackage, PackageBody, PackageProvider, PackageStore,
    Page, ProjectDetail, ProjectSummary, SearchFilters, VersionDependency, VersionDetail,
    VersionFile, VersionLookup, VersionSummary,
};
use oneclient_content::{ContentCtx, ContentResult};
use oneclient_core::LauncherState;
use oneclient_core::clusters::CreateClusterOptions;
use oneclient_db::dao::cluster::ClusterMigration;
use oneclient_db::dao::{
    artifact as artifact_dao, cluster as cluster_dao, cluster_bisect as bisect_dao,
    cluster_bundle as bundle_dao, cluster_flag as flag_dao,
};
use oneclient_db::models::OverrideType;

const BUNDLE: &str = "Test Bundle";
const MC_VERSION: &str = "1.21.1";
const PROJECT_ID: &str = "sodium";
const VERSION_ID: &str = "v1";
const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SHARED_BUNDLE: &str = "Shared Bundle";
const FABRIC_API_HASH: &str = "dddddddddddddddddddddddddddddddddddddddd";
const HIDDEN_DEP_HASH: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";

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

async fn cluster_with_tracked_mod(state: &LauncherState) -> i64 {
    let global = state.settings.read().global_game_settings.clone();
    let cluster = state
        .clusters
        .create(
            &global,
            CreateClusterOptions::new("Bundle Cluster", MC_VERSION, GameLoader::Fabric),
        )
        .await
        .unwrap();

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

    artifact_dao::link_cluster_artifact(&state.services.db, cluster.id, HASH, "sodium.jar")
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

    bundle_dao::track_bundle_artifact(
        &state.services.db,
        cluster.id,
        HASH,
        BUNDLE,
        VERSION_ID,
        PROJECT_ID,
    )
    .await
    .unwrap();

    cluster.id
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

async fn opted_in(state: &LauncherState, cluster_id: i64, name: &str) -> HashSet<ContentType> {
    get_bundles_with_update_status(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap()
    .into_iter()
    .find(|b| b.archive.manifest.name == name)
    .unwrap_or_else(|| panic!("{name} missing from the package list"))
    .opted_in_types
}

#[tokio::test]
async fn a_bundle_not_installed_yet_counts_as_opted_in() {
    const NEW_BUNDLE: &str = "New Bundle";
    const DECLINED: &str = "Declined Bundle";
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let db = &state.services.db;
    let mut retained = manifest(vec![managed_file(true)]);
    let mut added = named_manifest(NEW_BUNDLE, vec![newly_shipped_file()]);
    let mut declined = named_manifest(DECLINED, vec![fabric_api_file()]);
    declined.enabled = false;
    for m in [&mut retained, &mut added, &mut declined] {
        m.mc_version = "26.3".to_string();
    }
    for m in [retained, added, declined] {
        oneclient_core::dev::seed_bundle_archive(&state, m)
            .await
            .unwrap();
    }
    let cluster_id = cluster_with_tracked_mod(&state).await;
    oneclient_content::bundles::set_bundle_choices(
        cluster_id,
        &[(DECLINED.to_string(), false)],
        &state.services.content(),
    )
    .await
    .unwrap();
    assert!(
        bundle_dao::list_bundle_choices(db, cluster_id)
            .await
            .unwrap()
            .contains(&(DECLINED.to_string(), false))
    );
    let source = cluster_dao::get_by_id(db, cluster_id)
        .await
        .unwrap()
        .unwrap();
    cluster_dao::migrate_version_if_unchanged(
        db,
        &source,
        ClusterMigration {
            mc_version: "26.3",
            mc_loader: source.mc_loader,
            mc_loader_version: source.mc_loader_version.as_deref(),
            stage: source.stage,
            name: None,
            folder_name: &source.folder_name,
        },
    )
    .await
    .unwrap()
    .unwrap();

    let bundles = get_bundles_with_update_status(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();
    let opted_in = |name: &str| {
        &bundles
            .iter()
            .find(|b| b.archive.manifest.name == name)
            .unwrap_or_else(|| panic!("{name} missing from the package list"))
            .opted_in_types
    };

    assert!(opted_in(BUNDLE).contains(&ContentType::Mod));
    let new_bundle = bundles
        .iter()
        .find(|b| b.archive.manifest.name == NEW_BUNDLE)
        .unwrap();
    assert!(
        new_bundle.files.iter().any(|(file, _)| {
            file.kind.package_id() == "newcomer"
                && new_bundle.opted_in_types.contains(&file.content_type())
        }),
        "a bundle the migrated cluster has not installed yet still belongs in the All tab"
    );
    assert!(
        opted_in(DECLINED).is_empty(),
        "a declined bundle stays opted out"
    );
}

#[tokio::test]
async fn an_accepted_opt_in_bundle_not_installed_yet_counts_as_opted_in() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let mut opt_in = named_manifest("Opt In", vec![newly_shipped_file()]);
    opt_in.enabled = false;
    oneclient_core::dev::seed_bundle_archive(&state, opt_in)
        .await
        .unwrap();
    let cluster_id = cluster_with_tracked_mod(&state).await;
    oneclient_content::bundles::set_bundle_choices(
        cluster_id,
        &[("Opt In".to_string(), true)],
        &state.services.content(),
    )
    .await
    .unwrap();

    assert!(
        opted_in(&state, cluster_id, "Opt In")
            .await
            .contains(&ContentType::Mod)
    );
}

#[tokio::test]
async fn a_bundle_not_installed_yet_with_nothing_switched_on_is_not_opted_in() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let mut optional = newly_shipped_file();
    optional.enabled = false;
    let mut optional_pack = resource_pack_file();
    optional_pack.enabled = false;
    for m in [
        manifest(vec![managed_file(true)]),
        named_manifest("Optional Only", vec![optional]),
        named_manifest("Switched Off", vec![newly_shipped_file()]),
        named_manifest("Mixed", vec![newly_shipped_file(), optional_pack]),
    ] {
        oneclient_core::dev::seed_bundle_archive(&state, m)
            .await
            .unwrap();
    }
    let cluster_id = cluster_with_tracked_mod(&state).await;
    for bundle in ["Switched Off", "Mixed"] {
        bundle_dao::save_override(
            &state.services.db,
            cluster_id,
            bundle,
            "newcomer",
            OverrideType::Disabled,
        )
        .await
        .unwrap();
    }

    for name in ["Optional Only", "Switched Off", "Mixed"] {
        let types = opted_in(&state, cluster_id, name).await;
        assert!(types.is_empty(), "{name} has nothing to install: {types:?}");
    }
}

#[tokio::test]
async fn an_installed_bundle_does_not_lose_its_opt_out_to_one_not_installed_yet() {
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
    for (package_id, choice) in [
        (PROJECT_ID, OverrideType::Disabled),
        ("looks", OverrideType::Enabled),
    ] {
        bundle_dao::save_override(&state.services.db, cluster_id, BUNDLE, package_id, choice)
            .await
            .unwrap();
    }

    let types = opted_in(&state, cluster_id, BUNDLE).await;
    assert!(
        !types.contains(&ContentType::Mod) && types.contains(&ContentType::ResourcePack),
        "the list must keep the opt-out the updater keeps: {types:?}"
    );
}

async fn cluster_with_every_copy_off(state: &LauncherState) -> i64 {
    oneclient_core::dev::seed_bundle_archive(
        state,
        manifest(vec![
            managed_file(true),
            newly_shipped_file(),
            resource_pack_file(),
        ]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_tracked_mod(state).await;
    artifact_dao::update_cluster_artifact(&state.services.db, cluster_id, HASH, "sodium.jar", 0)
        .await
        .unwrap();
    cluster_id
}

#[tokio::test]
async fn a_bundle_with_every_copy_switched_off_by_accident_counts_as_opted_in() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_every_copy_off(&state).await;

    let types = opted_in(&state, cluster_id, BUNDLE).await;
    assert!(
        types.contains(&ContentType::Mod),
        "launch switches these copies back on so the bundle is still live: {types:?}"
    );
    assert!(
        added_ids(&state, cluster_id)
            .await
            .contains(&"newcomer".to_string())
    );
}

async fn track_bundle_copy(state: &LauncherState, cluster_id: i64, file: &BundleFile) {
    let BundleFileKind::Managed {
        project_id,
        version_id,
        sha1,
        ..
    } = &file.kind
    else {
        unreachable!("only managed files are tracked here");
    };
    let db = &state.services.db;
    let file_name = file.path.rsplit('/').next().unwrap();
    artifact_dao::insert_artifact(
        db,
        sha1,
        file.content_type() as i64,
        &format!("artifacts/{file_name}"),
        file_name,
        Some(1),
    )
    .await
    .unwrap();
    artifact_dao::link_cluster_artifact(db, cluster_id, sha1, file_name)
        .await
        .unwrap();
    artifact_dao::upsert_provider_release(
        db,
        ProviderId::Modrinth as i64,
        project_id,
        version_id,
        sha1,
        file_name,
        "1.0.0",
        None,
        MC_VERSION,
        "fabric",
    )
    .await
    .unwrap();
    bundle_dao::track_bundle_artifact(db, cluster_id, sha1, BUNDLE, version_id, project_id)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_hidden_dependency_switched_off_by_accident_keeps_the_bundle_live() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![hidden_dependency_file(), newly_shipped_file()]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_tracked_mod(&state).await;
    track_bundle_copy(&state, cluster_id, &hidden_dependency_file()).await;
    let db = &state.services.db;
    for (hash, file_name) in [
        (HASH, "sodium.jar"),
        (HIDDEN_DEP_HASH, "sodium-extra-lib.jar"),
    ] {
        artifact_dao::update_cluster_artifact(db, cluster_id, hash, file_name, 0)
            .await
            .unwrap();
    }

    let types = opted_in(&state, cluster_id, BUNDLE).await;
    assert!(types.contains(&ContentType::Mod), "{types:?}");
    assert!(
        added_ids(&state, cluster_id)
            .await
            .contains(&"newcomer".to_string())
    );
    heal(&state, cluster_id).await;
    let restored = artifact_dao::get_cluster_artifact(db, cluster_id, HIDDEN_DEP_HASH)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(restored.enabled, 0);
}

#[tokio::test]
async fn a_bundle_with_every_copy_switched_off_on_purpose_stays_opted_out() {
    for (filed_under, choice) in [
        (BUNDLE, OverrideType::Disabled),
        (BUNDLE, OverrideType::Removed),
        ("Old Bundle", OverrideType::Disabled),
        ("Old Bundle", OverrideType::Removed),
    ] {
        let state = oneclient_core::dev::ephemeral_state().await.unwrap();
        let cluster_id = cluster_with_every_copy_off(&state).await;
        bundle_dao::save_override(
            &state.services.db,
            cluster_id,
            filed_under,
            PROJECT_ID,
            choice,
        )
        .await
        .unwrap();
        let case = format!("{filed_under} {choice:?}");

        assert_stays_opted_out(&state, cluster_id, &case).await;
        heal(&state, cluster_id).await;
        assert!(!cluster_mod_enabled(&state, cluster_id).await, "{case}");
    }
}

async fn assert_stays_opted_out(state: &LauncherState, cluster_id: i64, case: &str) {
    let types = opted_in(state, cluster_id, BUNDLE).await;
    assert!(types.is_empty(), "{case}: {types:?}");
    let added = added_ids(state, cluster_id).await;
    assert!(added.is_empty(), "{case}: {added:?}");
}

#[tokio::test]
async fn a_switched_off_copy_launch_leaves_off_keeps_the_bundle_opted_out() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(
        &state,
        manifest(vec![newly_shipped_file(), resource_pack_file()]),
    )
    .await
    .unwrap();
    let cluster_id = cluster_with_tracked_mod(&state).await;
    artifact_dao::update_cluster_artifact(&state.services.db, cluster_id, HASH, "sodium.jar", 0)
        .await
        .unwrap();
    assert_stays_opted_out(&state, cluster_id, "no longer shipped").await;

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_every_copy_off(&state).await;
    bisect_dao::start(
        &state.services.db,
        cluster_id,
        &[("other".to_string(), "Other".to_string())],
    )
    .await
    .unwrap();
    assert_stays_opted_out(&state, cluster_id, "bisect running").await;

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_every_copy_off(&state).await;
    track_other_sodium_copy(&state, cluster_id, "Other Bundle").await;
    assert_stays_opted_out(&state, cluster_id, "another copy live").await;
}

const OTHER_COPY: &str = "ffffffffffffffffffffffffffffffffffffffff";

async fn track_other_sodium_copy(state: &LauncherState, cluster_id: i64, bundle: &str) {
    let db = &state.services.db;
    artifact_dao::insert_artifact(
        db,
        OTHER_COPY,
        ContentType::Mod as i64,
        "artifacts/sodium-other.jar",
        "sodium-other.jar",
        Some(1),
    )
    .await
    .unwrap();
    artifact_dao::link_cluster_artifact(db, cluster_id, OTHER_COPY, "sodium-other.jar")
        .await
        .unwrap();
    bundle_dao::track_bundle_artifact(db, cluster_id, OTHER_COPY, bundle, VERSION_ID, PROJECT_ID)
        .await
        .unwrap();
}

async fn reverse_unordered_selects(state: &LauncherState) {
    let db = &state.services.db;
    let mut held = Vec::new();
    for _ in 0..db.options().get_max_connections() {
        let mut conn = db.acquire().await.unwrap();
        sqlx::query("PRAGMA reverse_unordered_selects = ON")
            .execute(&mut *conn)
            .await
            .unwrap();
        held.push(conn);
    }
}

#[tokio::test]
async fn the_bundle_launch_heals_first_takes_a_shared_switched_off_mod() {
    // Tracked rows come back in bundle name order through an index, the same order launch visits archives, so the scan is reversed to catch a prediction that follows row order
    for other in ["A Bundle", "Z Bundle"] {
        let state = oneclient_core::dev::ephemeral_state().await.unwrap();
        reverse_unordered_selects(&state).await;
        for name in [BUNDLE, other] {
            seed_bundle_with_pack(&state, named_manifest(name, vec![managed_file(true)])).await;
        }
        let cluster_id = cluster_with_tracked_mod(&state).await;
        track_other_sodium_copy(&state, cluster_id, other).await;
        let db = &state.services.db;
        for (hash, file_name) in [(HASH, "sodium.jar"), (OTHER_COPY, "sodium-other.jar")] {
            artifact_dao::update_cluster_artifact(db, cluster_id, hash, file_name, 0)
                .await
                .unwrap();
        }

        let mut predicted = Vec::new();
        for name in [BUNDLE, other] {
            if opted_in(&state, cluster_id, name)
                .await
                .contains(&ContentType::Mod)
            {
                predicted.push(name.to_string());
            }
        }
        install_cluster_bundles(
            cluster_id,
            state.bundles.as_ref(),
            None,
            &state.services.content(),
            None,
        )
        .await
        .unwrap();
        let healed: Vec<String> = bundle_dao::list_bundle_tracked(db, cluster_id)
            .await
            .unwrap()
            .into_iter()
            .filter(|row| row.enabled != 0)
            .filter_map(|row| row.bundle_name)
            .collect();

        assert_eq!(healed, [BUNDLE.min(other)], "{other}");
        assert_eq!(predicted, healed, "{other}");
    }
}

#[tokio::test]
async fn a_bundle_on_a_cluster_with_nothing_installed_counts_as_opted_in() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(&state, manifest(vec![newly_shipped_file()]))
        .await
        .unwrap();
    let global = state.settings.read().global_game_settings.clone();
    let cluster = state
        .clusters
        .create(
            &global,
            CreateClusterOptions::new("Fresh Cluster", MC_VERSION, GameLoader::Fabric),
        )
        .await
        .unwrap();

    assert!(
        opted_in(&state, cluster.id, BUNDLE)
            .await
            .contains(&ContentType::Mod)
    );
}

/// Package ids launch installs, picked from the same choices and overrides launch reads
async fn launch_installs(state: &LauncherState, cluster_id: i64) -> Vec<String> {
    let db = &state.services.db;
    let bundles = get_bundles_with_update_status(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();
    let accepted: HashSet<String> = bundle_dao::list_bundle_choices(db, cluster_id)
        .await
        .unwrap()
        .into_iter()
        .filter_map(|(name, accepted)| accepted.then_some(name))
        .collect();
    let overrides = bundle_dao::list_overrides(db, cluster_id).await.unwrap();
    wanted_bundle_files(
        bundles.iter().map(|b| &b.archive.manifest),
        &accepted,
        |bundle, package| {
            overrides
                .iter()
                .find(|o| o.bundle_name == bundle && o.package_id == package)
                .and_then(|o| OverrideType::parse(&o.override_type))
        },
    )
    .into_iter()
    .map(|file| file.kind.package_id())
    .collect()
}

#[tokio::test]
async fn a_mod_unticked_at_onboarding_keeps_the_rest_of_the_bundle_opted_in() {
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
    let global = state.settings.read().global_game_settings.clone();
    let cluster = state
        .clusters
        .create(
            &global,
            CreateClusterOptions::new("Fresh Cluster", MC_VERSION, GameLoader::Fabric),
        )
        .await
        .unwrap();
    oneclient_core::set_bundle_package_overrides(
        cluster.id,
        &[(
            BUNDLE.to_string(),
            "newcomer".to_string(),
            OverrideType::Removed,
        )],
        &state.services.content(),
    )
    .await
    .unwrap();

    let bundles = get_bundles_with_update_status(
        cluster.id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();
    let bundle = bundles
        .iter()
        .find(|b| b.archive.manifest.name == BUNDLE)
        .unwrap();

    assert!(
        bundle.opted_in_types.contains(&ContentType::Mod),
        "launch still installs the mods left ticked: {:?}",
        bundle.opted_in_types
    );
    assert_eq!(
        launch_installs(&state, cluster.id).await,
        [PROJECT_ID, "looks"]
    );
}

#[tokio::test]
async fn every_mod_unticked_at_onboarding_keeps_the_bundle_out_of_mods() {
    let optional_mod = BundleFile {
        enabled: false,
        path: "mods/optional.jar".to_string(),
        kind: BundleFileKind::Managed {
            provider: ProviderId::Modrinth,
            project_id: "optional".to_string(),
            version_id: "v1".to_string(),
            sha1: "ffffffffffffffffffffffffffffffffffffffff".to_string(),
        },
        ..newly_shipped_file()
    };
    for extra in [None, Some(optional_mod)] {
        let state = oneclient_core::dev::ephemeral_state().await.unwrap();
        let mut files = vec![
            managed_file(true),
            newly_shipped_file(),
            resource_pack_file(),
        ];
        files.extend(extra.clone());
        oneclient_core::dev::seed_bundle_archive(&state, manifest(files))
            .await
            .unwrap();
        let global = state.settings.read().global_game_settings.clone();
        let cluster = state
            .clusters
            .create(
                &global,
                CreateClusterOptions::new("Fresh Cluster", MC_VERSION, GameLoader::Fabric),
            )
            .await
            .unwrap();
        let unticked: Vec<_> = [PROJECT_ID, "newcomer"]
            .map(|id| (BUNDLE.to_string(), id.to_string(), OverrideType::Removed))
            .into();
        oneclient_core::set_bundle_package_overrides(
            cluster.id,
            &unticked,
            &state.services.content(),
        )
        .await
        .unwrap();

        let types = opted_in(&state, cluster.id, BUNDLE).await;
        assert!(
            !types.contains(&ContentType::Mod) && types.contains(&ContentType::ResourcePack),
            "{extra:?}: {types:?}"
        );
        assert_eq!(
            launch_installs(&state, cluster.id).await,
            ["looks"],
            "{extra:?}"
        );
    }
}

#[tokio::test]
async fn a_mod_type_not_installed_yet_ignores_the_resource_pack_already_installed() {
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
    let global = state.settings.read().global_game_settings.clone();
    let cluster = state
        .clusters
        .create(
            &global,
            CreateClusterOptions::new("Fresh Cluster", MC_VERSION, GameLoader::Fabric),
        )
        .await
        .unwrap();
    oneclient_core::set_bundle_package_overrides(
        cluster.id,
        &[(
            BUNDLE.to_string(),
            PROJECT_ID.to_string(),
            OverrideType::Removed,
        )],
        &state.services.content(),
    )
    .await
    .unwrap();
    track_bundle_copy(&state, cluster.id, &resource_pack_file()).await;

    let types = opted_in(&state, cluster.id, BUNDLE).await;
    assert!(
        types.contains(&ContentType::Mod),
        "launch still installs the mod left ticked: {types:?}"
    );
    assert_eq!(
        launch_installs(&state, cluster.id).await,
        ["newcomer", "looks"]
    );
}

#[tokio::test]
async fn an_installed_bundle_opted_out_of_mods_stays_out_when_a_mod_is_shipped() {
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
    for (package_id, choice) in [
        (PROJECT_ID, OverrideType::Disabled),
        ("looks", OverrideType::Enabled),
    ] {
        bundle_dao::save_override(&state.services.db, cluster_id, BUNDLE, package_id, choice)
            .await
            .unwrap();
    }

    let types = opted_in(&state, cluster_id, BUNDLE).await;
    assert!(
        !types.contains(&ContentType::Mod) && types.contains(&ContentType::ResourcePack),
        "{types:?}"
    );
    let check = check_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .unwrap();
    assert!(
        !check
            .additions_available
            .iter()
            .any(|a| a.new_file.kind.package_id() == "newcomer")
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
        None,
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

#[tokio::test]
async fn the_deadline_bounds_downloads_already_running() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let body: Vec<u8> = (0..64 * 1024).map(|i| (i % 251) as u8).collect();
    let sha1 = polyio::sha1_bytes(&body);
    let (url, connections) = serve_jar(body, Some(std::time::Duration::from_millis(1500))).await;

    let stalled = github_mod_served(&url, &sha1, 64 * 1024);
    oneclient_core::dev::seed_bundle_archive(&state, manifest(vec![stalled]))
        .await
        .unwrap();
    clusters_numbered_from(&state, 7000).await;
    let cluster_id = cluster_with_tracked_mod(&state).await;
    let ctx = state.services.content();

    for _ in 0..2 {
        let started = std::time::Instant::now();
        let result = oneclient_content::bundles::apply_bundle_updates(
            cluster_id,
            state.bundles.as_ref(),
            &ctx,
            Some(started + std::time::Duration::from_millis(300)),
        )
        .await
        .unwrap();

        assert!(
            started.elapsed() < std::time::Duration::from_secs(1),
            "took {:?}",
            started.elapsed()
        );
        assert!(result.stopped_early);
        assert_eq!(result.downloads_failed.len(), 1);
        assert!(result.downloads_failed[0].contains("ran out of time"));
    }

    let result = oneclient_content::bundles::apply_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &ctx,
        None,
    )
    .await
    .unwrap();
    assert!(
        result.downloads_failed.is_empty(),
        "{:?}",
        result.downloads_failed
    );
    assert_eq!(
        connections.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "later launches join the download already running"
    );
    assert!(
        artifact_dao::get_cluster_artifact(&state.services.db, cluster_id, &sha1)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn downloads_left_running_still_count_against_the_limit() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let in_flight = Arc::new(AtomicUsize::new(0));
    let most = Arc::new(AtomicUsize::new(0));
    let served = Arc::new(AtomicUsize::new(0));
    let mut files = Vec::new();
    for i in 0..12u8 {
        let body: Vec<u8> = (0..4096).map(|b| (b as u8) ^ i).collect();
        let sha1 = polyio::sha1_bytes(&body);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/mod-{i}.jar", listener.local_addr().unwrap());
        let (in_flight, most, served) = (in_flight.clone(), most.clone(), served.clone());
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let (body, in_flight, most, served) = (
                    body.clone(),
                    in_flight.clone(),
                    most.clone(),
                    served.clone(),
                );
                tokio::spawn(async move {
                    let mut request = [0u8; 1024];
                    let _ = stream.read(&mut request).await;
                    let now = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                    most.fetch_max(now, Ordering::SeqCst);
                    let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len());
                    let _ = stream.write_all(head.as_bytes()).await;
                    let (first, rest) = body.split_at(body.len() / 2);
                    let _ = stream.write_all(first).await;
                    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                    let _ = stream.write_all(rest).await;
                    in_flight.fetch_sub(1, Ordering::SeqCst);
                    served.fetch_add(1, Ordering::SeqCst);
                });
            }
        });
        let mut file = github_mod_served(&url, &sha1, 4096);
        file.path = format!("mods/mod-{i}.jar");
        if let BundleFileKind::External { file: ext, id, .. } = &mut file.kind {
            ext.name = format!("mod-{i}.jar");
            *id = Some(format!("mod-{i}"));
        }
        files.push(file);
    }
    let later = files.split_off(6);
    clusters_numbered_from(&state, 8000).await;
    let cluster_id = cluster_with_tracked_mod(&state).await;
    let ctx = state.services.content();

    for (shipped, deadline) in [
        (files, Some(std::time::Duration::from_millis(300))),
        (later, None),
    ] {
        oneclient_core::dev::seed_bundle_archive(&state, manifest(shipped))
            .await
            .unwrap();
        oneclient_content::bundles::apply_bundle_updates(
            cluster_id,
            state.bundles.as_ref(),
            &ctx,
            deadline.map(|budget| std::time::Instant::now() + budget),
        )
        .await
        .unwrap();
    }
    let started = std::time::Instant::now();
    while served.load(Ordering::SeqCst) < 12
        && started.elapsed() < std::time::Duration::from_secs(10)
    {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    assert_eq!(served.load(Ordering::SeqCst), 12);
    assert!(
        most.load(Ordering::SeqCst) <= 6,
        "{} downloads ran at once",
        most.load(Ordering::SeqCst)
    );
}

async fn clusters_numbered_from(state: &LauncherState, first: i64) {
    sqlx::query("INSERT INTO sqlite_sequence (name, seq) VALUES ('clusters', ?)")
        .bind(first - 1)
        .execute(&state.services.db)
        .await
        .unwrap();
}

async fn serve_jar(
    body: Vec<u8>,
    stall: Option<std::time::Duration>,
) -> (String, Arc<std::sync::atomic::AtomicUsize>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let connections = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn({
        let connections = connections.clone();
        async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                connections.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let body = body.clone();
                tokio::spawn(async move {
                    let mut request = [0u8; 1024];
                    let _ = stream.read(&mut request).await;
                    let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len());
                    let _ = stream.write_all(head.as_bytes()).await;
                    let (first, rest) = body.split_at(body.len() / 2);
                    let _ = stream.write_all(first).await;
                    if let Some(stall) = stall {
                        tokio::time::sleep(stall).await;
                    }
                    let _ = stream.write_all(rest).await;
                });
            }
        }
    });
    (format!("http://{addr}/github-mod.jar"), connections)
}

fn github_mod_served(url: &str, sha1: &str, size: u64) -> BundleFile {
    let mut file = github_mod_file();
    file.size = size;
    if let BundleFileKind::External { file: ext, .. } = &mut file.kind {
        ext.url = url.to_string();
        ext.sha1 = sha1.to_string();
        ext.size = size;
    }
    file
}

const OLD_VERSION_BUNDLE: &str = "Old Version Bundle";

async fn cluster_with_other_version_copy(
    state: &LauncherState,
    old_bundle: &str,
    current: BundleFile,
) -> i64 {
    let old_path = format!("bundles/1.20.1/{old_bundle}.mrpack");
    oneclient_db::dao::bundle::upsert_bundle(
        &state.services.db,
        oneclient_db::models::NewBundle {
            remote_path: &old_path,
            mc_version: "1.20.1",
            mc_loader: GameLoader::Fabric as i64,
            file_name: &format!("{old_bundle}.mrpack"),
            name: Some(old_bundle),
            version_id: Some("1"),
            category: Some("test"),
            loader_version: Some("0.16.0"),
            disk_path: &old_path,
            hidden: false,
            etag: None,
            synced_at: None,
        },
    )
    .await
    .unwrap();
    seed_bundle_with_pack(state, manifest(vec![current])).await;

    let global = state.settings.read().global_game_settings.clone();
    let cluster = state
        .clusters
        .create(
            &global,
            CreateClusterOptions::new("Moved Cluster", MC_VERSION, GameLoader::Fabric),
        )
        .await
        .unwrap();
    let db = &state.services.db;
    artifact_dao::insert_artifact(
        db,
        GITHUB_OLD_HASH,
        ContentType::Mod as i64,
        "artifacts/github-mod-old.jar",
        "github-mod.jar",
        Some(1),
    )
    .await
    .unwrap();
    artifact_dao::link_cluster_artifact(db, cluster.id, GITHUB_OLD_HASH, "github-mod.jar")
        .await
        .unwrap();
    bundle_dao::track_bundle_artifact(
        db,
        cluster.id,
        GITHUB_OLD_HASH,
        old_bundle,
        GITHUB_OLD_HASH,
        GITHUB_MOD_ID,
    )
    .await
    .unwrap();
    cluster.id
}

async fn linked(state: &LauncherState, cluster_id: i64, hash: &str) -> bool {
    artifact_dao::get_cluster_artifact(&state.services.db, cluster_id, hash)
        .await
        .unwrap()
        .is_some()
}

#[tokio::test]
async fn another_versions_bundle_copy_gives_way_to_the_current_build() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let body = b"github mod for the current version".to_vec();
    let sha1 = polyio::sha1_bytes(&body);
    let size = body.len() as u64;
    let (url, _) = serve_jar(body, None).await;
    let cluster_id = cluster_with_other_version_copy(
        &state,
        OLD_VERSION_BUNDLE,
        github_mod_served(&url, &sha1, size),
    )
    .await;

    install_cluster_bundles(
        cluster_id,
        state.bundles.as_ref(),
        None,
        &state.services.content(),
        None,
    )
    .await
    .unwrap();

    assert!(!linked(&state, cluster_id, GITHUB_OLD_HASH).await);
    assert!(linked(&state, cluster_id, &sha1).await);
}

#[tokio::test]
async fn another_versions_bundle_copy_is_switched_off_while_its_replacement_fails() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let unreachable = github_mod_served(
        "unsupported://example.invalid/github-mod.jar",
        "3333333333333333333333333333333333333333",
        1,
    );
    let cluster_id = cluster_with_other_version_copy(&state, OLD_VERSION_BUNDLE, unreachable).await;
    let ctx = state.services.content();

    install_cluster_bundles(cluster_id, state.bundles.as_ref(), None, &ctx, None)
        .await
        .unwrap();

    assert_eq!(
        enabled(&state, cluster_id, GITHUB_OLD_HASH).await,
        Some(false)
    );
    let overrides = bundle_dao::list_overrides(&state.services.db, cluster_id)
        .await
        .unwrap();
    assert!(overrides.is_empty(), "{overrides:?}");

    let body = b"github mod for the current version, served at last".to_vec();
    let sha1 = polyio::sha1_bytes(&body);
    let size = body.len() as u64;
    let (url, _) = serve_jar(body, None).await;
    seed_bundle_with_pack(&state, manifest(vec![github_mod_served(&url, &sha1, size)])).await;

    install_cluster_bundles(cluster_id, state.bundles.as_ref(), None, &ctx, None)
        .await
        .unwrap();

    assert!(!linked(&state, cluster_id, GITHUB_OLD_HASH).await);
    assert_eq!(enabled(&state, cluster_id, &sha1).await, Some(true));
}

#[tokio::test]
async fn a_switched_off_copy_the_current_bundle_ships_comes_back_on_while_staging_fails() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let mut unreachable = github_mod_served(
        "unsupported://example.invalid/other-mod.jar",
        "3333333333333333333333333333333333333333",
        1,
    );
    unreachable.path = "mods/other-mod.jar".to_string();
    if let BundleFileKind::External { file, id, .. } = &mut unreachable.kind {
        file.name = "other-mod.jar".to_string();
        *id = Some("other-mod".to_string());
    }
    let same = github_mod_served(
        "unsupported://example.invalid/github-mod.jar",
        GITHUB_OLD_HASH,
        1,
    );
    let cluster_id =
        cluster_with_other_version_copy(&state, OLD_VERSION_BUNDLE, same.clone()).await;
    seed_bundle_with_pack(&state, manifest(vec![same, unreachable])).await;
    let ctx = state.services.content();
    PackageStore::set_artifact_enabled_to(cluster_id, GITHUB_OLD_HASH, false, &ctx)
        .await
        .unwrap();

    let deadline = Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
    install_cluster_bundles(cluster_id, state.bundles.as_ref(), None, &ctx, deadline)
        .await
        .unwrap();

    assert_eq!(
        enabled(&state, cluster_id, GITHUB_OLD_HASH).await,
        Some(true)
    );
    let overrides = bundle_dao::list_overrides(&state.services.db, cluster_id)
        .await
        .unwrap();
    assert!(overrides.is_empty(), "{overrides:?}");
}

#[tokio::test]
async fn launch_install_stops_waiting_at_the_deadline() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let body: Vec<u8> = (0..64 * 1024).map(|i| (i % 239) as u8).collect();
    let sha1 = polyio::sha1_bytes(&body);
    let size = body.len() as u64;
    let (url, connections) = serve_jar(body, Some(std::time::Duration::from_millis(1500))).await;
    let moved = cluster_with_other_version_copy(
        &state,
        OLD_VERSION_BUNDLE,
        github_mod_served(&url, &sha1, size),
    )
    .await;
    let fresh = cluster_with_tracked_mod(&state).await;
    let ctx = state.services.content();

    let started = std::time::Instant::now();
    let deadline = Some(started + std::time::Duration::from_millis(300));
    let (a, b) = tokio::join!(
        install_cluster_bundles(fresh, state.bundles.as_ref(), None, &ctx, deadline),
        install_cluster_bundles(moved, state.bundles.as_ref(), None, &ctx, deadline),
    );
    a.unwrap();
    b.unwrap();

    assert!(
        started.elapsed() < std::time::Duration::from_secs(1),
        "took {:?}",
        started.elapsed()
    );
    assert!(!linked(&state, fresh, &sha1).await);
    assert!(!linked(&state, moved, &sha1).await);
    assert_eq!(enabled(&state, moved, GITHUB_OLD_HASH).await, Some(false));

    let (a, b) = tokio::join!(
        install_cluster_bundles(fresh, state.bundles.as_ref(), None, &ctx, None),
        install_cluster_bundles(moved, state.bundles.as_ref(), None, &ctx, None),
    );
    a.unwrap();
    b.unwrap();

    assert_eq!(connections.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(linked(&state, fresh, &sha1).await);
    assert_eq!(enabled(&state, moved, &sha1).await, Some(true));
    assert!(!linked(&state, moved, GITHUB_OLD_HASH).await);
}

#[tokio::test]
async fn launch_install_keeps_the_deadline_for_a_linked_build_missing_its_file() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let body: Vec<u8> = (0..64 * 1024).map(|i| (i % 233) as u8).collect();
    let sha1 = polyio::sha1_bytes(&body);
    let size = body.len() as u64;
    let (url, _) = serve_jar(body, Some(std::time::Duration::from_millis(1500))).await;
    let moved = cluster_with_other_version_copy(
        &state,
        OLD_VERSION_BUNDLE,
        github_mod_served(&url, &sha1, size),
    )
    .await;
    let db = &state.services.db;
    artifact_dao::insert_artifact(
        db,
        &sha1,
        ContentType::Mod as i64,
        "artifacts/missing-github-mod.jar",
        "missing-github-mod.jar",
        Some(size as i64),
    )
    .await
    .unwrap();
    artifact_dao::link_cluster_artifact(db, moved, &sha1, "missing-github-mod.jar")
        .await
        .unwrap();
    let ctx = state.services.content();

    let started = std::time::Instant::now();
    let deadline = Some(started + std::time::Duration::from_millis(300));
    install_cluster_bundles(moved, state.bundles.as_ref(), None, &ctx, deadline)
        .await
        .unwrap();

    assert!(
        started.elapsed() < std::time::Duration::from_secs(1),
        "took {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn a_disable_filed_under_another_versions_bundle_carries_over() {
    disable_carries_over_when_filed_under(OLD_VERSION_BUNDLE).await;
}

#[tokio::test]
async fn a_disable_filed_under_an_earlier_bundle_carries_over() {
    disable_carries_over_when_filed_under("Earlier Bundle").await;
}

async fn disable_carries_over_when_filed_under(filed_under: &str) {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let body = b"github mod the user switched off".to_vec();
    let sha1 = polyio::sha1_bytes(&body);
    let size = body.len() as u64;
    let (url, connections) = serve_jar(body, None).await;
    let cluster_id = cluster_with_other_version_copy(
        &state,
        OLD_VERSION_BUNDLE,
        github_mod_served(&url, &sha1, size),
    )
    .await;
    bundle_dao::save_override(
        &state.services.db,
        cluster_id,
        filed_under,
        GITHUB_MOD_ID,
        OverrideType::Disabled,
    )
    .await
    .unwrap();

    install_cluster_bundles(
        cluster_id,
        state.bundles.as_ref(),
        None,
        &state.services.content(),
        None,
    )
    .await
    .unwrap();

    let overrides = bundle_dao::list_overrides(&state.services.db, cluster_id)
        .await
        .unwrap();
    assert!(
        overrides.iter().any(|o| o.bundle_name == BUNDLE
            && o.package_id == GITHUB_MOD_ID
            && OverrideType::parse(&o.override_type) == Some(OverrideType::Disabled)),
        "{overrides:?}"
    );
    assert!(!linked(&state, cluster_id, &sha1).await);
    assert_eq!(connections.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[tokio::test]
async fn launch_install_links_nothing_while_part_of_the_bundle_is_downloading() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let fast_body: Vec<u8> = (0..64 * 1024).map(|i| (i % 227) as u8).collect();
    let slow_body: Vec<u8> = (0..64 * 1024).map(|i| (i % 229) as u8).collect();
    let fast_sha1 = polyio::sha1_bytes(&fast_body);
    let slow_sha1 = polyio::sha1_bytes(&slow_body);
    let size = fast_body.len() as u64;
    let (fast_url, _) = serve_jar(fast_body, None).await;
    let (slow_url, slow_connections) =
        serve_jar(slow_body, Some(std::time::Duration::from_millis(1500))).await;
    let fast = github_mod_served(&fast_url, &fast_sha1, size);
    let mut slow = github_mod_served(&slow_url, &slow_sha1, size);
    slow.path = "mods/slow-mod.jar".to_string();
    if let BundleFileKind::External { file: ext, id, .. } = &mut slow.kind {
        ext.name = "slow-mod.jar".to_string();
        *id = Some("slow-mod".to_string());
    }
    seed_bundle_with_pack(&state, manifest(vec![fast, slow])).await;
    let fresh = cluster_with_tracked_mod(&state).await;
    let ctx = state.services.content();

    let deadline = Some(std::time::Instant::now() + std::time::Duration::from_millis(300));
    install_cluster_bundles(fresh, state.bundles.as_ref(), None, &ctx, deadline)
        .await
        .unwrap();

    assert!(!linked(&state, fresh, &fast_sha1).await);
    assert!(!linked(&state, fresh, &slow_sha1).await);

    install_cluster_bundles(fresh, state.bundles.as_ref(), None, &ctx, None)
        .await
        .unwrap();

    assert_eq!(
        slow_connections.load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    assert!(linked(&state, fresh, &fast_sha1).await);
    assert!(linked(&state, fresh, &slow_sha1).await);
}

#[tokio::test]
async fn launch_install_does_not_stage_a_file_an_installed_copy_satisfies() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let body = b"github mod next to a satisfied sodium".to_vec();
    let sha1 = polyio::sha1_bytes(&body);
    let size = body.len() as u64;
    let (url, _) = serve_jar(body, None).await;
    let mut sodium = managed_file(true);
    if let BundleFileKind::Managed { sha1, .. } = &mut sodium.kind {
        *sha1 = "9898989898989898989898989898989898989898".to_string();
    }
    seed_bundle_with_pack(
        &state,
        manifest(vec![sodium, github_mod_served(&url, &sha1, size)]),
    )
    .await;
    let fresh = cluster_with_tracked_mod(&state).await;
    let mut ctx = state.services.content();
    ctx.providers.register(Arc::new(FixtureModrinth {
        fail_latest_with: Mutex::default(),
        projects: HashMap::new(),
        versions: HashMap::new(),
        latest: HashMap::new(),
    }));

    let deadline = Some(std::time::Instant::now() + std::time::Duration::from_secs(30));
    install_cluster_bundles(fresh, state.bundles.as_ref(), None, &ctx, deadline)
        .await
        .unwrap();

    assert!(linked(&state, fresh, &sha1).await);
    assert!(linked(&state, fresh, HASH).await);
}

#[tokio::test]
async fn launch_install_does_not_retry_a_failed_download() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let body: Vec<u8> = (0..64 * 1024).map(|i| (i % 233) as u8).collect();
    let wrong_sha1 = polyio::sha1_bytes(b"not the served body");
    let size = body.len() as u64;
    let (url, connections) = serve_jar(body, None).await;
    seed_bundle_with_pack(&state, manifest(vec![github_mod_served(&url, &wrong_sha1, size)])).await;
    let fresh = cluster_with_tracked_mod(&state).await;
    let ctx = state.services.content();

    let deadline = Some(std::time::Instant::now() + std::time::Duration::from_secs(30));
    install_cluster_bundles(fresh, state.bundles.as_ref(), None, &ctx, deadline)
        .await
        .unwrap();

    assert_eq!(connections.load(std::sync::atomic::Ordering::SeqCst), 3);
    assert!(!linked(&state, fresh, &wrong_sha1).await);
}

#[tokio::test]
async fn launch_install_links_nothing_when_part_of_the_bundle_fails_to_download() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let good_body: Vec<u8> = (0..64 * 1024).map(|i| (i % 211) as u8).collect();
    let bad_body: Vec<u8> = (0..64 * 1024).map(|i| (i % 199) as u8).collect();
    let good_sha1 = polyio::sha1_bytes(&good_body);
    let wrong_sha1 = polyio::sha1_bytes(b"not what the broken url serves");
    let size = good_body.len() as u64;
    let (good_url, _) = serve_jar(good_body, None).await;
    let (bad_url, _) = serve_jar(bad_body, None).await;
    let good = github_mod_served(&good_url, &good_sha1, size);
    let mut bad = github_mod_served(&bad_url, &wrong_sha1, size);
    bad.path = "mods/broken-mod.jar".to_string();
    if let BundleFileKind::External { file: ext, id, .. } = &mut bad.kind {
        ext.name = "broken-mod.jar".to_string();
        *id = Some("broken-mod".to_string());
    }
    seed_bundle_with_pack(&state, manifest(vec![good, bad])).await;
    let fresh = cluster_with_tracked_mod(&state).await;
    let ctx = state.services.content();

    let deadline = Some(std::time::Instant::now() + std::time::Duration::from_secs(30));
    install_cluster_bundles(fresh, state.bundles.as_ref(), None, &ctx, deadline)
        .await
        .unwrap();

    assert!(!linked(&state, fresh, &good_sha1).await);
    assert!(!linked(&state, fresh, &wrong_sha1).await);
}

#[tokio::test]
async fn install_without_a_deadline_retries_a_failed_download() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let body: Vec<u8> = (0..64 * 1024).map(|i| (i % 239) as u8).collect();
    let wrong_sha1 = polyio::sha1_bytes(b"not the served body either");
    let size = body.len() as u64;
    let (url, connections) = serve_jar(body, None).await;
    seed_bundle_with_pack(&state, manifest(vec![github_mod_served(&url, &wrong_sha1, size)])).await;
    let fresh = cluster_with_tracked_mod(&state).await;
    let ctx = state.services.content();

    install_cluster_bundles(fresh, state.bundles.as_ref(), None, &ctx, None)
        .await
        .unwrap();

    assert_eq!(connections.load(std::sync::atomic::Ordering::SeqCst), 6);
    assert!(!linked(&state, fresh, &wrong_sha1).await);
}

#[tokio::test]
async fn launch_install_joins_a_download_left_running() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let body: Vec<u8> = (0..64 * 1024).map(|i| (i % 241) as u8).collect();
    let sha1 = polyio::sha1_bytes(&body);
    let size = body.len() as u64;
    let (url, connections) = serve_jar(body, Some(std::time::Duration::from_millis(1500))).await;
    clusters_numbered_from(&state, 9000).await;
    let moved = cluster_with_other_version_copy(
        &state,
        OLD_VERSION_BUNDLE,
        github_mod_served(&url, &sha1, size),
    )
    .await;
    let fresh = cluster_with_tracked_mod(&state).await;
    let ctx = state.services.content();

    let result = oneclient_content::bundles::apply_bundle_updates(
        fresh,
        state.bundles.as_ref(),
        &ctx,
        Some(std::time::Instant::now() + std::time::Duration::from_millis(300)),
    )
    .await
    .unwrap();
    assert!(result.stopped_early);

    let (a, b) = tokio::join!(
        install_cluster_bundles(fresh, state.bundles.as_ref(), None, &ctx, None),
        install_cluster_bundles(moved, state.bundles.as_ref(), None, &ctx, None),
    );
    a.unwrap();
    b.unwrap();

    assert_eq!(connections.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(linked(&state, fresh, &sha1).await);
    assert!(linked(&state, moved, &sha1).await);
    assert!(!linked(&state, moved, GITHUB_OLD_HASH).await);
}

#[tokio::test]
async fn another_versions_copy_of_the_same_build_moves_to_the_current_bundle() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let same_build = github_mod_served(
        "unsupported://example.invalid/github-mod.jar",
        GITHUB_OLD_HASH,
        1,
    );
    let cluster_id = cluster_with_other_version_copy(&state, OLD_VERSION_BUNDLE, same_build).await;

    install_cluster_bundles(
        cluster_id,
        state.bundles.as_ref(),
        None,
        &state.services.content(),
        None,
    )
    .await
    .unwrap();

    assert!(linked(&state, cluster_id, GITHUB_OLD_HASH).await);
    let tracked = bundle_dao::get_bundle_tracked(&state.services.db, cluster_id, GITHUB_OLD_HASH)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(tracked.bundle_name.as_deref(), Some(BUNDLE));
}

#[tokio::test]
async fn a_bundle_name_reused_across_versions_still_replaces_the_old_build() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let body = b"github mod rebuilt for the current version".to_vec();
    let sha1 = polyio::sha1_bytes(&body);
    let size = body.len() as u64;
    let (url, _) = serve_jar(body, None).await;
    let cluster_id =
        cluster_with_other_version_copy(&state, BUNDLE, github_mod_served(&url, &sha1, size)).await;
    let ctx = state.services.content();

    oneclient_content::bundles::apply_bundle_updates(
        cluster_id,
        state.bundles.as_ref(),
        &ctx,
        None,
    )
    .await
    .unwrap();
    install_cluster_bundles(cluster_id, state.bundles.as_ref(), None, &ctx, None)
        .await
        .unwrap();

    assert!(!linked(&state, cluster_id, GITHUB_OLD_HASH).await);
    assert!(linked(&state, cluster_id, &sha1).await);
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

struct FixtureModrinth {
    projects: HashMap<String, ProjectDetail>,
    versions: HashMap<String, VersionDetail>,
    latest: HashMap<String, VersionDetail>,
    fail_latest_with: Mutex<Option<u16>>,
}

#[async_trait::async_trait]
impl PackageProvider for FixtureModrinth {
    fn id(&self) -> ProviderId {
        ProviderId::Modrinth
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
        Ok(self.projects[project_id].clone())
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
        _game_version: Option<&str>,
        _loader: Option<GameLoader>,
        _offset: usize,
        _limit: usize,
        _ctx: &ContentCtx,
    ) -> ContentResult<Page<VersionSummary>> {
        unimplemented!()
    }

    async fn get_version(
        &self,
        _project_id: &str,
        version_id: &str,
        _ctx: &ContentCtx,
    ) -> ContentResult<VersionDetail> {
        Ok(self.versions[version_id].clone())
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
        _identities: &[FileIdentity],
        _ctx: &ContentCtx,
    ) -> ContentResult<VersionLookup> {
        unimplemented!()
    }

    async fn latest_for_game_version(
        &self,
        packages: &[InstalledPackage],
        _mc_version: &str,
        _loader: GameLoader,
        _ctx: &ContentCtx,
    ) -> ContentResult<HashMap<String, VersionDetail>> {
        if let Some(status) = self.fail_latest_with.lock().unwrap().take() {
            return Err(oneclient_net::RequestError::HttpStatus {
                status,
                url: String::new(),
                snippet: String::new(),
            }
            .into());
        }
        Ok(packages
            .iter()
            .filter_map(|package| {
                Some((
                    package.hash.clone(),
                    self.latest.get(&package.hash)?.clone(),
                ))
            })
            .collect())
    }
}

fn fixture_project(id: &str) -> ProjectDetail {
    ProjectDetail {
        id: id.to_string(),
        slug: id.to_string(),
        provider: ProviderId::Modrinth,
        content_type: ContentType::Mod,
        name: id.to_string(),
        summary: String::new(),
        author: String::new(),
        members: Vec::new(),
        gallery: Vec::new(),
        body: PackageBody::raw(""),
        license: None,
        links: Vec::new(),
        version_ids: Vec::new(),
        game_versions: vec![MC_VERSION.to_string()],
        loaders: vec![GameLoader::Fabric],
        icon_url: None,
        created: chrono::Utc::now(),
        updated: chrono::Utc::now(),
        downloads: 0,
    }
}

fn fixture_version(
    project_id: &str,
    version_id: &str,
    url: &str,
    sha1: &str,
    dependencies: Vec<VersionDependency>,
) -> VersionDetail {
    VersionDetail {
        version_id: version_id.to_string(),
        project_id: project_id.to_string(),
        name: version_id.to_string(),
        version_number: version_id.to_string(),
        changelog: None,
        game_versions: vec![MC_VERSION.to_string()],
        loaders: vec![GameLoader::Fabric],
        published: chrono::Utc::now(),
        downloads: 0,
        files: vec![VersionFile {
            sha1: sha1.to_string(),
            url: url.to_string(),
            file_name: format!("{version_id}.jar"),
            primary: true,
            size: 1,
            fingerprint: None,
        }],
        dependencies,
    }
}

async fn link_release(
    state: &LauncherState,
    cluster_id: i64,
    hash: &str,
    project_id: &str,
    version_id: &str,
    mc_version: &str,
    enabled: bool,
) {
    let db = &state.services.db;
    let file_name = format!("{version_id}.jar");
    artifact_dao::insert_artifact(
        db,
        hash,
        ContentType::Mod as i64,
        &format!("artifacts/{file_name}"),
        &file_name,
        Some(1),
    )
    .await
    .unwrap();
    artifact_dao::link_cluster_artifact(db, cluster_id, hash, &file_name)
        .await
        .unwrap();
    artifact_dao::update_cluster_artifact(db, cluster_id, hash, &file_name, i64::from(enabled))
        .await
        .unwrap();
    artifact_dao::upsert_provider_release(
        db,
        ProviderId::Modrinth as i64,
        project_id,
        version_id,
        hash,
        project_id,
        version_id,
        None,
        mc_version,
        "fabric",
    )
    .await
    .unwrap();
}

async fn enabled(state: &LauncherState, cluster_id: i64, hash: &str) -> Option<bool> {
    artifact_dao::get_cluster_artifact(&state.services.db, cluster_id, hash)
        .await
        .unwrap()
        .map(|link| link.enabled != 0)
}

#[tokio::test]
async fn retargeting_leaves_new_builds_on_and_the_rest_off() {
    const TOP_OLD: &str = "4444444444444444444444444444444444444444";
    const GONE: &str = "5555555555555555555555555555555555555555";

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let top_body = b"top for the new version".to_vec();
    let lib_body = b"lib for the new version".to_vec();
    let top_new = polyio::sha1_bytes(&top_body);
    let lib_new = polyio::sha1_bytes(&lib_body);
    let (top_url, _) = serve_jar(top_body, None).await;
    let (lib_url, _) = serve_jar(lib_body, None).await;

    let global = state.settings.read().global_game_settings.clone();
    let cluster_id = state
        .clusters
        .create(
            &global,
            CreateClusterOptions::new("Retargeted", MC_VERSION, GameLoader::Fabric),
        )
        .await
        .unwrap()
        .id;
    link_release(&state, cluster_id, TOP_OLD, "top", "top-1", "1.20.1", true).await;
    link_release(&state, cluster_id, GONE, "gone", "gone-1", "1.20.1", true).await;
    link_release(
        &state, cluster_id, &lib_new, "lib", "lib-2", MC_VERSION, false,
    )
    .await;

    let lib = fixture_version("lib", "lib-2", &lib_url, &lib_new, Vec::new());
    let top = fixture_version(
        "top",
        "top-2",
        &top_url,
        &top_new,
        vec![VersionDependency {
            project_id: Some("lib".to_string()),
            version_id: Some("lib-2".to_string()),
            kind: DependencyKind::Required,
        }],
    );
    let mut ctx = state.services.content();
    ctx.providers.register(Arc::new(FixtureModrinth {
        fail_latest_with: Mutex::default(),
        projects: ["top", "lib"]
            .into_iter()
            .map(|id| (id.to_string(), fixture_project(id)))
            .collect(),
        versions: HashMap::from([("lib-2".to_string(), lib.clone())]),
        latest: HashMap::from([(TOP_OLD.to_string(), top), (lib_new.clone(), lib)]),
    }));

    retarget_cluster_mods(cluster_id, &mut HashSet::new(), &ctx)
        .await
        .unwrap();

    assert_eq!(enabled(&state, cluster_id, &top_new).await, Some(true));
    assert_eq!(
        enabled(&state, cluster_id, &lib_new).await,
        Some(true),
        "the new build of `top` requires it"
    );
    assert_eq!(enabled(&state, cluster_id, GONE).await, Some(false));
    assert_ne!(enabled(&state, cluster_id, TOP_OLD).await, Some(true));
}

async fn retarget_cluster(state: &LauncherState) -> i64 {
    let global = state.settings.read().global_game_settings.clone();
    state
        .clusters
        .create(
            &global,
            CreateClusterOptions::new("Retargeted", MC_VERSION, GameLoader::Fabric),
        )
        .await
        .unwrap()
        .id
}

#[tokio::test]
async fn retargeting_switches_off_a_mod_it_cannot_look_up() {
    const UNKNOWN: &str = "6666666666666666666666666666666666666666";

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = retarget_cluster(&state).await;
    let db = &state.services.db;
    artifact_dao::insert_artifact(
        db,
        UNKNOWN,
        ContentType::Mod as i64,
        "artifacts/unknown.jar",
        "unknown.jar",
        Some(1),
    )
    .await
    .unwrap();
    artifact_dao::link_cluster_artifact(db, cluster_id, UNKNOWN, "unknown.jar")
        .await
        .unwrap();

    let (disabled, _) =
        retarget_cluster_mods(cluster_id, &mut HashSet::new(), &state.services.content())
            .await
            .unwrap();

    assert_eq!(disabled, 1);
    assert_eq!(enabled(&state, cluster_id, UNKNOWN).await, Some(false));
}

#[tokio::test]
async fn retargeting_switches_off_a_user_copy_of_a_bundle_tracked_project() {
    const BUNDLED: &str = "5656565656565656565656565656565656565656";
    const USER_OLD: &str = "3434343434343434343434343434343434343434";

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = retarget_cluster(&state).await;
    link_release(
        &state, cluster_id, BUNDLED, "shared", "shared-2", MC_VERSION, true,
    )
    .await;
    bundle_dao::track_bundle_artifact(
        &state.services.db,
        cluster_id,
        BUNDLED,
        BUNDLE,
        "shared-2",
        "shared",
    )
    .await
    .unwrap();
    link_release(
        &state, cluster_id, USER_OLD, "shared", "shared-1", "1.20.1", true,
    )
    .await;

    retarget_cluster_mods(cluster_id, &mut HashSet::new(), &state.services.content())
        .await
        .unwrap();

    assert_eq!(enabled(&state, cluster_id, USER_OLD).await, Some(false));
    assert_eq!(enabled(&state, cluster_id, BUNDLED).await, Some(true));
}

#[tokio::test]
async fn removing_a_modpack_after_retargeting_removes_the_new_builds_of_its_mods() {
    const PACK_OLD: &str = "8989898989898989898989898989898989898989";
    const PACK: &str = "modpack:modrinth:retargeted-pack";

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = retarget_cluster(&state).await;
    let body = b"pack mod for the new version".to_vec();
    let pack_new = polyio::sha1_bytes(&body);
    let (url, _) = serve_jar(body, None).await;
    link_release(
        &state,
        cluster_id,
        PACK_OLD,
        "packmod",
        "packmod-1",
        "1.20.1",
        true,
    )
    .await;
    bundle_dao::track_bundle_artifact(
        &state.services.db,
        cluster_id,
        PACK_OLD,
        PACK,
        "packmod-1",
        "packmod",
    )
    .await
    .unwrap();

    let mut ctx = state.services.content();
    ctx.providers.register(Arc::new(FixtureModrinth {
        fail_latest_with: Mutex::default(),
        projects: HashMap::from([("packmod".to_string(), fixture_project("packmod"))]),
        versions: HashMap::new(),
        latest: HashMap::from([(
            PACK_OLD.to_string(),
            fixture_version("packmod", "packmod-2", &url, &pack_new, Vec::new()),
        )]),
    }));

    retarget_cluster_mods(cluster_id, &mut HashSet::new(), &ctx)
        .await
        .unwrap();
    assert_eq!(enabled(&state, cluster_id, &pack_new).await, Some(true));
    assert_eq!(enabled(&state, cluster_id, PACK_OLD).await, Some(false));

    let failed = oneclient_content::modpacks::remove_modpack_files(cluster_id, PACK, None, &ctx)
        .await
        .unwrap();

    assert!(failed.is_empty());
    assert_eq!(enabled(&state, cluster_id, &pack_new).await, None);
    assert_eq!(enabled(&state, cluster_id, PACK_OLD).await, None);
}

#[tokio::test]
async fn removing_a_modpack_after_retargeting_removes_the_new_builds_of_its_libraries() {
    const TOP_OLD: &str = "8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a";
    const LIB_OLD: &str = "8b8b8b8b8b8b8b8b8b8b8b8b8b8b8b8b8b8b8b8b";
    const PACK: &str = "modpack:modrinth:retargeted-library-pack";

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = retarget_cluster(&state).await;
    let top_body = b"pack top mod for the new version".to_vec();
    let lib_body = b"pack library for the new version".to_vec();
    let top_new = polyio::sha1_bytes(&top_body);
    let lib_new = polyio::sha1_bytes(&lib_body);
    let (top_url, _) = serve_jar(top_body, None).await;
    let (lib_url, _) = serve_jar(lib_body, None).await;
    for (hash, project, version) in [
        (TOP_OLD, "packtop", "packtop-1"),
        (LIB_OLD, "packlib", "packlib-1"),
    ] {
        link_release(&state, cluster_id, hash, project, version, "1.20.1", true).await;
        bundle_dao::track_bundle_artifact(
            &state.services.db,
            cluster_id,
            hash,
            PACK,
            version,
            project,
        )
        .await
        .unwrap();
    }

    let lib = fixture_version("packlib", "packlib-2", &lib_url, &lib_new, Vec::new());
    let top = fixture_version(
        "packtop",
        "packtop-2",
        &top_url,
        &top_new,
        vec![VersionDependency {
            project_id: Some("packlib".to_string()),
            version_id: Some("packlib-2".to_string()),
            kind: DependencyKind::Required,
        }],
    );
    let mut ctx = state.services.content();
    ctx.providers.register(Arc::new(FixtureModrinth {
        fail_latest_with: Mutex::default(),
        projects: ["packtop", "packlib"]
            .into_iter()
            .map(|id| (id.to_string(), fixture_project(id)))
            .collect(),
        versions: HashMap::from([("packlib-2".to_string(), lib.clone())]),
        latest: HashMap::from([(TOP_OLD.to_string(), top), (LIB_OLD.to_string(), lib)]),
    }));

    retarget_cluster_mods(cluster_id, &mut HashSet::new(), &ctx)
        .await
        .unwrap();
    assert_eq!(enabled(&state, cluster_id, &lib_new).await, Some(true));
    assert_eq!(enabled(&state, cluster_id, LIB_OLD).await, Some(false));

    let failed = oneclient_content::modpacks::remove_modpack_files(cluster_id, PACK, None, &ctx)
        .await
        .unwrap();

    assert!(failed.is_empty());
    for hash in [TOP_OLD, LIB_OLD, top_new.as_str(), lib_new.as_str()] {
        assert_eq!(enabled(&state, cluster_id, hash).await, None);
    }
}

#[tokio::test]
async fn retargeting_keeps_a_dependency_that_was_on() {
    const TOP_OLD: &str = "7777777777777777777777777777777777777777";

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let top_body = b"switched off top for the new version".to_vec();
    let lib_body = b"enabled lib for the new version".to_vec();
    let top_new = polyio::sha1_bytes(&top_body);
    let lib_new = polyio::sha1_bytes(&lib_body);
    let (top_url, _) = serve_jar(top_body, None).await;
    let (lib_url, _) = serve_jar(lib_body, None).await;

    let cluster_id = retarget_cluster(&state).await;
    link_release(
        &state,
        cluster_id,
        TOP_OLD,
        "off-top",
        "off-top-1",
        "1.20.1",
        false,
    )
    .await;
    link_release(
        &state, cluster_id, &lib_new, "on-lib", "on-lib-2", MC_VERSION, true,
    )
    .await;

    let lib = fixture_version("on-lib", "on-lib-2", &lib_url, &lib_new, Vec::new());
    let top = fixture_version(
        "off-top",
        "off-top-2",
        &top_url,
        &top_new,
        vec![VersionDependency {
            project_id: Some("on-lib".to_string()),
            version_id: Some("on-lib-2".to_string()),
            kind: DependencyKind::Required,
        }],
    );
    let mut ctx = state.services.content();
    ctx.providers.register(Arc::new(FixtureModrinth {
        fail_latest_with: Mutex::default(),
        projects: ["off-top", "on-lib"]
            .into_iter()
            .map(|id| (id.to_string(), fixture_project(id)))
            .collect(),
        versions: HashMap::from([("on-lib-2".to_string(), lib.clone())]),
        latest: HashMap::from([(TOP_OLD.to_string(), top), (lib_new.clone(), lib)]),
    }));

    retarget_cluster_mods(cluster_id, &mut HashSet::new(), &ctx)
        .await
        .unwrap();

    assert_eq!(enabled(&state, cluster_id, &lib_new).await, Some(true));
    assert_eq!(enabled(&state, cluster_id, &top_new).await, Some(false));
}

#[tokio::test]
async fn retargeting_keeps_a_switched_off_dependency_build_off() {
    const TOP_OLD: &str = "9999999999999999999999999999999999999999";
    const LIB_OLD: &str = "abababababababababababababababababababab";

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let top_body = b"off top needing an off lib".to_vec();
    let lib_body = b"lib switched off on the new version".to_vec();
    let top_new = polyio::sha1_bytes(&top_body);
    let lib_new = polyio::sha1_bytes(&lib_body);
    let (top_url, _) = serve_jar(top_body, None).await;
    let (lib_url, _) = serve_jar(lib_body, None).await;

    let cluster_id = retarget_cluster(&state).await;
    link_release(
        &state,
        cluster_id,
        TOP_OLD,
        "off-dep-top",
        "off-dep-top-1",
        "1.20.1",
        false,
    )
    .await;
    link_release(
        &state,
        cluster_id,
        LIB_OLD,
        "off-dep-lib",
        "off-dep-lib-1",
        "1.20.1",
        true,
    )
    .await;
    link_release(
        &state,
        cluster_id,
        &lib_new,
        "off-dep-lib",
        "off-dep-lib-2",
        MC_VERSION,
        false,
    )
    .await;

    let lib = fixture_version(
        "off-dep-lib",
        "off-dep-lib-2",
        &lib_url,
        &lib_new,
        Vec::new(),
    );
    let top = fixture_version(
        "off-dep-top",
        "off-dep-top-2",
        &top_url,
        &top_new,
        vec![VersionDependency {
            project_id: Some("off-dep-lib".to_string()),
            version_id: Some("off-dep-lib-2".to_string()),
            kind: DependencyKind::Required,
        }],
    );
    let mut ctx = state.services.content();
    ctx.providers.register(Arc::new(FixtureModrinth {
        fail_latest_with: Mutex::default(),
        projects: ["off-dep-top", "off-dep-lib"]
            .into_iter()
            .map(|id| (id.to_string(), fixture_project(id)))
            .collect(),
        versions: HashMap::from([("off-dep-lib-2".to_string(), lib.clone())]),
        latest: HashMap::from([
            (TOP_OLD.to_string(), top),
            (LIB_OLD.to_string(), lib.clone()),
            (lib_new.clone(), lib),
        ]),
    }));

    retarget_cluster_mods(cluster_id, &mut HashSet::new(), &ctx)
        .await
        .unwrap();

    assert_eq!(enabled(&state, cluster_id, &lib_new).await, Some(false));
    assert_ne!(enabled(&state, cluster_id, LIB_OLD).await, Some(true));
    assert_eq!(enabled(&state, cluster_id, &top_new).await, Some(false));
}

#[tokio::test]
async fn retargeting_keeps_a_switched_off_copy_already_on_the_new_build() {
    const TOP_OLD: &str = "8888888888888888888888888888888888888888";

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let top_body = b"top already on the new version".to_vec();
    let top_new = polyio::sha1_bytes(&top_body);

    let cluster_id = retarget_cluster(&state).await;
    link_release(&state, cluster_id, TOP_OLD, "top", "top-1", "1.20.1", true).await;
    link_release(
        &state, cluster_id, &top_new, "top", "top-2", MC_VERSION, false,
    )
    .await;

    let top = fixture_version(
        "top",
        "top-2",
        "unsupported://example.invalid/top.jar",
        &top_new,
        Vec::new(),
    );
    let mut ctx = state.services.content();
    ctx.providers.register(Arc::new(FixtureModrinth {
        fail_latest_with: Mutex::default(),
        projects: HashMap::from([("top".to_string(), fixture_project("top"))]),
        versions: HashMap::new(),
        latest: HashMap::from([(TOP_OLD.to_string(), top.clone()), (top_new.clone(), top)]),
    }));

    retarget_cluster_mods(cluster_id, &mut HashSet::new(), &ctx)
        .await
        .unwrap();

    assert_eq!(enabled(&state, cluster_id, &top_new).await, Some(false));
    assert_eq!(enabled(&state, cluster_id, TOP_OLD).await, Some(false));
}

#[tokio::test]
async fn retargeting_puts_back_on_a_mod_that_was_on_before_an_earlier_pass() {
    const TOP_OLD: &str = "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let top_body = b"top swapped on a later pass".to_vec();
    let top_new = polyio::sha1_bytes(&top_body);
    let (top_url, _) = serve_jar(top_body, None).await;

    let cluster_id = retarget_cluster(&state).await;
    link_release(
        &state,
        cluster_id,
        TOP_OLD,
        "resumed-top",
        "resumed-top-1",
        "1.20.1",
        false,
    )
    .await;

    let top = fixture_version(
        "resumed-top",
        "resumed-top-2",
        &top_url,
        &top_new,
        Vec::new(),
    );
    let mut ctx = state.services.content();
    ctx.providers.register(Arc::new(FixtureModrinth {
        fail_latest_with: Mutex::default(),
        projects: HashMap::from([("resumed-top".to_string(), fixture_project("resumed-top"))]),
        versions: HashMap::new(),
        latest: HashMap::from([(TOP_OLD.to_string(), top)]),
    }));

    let (_, reached) =
        retarget_cluster_mods(cluster_id, &mut HashSet::from([TOP_OLD.to_string()]), &ctx)
            .await
            .unwrap();

    assert!(reached);
    assert_eq!(enabled(&state, cluster_id, &top_new).await, Some(true));
}

#[tokio::test]
async fn retargeting_puts_back_on_a_copy_already_on_the_new_build() {
    const BOTH: &str = "dededededededededededededededededededede";

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = retarget_cluster(&state).await;
    link_release(
        &state,
        cluster_id,
        BOTH,
        "both-versions",
        "both-1",
        "1.20.1",
        false,
    )
    .await;

    let mut ctx = state.services.content();
    ctx.providers.register(Arc::new(FixtureModrinth {
        fail_latest_with: Mutex::default(),
        projects: HashMap::from([(
            "both-versions".to_string(),
            fixture_project("both-versions"),
        )]),
        versions: HashMap::new(),
        latest: HashMap::from([(
            BOTH.to_string(),
            fixture_version("both-versions", "both-1", "", BOTH, Vec::new()),
        )]),
    }));

    let (_, reached) =
        retarget_cluster_mods(cluster_id, &mut HashSet::from([BOTH.to_string()]), &ctx)
            .await
            .unwrap();

    assert!(reached);
    assert_eq!(enabled(&state, cluster_id, BOTH).await, Some(true));
}

#[tokio::test]
async fn retargeting_is_unfinished_when_a_new_build_fails_to_install() {
    const TOP_OLD: &str = "edededededededededededededededededededed";

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let wrong_sha1 = polyio::sha1_bytes(b"not what the url serves");
    let (top_url, _) = serve_jar(b"served body".to_vec(), None).await;

    let cluster_id = retarget_cluster(&state).await;
    link_release(
        &state,
        cluster_id,
        TOP_OLD,
        "broken-top",
        "broken-top-1",
        "1.20.1",
        true,
    )
    .await;

    let mut ctx = state.services.content();
    ctx.providers.register(Arc::new(FixtureModrinth {
        fail_latest_with: Mutex::default(),
        projects: HashMap::from([("broken-top".to_string(), fixture_project("broken-top"))]),
        versions: HashMap::new(),
        latest: HashMap::from([(
            TOP_OLD.to_string(),
            fixture_version(
                "broken-top",
                "broken-top-2",
                &top_url,
                &wrong_sha1,
                Vec::new(),
            ),
        )]),
    }));

    let (_, reached) = retarget_cluster_mods(cluster_id, &mut HashSet::new(), &ctx)
        .await
        .unwrap();

    assert!(!reached);
    assert_eq!(enabled(&state, cluster_id, TOP_OLD).await, Some(false));
}

#[tokio::test]
async fn retargeting_switches_on_what_a_library_left_on_needs() {
    const APP_OLD: &str = "1212121212121212121212121212121212121212";
    const LIB_OLD: &str = "2323232323232323232323232323232323232323";

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let app_body = b"off app needing a chained lib".to_vec();
    let lib_body = b"on lib needing a new core".to_vec();
    let core_body = b"core new to the lib".to_vec();
    let app_new = polyio::sha1_bytes(&app_body);
    let lib_new = polyio::sha1_bytes(&lib_body);
    let core_new = polyio::sha1_bytes(&core_body);
    let (app_url, _) = serve_jar(app_body, None).await;
    let (lib_url, _) = serve_jar(lib_body, None).await;
    let (core_url, _) = serve_jar(core_body, None).await;

    let cluster_id = retarget_cluster(&state).await;
    link_release(
        &state,
        cluster_id,
        APP_OLD,
        "chain-app",
        "chain-app-1",
        "1.20.1",
        false,
    )
    .await;
    link_release(
        &state,
        cluster_id,
        LIB_OLD,
        "chain-lib",
        "chain-lib-1",
        "1.20.1",
        true,
    )
    .await;

    let core = fixture_version(
        "chain-core",
        "chain-core-2",
        &core_url,
        &core_new,
        Vec::new(),
    );
    let lib = fixture_version(
        "chain-lib",
        "chain-lib-2",
        &lib_url,
        &lib_new,
        vec![VersionDependency {
            project_id: Some("chain-core".to_string()),
            version_id: Some("chain-core-2".to_string()),
            kind: DependencyKind::Required,
        }],
    );
    let app = fixture_version(
        "chain-app",
        "chain-app-2",
        &app_url,
        &app_new,
        vec![VersionDependency {
            project_id: Some("chain-lib".to_string()),
            version_id: Some("chain-lib-2".to_string()),
            kind: DependencyKind::Required,
        }],
    );
    let mut ctx = state.services.content();
    ctx.providers.register(Arc::new(FixtureModrinth {
        fail_latest_with: Mutex::default(),
        projects: ["chain-app", "chain-lib", "chain-core"]
            .into_iter()
            .map(|id| (id.to_string(), fixture_project(id)))
            .collect(),
        versions: HashMap::from([
            ("chain-lib-2".to_string(), lib.clone()),
            ("chain-core-2".to_string(), core),
        ]),
        latest: HashMap::from([(APP_OLD.to_string(), app), (LIB_OLD.to_string(), lib)]),
    }));

    retarget_cluster_mods(cluster_id, &mut HashSet::new(), &ctx)
        .await
        .unwrap();

    assert_eq!(enabled(&state, cluster_id, &app_new).await, Some(false));
    assert_eq!(enabled(&state, cluster_id, &lib_new).await, Some(true));
    assert_eq!(enabled(&state, cluster_id, &core_new).await, Some(true));
}

#[tokio::test]
async fn an_unfinished_retarget_forgets_the_mods_it_settled() {
    const SETTLED: &str = "3535353535353535353535353535353535353535";
    const FAILING_OLD: &str = "4646464646464646464646464646464646464646";

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let wrong_sha1 = polyio::sha1_bytes(b"not what the failing url serves");
    let (failing_url, _) = serve_jar(b"failing served body".to_vec(), None).await;

    let cluster_id = retarget_cluster(&state).await;
    link_release(
        &state,
        cluster_id,
        SETTLED,
        "settled",
        "settled-1",
        MC_VERSION,
        true,
    )
    .await;
    link_release(
        &state,
        cluster_id,
        FAILING_OLD,
        "failing",
        "failing-1",
        "1.20.1",
        true,
    )
    .await;

    let mut ctx = state.services.content();
    ctx.providers.register(Arc::new(FixtureModrinth {
        fail_latest_with: Mutex::default(),
        projects: ["settled", "failing"]
            .into_iter()
            .map(|id| (id.to_string(), fixture_project(id)))
            .collect(),
        versions: HashMap::new(),
        latest: HashMap::from([
            (
                SETTLED.to_string(),
                fixture_version("settled", "settled-1", "", SETTLED, Vec::new()),
            ),
            (
                FAILING_OLD.to_string(),
                fixture_version(
                    "failing",
                    "failing-2",
                    &failing_url,
                    &wrong_sha1,
                    Vec::new(),
                ),
            ),
        ]),
    }));

    let mut pending = HashSet::from([SETTLED.to_string(), FAILING_OLD.to_string()]);
    let (_, reached) = retarget_cluster_mods(cluster_id, &mut pending, &ctx)
        .await
        .unwrap();

    assert!(!reached);
    assert_eq!(pending, HashSet::from([FAILING_OLD.to_string()]));
}

#[tokio::test]
async fn an_unfinished_retarget_forgets_the_old_copies_of_mods_it_replaced() {
    const MOVED_OLD: &str = "9c9c9c9c9c9c9c9c9c9c9c9c9c9c9c9c9c9c9c9c";
    const FAILING_OLD: &str = "9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d";

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let moved_body = b"moved mod for the new version".to_vec();
    let moved_new = polyio::sha1_bytes(&moved_body);
    let (moved_url, _) = serve_jar(moved_body, None).await;
    let wrong_sha1 = polyio::sha1_bytes(b"not what the stuck url serves");
    let (failing_url, _) = serve_jar(b"stuck served body".to_vec(), None).await;

    let cluster_id = retarget_cluster(&state).await;
    for (hash, project, version) in [
        (MOVED_OLD, "moved", "moved-1"),
        (FAILING_OLD, "stuck", "stuck-1"),
    ] {
        link_release(&state, cluster_id, hash, project, version, "1.20.1", true).await;
    }
    bundle_dao::track_bundle_artifact(
        &state.services.db,
        cluster_id,
        MOVED_OLD,
        "modpack:modrinth:moving-pack",
        "moved-1",
        "moved",
    )
    .await
    .unwrap();

    let moved = fixture_version("moved", "moved-2", &moved_url, &moved_new, Vec::new());
    let mut ctx = state.services.content();
    ctx.providers.register(Arc::new(FixtureModrinth {
        fail_latest_with: Mutex::default(),
        projects: ["moved", "stuck"]
            .into_iter()
            .map(|id| (id.to_string(), fixture_project(id)))
            .collect(),
        versions: HashMap::new(),
        latest: HashMap::from([
            (MOVED_OLD.to_string(), moved.clone()),
            (moved_new.clone(), moved),
            (
                FAILING_OLD.to_string(),
                fixture_version("stuck", "stuck-2", &failing_url, &wrong_sha1, Vec::new()),
            ),
        ]),
    }));

    let mut pending = HashSet::from([MOVED_OLD.to_string(), FAILING_OLD.to_string()]);
    let (_, reached) = retarget_cluster_mods(cluster_id, &mut pending, &ctx)
        .await
        .unwrap();
    assert!(!reached);
    assert_eq!(pending, HashSet::from([FAILING_OLD.to_string()]));
    assert_eq!(enabled(&state, cluster_id, &moved_new).await, Some(true));
    assert_eq!(enabled(&state, cluster_id, MOVED_OLD).await, Some(false));

    PackageStore::set_artifact_enabled_to(cluster_id, &moved_new, false, &ctx)
        .await
        .unwrap();
    let next_body = b"moved mod for the next version".to_vec();
    let moved_next = polyio::sha1_bytes(&next_body);
    let (next_url, _) = serve_jar(next_body, None).await;
    let next = fixture_version("moved", "moved-3", &next_url, &moved_next, Vec::new());
    let mut ctx = state.services.content();
    ctx.providers.register(Arc::new(FixtureModrinth {
        fail_latest_with: Mutex::default(),
        projects: HashMap::from([("moved".to_string(), fixture_project("moved"))]),
        versions: HashMap::new(),
        latest: HashMap::from([(MOVED_OLD.to_string(), next.clone()), (moved_new, next)]),
    }));
    retarget_cluster_mods(cluster_id, &mut pending, &ctx)
        .await
        .unwrap();

    assert_eq!(enabled(&state, cluster_id, &moved_next).await, Some(false));
}

#[tokio::test]
async fn a_modpack_update_dropping_a_retargeted_mod_removes_both_copies() {
    const PACK_OLD: &str = "9e9e9e9e9e9e9e9e9e9e9e9e9e9e9e9e9e9e9e9e";
    const PACK: &str = "modpack:modrinth:dropping-pack";

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = retarget_cluster(&state).await;
    let body = b"dropped pack mod for the new version".to_vec();
    let pack_new = polyio::sha1_bytes(&body);
    let (url, _) = serve_jar(body, None).await;
    link_release(
        &state,
        cluster_id,
        PACK_OLD,
        "droppedmod",
        "droppedmod-1",
        "1.20.1",
        true,
    )
    .await;
    bundle_dao::track_bundle_artifact(
        &state.services.db,
        cluster_id,
        PACK_OLD,
        PACK,
        "droppedmod-1",
        "droppedmod",
    )
    .await
    .unwrap();

    let mut ctx = state.services.content();
    ctx.providers.register(Arc::new(FixtureModrinth {
        fail_latest_with: Mutex::default(),
        projects: HashMap::from([("droppedmod".to_string(), fixture_project("droppedmod"))]),
        versions: HashMap::new(),
        latest: HashMap::from([(
            PACK_OLD.to_string(),
            fixture_version("droppedmod", "droppedmod-2", &url, &pack_new, Vec::new()),
        )]),
    }));
    retarget_cluster_mods(cluster_id, &mut HashSet::new(), &ctx)
        .await
        .unwrap();
    assert_eq!(enabled(&state, cluster_id, &pack_new).await, Some(true));

    let scratch = polyio::testing::ScratchDir::new("dropping-pack");
    let archive = scratch.path().join("pack.mrpack");
    let file = tokio::fs::File::create(&archive).await.unwrap();
    let mut writer = ZipFileWriter::with_tokio(file);
    let entry = ZipEntryBuilder::new("modrinth.index.json".into(), Compression::Stored);
    writer.write_entry_whole(entry, b"{}").await.unwrap();
    writer.close().await.unwrap();
    let manifest = ModpackManifest {
        name: "Dropping".to_string(),
        version: "2".to_string(),
        summary: None,
        mc_version: MC_VERSION.to_string(),
        loader: GameLoader::Fabric,
        loader_version: None,
        contents: ModpackContents::default(),
        override_prefixes: vec!["overrides/".to_string()],
    };

    install_modpack(&archive, &manifest, cluster_id, PACK, None, &ctx)
        .await
        .unwrap();

    assert_eq!(enabled(&state, cluster_id, &pack_new).await, None);
    assert_eq!(enabled(&state, cluster_id, PACK_OLD).await, None);
}

#[tokio::test]
async fn launch_install_extracts_no_overrides_when_the_bundle_fails_to_download() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let body: Vec<u8> = (0..64 * 1024).map(|i| (i % 193) as u8).collect();
    let wrong_sha1 = polyio::sha1_bytes(b"not the served body");
    let size = body.len() as u64;
    let (url, _) = serve_jar(body, None).await;
    let m = named_manifest(
        "Override Bundle",
        vec![github_mod_served(&url, &wrong_sha1, size)],
    );
    let name = m.name.clone();
    oneclient_core::dev::seed_bundle_archive(&state, m)
        .await
        .unwrap();
    let dir = oneclient_common::paths::data_dir().unwrap().join("bundles");
    tokio::fs::create_dir_all(&dir).await.unwrap();
    let file = tokio::fs::File::create(dir.join(format!("{name}.mrpack")))
        .await
        .unwrap();
    let mut writer = async_zip::tokio::write::ZipFileWriter::with_tokio(file);
    let entry = async_zip::ZipEntryBuilder::new(
        "overrides/config/staged.txt".to_string().into(),
        async_zip::Compression::Stored,
    );
    writer.write_entry_whole(entry, b"staged").await.unwrap();
    writer.close().await.unwrap();

    let fresh = cluster_with_tracked_mod(&state).await;
    let marker = oneclient_common::paths::clusters_dir()
        .unwrap()
        .join(state.clusters.get(fresh).await.unwrap().folder_name)
        .join("config/staged.txt");
    let ctx = state.services.content();

    let deadline = Some(std::time::Instant::now() + std::time::Duration::from_secs(30));
    install_cluster_bundles(fresh, state.bundles.as_ref(), None, &ctx, deadline)
        .await
        .unwrap();
    assert!(!marker.exists());

    install_cluster_bundles(fresh, state.bundles.as_ref(), None, &ctx, None)
        .await
        .unwrap();
    assert!(marker.exists());
}

#[tokio::test]
async fn a_newer_user_copy_for_another_game_version_gives_way_to_the_bundle() {
    const OTHER_VERSION: &str = "7777777777777777777777777777777777777777";
    const SAME_VERSION: &str = "8888888888888888888888888888888888888888";

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    seed_bundle_with_pack(&state, manifest(vec![managed_file(true)])).await;

    let now = chrono::Utc::now();
    let mut shipped = fixture_version(PROJECT_ID, VERSION_ID, "", HASH, Vec::new());
    shipped.published = now - chrono::Duration::days(30);
    let mut other = fixture_version(PROJECT_ID, "other1", "", OTHER_VERSION, Vec::new());
    other.game_versions = vec!["1.20.1".to_string()];
    let same = fixture_version(PROJECT_ID, "same1", "", SAME_VERSION, Vec::new());

    let mut ctx = state.services.content();
    ctx.providers.register(Arc::new(FixtureModrinth {
        fail_latest_with: Mutex::default(),
        projects: HashMap::from([(PROJECT_ID.to_string(), fixture_project(PROJECT_ID))]),
        versions: [&shipped, &other, &same]
            .into_iter()
            .map(|version| (version.version_id.clone(), version.clone()))
            .collect(),
        latest: HashMap::new(),
    }));
    let archives = state
        .bundles
        .archives_for(&ctx, MC_VERSION, GameLoader::Fabric)
        .await
        .unwrap();

    let published = now.to_rfc3339();
    let mut clusters = Vec::new();
    for (hash, version_id, migrating) in [
        (OTHER_VERSION, "other1", true),
        (SAME_VERSION, "same1", true),
        (OTHER_VERSION, "other1", false),
    ] {
        let cluster_id = retarget_cluster(&state).await;
        if migrating {
            flag_dao::set(
                &state.services.db,
                cluster_id,
                flag_dao::ClusterFlag::RetargetMods,
                None,
            )
            .await
            .unwrap();
        }
        link_release(
            &state, cluster_id, hash, PROJECT_ID, version_id, MC_VERSION, true,
        )
        .await;
        artifact_dao::upsert_provider_release(
            &state.services.db,
            ProviderId::Modrinth as i64,
            PROJECT_ID,
            version_id,
            hash,
            PROJECT_ID,
            version_id,
            Some(&published),
            MC_VERSION,
            "fabric",
        )
        .await
        .unwrap();
        oneclient_content::bundles::heal_bundle_activity(cluster_id, &archives, &ctx)
            .await
            .unwrap();
        clusters.push((cluster_id, hash));
    }

    let [
        (other_cluster, other_hash),
        (same_cluster, same_hash),
        (chosen_cluster, chosen_hash),
    ] = clusters[..]
    else {
        unreachable!()
    };
    assert_eq!(
        enabled(&state, other_cluster, other_hash).await,
        Some(false),
        "a copy built for another game version gives way even to an older release"
    );
    assert_eq!(
        enabled(&state, same_cluster, same_hash).await,
        Some(true),
        "a newer copy for the cluster's version stays on"
    );
    assert_eq!(
        enabled(&state, chosen_cluster, chosen_hash).await,
        Some(true),
        "a newer copy for another version stays on outside a version migration"
    );
}

#[tokio::test]
async fn retargeting_retries_a_rate_limited_provider() {
    const TOP_OLD: &str = "5757575757575757575757575757575757575757";

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let top_body = b"top behind a rate limit".to_vec();
    let top_new = polyio::sha1_bytes(&top_body);
    let (top_url, _) = serve_jar(top_body, None).await;

    let cluster_id = retarget_cluster(&state).await;
    link_release(
        &state,
        cluster_id,
        TOP_OLD,
        "limited-top",
        "limited-top-1",
        "1.20.1",
        true,
    )
    .await;

    let top = fixture_version(
        "limited-top",
        "limited-top-2",
        &top_url,
        &top_new,
        Vec::new(),
    );
    let mut ctx = state.services.content();
    ctx.providers.register(Arc::new(FixtureModrinth {
        fail_latest_with: Mutex::new(Some(429)),
        projects: HashMap::from([("limited-top".to_string(), fixture_project("limited-top"))]),
        versions: HashMap::new(),
        latest: HashMap::from([(TOP_OLD.to_string(), top)]),
    }));

    let mut pending = HashSet::from([TOP_OLD.to_string()]);
    let (_, reached) = retarget_cluster_mods(cluster_id, &mut pending, &ctx)
        .await
        .unwrap();
    assert!(!reached);
    assert!(pending.contains(TOP_OLD));

    let (_, reached) = retarget_cluster_mods(cluster_id, &mut pending, &ctx)
        .await
        .unwrap();
    assert!(reached);
    assert_eq!(enabled(&state, cluster_id, &top_new).await, Some(true));
}

#[tokio::test]
async fn retargeting_switches_off_a_dependency_whose_own_dependency_failed() {
    const APP_OLD: &str = "6868686868686868686868686868686868686868";

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let app_body = b"app needing a lib needing a broken core".to_vec();
    let lib_body = b"lib needing a broken core".to_vec();
    let app_new = polyio::sha1_bytes(&app_body);
    let lib_new = polyio::sha1_bytes(&lib_body);
    let wrong_sha1 = polyio::sha1_bytes(b"not what the core url serves");
    let (app_url, _) = serve_jar(app_body, None).await;
    let (lib_url, _) = serve_jar(lib_body, None).await;
    let (core_url, _) = serve_jar(b"core served body".to_vec(), None).await;

    let cluster_id = retarget_cluster(&state).await;
    link_release(
        &state,
        cluster_id,
        APP_OLD,
        "broken-chain-app",
        "broken-chain-app-1",
        "1.20.1",
        true,
    )
    .await;

    let core = fixture_version(
        "broken-chain-core",
        "broken-chain-core-2",
        &core_url,
        &wrong_sha1,
        Vec::new(),
    );
    let lib = fixture_version(
        "broken-chain-lib",
        "broken-chain-lib-2",
        &lib_url,
        &lib_new,
        vec![VersionDependency {
            project_id: Some("broken-chain-core".to_string()),
            version_id: Some("broken-chain-core-2".to_string()),
            kind: DependencyKind::Required,
        }],
    );
    let app = fixture_version(
        "broken-chain-app",
        "broken-chain-app-2",
        &app_url,
        &app_new,
        vec![VersionDependency {
            project_id: Some("broken-chain-lib".to_string()),
            version_id: Some("broken-chain-lib-2".to_string()),
            kind: DependencyKind::Required,
        }],
    );
    let mut ctx = state.services.content();
    ctx.providers.register(Arc::new(FixtureModrinth {
        fail_latest_with: Mutex::default(),
        projects: ["broken-chain-app", "broken-chain-lib", "broken-chain-core"]
            .into_iter()
            .map(|id| (id.to_string(), fixture_project(id)))
            .collect(),
        versions: HashMap::from([
            ("broken-chain-lib-2".to_string(), lib),
            ("broken-chain-core-2".to_string(), core),
        ]),
        latest: HashMap::from([(APP_OLD.to_string(), app)]),
    }));

    let (_, reached) = retarget_cluster_mods(cluster_id, &mut HashSet::new(), &ctx)
        .await
        .unwrap();

    assert!(!reached);
    assert_ne!(enabled(&state, cluster_id, &lib_new).await, Some(true));
    assert_eq!(enabled(&state, cluster_id, &app_new).await, None);
    assert_eq!(enabled(&state, cluster_id, APP_OLD).await, Some(false));
}

#[tokio::test]
async fn retargeting_installs_mods_that_require_each_other() {
    const LEFT_OLD: &str = "6969696969696969696969696969696969696969";
    const RIGHT_OLD: &str = "7070707070707070707070707070707070707070";

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let left_body = b"left mod requiring right".to_vec();
    let right_body = b"right mod requiring left".to_vec();
    let left_new = polyio::sha1_bytes(&left_body);
    let right_new = polyio::sha1_bytes(&right_body);
    let (left_url, _) = serve_jar(left_body, None).await;
    let (right_url, _) = serve_jar(right_body, None).await;

    let cluster_id = retarget_cluster(&state).await;
    for (hash, project, version) in [
        (LEFT_OLD, "cycle-left", "cycle-left-1"),
        (RIGHT_OLD, "cycle-right", "cycle-right-1"),
    ] {
        link_release(&state, cluster_id, hash, project, version, "1.20.1", true).await;
    }

    let requires = |project: &str, version: &str| {
        vec![VersionDependency {
            project_id: Some(project.to_string()),
            version_id: Some(version.to_string()),
            kind: DependencyKind::Required,
        }]
    };
    let left = fixture_version(
        "cycle-left",
        "cycle-left-2",
        &left_url,
        &left_new,
        requires("cycle-right", "cycle-right-2"),
    );
    let right = fixture_version(
        "cycle-right",
        "cycle-right-2",
        &right_url,
        &right_new,
        requires("cycle-left", "cycle-left-2"),
    );
    let mut ctx = state.services.content();
    ctx.providers.register(Arc::new(FixtureModrinth {
        fail_latest_with: Mutex::default(),
        projects: ["cycle-left", "cycle-right"]
            .into_iter()
            .map(|id| (id.to_string(), fixture_project(id)))
            .collect(),
        versions: HashMap::from([
            ("cycle-left-2".to_string(), left.clone()),
            ("cycle-right-2".to_string(), right.clone()),
        ]),
        latest: HashMap::from([(LEFT_OLD.to_string(), left), (RIGHT_OLD.to_string(), right)]),
    }));

    let (_, reached) = retarget_cluster_mods(cluster_id, &mut HashSet::new(), &ctx)
        .await
        .unwrap();

    assert!(reached);
    assert_eq!(enabled(&state, cluster_id, &left_new).await, Some(true));
    assert_eq!(enabled(&state, cluster_id, &right_new).await, Some(true));
}

#[tokio::test]
async fn a_bundle_name_reused_across_versions_drops_the_old_build_while_its_replacement_fails() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let unreachable = github_mod_served(
        "unsupported://example.invalid/github-mod.jar",
        "3333333333333333333333333333333333333333",
        1,
    );
    let cluster_id = cluster_with_other_version_copy(&state, BUNDLE, unreachable).await;
    let old_build = github_mod_served(
        "unsupported://example.invalid/github-mod-old.jar",
        GITHUB_OLD_HASH,
        1,
    );
    state
        .bundles
        .cache_archive_manifest(
            oneclient_common::paths::data_dir()
                .unwrap()
                .join(format!("bundles/1.20.1/{BUNDLE}.mrpack")),
            BundleManifest {
                mc_version: "1.20.1".to_string(),
                ..manifest(vec![old_build])
            },
        )
        .await;
    let ctx = state.services.content();

    install_cluster_bundles(cluster_id, state.bundles.as_ref(), None, &ctx, None)
        .await
        .unwrap();

    assert!(!linked(&state, cluster_id, GITHUB_OLD_HASH).await);
    let overrides = bundle_dao::list_overrides(&state.services.db, cluster_id)
        .await
        .unwrap();
    assert!(overrides.is_empty(), "{overrides:?}");
}

#[tokio::test]
async fn a_reused_bundle_name_drops_the_old_build_after_the_catalog_hides_its_bundle() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let unreachable = github_mod_served(
        "unsupported://example.invalid/github-mod.jar",
        "3333333333333333333333333333333333333333",
        1,
    );
    let cluster_id = cluster_with_other_version_copy(&state, BUNDLE, unreachable).await;
    sqlx::query("UPDATE bundles SET hidden = 1 WHERE mc_version = '1.20.1'")
        .execute(&state.services.db)
        .await
        .unwrap();
    let old_build = github_mod_served(
        "unsupported://example.invalid/github-mod-old.jar",
        GITHUB_OLD_HASH,
        1,
    );
    state
        .bundles
        .cache_archive_manifest(
            oneclient_common::paths::data_dir()
                .unwrap()
                .join(format!("bundles/1.20.1/{BUNDLE}.mrpack")),
            BundleManifest {
                mc_version: "1.20.1".to_string(),
                ..manifest(vec![old_build])
            },
        )
        .await;
    let ctx = state.services.content();

    install_cluster_bundles(cluster_id, state.bundles.as_ref(), None, &ctx, None)
        .await
        .unwrap();

    assert!(!linked(&state, cluster_id, GITHUB_OLD_HASH).await);
}

#[tokio::test]
async fn launch_install_keeps_the_deadline_for_a_reused_bundle_names_replacement() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let body: Vec<u8> = (0..64 * 1024).map(|i| (i % 229) as u8).collect();
    let sha1 = polyio::sha1_bytes(&body);
    let size = body.len() as u64;
    let (url, _) = serve_jar(body, Some(std::time::Duration::from_millis(1500))).await;
    let cluster_id =
        cluster_with_other_version_copy(&state, BUNDLE, github_mod_served(&url, &sha1, size)).await;
    let old_build = github_mod_served(
        "unsupported://example.invalid/github-mod-old.jar",
        GITHUB_OLD_HASH,
        1,
    );
    state
        .bundles
        .cache_archive_manifest(
            oneclient_common::paths::data_dir()
                .unwrap()
                .join(format!("bundles/1.20.1/{BUNDLE}.mrpack")),
            BundleManifest {
                mc_version: "1.20.1".to_string(),
                ..manifest(vec![old_build])
            },
        )
        .await;
    let ctx = state.services.content();

    let started = std::time::Instant::now();
    let deadline = Some(started + std::time::Duration::from_millis(300));
    install_cluster_bundles(cluster_id, state.bundles.as_ref(), None, &ctx, deadline)
        .await
        .unwrap();

    assert!(
        started.elapsed() < std::time::Duration::from_secs(1),
        "took {:?}",
        started.elapsed()
    );
    assert!(!linked(&state, cluster_id, &sha1).await);
}
