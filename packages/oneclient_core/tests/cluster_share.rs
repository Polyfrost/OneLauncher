#![recursion_limit = "256"]
use oneclient_common::domain::{ContentType, GameLoader};
use oneclient_content::packages::PackageStore;
use oneclient_core::clusters::{
    ClusterKind, CreateClusterOptions, ExportMode, ModpackSource, create_modpack_instance,
    export_cluster, install_modpack_instance, prepare_modpack,
};

fn jar(tag: &str) -> Vec<u8> {
    let mut bytes = b"PK\x05\x06".to_vec();
    bytes.extend([0; 16]);
    bytes.extend((tag.len() as u16).to_le_bytes());
    bytes.extend(tag.as_bytes());
    bytes
}

#[tokio::test]
async fn an_exported_instance_imports_with_its_content_and_details() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let global = state.settings.read().global_game_settings.clone();
    let ctx = state.services.content();

    let mut options = CreateClusterOptions::new("Shared Pack", "1.21.1", GameLoader::Fabric)
        .kind(ClusterKind::Modded)
        .user_created(true)
        .tags(vec!["pvp".into()]);
    options.mc_loader_version = Some("0.16.5".into());
    options.description = Some("Made for the weekend".into());
    let source = state.clusters.create(&global, options).await.unwrap();

    let scratch = polyio::testing::ScratchDir::new("cluster-share");
    let on = scratch.path().join("kept-on.jar");
    let off = scratch.path().join("switched-off.jar");
    std::fs::write(&on, jar("on")).unwrap();
    std::fs::write(&off, jar("off")).unwrap();
    let imported = PackageStore::import_local_files(
        &[(on, ContentType::Mod), (off, ContentType::Mod)],
        source.id,
        &ctx,
    )
    .await
    .unwrap();
    assert_eq!(imported.imported.len(), 2);
    let off_hash = imported
        .imported
        .iter()
        .find(|row| row.file_name == "switched-off.jar")
        .unwrap()
        .hash
        .clone();
    PackageStore::set_artifact_enabled_to(source.id, &off_hash, false, &ctx)
        .await
        .unwrap();

    let cover = scratch.path().join("cover.png");
    std::fs::write(&cover, b"not really a png").unwrap();
    state
        .clusters
        .set_cover_from_file(source.id, &cover)
        .await
        .unwrap();

    let dest = scratch.path().join("Shared Pack.mrpack");
    let report = export_cluster(&state, source.id, &dest, ExportMode::OneClient, None)
        .await
        .unwrap();
    assert_eq!(
        report.embedded.len(),
        2,
        "local jars travel inside the file"
    );
    assert!(report.missing.is_empty());
    assert!(dest.is_file());

    let prepared = prepare_modpack(&state, &ModpackSource::File(dest.clone()), None)
        .await
        .unwrap();
    let shared = prepared
        .share
        .as_ref()
        .expect("the export is read as a share");
    assert_eq!(shared.share.name, "Shared Pack");
    assert_eq!(prepared.instance_name, "Shared Pack (2)");

    let cluster = create_modpack_instance(&state, &prepared).await.unwrap();
    let installed = install_modpack_instance(&state, cluster.id, &prepared, None)
        .await
        .unwrap();
    assert!(
        installed.report.failed.is_empty(),
        "{:?}",
        installed.report.failed
    );

    let imported = installed.cluster;
    assert_eq!(imported.kind, ClusterKind::Modded);
    assert_eq!(imported.mc_version, "1.21.1");
    assert_eq!(imported.mc_loader, GameLoader::Fabric);
    assert_eq!(imported.mc_loader_version.as_deref(), Some("0.16.5"));
    assert_eq!(
        imported.description.as_deref(),
        Some("Made for the weekend")
    );
    assert_eq!(imported.tags, vec!["pvp".to_string()]);
    assert_eq!(imported.linked_modpack_hash, None);
    assert!(imported.cover_file().is_some_and(|path| path.is_file()));

    let mut links: Vec<(String, bool)> = PackageStore::list_linked_artifacts(imported.id, &ctx)
        .await
        .unwrap()
        .into_iter()
        .map(|link| (link.cluster_file_name, link.enabled))
        .collect();
    links.sort();
    assert_eq!(
        links,
        vec![
            ("kept-on.jar".to_string(), true),
            ("switched-off.jar".to_string(), false),
        ]
    );
}

