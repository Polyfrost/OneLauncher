#![recursion_limit = "256"]

use oneclient_common::domain::{ContentType, GameLoader};
use oneclient_common::paths;
use oneclient_content::packages::PackageStore;
use oneclient_core::clusters::CreateClusterOptions;
use oneclient_core::game::materialize_content;

fn pack(tag: &str) -> Vec<u8> {
    let mut bytes = b"PK\x05\x06".to_vec();
    bytes.extend([0; 16]);
    bytes.extend((tag.len() as u16).to_le_bytes());
    bytes.extend(tag.as_bytes());
    bytes
}

#[tokio::test]
async fn packs_survive_hand_edits() {
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
}
