#![recursion_limit = "256"]
use oneclient_common::domain::GameLoader;
use oneclient_content::packages::PackageStore;
use oneclient_core::{
    ImportChoices, discard_instance_archive, import_external_instance, is_instance_archive,
    open_instance_archive,
};

fn jar(tag: &str) -> Vec<u8> {
    let mut bytes = b"PK\x05\x06".to_vec();
    bytes.extend([0; 16]);
    bytes.extend((tag.len() as u16).to_le_bytes());
    bytes.extend(tag.as_bytes());
    bytes
}

async fn write_archive(path: &std::path::Path, entries: Vec<(&str, Vec<u8>)>) {
    let file = tokio::fs::File::create(path).await.unwrap();
    let mut writer = async_zip::tokio::write::ZipFileWriter::with_tokio(file);
    for (name, data) in entries {
        let builder = async_zip::ZipEntryBuilder::new(
            name.to_string().into(),
            async_zip::Compression::Stored,
        );
        writer.write_entry_whole(builder, &data).await.unwrap();
    }
    writer.close().await.unwrap();
}

#[tokio::test]
async fn a_prism_export_zip_imports_as_an_instance() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let scratch = polyio::testing::ScratchDir::new("instance-archive");
    let archive = scratch.path().join("Skyblock.zip");
    let pack = serde_json::json!({
        "formatVersion": 1,
        "components": [
            {"uid": "net.minecraft", "version": "1.21.1"},
            {"uid": "net.fabricmc.fabric-loader", "version": "0.16.5"}
        ]
    });
    write_archive(
        &archive,
        vec![
            (
                "Skyblock/instance.cfg",
                b"[General]\nInstanceType=OneSix\nname=Skyblock Run\ntotalTimePlayed=3600\nMaxMemAlloc=6144\nOverrideMemory=true\n".to_vec(),
            ),
            ("Skyblock/mmc-pack.json", serde_json::to_vec(&pack).unwrap()),
            ("Skyblock/.minecraft/mods/skyblock-helper.jar", jar("helper")),
            ("Skyblock/.minecraft/config/helper.json", br#"{"on":true}"#.to_vec()),
            ("Skyblock/.minecraft/saves/Island/level.dat", b"level".to_vec()),
        ],
    )
    .await;

    assert!(is_instance_archive(&archive).await);
    let mrpack = scratch.path().join("not-an-instance.zip");
    write_archive(&mrpack, vec![("modrinth.index.json", b"{}".to_vec())]).await;
    assert!(!is_instance_archive(&mrpack).await);
    assert!(open_instance_archive(&mrpack).await.is_err());

    let opened = open_instance_archive(&archive).await.unwrap();
    assert!(opened.workdir.is_dir());
    assert!(
        opened
            .workdir
            .starts_with(oneclient_common::paths::import_staging_dir().unwrap()),
        "unpacked next to the launcher's data, not in the system temp folder"
    );
    let instance = &opened.instances[0];
    assert_eq!(instance.name, "Skyblock Run");
    assert_eq!(instance.mc_version, "1.21.1");
    assert_eq!(instance.loader, GameLoader::Fabric);
    assert_eq!(instance.loader_version.as_deref(), Some("0.16.5"));
    assert_eq!(instance.played_secs, 0);

    let report = import_external_instance(&state, instance, &ImportChoices::default(), None)
        .await
        .unwrap();
    discard_instance_archive(&opened.workdir).await;
    assert!(!opened.workdir.exists());

    let cluster = state.clusters.get(report.cluster_id).await.unwrap();
    assert_eq!(cluster.name, "Skyblock Run");
    assert_eq!(cluster.overall_played, std::time::Duration::ZERO);
    assert_eq!(cluster.mc_loader, GameLoader::Fabric);
    let game_dir = cluster.game_dir().unwrap();
    assert!(game_dir.join("config/helper.json").is_file());
    assert!(game_dir.join("saves/Island/level.dat").is_file());

    let mods: Vec<String> =
        PackageStore::list_linked_artifacts(cluster.id, &state.services.content())
            .await
            .unwrap()
            .into_iter()
            .map(|link| link.cluster_file_name)
            .collect();
    assert_eq!(mods, vec!["skyblock-helper.jar".to_string()]);
}

