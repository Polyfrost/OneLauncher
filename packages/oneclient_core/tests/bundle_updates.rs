use oneclient_common::domain::{ContentType, GameLoader, ProviderId};
use oneclient_content::bundles::{
    BundleFile, BundleFileKind, BundleFileType, BundleManifest, check_bundle_updates,
    get_bundles_with_update_status,
};
use oneclient_core::LauncherState;
use oneclient_core::clusters::CreateClusterOptions;
use oneclient_db::dao::{artifact as artifact_dao, cluster_bundle as bundle_dao};
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
