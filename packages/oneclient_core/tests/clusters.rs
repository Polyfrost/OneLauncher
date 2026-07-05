use std::time::Duration;

use oneclient_core::clusters::{ClusterManager, ClusterStage, CreateClusterOptions};
use oneclient_core::packages::domain::GameLoader;
use oneclient_core::patch::Patch;
use oneclient_core::settings::store::create_settings_profile;
use oneclient_core::ProfileUpdate;

#[tokio::test]
async fn cluster_lifecycle_with_settings_profile() {
	let state = oneclient_core::dev::ephemeral_state().await.unwrap();
	let settings = state.settings.read().clone();

	let cluster = ClusterManager::create(
		&state,
		CreateClusterOptions::new("Test Cluster", "1.21.1", GameLoader::Fabric),
	)
	.await
	.unwrap();

	assert_eq!(cluster.name, "Test Cluster");
	assert_eq!(cluster.mc_version, "1.21.1");
	assert_eq!(cluster.stage, ClusterStage::NotReady);
	assert!(cluster.setting_profile_name.is_some());
	assert!(cluster.created_at.is_some());

	let resolved = ClusterManager::resolve_settings(&state, &cluster)
		.await
		.unwrap();
	assert_eq!(resolved.mem_max, Some(2048));

	ClusterManager::update_profile(
		&state,
		cluster.id,
		ProfileUpdate {
			mem_max: Patch::Set(4096),
			..Default::default()
		},
	)
	.await
	.unwrap();

	let updated = ClusterManager::resolve_settings(&state, &cluster)
		.await
		.unwrap();
	assert_eq!(updated.mem_max, Some(4096));

	let shared = create_settings_profile(&state.services.db, &settings, "Shared Profile")
		.await
		.unwrap();

	ClusterManager::update(
		&state,
		cluster.id,
		oneclient_core::ClusterUpdate::default().setting_profile(shared.name.clone()),
	)
	.await
	.unwrap();

	let reassigned = ClusterManager::get(&state, cluster.id).await.unwrap();
	assert_eq!(
		reassigned.setting_profile_name.as_deref(),
		Some("Shared Profile")
	);

	let after_play = ClusterManager::add_playtime(&state, cluster.id, Duration::from_secs(120))
		.await
		.unwrap();
	assert_eq!(after_play.overall_played, Duration::from_secs(120));
	assert!(after_play.last_played.is_some());

	ClusterManager::delete(&state, cluster.id, true).await.unwrap();
	assert!(ClusterManager::get(&state, cluster.id).await.is_err());
}

#[tokio::test]
async fn cleared_profile_fields_inherit_from_global() {
	let state = oneclient_core::dev::ephemeral_state().await.unwrap();

	{
		let mut settings = state.settings.write();
		settings.global_game_settings.mem_max = Some(8192);
	}

	let cluster = ClusterManager::create(
		&state,
		CreateClusterOptions::new("Inherit Test", "1.21.1", GameLoader::Vanilla).mem_max(2048),
	)
	.await
	.unwrap();

	let profile_name = cluster.setting_profile_name.clone().unwrap();

	ClusterManager::update_profile(
		&state,
		cluster.id,
		ProfileUpdate {
			mem_max: Patch::Clear,
			force_fullscreen: Patch::Clear,
			..Default::default()
		},
	)
	.await
	.unwrap();

    let settings = state.settings.read().clone();
	let stored = oneclient_core::settings::store::get_profile_or_default(
		&state.services.db,
		&settings,
		Some(&profile_name),
	)
	.await
	.unwrap();
	assert_eq!(stored.mem_max, None);
	assert_eq!(stored.force_fullscreen, None);

	let resolved = ClusterManager::resolve_settings(&state, &cluster)
		.await
		.unwrap();
	assert_eq!(resolved.mem_max, Some(8192));
	assert_eq!(resolved.force_fullscreen, Some(false));
}