fn instance_pack(mc: &str, components: serde_json::Value) -> Vec<u8> {
    let mut list = vec![serde_json::json!({"uid": "net.minecraft", "version": mc})];
    list.extend(components.as_array().unwrap().iter().cloned());
    serde_json::to_vec(&serde_json::json!({"formatVersion": 1, "components": list})).unwrap()
}

#[tokio::test]
async fn commands_from_an_instance_file_are_not_imported() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let scratch = polyio::testing::ScratchDir::new("instance-archive-commands");
    let archive = scratch.path().join("Hooked.zip");
    let cfg = concat!(
        "[General]\n",
        "InstanceType=OneSix\n",
        "name=Hooked\n",
        "OverrideMemory=true\n",
        "MaxMemAlloc=5120\n",
        "OverrideCommands=true\n",
        "PreLaunchCommand=calc.exe\n",
        "WrapperCommand=cmd /c evil.bat\n",
        "PostExitCommand=shutdown /s\n",
        "OverrideEnv=true\n",
        "Env={\"JAVA_TOOL_OPTIONS\":\"-javaagent:evil.jar\"}\n",
        "OverrideJavaArgs=true\n",
        "JvmArgs=-XX:OnOutOfMemoryError=calc.exe\n",
    );
    write_archive(
        &archive,
        vec![
            ("instance.cfg", cfg.as_bytes().to_vec()),
            (
                "mmc-pack.json",
                instance_pack(
                    "1.21.1",
                    serde_json::json!([{"uid": "net.fabricmc.fabric-loader", "version": "0.16.5"}]),
                ),
            ),
            (".minecraft/options.txt", b"fov:1.0".to_vec()),
        ],
    )
    .await;

    let opened = open_instance_archive(&archive).await.unwrap();
    let settings = &opened.instances[0].settings;
    assert_eq!(settings.mem_max, Some(5120));
    assert_eq!(settings.hook_pre, None);
    assert_eq!(settings.hook_wrapper, None);
    assert_eq!(settings.hook_post, None);
    assert!(settings.env.is_empty());
    assert_eq!(settings.jvm_args, None);

    let report = import_external_instance(
        &state,
        &opened.instances[0],
        &ImportChoices::default(),
        None,
    )
    .await
    .unwrap();
    discard_instance_archive(&opened.workdir).await;

    let cluster = state.clusters.get(report.cluster_id).await.unwrap();
    let global = state.settings.read().global_game_settings.clone();
    let profile = state
        .clusters
        .resolve_settings(&global, &cluster)
        .await
        .unwrap();
    assert_eq!(profile.hook_pre, global.hook_pre);
    assert_eq!(profile.hook_wrapper, global.hook_wrapper);
    assert_eq!(profile.hook_post, global.hook_post);
    assert_eq!(profile.launch_env, global.launch_env);
    assert_eq!(profile.launch_args, global.launch_args);
    assert_eq!(profile.mem_max, Some(5120));
}

#[tokio::test]
async fn ornithe_instances_are_not_mistaken_for_fabric() {
    let scratch = polyio::testing::ScratchDir::new("instance-archive-ornithe");
    let cfg = b"[General]\nInstanceType=OneSix\nname=Legacy\n".to_vec();
    let fabric = serde_json::json!([
        {"uid": "net.fabricmc.intermediary", "version": "1.8.9"},
        {"uid": "net.fabricmc.fabric-loader", "version": "0.16.5"}
    ]);

    let patched = scratch.path().join("patched.zip");
    write_archive(
        &patched,
        vec![
            ("instance.cfg", cfg.clone()),
            ("mmc-pack.json", instance_pack("1.8.9", fabric.clone())),
            (
                "patches/net.fabricmc.intermediary.json",
                br#"{"libraries":[{"name":"net.ornithemc:calamus-intermediary:1.8.9"}]}"#.to_vec(),
            ),
        ],
    )
    .await;
    let opened = open_instance_archive(&patched).await.unwrap();
    assert_eq!(opened.instances[0].loader, GameLoader::Ornithe);
    assert_eq!(
        opened.instances[0].loader_version.as_deref(),
        Some("0.16.5")
    );
    discard_instance_archive(&opened.workdir).await;

    let component = scratch.path().join("component.zip");
    write_archive(
        &component,
        vec![
            ("instance.cfg", cfg.clone()),
            (
                "mmc-pack.json",
                instance_pack(
                    "1.8.9",
                    serde_json::json!([
                        {"uid": "net.ornithemc.calamus-intermediary", "version": "1.8.9"},
                        {"uid": "net.fabricmc.fabric-loader", "version": "0.16.5"}
                    ]),
                ),
            ),
        ],
    )
    .await;
    let opened = open_instance_archive(&component).await.unwrap();
    assert_eq!(opened.instances[0].loader, GameLoader::Ornithe);
    discard_instance_archive(&opened.workdir).await;

    let plain = scratch.path().join("plain.zip");
    write_archive(
        &plain,
        vec![
            ("instance.cfg", cfg),
            ("mmc-pack.json", instance_pack("1.21.1", fabric)),
        ],
    )
    .await;
    let opened = open_instance_archive(&plain).await.unwrap();
    assert_eq!(opened.instances[0].loader, GameLoader::Fabric);
    discard_instance_archive(&opened.workdir).await;
}