#[tokio::test]
#[ignore = "requires network"]
async fn a_cached_modrinth_mod_is_exported_as_a_link() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let global = state.settings.read().global_game_settings.clone();
    let ctx = state.services.content();

    let source = state
        .clusters
        .create(
            &global,
            CreateClusterOptions::new("Linked", "26.1", GameLoader::Fabric)
                .kind(ClusterKind::Modded),
        )
        .await
        .unwrap();

    let artifact = PackageStore::resolve_or_download(
        oneclient_common::domain::ProviderId::Modrinth,
        "5ZwdcRci",
        "xOA9hbpf",
        None,
        &ctx,
    )
    .await
    .unwrap();
    PackageStore::link_by_hash(&artifact.hash, source.id, None, &ctx)
        .await
        .unwrap();

    let scratch = polyio::testing::ScratchDir::new("cluster-share-linked");
    let dest = scratch.path().join("Linked.mrpack");
    let report = export_cluster(&state, source.id, &dest, ExportMode::OneClient, None)
        .await
        .unwrap();

    assert_eq!(report.linked, 1);
    assert!(report.embedded.is_empty(), "{:?}", report.embedded);
}

async fn link_modrinth_cached_jar(
    state: &std::sync::Arc<oneclient_core::LauncherState>,
    cluster_id: i64,
    version_id: &str,
    on_disk: &[u8],
    hash: &str,
) {
    use oneclient_common::domain::ProviderId;
    use oneclient_content::packages::store::{cache_file_path, relative_cache_path};
    use oneclient_db::dao::artifact as artifact_dao;

    let db = &state.services.db;
    let file_name = "not-a-real-mod+1.0.jar";
    let cached = cache_file_path(
        ContentType::Mod,
        ProviderId::Modrinth,
        "NoSuchProj",
        version_id,
        file_name,
    )
    .unwrap();
    std::fs::create_dir_all(cached.parent().unwrap()).unwrap();
    std::fs::write(&cached, on_disk).unwrap();
    artifact_dao::insert_artifact(
        db,
        hash,
        ContentType::Mod as i64,
        &relative_cache_path(&cached).unwrap(),
        file_name,
        Some(on_disk.len() as i64),
    )
    .await
    .unwrap();
    artifact_dao::upsert_provider_release(
        db,
        ProviderId::Modrinth as i64,
        "NoSuchProj",
        version_id,
        hash,
        "Not A Real Mod",
        "1.0",
        None,
        "[]",
        "[]",
    )
    .await
    .unwrap();
    artifact_dao::link_cluster_artifact(db, cluster_id, hash, file_name)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_jar_from_the_modrinth_cache_is_linked_without_asking_modrinth() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let global = state.settings.read().global_game_settings.clone();

    let mut options =
        CreateClusterOptions::new("Offline", "26.1", GameLoader::Fabric).kind(ClusterKind::Modded);
    options.mc_loader_version = Some("0.16.5".into());
    let source = state.clusters.create(&global, options).await.unwrap();

    let bytes = jar("cached");
    link_modrinth_cached_jar(
        &state,
        source.id,
        "NoSuchVer",
        &bytes,
        &polyio::sha1_bytes(&bytes),
    )
    .await;

    let scratch = polyio::testing::ScratchDir::new("cluster-share-offline");
    let dest = scratch.path().join("Offline.mrpack");
    let report = export_cluster(&state, source.id, &dest, ExportMode::OneClient, None)
        .await
        .unwrap();

    assert_eq!(report.linked, 1, "the link is built from the cache path");
    assert!(report.embedded.is_empty());
}

