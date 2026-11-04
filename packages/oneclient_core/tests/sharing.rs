#![recursion_limit = "256"]

use oneclient_common::domain::{ContentType, GameLoader};
use oneclient_content::packages::PackageStore;
use oneclient_core::LauncherError;
use oneclient_core::clusters::{
    ClusterKind, CreateClusterOptions, ExportMode, ExportOptions, ModpackSource, SettingsChoice,
    create_modpack_instance, install_modpack_instance, prepare_modpack,
};
use oneclient_core::sharing::{
    CODE_LENGTH, LinkedLedger, LocalShareStore, MAX_SHARE_BYTES, ShareCode, ShareLedger, ShareMeta,
    ShareStore, ShareUpdate, SharingError, apply_update, check_update, describe_payload,
    fetch_share_into, link_import, owner_status, share_cluster, share_settings_only, stop_sharing,
};

fn jar(tag: &str) -> Vec<u8> {
    let mut bytes = b"PK\x05\x06".to_vec();
    bytes.extend([0; 16]);
    bytes.extend((tag.len() as u16).to_le_bytes());
    bytes.extend(tag.as_bytes());
    bytes
}

fn meta(size: u64) -> ShareMeta {
    ShareMeta {
        format: 1,
        name: "Pack".into(),
        mc_version: "1.21.1".into(),
        mc_loader: GameLoader::Fabric,
        kind: ClusterKind::Modded,
        size,
        sha512: "00".into(),
        created_at: None,
    }
}

#[test]
fn generated_codes_parse_back() {
    for _ in 0..64 {
        let code = ShareCode::generate();
        assert_eq!(code.as_str().len(), CODE_LENGTH);
        assert_eq!(ShareCode::parse(code.as_str()), Some(code.clone()));
        assert_eq!(ShareCode::parse(&code.pretty()), Some(code));
    }
}

#[test]
fn typed_codes_forgive_case_dashes_and_lookalikes() {
    assert_eq!(ShareCode::parse("abcd-efgh").unwrap().as_str(), "ABCDEFGH");
    assert_eq!(
        ShareCode::parse(" o1lI 2345 ").unwrap().as_str(),
        "01112345"
    );
    assert_eq!(
        ShareCode::parse("ABCDEFGH").unwrap().to_string(),
        "ABCD-EFGH"
    );
}

#[test]
fn bad_codes_are_refused() {
    for bad in ["", "ABC", "ABCDEFGHJ", "ABCD-EFGU", "ABCD/EFGH", "ABCDÉFGH"] {
        assert_eq!(ShareCode::parse(bad), None, "{bad}");
    }
}

#[test]
fn links_carry_the_code() {
    let code = ShareCode::parse("ABCDEFGH").unwrap();
    assert_eq!(ShareCode::parse(&code.web_link()), Some(code.clone()));
    assert_eq!(
        ShareCode::parse(&code.app_link("oneclient")),
        Some(code.clone())
    );
    assert_eq!(
        ShareCode::parse("https://share.polyfrost.org/ABCD-EFGH/"),
        Some(code.clone())
    );
    assert_eq!(ShareCode::parse("ONECLIENT://share/abcdefgh"), Some(code));
}

#[test]
fn foreign_links_are_not_codes() {
    assert_eq!(ShareCode::parse("https://example.com/share/ABCDEFGH"), None);
    assert_eq!(
        ShareCode::parse("http://polyfrost.org/share/ABCDEFGH"),
        None
    );
    assert_eq!(ShareCode::parse("oneclient://launch/ABCDEFGH"), None);
}

#[test]
fn codes_serialize_as_plain_strings() {
    let code = ShareCode::parse("ABCDEFGH").unwrap();
    let json = serde_json::to_string(&code).unwrap();
    assert_eq!(json, "\"ABCDEFGH\"");
    assert_eq!(serde_json::from_str::<ShareCode>(&json).unwrap(), code);
    assert!(serde_json::from_str::<ShareCode>("\"nope\"").is_err());
}