#[tokio::test]
async fn a_fresh_unpacked_instance_survives_the_startup_sweep() {
    let _state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let dir = oneclient_common::paths::import_staging_dir()
        .unwrap()
        .join(format!(
            "oneclient-instance-sweep-test-{}",
            std::process::id()
        ));
    std::fs::create_dir_all(&dir).unwrap();
    oneclient_core::sweep_instance_archives().await;
    assert!(dir.is_dir(), "an import may still be using it");
    std::fs::remove_dir_all(&dir).unwrap();
}

async fn import_pinned_ornithe(version: &str) -> Option<String> {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let scratch = polyio::testing::ScratchDir::new("instance-archive-pin");
    let archive = scratch.path().join("Pinned.zip");
    write_archive(
        &archive,
        vec![
            (
                "instance.cfg",
                b"[General]\nInstanceType=OneSix\nname=Pinned\n".to_vec(),
            ),
            (
                "mmc-pack.json",
                instance_pack(
                    "1.8.9",
                    serde_json::json!([
                        {"uid": "net.ornithemc.calamus-intermediary", "version": "1.8.9"},
                        {"uid": "net.fabricmc.fabric-loader", "version": version}
                    ]),
                ),
            ),
        ],
    )
    .await;
    let opened = open_instance_archive(&archive).await.unwrap();
    let report = import_external_instance(
        &state,
        &opened.instances[0],
        &ImportChoices::default(),
        None,
    )
    .await
    .unwrap();
    discard_instance_archive(&opened.workdir).await;
    let cluster = state.clusters.get(report.cluster_id).await.unwrap();
    assert_eq!(cluster.mc_loader, GameLoader::Ornithe);
    cluster.mc_loader_version
}

#[tokio::test]
#[ignore = "requires network"]
async fn a_loader_version_oneclient_does_not_offer_is_left_unpinned() {
    assert_eq!(import_pinned_ornithe("0.16.5").await, None);
    assert_eq!(
        import_pinned_ornithe("0.19.5").await.as_deref(),
        Some("0.19.5")
    );
}

