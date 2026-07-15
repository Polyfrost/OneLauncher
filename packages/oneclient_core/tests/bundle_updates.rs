use oneclient_core::bundles::{BundleFile, BundleFileKind, BundleManifest, check_bundle_updates};
use oneclient_core::clusters::{ClusterManager, CreateClusterOptions};
use oneclient_core::packages::domain::{ContentType, GameLoader, ProviderId};
use oneclient_core::LauncherState;
use oneclient_db::dao::{artifact as artifact_dao, cluster_bundle as bundle_dao};
use oneclient_db::models::OverrideType;

const BUNDLE: &str = "Test Bundle";
const MC_VERSION: &str = "1.21.1";
const PROJECT_ID: &str = "sodium";
const VERSION_ID: &str = "v1";
const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn managed_file(enabled: bool) -> BundleFile {
    BundleFile {
        enabled,
        hidden: false,
        path: "mods/sodium.jar".to_string(),
        size: 1,
        kind: BundleFileKind::Managed {
            provider: ProviderId::Modrinth,
            project_id: PROJECT_ID.to_string(),
            version_id: VERSION_ID.to_string(),
            sha1: HASH.to_string(),
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
        files,
    }
}

async fn cluster_with_tracked_mod(state: &LauncherState) -> i64 {
    let cluster = ClusterManager::create(
        state,
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

    let check = check_bundle_updates(cluster_id, state.bundles.as_ref(), &state.services)
        .await
        .unwrap();

    assert!(
        check.removals_available.is_empty(),
        "an unchanged tracked mod was flagged for removal: {:?}",
        check.removals_available
    );
    assert!(check.updates_available.is_empty());
}

/// behaviour the disabled-mod case must not be allowed to imitate.
#[tokio::test]
async fn mod_dropped_from_manifest_is_removed() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    oneclient_core::dev::seed_bundle_archive(&state, manifest(vec![]))
        .await
        .unwrap();
    let cluster_id = cluster_with_tracked_mod(&state).await;

    let check = check_bundle_updates(cluster_id, state.bundles.as_ref(), &state.services)
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

    let check = check_bundle_updates(cluster_id, state.bundles.as_ref(), &state.services)
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

    let check = check_bundle_updates(cluster_id, state.bundles.as_ref(), &state.services)
        .await
        .unwrap();

    assert!(
        check.removals_available.is_empty(),
        "a user-disabled mod is still shipped by the bundle and must not be removed: {:?}",
        check.removals_available
    );
}