#[tokio::test]
async fn the_local_store_publishes_updates_and_revokes() {
    let scratch = polyio::testing::ScratchDir::new("share-local-store");
    let store = LocalShareStore::new(scratch.path().join("store"));
    let payload = scratch.path().join("a.mrpack");
    std::fs::write(&payload, b"first").unwrap();

    let published = store.publish(&payload, &meta(5)).await.unwrap();
    assert_eq!(published.link, published.code.web_link());
    let described = store.describe(&published.code).await.unwrap();
    assert!(described.meta.created_at.is_some());
    assert_eq!(described.revision, 1);

    let fetched = scratch.path().join("out/a.mrpack");
    store.fetch(&published.code, &fetched).await.unwrap();
    assert_eq!(std::fs::read(&fetched).unwrap(), b"first");

    std::fs::write(&payload, b"second!").unwrap();
    let mut stranger = published.clone();
    stranger.manage_token = "not the token".into();
    assert!(matches!(
        store.update(&stranger, &payload, &meta(7)).await,
        Err(SharingError::Forbidden(_))
    ));
    store.update(&published, &payload, &meta(7)).await.unwrap();
    store.fetch(&published.code, &fetched).await.unwrap();
    assert_eq!(std::fs::read(&fetched).unwrap(), b"second!");
    let updated = store.describe(&published.code).await.unwrap();
    assert_eq!(updated.meta.size, 7);
    assert_eq!(updated.revision, 2);

    assert!(store.revoke(&stranger).await.is_err());
    store.revoke(&published).await.unwrap();
    assert!(matches!(
        store.describe(&published.code).await,
        Err(SharingError::NotFound(_))
    ));
}

#[tokio::test]
async fn the_local_store_refuses_oversized_payloads() {
    let scratch = polyio::testing::ScratchDir::new("share-too-large");
    let store = LocalShareStore::new(scratch.path().join("store"));
    let payload = scratch.path().join("a.mrpack");
    std::fs::write(&payload, b"x").unwrap();
    assert!(matches!(
        store.publish(&payload, &meta(MAX_SHARE_BYTES + 1)).await,
        Err(SharingError::TooLarge { .. })
    ));
}

#[tokio::test]
async fn fetching_an_unknown_or_malformed_code_fails_cleanly() {
    let scratch = polyio::testing::ScratchDir::new("share-unknown");
    let store = LocalShareStore::new(scratch.path().join("store"));
    let dir = scratch.path().join("incoming");
    assert!(matches!(
        fetch_share_into(&store, "nope?", &dir).await,
        Err(LauncherError::SharingError(SharingError::InvalidCode))
    ));
    assert!(matches!(
        fetch_share_into(&store, "ABCD-EFGH", &dir).await,
        Err(LauncherError::SharingError(SharingError::NotFound(_)))
    ));
}

#[tokio::test]
async fn a_tampered_share_is_not_imported() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let global = state.settings.read().global_game_settings.clone();
    let mut options = CreateClusterOptions::new("Tamper", "1.21.1", GameLoader::Fabric)
        .kind(ClusterKind::Modded)
        .user_created(true);
    options.mc_loader_version = Some("0.16.5".into());
    let source = state.clusters.create(&global, options).await.unwrap();

    let scratch = polyio::testing::ScratchDir::new("share-tamper");
    let store = LocalShareStore::new(scratch.path().join("store"));
    let ledger = ShareLedger::at(scratch.path().join("published.json"));
    let shared = share_cluster(
        &state,
        source.id,
        ExportOptions::new(ExportMode::OneClient),
        &store,
        &ledger,
        None,
    )
    .await
    .unwrap();

    let stored = store
        .root()
        .join(format!("{}.mrpack", shared.published.code.as_str()));
    let mut bytes = std::fs::read(&stored).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    std::fs::write(&stored, bytes).unwrap();

    let dir = scratch.path().join("incoming");
    let fetched = fetch_share_into(&store, shared.published.code.as_str(), &dir).await;
    assert!(matches!(
        fetched,
        Err(LauncherError::SharingError(SharingError::Corrupt(_)))
    ));
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
}

