use oneclient_common::domain::GameLoader;
use oneclient_content::packages::PackageStore;
use oneclient_content::packages::store::manifest::{self, MaterializedManifest};
use oneclient_core::clusters::CreateClusterOptions;
use oneclient_core::game::sync_cluster_mods;

fn jar(tag: &str) -> Vec<u8> {
    let mut bytes = b"PK\x05\x06".to_vec();
    bytes.extend([0; 16]);
    bytes.extend((tag.len() as u16).to_le_bytes());
    bytes.extend(tag.as_bytes());
    bytes
}

#[tokio::test]
async fn hand_edits_to_the_mods_folder_reach_the_database() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let global = state.settings.read().global_game_settings.clone();
    let cluster = state
        .clusters
        .create(
            &global,
            CreateClusterOptions::new("Sync", "1.21.1", GameLoader::Fabric),
        )
        .await
        .unwrap();

    let cluster_dir = cluster.dir().unwrap();
    let mods = cluster_dir.join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    manifest::save(
        &cluster_dir,
        manifest::MODS_MANIFEST_NAME,
        &MaterializedManifest::new(cluster.id, Vec::new()),
    )
    .await;

    let ctx = state.services.content();
    let links = || async {
        let mut links: Vec<(String, bool)> = PackageStore::list_linked_artifacts(cluster.id, &ctx)
            .await
            .unwrap()
            .into_iter()
            .map(|link| (link.cluster_file_name, link.enabled))
            .collect();
        links.sort();
        links
    };

    std::fs::write(mods.join("a.jar"), jar("one")).unwrap();
    assert!(sync_cluster_mods(&state, cluster.id).await);
    assert_eq!(links().await, vec![("a.jar".to_owned(), true)]);
    assert!(
        !sync_cluster_mods(&state, cluster.id).await,
        "a second pass over an untouched folder changes nothing"
    );

    std::fs::rename(mods.join("a.jar"), mods.join("b.jar")).unwrap();
    assert!(sync_cluster_mods(&state, cluster.id).await);
    assert_eq!(links().await, vec![("b.jar".to_owned(), true)]);

    std::fs::write(mods.join("b.jar"), jar("two")).unwrap();
    assert!(sync_cluster_mods(&state, cluster.id).await);
    assert_eq!(
        links().await,
        vec![("b.jar".to_owned(), false), ("b.jar".to_owned(), true)],
        "the old build is switched off and the new one linked"
    );
    assert!(
        mods.join("b.jar").exists(),
        "switching the old build off must not take the new jar with it"
    );

    std::fs::remove_file(mods.join("b.jar")).unwrap();
    assert!(sync_cluster_mods(&state, cluster.id).await);
    assert_eq!(
        links().await,
        vec![("b.jar".to_owned(), false), ("b.jar".to_owned(), false)]
    );

    std::fs::write(mods.join("b.jar"), jar("two")).unwrap();
    assert!(sync_cluster_mods(&state, cluster.id).await);
    assert_eq!(
        links().await,
        vec![("b.jar".to_owned(), false), ("b.jar".to_owned(), true)],
        "a jar put back is switched on again"
    );
}

#[tokio::test]
async fn a_mod_removed_in_the_launcher_stays_removed_before_the_first_launch() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let global = state.settings.read().global_game_settings.clone();
    let cluster = state
        .clusters
        .create(
            &global,
            CreateClusterOptions::new("Unlaunched", "1.21.1", GameLoader::Fabric),
        )
        .await
        .unwrap();

    let mods = cluster.dir().unwrap().join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    let ctx = state.services.content();

    std::fs::write(mods.join("a.jar"), jar("one")).unwrap();
    assert!(sync_cluster_mods(&state, cluster.id).await);
    let linked = PackageStore::list_linked_artifacts(cluster.id, &ctx)
        .await
        .unwrap();

    oneclient_content::bundles::remove_artifact_from_cluster(
        cluster.id,
        &linked[0].hash,
        false,
        &ctx,
    )
    .await
    .unwrap();

    assert!(!mods.join("a.jar").exists());
    assert!(!sync_cluster_mods(&state, cluster.id).await);
}