#[tokio::test]
async fn jar_mods_are_reported_and_unsupported_loaders_refused() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let scratch = polyio::testing::ScratchDir::new("instance-archive-parts");
    let cfg = b"[General]\nInstanceType=OneSix\nname=Old Setup\n".to_vec();

    let jarmod = scratch.path().join("jarmod.zip");
    write_archive(
        &jarmod,
        vec![
            ("instance.cfg", cfg.clone()),
            (
                "mmc-pack.json",
                instance_pack(
                    "1.8.9",
                    serde_json::json!([
                        {"uid": "org.lwjgl", "version": "2.9.4"},
                        {"uid": "org.multimc.jarmod.1234", "cachedName": "OptiFine HD U M5"}
                    ]),
                ),
            ),
            ("jarmods/optifine.jar", jar("optifine")),
        ],
    )
    .await;
    let opened = open_instance_archive(&jarmod).await.unwrap();
    assert_eq!(
        opened.instances[0].skipped_parts,
        vec!["OptiFine HD U M5".to_string()]
    );
    let report = import_external_instance(
        &state,
        &opened.instances[0],
        &ImportChoices::default(),
        None,
    )
    .await
    .unwrap();
    discard_instance_archive(&opened.workdir).await;
    assert_eq!(report.parts_skipped, vec!["OptiFine HD U M5".to_string()]);

    let legacy = scratch.path().join("legacy.zip");
    write_archive(
        &legacy,
        vec![
            ("instance.cfg", cfg),
            (
                "mmc-pack.json",
                instance_pack(
                    "1.8.9",
                    serde_json::json!([
                        {"uid": "net.legacyfabric.intermediary", "version": "1.8.9"},
                        {"uid": "net.fabricmc.fabric-loader", "version": "0.16.5"}
                    ]),
                ),
            ),
        ],
    )
    .await;
    let refused = open_instance_archive(&legacy).await.unwrap_err();
    assert!(
        refused
            .to_string()
            .contains("only holds instances that use Legacy Fabric"),
        "{refused}"
    );

    let mixed = scratch.path().join("mixed.zip");
    write_archive(
        &mixed,
        vec![
            (
                "Legacy/instance.cfg",
                b"[General]\nInstanceType=OneSix\nname=Legacy\n".to_vec(),
            ),
            (
                "Legacy/mmc-pack.json",
                instance_pack(
                    "1.8.9",
                    serde_json::json!([
                        {"uid": "net.legacyfabric.intermediary", "version": "1.8.9"},
                        {"uid": "net.fabricmc.fabric-loader", "version": "0.16.5"}
                    ]),
                ),
            ),
            (
                "Modern/instance.cfg",
                b"[General]\nInstanceType=OneSix\nname=Modern\n".to_vec(),
            ),
            (
                "Modern/mmc-pack.json",
                instance_pack(
                    "1.21.1",
                    serde_json::json!([{"uid": "net.fabricmc.fabric-loader", "version": "0.16.5"}]),
                ),
            ),
            ("Extras/huge-video.mp4", vec![0u8; 4096]),
        ],
    )
    .await;
    let opened = open_instance_archive(&mixed).await.unwrap();
    assert!(
        !opened.workdir.join("Extras").exists(),
        "only the instance folders are unpacked"
    );
    let legacy = opened
        .instances
        .iter()
        .find(|instance| instance.name == "Legacy")
        .unwrap();
    assert_eq!(legacy.unsupported_loader.as_deref(), Some("Legacy Fabric"));
    let before = state.clusters.list().await.unwrap().len();
    let refused = import_external_instance(&state, legacy, &ImportChoices::default(), None)
        .await
        .unwrap_err();
    assert!(refused.to_string().contains("Legacy Fabric"), "{refused}");
    assert_eq!(state.clusters.list().await.unwrap().len(), before);
    discard_instance_archive(&opened.workdir).await;
}

#[tokio::test]
async fn a_zip_with_several_instances_yields_all_of_them_and_moves_their_files() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let scratch = polyio::testing::ScratchDir::new("instance-archive-several");
    let archive = scratch.path().join("instances.zip");
    let pack = instance_pack(
        "1.21.1",
        serde_json::json!([{"uid": "net.fabricmc.fabric-loader", "version": "0.16.5"}]),
    );
    write_archive(
        &archive,
        vec![
            (
                "Alpha/instance.cfg",
                b"[General]\nInstanceType=OneSix\nname=Alpha\n".to_vec(),
            ),
            ("Alpha/mmc-pack.json", pack.clone()),
            ("Alpha/.minecraft/config/alpha.json", b"{}".to_vec()),
            (
                "Beta/instance.cfg",
                b"[General]\nInstanceType=OneSix\nname=Beta\n".to_vec(),
            ),
            ("Beta/mmc-pack.json", pack),
        ],
    )
    .await;

    let opened = open_instance_archive(&archive).await.unwrap();
    let names: Vec<&str> = opened.instances.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, vec!["Alpha", "Beta"]);

    let alpha = &opened.instances[0];
    let moved_from = alpha.game_dir.join("config/alpha.json");
    assert!(moved_from.is_file());
    let choices = ImportChoices {
        move_files: true,
        ..ImportChoices::default()
    };
    let report = import_external_instance(&state, alpha, &choices, None)
        .await
        .unwrap();
    assert!(
        !moved_from.exists(),
        "the unpacked file is moved, not copied"
    );
    let cluster = state.clusters.get(report.cluster_id).await.unwrap();
    assert!(
        cluster
            .game_dir()
            .unwrap()
            .join("config/alpha.json")
            .is_file()
    );

    discard_instance_archive(&opened.workdir).await;
}