#[tokio::test]
async fn a_cached_jar_that_changed_on_disk_stays_out_of_the_modrinth_index() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let global = state.settings.read().global_game_settings.clone();

    let mut options = CreateClusterOptions::new("Changed", "1.21.1", GameLoader::Fabric)
        .kind(ClusterKind::Modded);
    options.mc_loader_version = Some("0.16.5".into());
    let source = state.clusters.create(&global, options).await.unwrap();

    let claimed = polyio::sha1_bytes(&jar("original"));
    link_modrinth_cached_jar(&state, source.id, "ChangedVer", &jar("changed"), &claimed).await;

    let scratch = polyio::testing::ScratchDir::new("cluster-share-changed");
    let dest = scratch.path().join("Changed.mrpack");
    export_cluster(&state, source.id, &dest, ExportMode::OneClient, None)
        .await
        .unwrap();

    let entries = polyio::read_zip_file_entries(&dest, |name| {
        name == "modrinth.index.json" || name == "oneclient.cluster.json"
    })
    .await
    .unwrap();
    let json = |name: &str| -> serde_json::Value {
        let (_, bytes) = entries.iter().find(|(entry, _)| entry == name).unwrap();
        serde_json::from_slice(bytes).unwrap()
    };
    let path = "mods/not-a-real-mod+1.0.jar";
    let indexed = json("modrinth.index.json")["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|file| file["path"] == path);
    assert!(
        !indexed,
        "other launchers would get a hash that doesn't match"
    );
    let downloads = json("oneclient.cluster.json")["downloads"].clone();
    let download = downloads
        .as_array()
        .unwrap()
        .iter()
        .find(|download| download["path"] == path)
        .expect("OneClient still gets the link");
    assert_eq!(download["sha1"], serde_json::Value::String(claimed));
}

async fn write_archive(path: &std::path::Path, entries: Vec<(String, Vec<u8>)>) {
    let file = tokio::fs::File::create(path).await.unwrap();
    let mut writer = async_zip::tokio::write::ZipFileWriter::with_tokio(file);
    for (name, data) in entries {
        let builder = async_zip::ZipEntryBuilder::new(name.into(), async_zip::Compression::Stored);
        writer.write_entry_whole(builder, &data).await.unwrap();
    }
    writer.close().await.unwrap();
}

#[tokio::test]
async fn a_crafted_share_cannot_write_outside_its_folder_or_reach_unknown_hosts() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let scratch = polyio::testing::ScratchDir::new("cluster-share-crafted");
    let archive = scratch.path().join("crafted.mrpack");
    let marker = format!("oneclient-share-escape-{}.jar", std::process::id());
    let index = serde_json::json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "1",
        "name": "Crafted",
        "files": [{
            "path": "mods/elsewhere.jar",
            "hashes": {"sha1": "0000000000000000000000000000000000000000"},
            "downloads": ["http://example.com/elsewhere.jar"],
            "fileSize": 10
        }],
        "dependencies": {"minecraft": "1.21.1", "fabric-loader": "0.16.5"}
    });
    write_archive(
        &archive,
        vec![
            ("modrinth.index.json".into(), serde_json::to_vec(&index).unwrap()),
            (
                "oneclient.cluster.json".into(),
                br#"{"format":1,"name":"Crafted","kind":"modded","mc_version":"1.21.1","mc_loader":"Fabric"}"#.to_vec(),
            ),
            (
                format!("overrides/mods/..{0}..{0}{marker}", '\\'),
                jar("escape"),
            ),
            ("overrides/mods/fine.jar".into(), jar("fine")),
        ],
    )
    .await;

    let prepared = prepare_modpack(&state, &ModpackSource::File(archive), None)
        .await
        .unwrap();
    assert_eq!(prepared.summary().unverified, 1);
    let cluster = create_modpack_instance(&state, &prepared).await.unwrap();
    let installed = install_modpack_instance(&state, cluster.id, &prepared, None)
        .await
        .unwrap();

    assert!(!std::env::temp_dir().join(&marker).exists());
    assert_eq!(installed.report.failed, vec!["elsewhere.jar".to_string()]);

    let links: Vec<String> =
        PackageStore::list_linked_artifacts(installed.cluster.id, &state.services.content())
            .await
            .unwrap()
            .into_iter()
            .map(|link| link.cluster_file_name)
            .collect();
    assert_eq!(links, vec!["fine.jar".to_string()]);
}

