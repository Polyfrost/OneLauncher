use interpulse::api::minecraft::Version;
use onelauncher_entity::{icon::Icon, loader::GameLoader};
use tauri::{AppHandle, Runtime};

use crate::{api::{self, cluster::dao::ClusterId}, error::LauncherResult, store::{credentials::MinecraftCredentials, Core}};

use onelauncher_entity::prelude::model::*;

#[taurpc::procedures(path = "core")]
pub trait TauriLauncherApi {

	// Clusters
	#[taurpc(alias = "getClusters")]
	async fn get_clusters() -> LauncherResult<Vec<Cluster>>;

	#[taurpc(alias = "getClusterById")]
	async fn get_cluster_by_id(id: ClusterId) -> LauncherResult<Option<Cluster>>;

	#[taurpc(alias = "removeCluster")]
	async fn remove_cluster(id: ClusterId) -> LauncherResult<()>;

	#[taurpc(alias = "createCluster")]
	async fn create_cluster(options: CreateCluster) -> LauncherResult<Cluster>;

	#[taurpc(alias = "launchCluster")]
	async fn launch_cluster(id: ClusterId, uuid: Option<uuid::Uuid>) -> LauncherResult<()>;


	// Setting Profiles
	#[taurpc(alias = "getProfileOrDefault")]
	async fn get_profile_or_default(name: Option<String>) -> LauncherResult<SettingsProfile>;

	#[taurpc(alias = "getGlobalProfile")]
	async fn get_global_profile() -> SettingsProfile;

	// Game Metadata
	#[taurpc(alias = "getGameVersions")]
	async fn get_game_versions() -> LauncherResult<Vec<Version>>;

	#[taurpc(alias = "getLoadersForVersion")]
	async fn get_loaders_for_version(mc_version: String) -> LauncherResult<Vec<GameLoader>>;


	// Users
	#[taurpc(alias = "getUsers")]
	async fn get_users() -> LauncherResult<Vec<MinecraftCredentials>>;

	#[taurpc(alias = "getUser")]
	async fn get_user(uuid: uuid::Uuid) -> LauncherResult<Option<MinecraftCredentials>>;

	#[taurpc(alias = "removeUser")]
	async fn remove_user(uuid: uuid::Uuid) -> LauncherResult<()>;

	#[taurpc(alias = "getDefaultUser")]
	async fn get_default_user(fallback: Option<bool>) -> LauncherResult<Option<MinecraftCredentials>>;

	#[taurpc(alias = "setDefaultUser")]
	async fn set_default_user(uuid: Option<uuid::Uuid>) -> LauncherResult<()>;

	#[taurpc(alias = "openMsaLogin")]
	async fn open_msa_login<R: Runtime>(app_handle: AppHandle<R>) -> LauncherResult<Option<MinecraftCredentials>>;
}


#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone)]
pub struct CreateCluster {
	name: String,
	mc_version: String,
	mc_loader: GameLoader,
	mc_loader_version: Option<String>,
	icon: Option<Icon>,
}


#[taurpc::ipc_type]
pub struct TauriLauncherApiImpl;

#[taurpc::resolvers]
impl TauriLauncherApi for TauriLauncherApiImpl {

	// Clusters
	async fn get_clusters(self) -> LauncherResult<Vec<Cluster>> {
		api::cluster::dao::get_all_clusters().await
	}

	async fn get_cluster_by_id(self, id: ClusterId) -> LauncherResult<Option<Cluster>> {
		api::cluster::dao::get_cluster_by_id(id).await
	}

	async fn remove_cluster(self, id: ClusterId) -> LauncherResult<()> {
		api::cluster::dao::delete_cluster_by_id(id).await
	}

	async fn create_cluster(self, options: CreateCluster) -> LauncherResult<Cluster> {
		let cluster = api::cluster::create_cluster(&options.name, &options.mc_version, options.mc_loader, options.mc_loader_version.as_deref(), options.icon).await?;

		Ok(cluster)
	}

