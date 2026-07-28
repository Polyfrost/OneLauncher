use std::time::Duration;

use oneclient_core::clusters::{ClusterStage, CreateClusterOptions};
use oneclient_common::domain::GameLoader;
use oneclient_common::patch::Patch;
use oneclient_cluster::profiles::create_settings_profile;
use oneclient_core::ProfileUpdate;

#[tokio::test]
async fn cluster_lifecycle_with_settings_profile() {
	let state = oneclient_core::dev::ephemeral_state().await.unwrap();
	let settings = state.settings.read().clone();

	let global = state.settings.read().global_game_settings.clone();

	// Deliberately not the global default of 4096, so the assertions below
	// distinguish "the cluster's own profile was used" from "we fell back to
	// global and happened to match".
	let cluster = state.clusters.create(
        &global,
        CreateClusterOptions::new("Test Cluster", "1.21.1", GameLoader::Fabric).mem_max(2048),
	)
	.await
	.unwrap();

	assert_eq!(cluster.name, "Test Cluster");
	assert_eq!(cluster.mc_version, "1.21.1");
	assert_eq!(cluster.stage, ClusterStage::NotReady);
	assert!(cluster.setting_profile_name.is_some());
	assert!(cluster.created_at.is_some());

	let resolved = state.clusters.resolve_settings(&global, &cluster)
		.await
		.unwrap();
	assert_eq!(resolved.mem_max, Some(2048));

	state.clusters.update_profile(cluster.id,
		ProfileUpdate {
			mem_max: Patch::Set(6144),
			..Default::default()
		},
	)
	.await
	.unwrap();

	let updated = state.clusters.resolve_settings(&global, &cluster)
		.await
		.unwrap();
	assert_eq!(updated.mem_max, Some(6144));

	let shared = create_settings_profile(&state.services.db, &settings.global_game_settings, "Shared Profile")
		.await
		.unwrap();

	state.clusters.update(cluster.id,
		oneclient_core::ClusterUpdate::default().setting_profile(shared.name.clone()),
	)
	.await
	.unwrap();

	let reassigned = state.clusters.get(cluster.id).await.unwrap();
	assert_eq!(
		reassigned.setting_profile_name.as_deref(),
		Some("Shared Profile")
	);

	let after_play = state.clusters.add_playtime(cluster.id, Duration::from_secs(120))
		.await
		.unwrap();
	assert_eq!(after_play.overall_played, Duration::from_secs(120));
	assert!(after_play.last_played.is_some());

	state.clusters.delete(cluster.id, true).await.unwrap();
	assert!(state.clusters.get(cluster.id).await.is_err());
}

#[tokio::test]
async fn cleared_profile_fields_inherit_from_global() {
	let state = oneclient_core::dev::ephemeral_state().await.unwrap();

	{
		let mut settings = state.settings.write();
		settings.global_game_settings.mem_max = Some(8192);
	}

	// Read the baseline *after* the write above, so the new cluster inherits it.
	let global = state.settings.read().global_game_settings.clone();
	let cluster = state.clusters.create(
        &global,
        CreateClusterOptions::new("Inherit Test", "1.21.1", GameLoader::Vanilla).mem_max(2048),
	)
	.await
	.unwrap();

	let profile_name = cluster.setting_profile_name.clone().unwrap();

	state.clusters.update_profile(cluster.id,
		ProfileUpdate {
			mem_max: Patch::Clear,
			force_fullscreen: Patch::Clear,
			..Default::default()
		},
	)
	.await
	.unwrap();

    let global = state.settings.read().global_game_settings.clone();
	let stored = oneclient_cluster::profiles::get_profile_or_default(
		&state.services.db,
		&global,
		Some(&profile_name),
	)
	.await
	.unwrap();
	assert_eq!(stored.mem_max, None);
	assert_eq!(stored.force_fullscreen, None);

	let resolved = state.clusters.resolve_settings(&global, &cluster)
		.await
		.unwrap();
	assert_eq!(resolved.mem_max, Some(8192));
	assert_eq!(resolved.force_fullscreen, Some(false));
}
