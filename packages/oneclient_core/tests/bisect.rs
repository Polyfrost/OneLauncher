use oneclient_common::domain::{ContentType, GameLoader};
use oneclient_core::clusters::CreateClusterOptions;
use oneclient_core::{BisectAnswer, BisectChoice, LauncherState};
use oneclient_db::dao::artifact as artifact_dao;

const FOUND: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const KEPT: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const WAS_OFF: &str = "cccccccccccccccccccccccccccccccccccccccc";

async fn cluster_with_mods(state: &LauncherState, name: &str) -> i64 {
    let global = state.settings.read().global_game_settings.clone();
    let cluster = state
        .clusters
        .create(
            &global,
            CreateClusterOptions::new(name, "1.21.1", GameLoader::Fabric),
        )
        .await
        .unwrap();
    let db = &state.services.db;
    for hash in [FOUND, KEPT, WAS_OFF] {
        let file = format!("{hash}.jar");
        artifact_dao::insert_artifact(db, hash, ContentType::Mod as i64, &file, &file, Some(1))
            .await
            .unwrap();
        artifact_dao::link_cluster_artifact(db, cluster.id, hash, &file)
            .await
            .unwrap();
    }
    artifact_dao::update_cluster_artifact(db, cluster.id, WAS_OFF, &format!("{WAS_OFF}.jar"), 0)
        .await
        .unwrap();
    cluster.id
}

async fn enabled(state: &LauncherState, cluster_id: i64, hash: &str) -> bool {
    artifact_dao::get_cluster_artifact(&state.services.db, cluster_id, hash)
        .await
        .unwrap()
        .unwrap()
        .enabled
        != 0
}

#[tokio::test]
async fn finishing_puts_every_mod_back_the_way_it_was() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_mods(&state, "Bisect Cluster").await;

    oneclient_core::start_bisect(&state, cluster_id, |_, _, _| {})
        .await
        .unwrap();
    assert!(
        enabled(&state, cluster_id, FOUND).await,
        "a mod whose file can't be read is still tested"
    );
    oneclient_core::set_bisect_mod_choice(
        &state,
        cluster_id,
        &[WAS_OFF.to_string()],
        BisectChoice::Required,
    )
    .await
    .unwrap();
    assert!(
        enabled(&state, cluster_id, WAS_OFF).await,
        "always on during the search"
    );
    oneclient_core::finish_bisect(&state, cluster_id, &[FOUND.to_string()])
        .await
        .unwrap();

    assert!(
        !enabled(&state, cluster_id, FOUND).await,
        "the found mod is turned off"
    );
    assert!(
        enabled(&state, cluster_id, KEPT).await,
        "a mod that was on is on again"
    );
    assert!(
        !enabled(&state, cluster_id, WAS_OFF).await,
        "a mod that was off before the search is off again"
    );
    assert!(
        oneclient_core::bisect_status(&state, cluster_id)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn a_problem_from_an_earlier_round_stays_off_after_finishing() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_mods(&state, "Two Rounds").await;
    oneclient_core::start_bisect(&state, cluster_id, |_, _, _| {})
        .await
        .unwrap();
    for answer in [BisectAnswer::Broken, BisectAnswer::Broken] {
        oneclient_core::answer_bisect(&state, cluster_id, answer)
            .await
            .unwrap();
    }
    oneclient_core::continue_bisect(&state, cluster_id)
        .await
        .unwrap();
    let status = oneclient_core::answer_bisect(&state, cluster_id, BisectAnswer::Works)
        .await
        .unwrap();
    assert!(status.finished());
    assert!(status.culprits().is_empty(), "nothing new in round two");

    let found: Vec<String> = status.all_found().map(|m| m.hash.clone()).collect();
    assert_eq!(found, vec![FOUND.to_string()]);
    oneclient_core::finish_bisect(&state, cluster_id, &found)
        .await
        .unwrap();
    assert!(!enabled(&state, cluster_id, FOUND).await);
    assert!(enabled(&state, cluster_id, KEPT).await);
}

