#![recursion_limit = "256"]

use oneclient_common::domain::{ContentType, GameLoader};
use oneclient_common::paths;
use oneclient_content::packages::PackageStore;
use oneclient_core::clusters::{CreateClusterOptions, DirTarget};
use oneclient_core::game::{join_shared_folder, leave_shared_folder, materialize_content};

fn pack(tag: &str) -> Vec<u8> {
    let mut bytes = b"PK\x05\x06".to_vec();
    bytes.extend([0; 16]);
    bytes.extend((tag.len() as u16).to_le_bytes());
    bytes.extend(tag.as_bytes());
    bytes
}

#[tokio::test]
async fn each_shared_folder_holds_only_its_own_clusters_packs() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let global = state.settings.read().global_game_settings.clone();
    let ctx = state.services.content();

    let create = |name: &'static str, group: &'static str| {
        let global = global.clone();
        let state = state.clone();
        async move {
            let cluster = state
                .clusters
                .create(
                    &global,
                    CreateClusterOptions::new(name, "1.21.1", GameLoader::Fabric),
                )
                .await
                .unwrap();
            let group = match state.clusters.find_shared_group(group).await.unwrap() {
                Some(existing) => existing,
                None => state.clusters.create_shared_group(group).await.unwrap(),
            };
            state
                .clusters
                .set_dir_target(cluster.id, &DirTarget::Shared(Some(group)), false)
                .await
                .unwrap();
            state.clusters.get(cluster.id).await.unwrap()
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

    let alpha = create("Alpha", "Friends").await;
    let beta = create("Beta", "Friends").await;
    let gamma = create("Gamma", "Family").await;
    import(
        alpha.id,
        alpha.dir().unwrap().join("incoming"),
        "friends.zip",
    )
    .await;
    import(
        gamma.id,
        gamma.dir().unwrap().join("incoming"),
        "family.zip",
    )
    .await;

    let friends = alpha.game_dir().unwrap();
    let family = gamma.game_dir().unwrap();
    assert_eq!(friends, paths::shared_group_dir(Some("Friends")).unwrap());
    assert_eq!(family, paths::shared_group_dir(Some("Family")).unwrap());
    assert_eq!(beta.game_dir().unwrap(), friends);
    assert_eq!(
        state.clusters.list_shared_groups().await.unwrap(),
        vec!["Family".to_owned(), "Friends".to_owned()]
    );
    assert!(
        state.clusters.create_shared_group("fRIENDS").await.is_err(),
        "a name differing only in case is taken"
    );
    state
        .clusters
        .set_dir_target(
            beta.id,
            &DirTarget::Shared(Some("friends".to_owned())),
            false,
        )
        .await
        .unwrap();
    assert_eq!(
        state.clusters.get(beta.id).await.unwrap().shared_group(),
        Some("Friends".to_owned()),
        "picking a folder stores its name as it is on disk"
    );

    materialize_content(&state.services, &alpha, &friends, false)
        .await
        .unwrap();
    materialize_content(&state.services, &gamma, &family, false)
        .await
        .unwrap();
    materialize_content(&state.services, &beta, &friends, false)
        .await
        .unwrap();

    assert!(
        friends.join("resourcepacks/friends.zip").exists(),
        "beta launches with the pack alpha put in their folder"
    );
    assert!(!friends.join("resourcepacks/family.zip").exists());
    assert!(family.join("resourcepacks/family.zip").exists());
    assert!(!family.join("resourcepacks/friends.zip").exists());

    state
        .clusters
        .set_dir_target(
            beta.id,
            &DirTarget::Shared(Some("Family".to_owned())),
            false,
        )
        .await
        .unwrap();
    let beta = state.clusters.get(beta.id).await.unwrap();
    leave_shared_folder(
        &state.services,
        &beta,
        &beta.dir().unwrap(),
        Some("Friends"),
    )
    .await
    .unwrap();
    join_shared_folder(&state.services, &beta).await;
    assert_eq!(beta.game_dir().unwrap(), family);

    materialize_content(&state.services, &beta, &family, false)
        .await
        .unwrap();
    materialize_content(&state.services, &alpha, &friends, false)
        .await
        .unwrap();

    assert!(
        friends.join("resourcepacks/friends.zip").exists(),
        "the pack stays in the folder beta left"
    );
    assert!(
        family.join("resourcepacks/friends.zip").exists(),
        "beta brings its copy of the pack along"
    );
    assert!(family.join("resourcepacks/family.zip").exists());

    let default_dir = paths::shared_minecraft_dir().unwrap();
    std::fs::create_dir_all(&default_dir).unwrap();
    std::fs::write(default_dir.join("options.txt"), b"from default").unwrap();
    state
        .clusters
        .sync_from_shared(gamma.id, None, false, |_, _, _| {})
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(family.join("options.txt")).unwrap(),
        b"from default",
        "a custom folder syncs into itself, not into the instance folder"
    );
    assert!(
        state
            .clusters
            .sync_from_shared(gamma.id, Some("Family"), false, |_, _, _| {})
            .await
            .is_err(),
        "a folder cannot sync from itself"
    );

    assert!(
        state.clusters.delete_shared_group("Friends").await.is_err(),
        "alpha still uses the folder"
    );
    assert!(friends.exists());

    state
        .clusters
        .set_dir_target(alpha.id, &DirTarget::Dedicated, false)
        .await
        .unwrap();
    let kept = alpha.dir().unwrap().join("kept.txt");
    std::fs::write(&kept, b"kept").unwrap();
    std::fs::create_dir_all(friends.join("mods")).unwrap();
    polyio::symlink_dir(alpha.dir().unwrap(), friends.join("mods").join("Alpha"))
        .await
        .unwrap();

    state.clusters.delete_shared_group("Friends").await.unwrap();
    assert!(!friends.exists());
    assert!(
        kept.exists(),
        "deleting follows no link into an instance folder"
    );
    assert_eq!(
        state.clusters.list_shared_groups().await.unwrap(),
        vec!["Family".to_owned()]
    );
}