#[tokio::test]
async fn the_enabled_build_wins_when_two_share_a_file_name() {
    use oneclient_db::dao::artifact as artifact_dao;

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let global = state.settings.read().global_game_settings.clone();
    let ctx = state.services.content();
    let mut options =
        CreateClusterOptions::new("Twins", "1.21.1", GameLoader::Fabric).kind(ClusterKind::Modded);
    options.mc_loader_version = Some("0.16.5".into());
    let source = state.clusters.create(&global, options).await.unwrap();

    let scratch = polyio::testing::ScratchDir::new("cluster-share-twins");
    let mut hashes = Vec::new();
    for tag in ["old", "new"] {
        let dir = scratch.path().join(tag);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("same.jar");
        std::fs::write(&path, jar(tag)).unwrap();
        let row = PackageStore::import_local_file(&path, ContentType::Mod, source.id, &ctx)
            .await
            .unwrap();
        hashes.push(row.hash);
    }
    artifact_dao::update_cluster_artifact(&state.services.db, source.id, &hashes[0], "same.jar", 0)
        .await
        .unwrap();

    let dest = scratch.path().join("Twins.mrpack");
    let report = export_cluster(&state, source.id, &dest, ExportMode::OneClient, None)
        .await
        .unwrap();
    assert_eq!(report.embedded, vec!["mods/same.jar".to_string()]);

    let packed = polyio::read_zip_file_entries(&dest, |name| name == "overrides/mods/same.jar")
        .await
        .unwrap();
    assert_eq!(packed.len(), 1);
    assert_eq!(polyio::sha1_bytes(&packed[0].1), hashes[1]);
}

#[test]
fn bundle_content_is_counted_with_the_instances_own_mods() {
    use oneclient_content::modpacks::{
        ClusterShare, ModpackContents, ModpackManifest, ShareExtras,
    };
    use oneclient_core::clusters::SharedCluster;

    let manifest = ModpackManifest {
        name: "Mine".into(),
        version: "1.0.0".into(),
        summary: None,
        mc_version: "1.21.1".into(),
        loader: GameLoader::Fabric,
        loader_version: Some("0.16.5".into()),
        contents: ModpackContents::default(),
        override_prefixes: Vec::new(),
    };
    let share: ClusterShare = serde_json::from_value(serde_json::json!({
        "format": 1,
        "name": "Mine",
        "kind": "one_client",
        "mc_version": "1.21.1",
        "mc_loader": "Fabric"
    }))
    .unwrap();
    let shared = SharedCluster {
        archive_path: "unused.mrpack".into(),
        manifest: manifest.clone(),
        share,
        extras: ShareExtras {
            embedded: vec![ContentType::Mod, ContentType::Mod, ContentType::Mod],
            downloads: Vec::new(),
            other_files: 0,
        },
        bundled: vec![
            (ContentType::Mod, 100),
            (ContentType::Mod, 200),
            (ContentType::Mod, 300),
            (ContentType::Mod, 400),
            (ContentType::ResourcePack, 50),
        ],
        notices: Vec::new(),
        substituted: false,
    };

    let summary = shared.summary(&manifest);
    assert_eq!(summary.mods, 7);
    assert_eq!(summary.resource_packs, 1);
    assert_eq!(summary.unverified, 3);
    assert_eq!(summary.download_bytes, 1050);
}

