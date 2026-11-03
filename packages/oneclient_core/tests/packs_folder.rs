#![recursion_limit = "256"]

use oneclient_common::domain::{ContentType, GameLoader};
use oneclient_common::paths;
use oneclient_content::packages::PackageStore;
use oneclient_core::clusters::{
    CreateClusterOptions, ExportMode, ModpackSource, create_modpack_instance, export_cluster,
    install_modpack_instance, prepare_modpack,
};
use oneclient_core::game::materialize_content;

fn pack(tag: &str) -> Vec<u8> {
    let mut bytes = b"PK\x05\x06".to_vec();
    bytes.extend([0; 16]);
    bytes.extend((tag.len() as u16).to_le_bytes());
    bytes.extend(tag.as_bytes());
    bytes
}

#[tokio::test]
async fn packs_survive_hand_edits_and_dedicated_clusters_keep_their_own() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let global = state.settings.read().global_game_settings.clone();
    let ctx = state.services.content();
    let create = |name: &'static str| {
        let global = global.clone();
        let state = state.clone();
        async move {
            state
                .clusters
                .create(
                    &global,
                    CreateClusterOptions::new(name, "1.21.1", GameLoader::Fabric),
                )
                .await
                .unwrap()
        }
    };
    let links = |cluster_id: i64| {
        let ctx = ctx.clone();
        async move {
            let mut links: Vec<(String, bool)> =
                PackageStore::list_linked_artifacts(cluster_id, &ctx)
                    .await
                    .unwrap()
                    .into_iter()
                    .filter(|link| link.content_type == ContentType::ResourcePack)
                    .map(|link| (link.cluster_file_name, link.enabled))
                    .collect();
            links.sort();
            links
        }
    };

    let import = |cluster_id: i64, dir: std::path::PathBuf, name: &'static str| {
        let ctx = ctx.clone();
        async move {
            let source = dir.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(&source, pack(name)).unwrap();
            PackageStore::import_local_file(&source, ContentType::ResourcePack, cluster_id, &ctx)
                .await
                .unwrap();
            std::fs::remove_file(&source).ok();
        }
    };
    let hash_of = |cluster_id: i64, name: &'static str| {
        let ctx = ctx.clone();
        async move {
            PackageStore::list_linked_artifacts(cluster_id, &ctx)
                .await
                .unwrap()
                .into_iter()
                .find(|link| link.cluster_file_name == name)
                .unwrap()
                .hash
        }
    };

    let shared = create("Shared").await;
    let shared_dir = paths::shared_minecraft_dir().unwrap();
    let source = shared.dir().unwrap().join("pack.zip");
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    std::fs::write(&source, pack("one")).unwrap();
    PackageStore::import_local_file(&source, ContentType::ResourcePack, shared.id, &ctx)
        .await
        .unwrap();
    std::fs::remove_file(&source).unwrap();

    materialize_content(&state.services, &shared, &shared_dir, false)
        .await
        .unwrap();
    let placed = shared_dir.join("resourcepacks/pack.zip");
    assert_eq!(std::fs::read(&placed).unwrap(), pack("one"));

    std::fs::write(&placed, pack("edited by hand")).unwrap();
    materialize_content(&state.services, &shared, &shared_dir, false)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(&placed).unwrap(),
        pack("edited by hand"),
        "the launch keeps the edit instead of restoring the cached original"
    );
    assert_eq!(
        links(shared.id).await,
        vec![
            ("pack.zip".to_owned(), false),
            ("pack.zip".to_owned(), true)
        ],
        "the original is switched off and the edit linked"
    );

    sqlx::query(
        "UPDATE cluster_artifacts SET bundle_name = 'bundle', package_id = 'package' WHERE cluster_id = ? AND enabled = 0",
    )
    .bind(shared.id)
    .execute(&state.services.db)
    .await
    .unwrap();

    let dedicated = create("Dedicated").await;
    state
        .clusters
        .set_dedicated_dir(dedicated.id, true, false)
        .await
        .unwrap();
    let dedicated = state.clusters.get(dedicated.id).await.unwrap();
    let own_dir = dedicated.dir().unwrap();
    import(dedicated.id, own_dir.join("incoming"), "solo.zip").await;
    std::fs::remove_dir_all(own_dir.join("resourcepacks")).ok();
    polyio::symlink_dir(
        shared_dir.join("resourcepacks"),
        own_dir.join("resourcepacks"),
    )
    .await
    .unwrap();

    oneclient_core::game::split_dedicated_packs(&state).await;
    assert!(
        links(shared.id)
            .await
            .contains(&("solo.zip".to_owned(), true)),
        "the startup split hands the shared cluster the pack only the dedicated one had"
    );

    materialize_content(&state.services, &dedicated, &own_dir, false)
        .await
        .unwrap();
    let own_packs = own_dir.join("resourcepacks");
    assert!(
        !std::fs::symlink_metadata(&own_packs)
            .unwrap()
            .file_type()
            .is_symlink(),
        "the dedicated cluster gets a real pack folder back"
    );
    assert_eq!(
        links(dedicated.id).await,
        vec![
            ("pack.zip".to_owned(), false),
            ("pack.zip".to_owned(), true),
            ("solo.zip".to_owned(), true)
        ],
        "it keeps the packs it was playing with"
    );
    assert_eq!(
        links(shared.id).await,
        vec![
            ("pack.zip".to_owned(), false),
            ("pack.zip".to_owned(), true),
            ("solo.zip".to_owned(), true)
        ],
        "the shared cluster keeps the pack only the dedicated one had a row for"
    );
    let copied: (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT bundle_name, package_id FROM cluster_artifacts WHERE cluster_id = ? AND enabled = 0",
    )
    .bind(dedicated.id)
    .fetch_one(&state.services.db)
    .await
    .unwrap();
    assert_eq!(
        copied,
        (Some("bundle".to_owned()), Some("package".to_owned())),
        "the copy stays bundle and package managed"
    );
    assert_eq!(
        std::fs::read(own_packs.join("pack.zip")).unwrap(),
        pack("edited by hand")
    );

    let edited = PackageStore::list_linked_artifacts(dedicated.id, &ctx)
        .await
        .unwrap()
        .into_iter()
        .find(|link| {
            link.content_type == ContentType::ResourcePack
                && link.enabled
                && link.cluster_file_name == "pack.zip"
        })
        .unwrap();
    PackageStore::set_artifact_enabled_to(dedicated.id, &edited.hash, false, &ctx)
        .await
        .unwrap();
    assert_eq!(
        links(shared.id).await,
        vec![
            ("pack.zip".to_owned(), false),
            ("pack.zip".to_owned(), true),
            ("solo.zip".to_owned(), true)
        ],
        "switching a pack off in the dedicated cluster leaves the shared ones alone"
    );

    let leaving = create("Leaving").await;
    import(leaving.id, leaving.dir().unwrap(), "leaving.zip").await;
    import(leaving.id, leaving.dir().unwrap(), "off.zip").await;
    PackageStore::set_artifact_enabled_to(
        leaving.id,
        &hash_of(leaving.id, "off.zip").await,
        false,
        &ctx,
    )
    .await
    .unwrap();
    state
        .clusters
        .set_dedicated_dir(leaving.id, true, false)
        .await
        .unwrap();
    let leaving = state.clusters.get(leaving.id).await.unwrap();
    let leaving_dir = leaving.dir().unwrap();
    oneclient_core::game::leave_shared_folder(&state.services, &leaving, &leaving_dir, None)
        .await
        .unwrap();
    materialize_content(&state.services, &leaving, &leaving_dir, false)
        .await
        .unwrap();
    assert!(leaving_dir.join("resourcepacks/leaving.zip").exists());
    assert_eq!(
        links(leaving.id).await,
        vec![
            ("leaving.zip".to_owned(), true),
            ("off.zip".to_owned(), false),
            ("pack.zip".to_owned(), false),
            ("pack.zip".to_owned(), true),
            ("solo.zip".to_owned(), true)
        ],
        "the cluster turning dedicated takes the shared packs along"
    );
    let shared_links = links(shared.id).await;
    assert!(
        shared_links.contains(&("leaving.zip".to_owned(), true)),
        "the shared cluster keeps the pack installed through the one that left"
    );
    assert!(
        !shared_links.iter().any(|(name, _)| name == "off.zip"),
        "a pack the leaving cluster had off is not handed over"
    );

    materialize_content(&state.services, &shared, &shared_dir, false)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(shared_dir.join("resourcepacks/leaving.zip")).unwrap(),
        pack("leaving.zip"),
        "the shared folder does not drop it"
    );
    assert!(shared_dir.join("resourcepacks/solo.zip").exists());

    PackageStore::set_artifact_enabled_to(
        leaving.id,
        &hash_of(leaving.id, "solo.zip").await,
        false,
        &ctx,
    )
    .await
    .unwrap();
    PackageStore::set_artifact_enabled_to(
        shared.id,
        &hash_of(shared.id, "leaving.zip").await,
        false,
        &ctx,
    )
    .await
    .unwrap();
    assert!(
        links(leaving.id)
            .await
            .contains(&("leaving.zip".to_owned(), true))
    );

    let own_pack = leaving_dir.join("resourcepacks/pack.zip");
    std::fs::remove_file(&own_pack).unwrap();
    std::fs::write(&own_pack, pack("replaced while apart")).unwrap();

    state
        .clusters
        .set_dedicated_dir(leaving.id, false, false)
        .await
        .unwrap();
    let rejoined = state.clusters.get(leaving.id).await.unwrap();
    oneclient_core::game::join_shared_folder(&state.services, &rejoined).await;
    let rejoined = links(rejoined.id).await;
    assert_eq!(
        rejoined
            .iter()
            .filter(|link| **link == ("pack.zip".to_owned(), false))
            .count(),
        2,
        "the replacement is kept, switched off as the shared folder holds the original"
    );
    assert!(
        links(shared.id)
            .await
            .contains(&("pack.zip".to_owned(), true)),
        "a replacement made apart does not switch the shared cluster's pack off"
    );
    assert!(
        rejoined.contains(&("solo.zip".to_owned(), true)),
        "on in the shared folder, so on again in the cluster coming back"
    );
    assert!(
        rejoined.contains(&("leaving.zip".to_owned(), false)),
        "off in the shared folder, so off in the cluster coming back"
    );
    assert!(
        rejoined.contains(&("off.zip".to_owned(), false)),
        "a pack only it has keeps its state"
    );

    let rejoined = state.clusters.get(leaving.id).await.unwrap();
    materialize_content(&state.services, &rejoined, &shared_dir, false)
        .await
        .unwrap();
    assert!(
        std::fs::symlink_metadata(shared_dir.join("resourcepacks/leaving.zip")).is_err(),
        "the copy it placed while dedicated does not follow it into the shared folder"
    );
    assert!(shared_dir.join("resourcepacks/solo.zip").exists());
    assert_eq!(
        std::fs::read(shared_dir.join("resourcepacks/pack.zip")).unwrap(),
        pack("edited by hand")
    );

    let source = create("Source").await;
    state
        .clusters
        .set_dedicated_dir(source.id, true, false)
        .await
        .unwrap();
    import(
        source.id,
        source.dir().unwrap().join("incoming"),
        "both.zip",
    )
    .await;
    import(
        shared.id,
        shared.dir().unwrap().join("incoming"),
        "both.zip",
    )
    .await;
    PackageStore::set_artifact_enabled_to(
        source.id,
        &hash_of(source.id, "both.zip").await,
        false,
        &ctx,
    )
    .await
    .unwrap();

    let scratch = polyio::testing::ScratchDir::new("packs-folder");
    let file = scratch.path().join("Source.mrpack");
    export_cluster(&state, source.id, &file, ExportMode::OneClient, None)
        .await
        .unwrap();
    let prepared = prepare_modpack(&state, &ModpackSource::File(file), None)
        .await
        .unwrap();
    let created = create_modpack_instance(&state, &prepared).await.unwrap();
    let imported = install_modpack_instance(&state, created.id, &prepared, None)
        .await
        .unwrap()
        .cluster;
    assert!(imported.uses_dedicated_dir());
    assert!(
        links(imported.id)
            .await
            .contains(&("both.zip".to_owned(), false)),
        "the pack the share has off stays off though a shared cluster has it on"
    );
    assert!(
        links(shared.id)
            .await
            .contains(&("both.zip".to_owned(), true)),
        "the shared cluster's pack is left alone"
    );
}
