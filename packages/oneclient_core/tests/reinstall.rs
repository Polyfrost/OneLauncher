mod common;

use oneclient_common::domain::{ContentType, GameLoader, ProviderId};
use oneclient_content::packages::{
    PackageStore, VersionDetail, VersionFile, cached_project_detail,
};
use oneclient_core::clusters::{ClusterKind, CreateClusterOptions};
use oneclient_db::dao::artifact as artifact_dao;

#[tokio::test]
async fn reinstalling_a_disabled_mod_switches_it_on() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let global = state.settings.read().global_game_settings.clone();
    let mut options = CreateClusterOptions::new("Reinstall", "1.21.1", GameLoader::Fabric)
        .kind(ClusterKind::Modded);
    options.mc_loader_version = Some("0.16.5".into());
    let cluster = state.clusters.create(&global, options).await.unwrap();
    let ctx = state.services.content();

    let bytes = b"PK\x05\x06reinstall".to_vec();
    let hash = polyio::sha1_bytes(&bytes);
    common::link_modrinth_cached_jar(&state, cluster.id, "Ver", &bytes, &hash).await;
    oneclient_content::bundles::set_artifact_enabled_to(cluster.id, &hash, false, &ctx)
        .await
        .unwrap();

    let project =
        cached_project_detail(&ctx, ProviderId::Modrinth, "NoSuchProj", ContentType::Mod).await;
    let version = VersionDetail {
        version_id: "Ver".into(),
        project_id: "NoSuchProj".into(),
        name: String::new(),
        version_number: "1.0".into(),
        changelog: None,
        game_versions: Vec::new(),
        loaders: Vec::new(),
        published: chrono::Utc::now(),
        downloads: 0,
        files: vec![VersionFile {
            sha1: hash.clone(),
            url: "https://example.invalid/reinstall.jar".into(),
            file_name: "reinstall.jar".into(),
            primary: true,
            size: bytes.len() as u64,
            fingerprint: None,
        }],
        dependencies: Vec::new(),
    };
    PackageStore::install_to_cluster(
        ProviderId::Modrinth,
        &project,
        &version,
        cluster.id,
        true,
        false,
        None,
        &ctx,
    )
    .await
    .unwrap();
    oneclient_content::bundles::set_artifact_enabled_to(cluster.id, &hash, true, &ctx)
        .await
        .unwrap();

    let link = artifact_dao::get_cluster_artifact(&state.services.db, cluster.id, &hash)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(link.enabled, 1, "an explicit reinstall switches it back on");
}