#[tokio::test]
#[ignore = "requires network"]
async fn a_shared_oneclient_instance_counts_its_bundle_mods() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let ctx = state.services.content();
    assert!(state.bundles.sync(&ctx).await.unwrap());
    let group = oneclient_db::dao::bundle::list_distinct_version_loaders(&state.services.db)
        .await
        .unwrap()
        .into_iter()
        .last()
        .expect("at least one bundle version");
    let loader = GameLoader::from_repr(group.mc_loader as u8).unwrap();

    let scratch = polyio::testing::ScratchDir::new("cluster-share-bundles");
    let archive = scratch.path().join("bundled.mrpack");
    let index = serde_json::json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "1",
        "name": "Bundled",
        "files": [],
        "dependencies": {"minecraft": group.mc_version}
    });
    let share = serde_json::json!({
        "format": 1,
        "name": "Bundled",
        "kind": "one_client",
        "mc_version": group.mc_version,
        "mc_loader": loader
    });
    write_archive(
        &archive,
        vec![
            (
                "modrinth.index.json".into(),
                serde_json::to_vec(&index).unwrap(),
            ),
            (
                "oneclient.cluster.json".into(),
                serde_json::to_vec(&share).unwrap(),
            ),
            ("overrides/mods/own.jar".into(), jar("own")),
        ],
    )
    .await;

    let prepared = prepare_modpack(&state, &ModpackSource::File(archive), None)
        .await
        .unwrap();
    let summary = prepared.summary();
    println!(
        "{} {}: {} mods, {} resource packs, {} shaders",
        group.mc_version, loader, summary.mods, summary.resource_packs, summary.shaders
    );
    assert!(
        summary.mods > 1,
        "bundle mods are counted next to the packed-in one"
    );
}

#[tokio::test]
async fn a_cover_that_is_not_a_png_is_exported_as_one() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let global = state.settings.read().global_game_settings.clone();
    let mut options = CreateClusterOptions::new("Covered", "1.21.1", GameLoader::Fabric)
        .kind(ClusterKind::Modded);
    options.mc_loader_version = Some("0.16.5".into());
    let source = state.clusters.create(&global, options).await.unwrap();

    let scratch = polyio::testing::ScratchDir::new("cluster-share-cover");
    let jpeg = scratch.path().join("cover.jpg");
    image::RgbImage::from_pixel(8, 8, image::Rgb([200, 40, 40]))
        .save_with_format(&jpeg, image::ImageFormat::Jpeg)
        .unwrap();
    state
        .clusters
        .set_cover_from_file(source.id, &jpeg)
        .await
        .unwrap();

    let dest = scratch.path().join("Covered.mrpack");
    export_cluster(&state, source.id, &dest, ExportMode::OneClient, None)
        .await
        .unwrap();

    let entries = polyio::read_zip_file_entries(&dest, |name| name.starts_with("icon."))
        .await
        .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].0, "icon.png");
    assert_eq!(
        image::guess_format(&entries[0].1).unwrap(),
        image::ImageFormat::Png
    );
}

#[tokio::test]
#[ignore = "requires network"]
async fn an_instance_without_a_pinned_loader_exports_a_concrete_one() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let global = state.settings.read().global_game_settings.clone();
    let source = state
        .clusters
        .create(
            &global,
            CreateClusterOptions::new("Unpinned", "1.21.1", GameLoader::Fabric)
                .kind(ClusterKind::Modded),
        )
        .await
        .unwrap();
    assert_eq!(source.mc_loader_version, None);

    let scratch = polyio::testing::ScratchDir::new("cluster-share-loader");
    let dest = scratch.path().join("Unpinned.mrpack");
    export_cluster(&state, source.id, &dest, ExportMode::OneClient, None)
        .await
        .unwrap();

    let prepared = prepare_modpack(&state, &ModpackSource::File(dest), None)
        .await
        .unwrap();
    assert_eq!(prepared.manifest.loader, GameLoader::Fabric);
    let version = prepared.manifest.loader_version.clone().unwrap();
    println!("exported fabric-loader {version}");
    assert_eq!(
        prepared.share.as_ref().unwrap().share.mc_loader_version,
        Some(version)
    );
}

