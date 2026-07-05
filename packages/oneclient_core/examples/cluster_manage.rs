
use oneclient_core::clusters::{ClusterManager, CreateClusterOptions};
use oneclient_core::dev;
use oneclient_core::packages::domain::GameLoader;
use oneclient_core::settings::store::list_named_profiles;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	tracing_subscriber::fmt::init();

	let state = dev::ephemeral_state().await?;

	let cluster = ClusterManager::create(
		&state,
		CreateClusterOptions::new("Example 1.21.1 Fabric", "1.21.1", GameLoader::Fabric).mem_max(3072),
	)
	.await?;

	println!("Created cluster #{} at folder '{}'", cluster.id, cluster.folder_name);
	println!("  setting profile: {:?}", cluster.setting_profile_name);
	println!("  created at: {:?}", cluster.created_at);

	let profile = ClusterManager::resolve_settings(&state, &cluster).await?;
	println!(
		"  resolved mem_max: {:?}, force_fullscreen: {:?}",
		profile.mem_max, profile.force_fullscreen
	);

	for c in ClusterManager::list(&state).await? {
		println!("  - {} ({}, {})", c.name, c.mc_version, c.mc_loader);
	}

	for p in list_named_profiles(&state.services.db).await? {
		println!("Profile {} (mem_max: {:?})", p.name, p.mem_max);
	}

	ClusterManager::delete(&state, cluster.id, true).await?;
	println!("Deleted cluster #{}", cluster.id);

	Ok(())
}
