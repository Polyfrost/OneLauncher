pub async fn link_modrinth_cached_jar(
    state: &std::sync::Arc<oneclient_core::LauncherState>,
    cluster_id: i64,
    version_id: &str,
    on_disk: &[u8],
    hash: &str,
) {
    use oneclient_common::domain::{ContentType, ProviderId};
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