	async fn launch_cluster(self, id: ClusterId, uuid: Option<uuid::Uuid>) -> LauncherResult<()> {
		let mut cluster = api::cluster::dao::get_cluster_by_id(id).await?
			.ok_or_else(|| anyhow::anyhow!("cluster with id {} not found", id))?;

		let uuid = match uuid {
			Some(uuid) => uuid,
			None => api::credentials::get_default_user().await?.ok_or_else(|| anyhow::anyhow!("no default user set"))?,
		};

		let user = api::credentials::get_user(uuid).await?
			.ok_or_else(|| anyhow::anyhow!("user with uuid {} not found", uuid))?;

		// let user = api::credentials::get_fake_user();

		let _ = api::game::launch::launch_minecraft(&mut cluster, user, None).await?;

		Ok(())
	}


	// Setting Profiles
	async fn get_global_profile(self) -> SettingsProfile {
		api::setting_profiles::get_global_profile().await
	}

	async fn get_profile_or_default(self, name: Option<String>) -> LauncherResult<SettingsProfile> {
		api::setting_profiles::dao::get_profile_or_default(name.as_ref()).await
	}


	// Game Metadata
	async fn get_loaders_for_version(self, mc_version: String) -> LauncherResult<Vec<GameLoader>> {
		api::game::metadata::get_loaders_for_version(&mc_version).await
	}

	async fn get_game_versions(self) -> LauncherResult<Vec<Version>> {
		api::game::metadata::get_game_versions().await
	}


	// Users
	async fn get_users(self) -> LauncherResult<Vec<MinecraftCredentials>> {
		api::credentials::get_users().await
	}

	async fn get_user(self, uuid: uuid::Uuid) -> LauncherResult<Option<MinecraftCredentials>> {
		api::credentials::get_user(uuid).await
	}

	async fn remove_user(self, uuid: uuid::Uuid) -> LauncherResult<()> {
		api::credentials::remove_user(uuid).await
	}

	async fn get_default_user(self, fallback: Option<bool>) -> LauncherResult<Option<MinecraftCredentials>> {
		let uuid = api::credentials::get_default_user().await?;

		if fallback.is_some_and(|fallback| fallback) && uuid.is_none() {
			return Ok(api::credentials::get_users().await?.first().cloned());
		}

		match uuid {
			Some(uuid) => Ok(api::credentials::get_user(uuid).await?),
			None => Ok(None),
		}
	}

	async fn set_default_user(self, uuid: Option<uuid::Uuid>) -> LauncherResult<()> {
		api::credentials::set_default_user(uuid).await
	}

	async fn open_msa_login<R: Runtime>(self, app_handle: AppHandle<R>) -> LauncherResult<Option<MinecraftCredentials>> {
		use tauri::Manager;

		let flow = api::credentials::begin().await?;

		let now = chrono::Utc::now();

		if let Some(win) = app_handle.get_webview_window("login") {
			win.close()?;
		}

		let win = tauri::WebviewWindowBuilder::new(
			&app_handle,
			"login",
			tauri::WebviewUrl::External(
				flow.redirect_uri
					.parse()
					.map_err(|_| anyhow::anyhow!("failed to parse auth redirect url"))?,
			),
		)
			.title(format!("Login to {}", Core::get().launcher_name))
			.center()
			.focused(true)
			.build()?;

		win.request_user_attention(Some(tauri::UserAttentionType::Critical))?;

		while (chrono::Utc::now() - now) < chrono::Duration::minutes(10) {
			if win.title().is_err() {
				return Ok(None);
			}

			if win
				.url()?
				.as_str()
				.starts_with("https://login.live.com/oauth20_desktop.srf")
			{
				if let Some((_, code)) = win
					.url()?
					.query_pairs()
					.find(|x| x.0 == "code")
				{
					win.close()?;
					let value = api::credentials::finish(&code.clone(), flow).await?;

					return Ok(Some(value));
				}
			}

			tokio::time::sleep(std::time::Duration::from_millis(50)).await;
		}

		win.close()?;

		Ok(None)
	}
}