#[tokio::test]
async fn compatibility_exports_write_bundle_mods_out_as_plain_mods() {
    use oneclient_content::modpacks::ClusterShare;
    use oneclient_db::dao::cluster_bundle as bundle_dao;

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let global = state.settings.read().global_game_settings.clone();
    let ctx = state.services.content();
    let mut options = CreateClusterOptions::new("Bundled", "1.21.1", GameLoader::Fabric)
        .kind(ClusterKind::OneClient)
        .user_created(true);
    options.mc_loader_version = Some("0.16.5".into());
    let source = state.clusters.create(&global, options).await.unwrap();

    let scratch = polyio::testing::ScratchDir::new("cluster-share-compat");
    let from_bundle = scratch.path().join("from-bundle.jar");
    let own = scratch.path().join("own.jar");
    std::fs::write(&from_bundle, jar("bundle")).unwrap();
    std::fs::write(&own, jar("own")).unwrap();
    let imported = PackageStore::import_local_files(
        &[(from_bundle, ContentType::Mod), (own, ContentType::Mod)],
        source.id,
        &ctx,
    )
    .await
    .unwrap();
    let bundle_hash = imported
        .imported
        .iter()
        .find(|row| row.file_name == "from-bundle.jar")
        .unwrap()
        .hash
        .clone();
    bundle_dao::track_bundle_artifact(
        &state.services.db,
        source.id,
        &bundle_hash,
        "Performance",
        "v1",
        "pkg",
    )
    .await
    .unwrap();
    bundle_dao::save_bundle_choices(
        &state.services.db,
        source.id,
        &[("Performance".into(), true)],
    )
    .await
    .unwrap();

    let read_share = |path: std::path::PathBuf| async move {
        let entries = polyio::read_zip_file_entries(&path, |name| name == "oneclient.cluster.json")
            .await
            .unwrap();
        serde_json::from_slice::<ClusterShare>(&entries[0].1).unwrap()
    };

    let linked = scratch.path().join("linked.mrpack");
    let report = export_cluster(&state, source.id, &linked, ExportMode::OneClient, None)
        .await
        .unwrap();
    assert_eq!(report.embedded, vec!["mods/own.jar".to_string()]);
    let share = read_share(linked).await;
    assert_eq!(share.kind, ClusterKind::OneClient);
    assert_eq!(share.bundles.len(), 1);

    let flat = scratch.path().join("flat.mrpack");
    let report = export_cluster(&state, source.id, &flat, ExportMode::Compatibility, None)
        .await
        .unwrap();
    assert_eq!(
        report.embedded,
        vec![
            "mods/from-bundle.jar".to_string(),
            "mods/own.jar".to_string()
        ]
    );
    let share = read_share(flat).await;
    assert_eq!(share.kind, ClusterKind::Modded);
    assert!(share.dedicated_dir);
    assert!(share.bundles.is_empty());
    assert!(share.overrides.is_empty());
    assert!(share.modpack.is_none());
}