#[tokio::test]
async fn a_second_search_names_the_cluster_already_searching() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let first = cluster_with_mods(&state, "First Pack").await;
    let second = cluster_with_mods(&state, "Second Pack").await;

    oneclient_core::start_bisect(&state, first, |_, _, _| {})
        .await
        .unwrap();
    let err = oneclient_core::start_bisect(&state, second, |_, _, _| {})
        .await
        .unwrap_err();

    assert!(err.to_string().contains("First Pack"), "{err}");
}

#[tokio::test]
async fn a_mod_added_during_the_search_stays_on_and_joins_the_search() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_mods(&state, "Joined Cluster").await;
    oneclient_core::start_bisect(&state, cluster_id, |_, _, _| {})
        .await
        .unwrap();

    let joined = "dddddddddddddddddddddddddddddddddddddddd";
    let file = format!("{joined}.jar");
    let db = &state.services.db;
    artifact_dao::insert_artifact(db, joined, ContentType::Mod as i64, &file, &file, Some(1))
        .await
        .unwrap();
    artifact_dao::link_cluster_artifact(db, cluster_id, joined, &file)
        .await
        .unwrap();

    let status = oneclient_core::bisect_status(&state, cluster_id)
        .await
        .unwrap()
        .unwrap();
    let entry = status.mods.iter().find(|m| m.hash == joined).unwrap();
    assert_eq!(entry.choice, BisectChoice::Required);

    oneclient_core::finish_bisect(&state, cluster_id, &[])
        .await
        .unwrap();
    assert!(enabled(&state, cluster_id, joined).await);
}

#[tokio::test]
async fn a_search_from_before_the_upgrade_restarts_and_still_restores_mods() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_mods(&state, "Old Search").await;
    let db = &state.services.db;
    oneclient_db::dao::cluster_bisect::start(
        db,
        cluster_id,
        &[
            (FOUND.to_string(), "Found".to_string()),
            (KEPT.to_string(), "Kept".to_string()),
        ],
        "{}",
    )
    .await
    .unwrap();
    sqlx::query("UPDATE cluster_bisect_sessions SET search = NULL WHERE cluster_id = ?")
        .bind(cluster_id)
        .execute(db)
        .await
        .unwrap();
    artifact_dao::update_cluster_artifact(db, cluster_id, KEPT, &format!("{KEPT}.jar"), 0)
        .await
        .unwrap();

    oneclient_core::bisect::resume_unfinished_bisects(&state).await;

    let session = oneclient_db::dao::cluster_bisect::get_session(db, cluster_id)
        .await
        .unwrap()
        .unwrap();
    assert!(session.search.is_some(), "moved to the new search");

    oneclient_core::finish_bisect(&state, cluster_id, &[])
        .await
        .unwrap();
    assert!(enabled(&state, cluster_id, FOUND).await);
    assert!(
        enabled(&state, cluster_id, KEPT).await,
        "a mod the old search turned off is on again"
    );
    assert!(
        !enabled(&state, cluster_id, WAS_OFF).await,
        "a mod outside the old search is left alone"
    );
}

#[tokio::test]
async fn a_first_round_that_cannot_be_saved_puts_the_mods_back() {
    let state = oneclient_core::dev::ephemeral_state().await.unwrap();
    let cluster_id = cluster_with_mods(&state, "Unsaved Round").await;
    sqlx::query(
        "CREATE TRIGGER fail_save BEFORE UPDATE OF search ON cluster_bisect_sessions
         BEGIN SELECT RAISE(ABORT, 'save failed'); END",
    )
    .execute(&state.services.db)
    .await
    .unwrap();

    assert!(
        oneclient_core::start_bisect(&state, cluster_id, |_, _, _| {})
            .await
            .is_err()
    );

    assert!(enabled(&state, cluster_id, FOUND).await);
    assert!(enabled(&state, cluster_id, KEPT).await);
    assert!(!enabled(&state, cluster_id, WAS_OFF).await);
    assert!(
        oneclient_core::bisect_status(&state, cluster_id)
            .await
            .unwrap()
            .is_none()
    );
}