#[tokio::test]
async fn a_shared_code_imports_with_mods_and_settings() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let global = state.settings.read().global_game_settings.clone();
    let ctx = state.services.content();

    let mut options = CreateClusterOptions::new("Code Pack", "1.21.1", GameLoader::Fabric)
        .kind(ClusterKind::Modded)
        .user_created(true);
    options.mc_loader_version = Some("0.16.5".into());
    let source = state.clusters.create(&global, options).await.unwrap();

    let scratch = polyio::testing::ScratchDir::new("share-code-flow");
    let local_jar = scratch.path().join("local-only.jar");
    std::fs::write(&local_jar, jar("local")).unwrap();
    PackageStore::import_local_files(&[(local_jar, ContentType::Mod)], source.id, &ctx)
        .await
        .unwrap();

    let game_dir = source.game_dir().unwrap();
    std::fs::create_dir_all(game_dir.join("config/polysprint")).unwrap();
    std::fs::create_dir_all(game_dir.join("oneconfig/profiles")).unwrap();
    std::fs::write(game_dir.join("options.txt"), b"fov:0.5").unwrap();
    std::fs::write(
        game_dir.join("config/polysprint/main.json"),
        b"{\"on\":true}",
    )
    .unwrap();
    std::fs::write(game_dir.join("config/session-token.json"), b"secret").unwrap();
    std::fs::write(game_dir.join("oneconfig/profiles/hud.json"), b"{}").unwrap();

    let store = LocalShareStore::new(scratch.path().join("store"));
    let ledger = ShareLedger::at(scratch.path().join("published.json"));
    let options = ExportOptions::new(ExportMode::OneClient).with_settings(true);
    let shared = share_cluster(&state, source.id, options.clone(), &store, &ledger, None)
        .await
        .unwrap();
    assert!(!shared.updated);
    assert_eq!(shared.report.settings, 3);
    assert_eq!(shared.meta.name, "Code Pack");
    assert_eq!(shared.meta.mc_loader, GameLoader::Fabric);

    let again = share_cluster(&state, source.id, options, &store, &ledger, None)
        .await
        .unwrap();
    assert!(again.updated, "sharing twice keeps the same code");
    assert_eq!(again.published.code, shared.published.code);

    let incoming = scratch.path().join("incoming");
    let fetched = fetch_share_into(&store, &shared.published.code.web_link(), &incoming)
        .await
        .unwrap();
    assert_eq!(fetched.code, shared.published.code);
    assert_eq!(describe_payload(&fetched.path).await.unwrap(), {
        let mut expected = again.meta.clone();
        expected.created_at = None;
        expected
    });

    let prepared = prepare_modpack(&state, &ModpackSource::File(fetched.path.clone()), None)
        .await
        .unwrap();
    let share = &prepared.share.as_ref().unwrap().share;
    assert!(share.dedicated_dir);
    assert_eq!(
        share.settings,
        vec![
            "config/polysprint/main.json".to_string(),
            "oneconfig/profiles/hud.json".to_string(),
            "options.txt".to_string(),
        ]
    );
    assert_eq!(prepared.summary().other_files, 3);

    let cluster = create_modpack_instance(&state, &prepared).await.unwrap();
    let installed = install_modpack_instance(&state, cluster.id, &prepared, None)
        .await
        .unwrap();
    assert!(
        installed.report.failed.is_empty(),
        "{:?}",
        installed.report.failed
    );

    let imported_dir = installed.cluster.game_dir().unwrap();
    assert_ne!(imported_dir, game_dir);
    assert_eq!(
        std::fs::read(imported_dir.join("config/polysprint/main.json")).unwrap(),
        b"{\"on\":true}"
    );
    assert_eq!(
        std::fs::read(imported_dir.join("options.txt")).unwrap(),
        b"fov:0.5"
    );
    assert!(imported_dir.join("oneconfig/profiles/hud.json").is_file());
    assert!(!imported_dir.join("config/session-token.json").exists());

    let links: Vec<String> = PackageStore::list_linked_artifacts(installed.cluster.id, &ctx)
        .await
        .unwrap()
        .into_iter()
        .map(|link| link.cluster_file_name)
        .collect();
    assert_eq!(links, vec!["local-only.jar".to_string()]);

    let stopped = stop_sharing(source.id, &store, &ledger).await.unwrap();
    assert_eq!(stopped, Some(shared.published.code.clone()));
    assert!(matches!(
        fetch_share_into(&store, shared.published.code.as_str(), &incoming).await,
        Err(LauncherError::SharingError(SharingError::NotFound(_)))
    ));
}

