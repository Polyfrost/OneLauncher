use chrono::{Days, Utc};
use onelauncher_core::api::cluster::dao::update_cluster;
use onelauncher_core::api::proxy::ProxyDynamic;
use onelauncher_core::api::{self, setting_profiles};
use onelauncher_core::error::LauncherResult;
use onelauncher_core::initialize_core;
use onelauncher_core::store::credentials::MinecraftCredentials;
use onelauncher_core::store::{CoreOptions, Dirs, State};
use onelauncher_entity::loader::GameLoader;
use sea_orm::ActiveValue::Set;

#[tokio::main]
pub async fn main() -> LauncherResult<()> {
	initialize_core(CoreOptions::default(), ProxyDynamic::default()).await?;

	let state = State::get().await?;
	let dirs = Dirs::get().await?;

	// We get the default credentials (or for the sake of this example, return an empty player profile)
	println!("Getting credentials...");
	let mut cred_store = state.credentials.write().await;
	let creds = cred_store
		.get_default()
		.await?
		.unwrap_or_else(|| MinecraftCredentials {
			access_token: String::new(),
			refresh_token: String::new(),
			username: "Player1".to_string(),
			id: uuid::Uuid::nil(),
			expires: Utc::now().checked_add_days(Days::new(7)).unwrap(),
		});

	println!("Credentials: {creds:?}");
	drop(cred_store);

	// We create a settings profile so that we can run hooks
	let profile_name = "profile with hooks example";
	if setting_profiles::dao::get_profile_by_name(profile_name)
		.await?
		.is_none()
	{
		setting_profiles::create_profile(profile_name, async |mut profile| {
			profile.hook_pre = Set(Some("echo this is a pre-hook!".to_string()));
			profile.hook_post = Set(Some("echo this is my post-hook!".to_string()));

			#[cfg(target_os = "linux")]
			{
				profile.hook_wrapper = Set(Some("mangohud".to_string()));
			}

			Ok(profile)
		})
		.await?;
	}

	// Here we create the cluster
	let cluster_name = "Launchable";
	let path = dirs.clusters_dir().join(cluster_name);

	let mut cluster = if let Some(cluster) =
		api::cluster::dao::get_cluster_by_folder_name(&path).await?
	{
		cluster
	} else {
		api::cluster::create_cluster(cluster_name, "1.20.4", GameLoader::Fabric, None, None).await?
	};

	// We set the clusters
	update_cluster(&mut cluster, async |mut cluster| {
		cluster.setting_profile_name = Set(Some(profile_name.to_string()));

		Ok(cluster)
	})
	.await?;

	println!(
		"Using cluster: {} at '{}'",
		cluster.name, cluster.folder_name
	);

	// Launch the game
	let process = api::game::launch::launch_minecraft(&mut cluster, creds, None, None).await?;
	println!("Process: {:?}", process.read().await);

	// We wait for the process to finish so that we can run code after
	process.read().await.child_type.write().await.wait().await?;
	println!("Successfully exited !");

	Ok(())
}