#[tokio::test]
async fn compatibility_exports_carry_the_modpacks_own_files_over() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let scratch = polyio::testing::ScratchDir::new("cluster-share-carry");
    let pack = scratch.path().join("pack.mrpack");
    let index = serde_json::json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "1.0",
        "name": "Carry Pack",
        "files": [],
        "dependencies": {"minecraft": "1.21.1", "fabric-loader": "0.16.5"}
    });
    write_archive(
        &pack,
        vec![
            (
                "modrinth.index.json".into(),
                serde_json::to_vec(&index).unwrap(),
            ),
            (
                "overrides/config/pack.json".into(),
                br#"{"tuned":true}"#.to_vec(),
            ),
            (
                "overrides/kubejs/startup_scripts/main.js".into(),
                b"console.log(1)".to_vec(),
            ),
            ("overrides/mods/inside.jar".into(), jar("inside")),
        ],
    )
    .await;

    let prepared = prepare_modpack(&state, &ModpackSource::File(pack), None)
        .await
        .unwrap();
    assert!(prepared.share.is_none());
    let source = create_modpack_instance(&state, &prepared).await.unwrap();
    install_modpack_instance(&state, source.id, &prepared, None)
        .await
        .unwrap();
    let source = state.clusters.get(source.id).await.unwrap();
    assert!(source.linked_modpack_hash.is_some());

    let dest = scratch.path().join("Carry.mrpack");
    let report = export_cluster(&state, source.id, &dest, ExportMode::Compatibility, None)
        .await
        .unwrap();
    assert_eq!(report.carried, 2);
    assert_eq!(report.embedded, vec!["mods/inside.jar".to_string()]);

    let entries = polyio::zip_entry_names(&dest).await.unwrap();
    assert!(entries.contains(&"overrides/config/pack.json".to_string()));
    assert!(entries.contains(&"overrides/kubejs/startup_scripts/main.js".to_string()));

    let reimport = prepare_modpack(&state, &ModpackSource::File(dest), None)
        .await
        .unwrap();
    assert_eq!(reimport.summary().other_files, 2);
    let cluster = create_modpack_instance(&state, &reimport).await.unwrap();
    let installed = install_modpack_instance(&state, cluster.id, &reimport, None)
        .await
        .unwrap();
    assert!(
        installed.report.failed.is_empty(),
        "{:?}",
        installed.report.failed
    );

    let game_dir = installed.cluster.game_dir().unwrap();
    assert_eq!(
        std::fs::read(game_dir.join("config/pack.json")).unwrap(),
        br#"{"tuned":true}"#
    );
    assert!(game_dir.join("kubejs/startup_scripts/main.js").is_file());
}

#[tokio::test]
async fn cache_paths_never_leave_their_folder() {
    use oneclient_common::domain::ProviderId;
    use oneclient_content::packages::store::cache_file_path;

    let _state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let folder = cache_file_path(
        ContentType::Mod,
        ProviderId::Local,
        "external",
        "abc",
        "ok.jar",
    )
    .unwrap()
    .parent()
    .unwrap()
    .to_path_buf();

    for crafted in [
        r"..\..\escape.jar",
        "../../escape.jar",
        r"C:\Windows\escape.jar",
        "/etc/escape.jar",
        r"mods\..\escape.jar",
    ] {
        let path = cache_file_path(
            ContentType::Mod,
            ProviderId::Local,
            "external",
            "abc",
            crafted,
        )
        .unwrap();
        assert_eq!(path.parent(), Some(folder.as_path()), "{crafted}");
        assert_eq!(path.file_name().unwrap(), "escape.jar", "{crafted}");
    }
    let reserved = cache_file_path(
        ContentType::Mod,
        ProviderId::Local,
        "external",
        "abc",
        "CON.jar",
    )
    .unwrap();
    assert_eq!(reserved.parent(), Some(folder.as_path()));
    let expected = if cfg!(windows) { "_CON.jar" } else { "CON.jar" };
    assert_eq!(reserved.file_name().unwrap(), expected);

    for unusable in ["..", ".", "", r"mods\..", "mods/"] {
        assert!(
            cache_file_path(
                ContentType::Mod,
                ProviderId::Local,
                "external",
                "abc",
                unusable
            )
            .is_err(),
            "{unusable}"
        );
    }
}