#[tokio::test]
async fn settings_stay_home_unless_asked_for() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let global = state.settings.read().global_game_settings.clone();
    let mut options = CreateClusterOptions::new("No Settings", "1.21.1", GameLoader::Fabric)
        .kind(ClusterKind::Modded)
        .user_created(true);
    options.mc_loader_version = Some("0.16.5".into());
    let source = state.clusters.create(&global, options).await.unwrap();
    let game_dir = source.game_dir().unwrap();
    std::fs::create_dir_all(game_dir.join("config")).unwrap();
    std::fs::write(game_dir.join("config/a.json"), b"{}").unwrap();

    let scratch = polyio::testing::ScratchDir::new("share-no-settings");
    let store = LocalShareStore::new(scratch.path().join("store"));
    let ledger = ShareLedger::at(scratch.path().join("published.json"));
    let shared = share_cluster(
        &state,
        source.id,
        ExportOptions::new(ExportMode::OneClient),
        &store,
        &ledger,
        None,
    )
    .await
    .unwrap();
    assert_eq!(shared.report.settings, 0);

    let fetched = fetch_share_into(
        &store,
        shared.published.code.as_str(),
        &scratch.path().join("incoming"),
    )
    .await
    .unwrap();
    let share = oneclient_content::modpacks::read_cluster_share(&fetched.path)
        .await
        .unwrap()
        .unwrap();
    assert!(share.settings.is_empty());
}

