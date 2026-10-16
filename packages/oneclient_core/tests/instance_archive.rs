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
    let instance = &opened.instance;
    assert_eq!(instance.name, "Skyblock Run");
    assert_eq!(instance.mc_version, "1.21.1");
    assert_eq!(instance.loader, GameLoader::Fabric);
    assert_eq!(instance.loader_version.as_deref(), Some("0.16.5"));
    assert_eq!(instance.played_secs, 3600);

    let report = import_external_instance(&state, instance, &ImportChoices::default(), None)
        .await
        .unwrap();
    discard_instance_archive(&opened.workdir).await;
    assert!(!opened.workdir.exists());

    let cluster = state.clusters.get(report.cluster_id).await.unwrap();
    assert_eq!(cluster.name, "Skyblock Run");
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