#[tokio::test]
async fn crafted_download_names_in_a_share_are_refused() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let scratch = polyio::testing::ScratchDir::new("cluster-share-download-names");
    let archive = scratch.path().join("names.mrpack");
    let marker = format!("oneclient-download-escape-{}.jar", std::process::id());
    let index = serde_json::json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "1",
        "name": "Names",
        "files": [],
        "dependencies": {"minecraft": "1.21.1", "fabric-loader": "0.16.5"}
    });
    let backslash = '\\';
    let downloads: Vec<serde_json::Value> = [
        format!("mods/..{backslash}..{backslash}{marker}"),
        format!("mods/C:{backslash}{marker}"),
        format!("mods/../../{marker}"),
    ]
    .into_iter()
    .map(|path| {
        serde_json::json!({
            "path": path,
            "sha1": "0000000000000000000000000000000000000000",
            "size": 1,
            "url": "https://edge.forgecdn.net/files/1/2/x.jar"
        })
    })
    .chain([serde_json::json!({
        "path": format!("mods/{marker}"),
        "sha1": "../../../../../.",
        "size": 1,
        "url": "https://edge.forgecdn.net/files/1/2/x.jar"
    })])
    .collect();
    let share = serde_json::json!({
        "format": 1,
        "name": "Names",
        "kind": "modded",
        "mc_version": "1.21.1",
        "mc_loader": "Fabric",
        "downloads": downloads
    });
    write_archive(
        &archive,
        vec![
            (
                "modrinth.index.json".into(),
                serde_json::to_vec(&index).unwrap(),
            ),
            (
                "oneclient.cluster.json".into(),
                serde_json::to_vec(&share).unwrap(),
            ),
        ],
    )
    .await;

    let prepared = prepare_modpack(&state, &ModpackSource::File(archive), None)
        .await
        .unwrap();
    let cluster = create_modpack_instance(&state, &prepared).await.unwrap();
    let installed = install_modpack_instance(&state, cluster.id, &prepared, None)
        .await
        .unwrap();

    assert_eq!(
        installed.report.failed.len(),
        4,
        "{:?}",
        installed.report.failed
    );
    assert_eq!(installed.report.installed, 0);
    let data = oneclient_common::paths::data_dir().unwrap().to_path_buf();
    for dir in [
        data.clone(),
        data.parent().unwrap().to_path_buf(),
        std::env::temp_dir(),
    ] {
        assert!(!dir.join(&marker).exists(), "{}", dir.display());
    }
}

#[tokio::test]
async fn ornithe_exports_name_fabric_for_other_launchers() {
    use oneclient_content::modpacks::ClusterShare;

    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let global = state.settings.read().global_game_settings.clone();
    let mut options =
        CreateClusterOptions::new("Legacy", "1.8.9", GameLoader::Ornithe).kind(ClusterKind::Modded);
    options.mc_loader_version = Some("0.16.5".into());
    let source = state.clusters.create(&global, options).await.unwrap();

    let scratch = polyio::testing::ScratchDir::new("cluster-share-ornithe");
    let dest = scratch.path().join("Legacy.mrpack");
    export_cluster(&state, source.id, &dest, ExportMode::OneClient, None)
        .await
        .unwrap();

    let entries = polyio::read_zip_file_entries(&dest, |name| {
        name == "modrinth.index.json" || name == "oneclient.cluster.json"
    })
    .await
    .unwrap();
    let read = |name: &str| {
        entries
            .iter()
            .find(|(entry, _)| entry == name)
            .unwrap()
            .1
            .clone()
    };
    let index: serde_json::Value = serde_json::from_slice(&read("modrinth.index.json")).unwrap();
    assert_eq!(index["dependencies"]["fabric-loader"], "0.16.5");
    assert!(index["dependencies"].get("ornithe").is_none());
    let share: ClusterShare = serde_json::from_slice(&read("oneclient.cluster.json")).unwrap();
    assert_eq!(share.mc_loader, GameLoader::Ornithe);

    let prepared = prepare_modpack(&state, &ModpackSource::File(dest), None)
        .await
        .unwrap();
    let cluster = create_modpack_instance(&state, &prepared).await.unwrap();
    assert_eq!(cluster.mc_loader, GameLoader::Ornithe);
}
