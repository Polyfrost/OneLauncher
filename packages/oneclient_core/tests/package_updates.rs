use oneclient_common::domain::{ContentType, GameLoader, ProviderId};
use oneclient_core::clusters::CreateClusterOptions;
use oneclient_db::dao::{artifact as artifact_dao, browser_package_update as update_dao};

const KEPT: [&str; 2] = [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "cccccccccccccccccccccccccccccccccccccccc",
];
const CHECKED: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[tokio::test]
async fn checking_one_package_keeps_the_other_packages_updates() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let global = state.settings.read().global_game_settings.clone();
    let cluster = state
        .clusters
        .create(
            &global,
            CreateClusterOptions::new("Updates Cluster", "1.21.1", GameLoader::Fabric),
        )
        .await
        .unwrap();
    let db = &state.services.db;

    for hash in KEPT {
        let file = format!("{hash}.jar");
        artifact_dao::insert_artifact(db, hash, ContentType::Mod as i64, &file, &file, Some(1))
            .await
            .unwrap();
        artifact_dao::link_cluster_artifact(db, cluster.id, hash, &file)
            .await
            .unwrap();
        update_dao::upsert(
            db,
            cluster.id,
            hash,
            ProviderId::Modrinth as i64,
            "project",
            "v1",
            "1.0",
            "v2",
            "2.0",
            "Mod",
        )
        .await
        .unwrap();
    }

    let check = oneclient_core::refresh_browser_package_updates_for(
        cluster.id,
        &[CHECKED.to_string()],
        &state.services.content(),
    )
    .await
    .unwrap();

    assert_eq!(check.unchecked, vec![CHECKED]);
    let mut left: Vec<String> = update_dao::list_for_cluster(db, cluster.id)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.hash)
        .collect();
    left.sort();
    assert_eq!(left, KEPT);
}