#[tokio::test]
async fn a_linked_import_follows_the_owners_updates() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let global = state.settings.read().global_game_settings.clone();
    let ctx = state.services.content();

    let mut options = CreateClusterOptions::new("Linked Pack", "1.21.1", GameLoader::Fabric)
        .kind(ClusterKind::Modded)
        .user_created(true);
    options.mc_loader_version = Some("0.16.5".into());
    let source = state.clusters.create(&global, options).await.unwrap();

    let scratch = polyio::testing::ScratchDir::new("share-linked-flow");
    let owner_jar = scratch.path().join("owner.jar");
    std::fs::write(&owner_jar, jar("owner")).unwrap();
    PackageStore::import_local_files(&[(owner_jar, ContentType::Mod)], source.id, &ctx)
        .await
        .unwrap();
    let game_dir = source.game_dir().unwrap();
    std::fs::create_dir_all(game_dir.join("config")).unwrap();
    std::fs::write(game_dir.join("options.txt"), b"fov:0.5").unwrap();
    std::fs::write(game_dir.join("config/kept.json"), b"{}").unwrap();
    std::fs::write(game_dir.join("config/private.json"), b"{}").unwrap();

    let store = LocalShareStore::new(scratch.path().join("store"));
    let ledger = ShareLedger::at(scratch.path().join("published.json"));
    let chosen = SettingsChoice::Only(
        ["options.txt".to_string(), "config/kept.json".to_string()]
            .into_iter()
            .collect(),
    );
    let options = ExportOptions::new(ExportMode::OneClient).with_setting_choice(chosen.clone());
    let shared = share_cluster(&state, source.id, options, &store, &ledger, None)
        .await
        .unwrap();
    assert_eq!(
        shared.report.settings, 2,
        "only the picked files are shared"
    );

    let status = owner_status(&state, source.id, &store, &ledger)
        .await
        .unwrap()
        .unwrap();
    assert!(!status.content_changed && !status.settings_changed);

    let incoming = scratch.path().join("incoming");
    let fetched = fetch_share_into(&store, shared.published.code.as_str(), &incoming)
        .await
        .unwrap();
    let prepared = prepare_modpack(&state, &ModpackSource::File(fetched.path.clone()), None)
        .await
        .unwrap();
    let cluster = create_modpack_instance(&state, &prepared).await.unwrap();
    install_modpack_instance(&state, cluster.id, &prepared, None)
        .await
        .unwrap();
    let links = LinkedLedger::at(scratch.path().join("linked.json"));
    let link = link_import(
        &state,
        &links,
        cluster.id,
        &fetched.code,
        &fetched.remote,
        &fetched.path,
    )
    .await
    .unwrap();
    assert_eq!(link.revision, 1);
    assert_eq!(link.owned.len(), 1);

    let mine = scratch.path().join("mine.jar");
    std::fs::write(&mine, jar("mine")).unwrap();
    PackageStore::import_local_files(&[(mine, ContentType::Mod)], cluster.id, &ctx)
        .await
        .unwrap();

    assert_eq!(
        check_update(&store, &links, cluster.id).await.unwrap(),
        Some(ShareUpdate::Current)
    );

    std::fs::write(game_dir.join("options.txt"), b"fov:1.0").unwrap();
    let status = owner_status(&state, source.id, &store, &ledger)
        .await
        .unwrap()
        .unwrap();
    assert!(status.settings_changed && !status.content_changed);

    let pushed = share_settings_only(&state, source.id, chosen, &store, &ledger, None)
        .await
        .unwrap();
    assert_eq!(pushed.published.code, shared.published.code);
    assert_eq!(pushed.report.settings, 2);
    let status = owner_status(&state, source.id, &store, &ledger)
        .await
        .unwrap()
        .unwrap();
    assert!(!status.content_changed && !status.settings_changed);

    let Some(ShareUpdate::Available { remote, .. }) =
        check_update(&store, &links, cluster.id).await.unwrap()
    else {
        panic!("the settings push should show up as an update");
    };
    assert_eq!(remote.revision, 2);

    let applied = apply_update(&state, &store, &links, cluster.id)
        .await
        .unwrap();
    assert_eq!(applied, cluster.id);
    let imported_dir = state
        .clusters
        .get(cluster.id)
        .await
        .unwrap()
        .game_dir()
        .unwrap();
    assert_eq!(
        std::fs::read(imported_dir.join("options.txt")).unwrap(),
        b"fov:1.0"
    );
    assert!(imported_dir.join("config/kept.json").is_file());
    assert!(!imported_dir.join("config/private.json").exists());

    let mut names: Vec<String> = PackageStore::list_linked_artifacts(cluster.id, &ctx)
        .await
        .unwrap()
        .into_iter()
        .map(|link| link.cluster_file_name)
        .collect();
    names.sort();
    assert_eq!(names, vec!["mine.jar".to_string(), "owner.jar".to_string()]);
    let link = links.find(cluster.id).await.unwrap().unwrap();
    assert_eq!(link.revision, 2);
    assert_eq!(link.owned.len(), 1, "your own mods stay yours");

    stop_sharing(source.id, &store, &ledger).await.unwrap();
    assert!(matches!(
        check_update(&store, &links, cluster.id).await.unwrap(),
        Some(ShareUpdate::Ended { .. })
    ));
}
