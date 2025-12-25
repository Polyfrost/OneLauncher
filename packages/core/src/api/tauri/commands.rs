use crate::api::packages::data::{
	ExternalPackage, ManagedPackage, ManagedPackageBody, ManagedUser, ManagedVersion,
	PackageAuthor, SearchQuery, SearchResult,
};
use crate::api::packages::modpack::data::ModpackArchive;
use crate::api::packages::provider::ProviderExt;
use crate::api::setting_profiles::get_global_profile;
use crate::store::processes::Process;
use crate::store::{Settings, State};
use crate::utils::pagination::Paginated;
use interpulse::api::minecraft::Version;
use onelauncher_entity::icon::Icon;
use onelauncher_entity::loader::GameLoader;
use onelauncher_entity::package::Provider;
use onelauncher_entity::packages;
use onelauncher_entity::resolution::Resolution;
use sea_orm::ActiveValue::Set;
use tauri::{AppHandle, Runtime};

use crate::api::cluster::dao::ClusterId;
use crate::api::{self};
use crate::error::LauncherResult;
use crate::store::Core;
use crate::store::credentials::MinecraftCredentials;

use onelauncher_entity::prelude::model::*;

#[taurpc::procedures(path = "core")]
pub trait TauriLauncherApi {
	// MARK: API: clusters
	// region: clusters
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

	#[taurpc(alias = "updateClusterById")]
	async fn update_cluster_by_id(id: ClusterId, request: ClusterUpdate) -> LauncherResult<()>;

	#[taurpc(alias = "getScreenshots")]
	async fn get_screenshots(id: ClusterId) -> LauncherResult<Vec<String>>;

	#[taurpc(alias = "getWorlds")]
	async fn get_worlds(id: ClusterId) -> LauncherResult<Vec<String>>;

	#[taurpc(alias = "getLogs")]
	async fn get_logs(id: ClusterId) -> LauncherResult<Vec<String>>;

	#[taurpc(alias = "getLogByName")]
	async fn get_log_by_name(id: ClusterId, name: String) -> LauncherResult<Option<String>>;
	// endregion: clusters

	// MARK: API: processes
	// region: processes
	#[taurpc(alias = "getRunningProcesses")]
	async fn get_running_processes() -> LauncherResult<Vec<Process>>;

	#[taurpc(alias = "getRunningProcessesByClusterId")]
	async fn get_running_processes_by_cluster_id(
		cluster_id: ClusterId,
	) -> LauncherResult<Vec<Process>>;

	#[taurpc(alias = "isClusterRunning")]
	async fn is_cluster_running(cluster_id: ClusterId) -> LauncherResult<bool>;

	#[taurpc(alias = "killProcess")]
	async fn kill_process(pid: u32) -> LauncherResult<()>;
	// endregion: processes

	// MARK: API: profiles
	// region: profiles
	#[taurpc(alias = "getProfileOrDefault")]
	async fn get_profile_or_default(name: Option<String>) -> LauncherResult<SettingsProfile>;

	#[taurpc(alias = "getGlobalProfile")]
	async fn get_global_profile() -> SettingsProfile;

	#[taurpc(alias = "updateClusterProfile")]
	async fn update_cluster_profile(
		name: String,
		profile: ProfileUpdate,
	) -> LauncherResult<SettingsProfile>;

	#[taurpc(alias = "createSettingsProfile")]
	async fn create_settings_profile(name: String) -> LauncherResult<SettingsProfile>;
	// endregion: profiles

	// MARK: API: metadata
	// region: metadata
	#[taurpc(alias = "getGameVersions")]
	async fn get_game_versions() -> LauncherResult<Vec<Version>>;

	#[taurpc(alias = "getLoadersForVersion")]
	async fn get_loaders_for_version(mc_version: String) -> LauncherResult<Vec<GameLoader>>;
	// endregion: metadata

	// MARK: API: users
	// region: users
	#[taurpc(alias = "getUsers")]
	async fn get_users() -> LauncherResult<Vec<MinecraftCredentials>>;

	#[taurpc(alias = "getUser")]
	async fn get_user(uuid: uuid::Uuid) -> LauncherResult<Option<MinecraftCredentials>>;

	#[taurpc(alias = "removeUser")]
	async fn remove_user(uuid: uuid::Uuid) -> LauncherResult<()>;

	#[taurpc(alias = "getDefaultUser")]
	async fn get_default_user(
		fallback: Option<bool>,
	) -> LauncherResult<Option<MinecraftCredentials>>;

	#[taurpc(alias = "setDefaultUser")]
	async fn set_default_user(uuid: Option<uuid::Uuid>) -> LauncherResult<()>;

	#[taurpc(alias = "openMsaLogin")]
	async fn open_msa_login<R: Runtime>(
		app_handle: AppHandle<R>,
	) -> LauncherResult<Option<MinecraftCredentials>>;
	// endregion: users

	// MARK: API: settings
	// region: settings
	#[taurpc(alias = "readSettings")]
	async fn read_settings() -> LauncherResult<Settings>;

	#[taurpc(alias = "writeSettings")]
	async fn write_settings(setting: Settings) -> LauncherResult<()>;
	// endregion: settings

	// MARK: API: package
	// region: package
	#[taurpc(alias = "searchPackages")]
	async fn search_packages(
		provider: Provider,
		query: SearchQuery,
	) -> LauncherResult<Paginated<SearchResult>>;

	#[taurpc(alias = "getPackage")]
	async fn get_package(provider: Provider, slug: String) -> LauncherResult<ManagedPackage>;

	#[taurpc(alias = "getPackageBody")]
	async fn get_package_body(
		provider: Provider,
		body: ManagedPackageBody,
	) -> LauncherResult<String>;

	#[taurpc(alias = "getMultiplePackages")]
	async fn get_multiple_packages(
		provider: Provider,
		slugs: Vec<String>,
	) -> LauncherResult<Vec<ManagedPackage>>;

	#[taurpc(alias = "getPackageVersions")]
	async fn get_package_versions(
		provider: Provider,
		slug: String,
		mc_version: Option<String>,
		loader: Option<GameLoader>,
		offset: usize,
		limit: usize,
	) -> LauncherResult<Paginated<ManagedVersion>>;

	// #[taurpc(alias = "getPackageUser")]
	// async fn get_package_user(provider: Provider, slug: String) -> LauncherResult<ManagedUser>;

	#[taurpc(alias = "downloadPackage")]
	async fn download_package(
		provider: Provider,
		package_id: String,
		version_id: String,
		cluster_id: ClusterId,
		skip_compatibility: Option<bool>,
	) -> LauncherResult<packages::Model>;

	#[taurpc(alias = "downloadExternalPackage")]
	async fn download_external_package(
		package: ExternalPackage,
		cluster_id: ClusterId,
		force: Option<bool>,
		skip_compatibility: Option<bool>,
	) -> LauncherResult<Option<packages::Model>>;

	#[taurpc(alias = "getUsersFromAuthor")]
	async fn get_users_from_author(
		provider: Provider,
		author: PackageAuthor,
	) -> LauncherResult<Vec<ManagedUser>>;

	#[taurpc(alias = "getLinkedPackages")]
	async fn get_linked_packages(cluster_id: ClusterId) -> LauncherResult<Vec<packages::Model>>;

	#[taurpc(alias = "removePackage")]
	async fn remove_package(cluster_id: ClusterId, package_hash: String) -> LauncherResult<()>;
	// endregion: package

	// MARK: API: modpack
	// region: modpack
	#[taurpc(alias = "installModpack")]
	async fn install_modpack(modpack: ModpackArchive, cluster_id: ClusterId) -> LauncherResult<()>;
	// endregion: modpack

	// MARK: API: minecraft
	// region: minecraft
	#[taurpc(alias = "fetchMinecraftProfile")]
	async fn fetch_player_profile(
		uuid: String,
	) -> LauncherResult<crate::utils::minecraft::MojangPlayerProfile>;

	#[taurpc(alias = "fetchLoggedInProfile")]
	async fn fetch_logged_in_profile(
		access_token: String,
	) -> LauncherResult<crate::utils::minecraft::MojangFullPlayerProfile>;

	#[taurpc(alias = "uploadSkinBytes")]
	async fn upload_skin_bytes(
		access_token: String,
		skin_data: Vec<u8>,
		image_format: String,
		skin_variant: crate::utils::minecraft::SkinVariant,
	) -> LauncherResult<crate::utils::minecraft::MojangSkin>;

	#[taurpc(alias = "changeSkin")]
	async fn change_skin(
		access_token: String,
		skin_url: String,
		skin_variant: crate::utils::minecraft::SkinVariant,
	) -> LauncherResult<crate::utils::minecraft::MojangSkin>;

	#[taurpc(alias = "changeCape")]
	async fn change_cape(
		access_token: String,
		cape_uuid: String,
	) -> LauncherResult<crate::utils::minecraft::MojangFullPlayerProfile>;

	#[taurpc(alias = "removeCape")]
	async fn remove_cape(
		access_token: String,
	) -> LauncherResult<crate::utils::minecraft::MojangFullPlayerProfile>;

	#[taurpc(alias = "convertUsernameUUID")]
	async fn convert_username_uuid(
		username_uuid: String,
	) -> LauncherResult<crate::utils::minecraft::MowojangProfile>;
	// endregion: minecraft

	// MARK: API: discord RPC
	// region: discord RPC
	#[taurpc(alias = "setDiscordRPCMessage")]
	async fn set_discord_rpc_message(message: String) -> LauncherResult<()>;
	// endregion: discord RPC

	// MARK: API: Other
	async fn open(input: String) -> LauncherResult<()>;
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone)]
pub struct CreateCluster {
	name: String,
	mc_version: String,
	mc_loader: GameLoader,
	mc_loader_version: Option<String>,
	icon: Option<Icon>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone)]
pub struct ClusterUpdate {
	name: Option<String>,
	icon_url: Option<Icon>,
	setting_profile_name: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone)]
pub struct ProfileUpdate {
	pub res: Option<Resolution>,
	pub force_fullscreen: Option<bool>,
	pub mem_max: Option<u32>,
	pub launch_args: Option<String>,
	pub launch_env: Option<String>,
	pub hook_pre: Option<String>,
	pub hook_wrapper: Option<String>,
	pub hook_post: Option<String>,
}

#[taurpc::ipc_type]
pub struct TauriLauncherApiImpl;

#[taurpc::resolvers]
impl TauriLauncherApi for TauriLauncherApiImpl {
	// MARK: Impl: clusters
	// region: clusters
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
		let cluster = api::cluster::create_cluster(
			&options.name,
			&options.mc_version,
			options.mc_loader,
			options.mc_loader_version.as_deref(),
			options.icon,
		)
		.await?;

		api::setting_profiles::create_profile(
			&options.name,
			|mut active_model: SettingsProfilePartial| async move {
				active_model.force_fullscreen = Set(Some(false));

				active_model.mem_max = Set(Some(2048));

				Ok(active_model)
			},
		)
		.await?;

		api::cluster::dao::update_cluster_by_id(
			cluster.id,
			|mut active_model: ClusterPartial| async move {
				active_model.setting_profile_name = Set(Some(options.name.clone()));

				Ok(active_model)
			},
		)
		.await?;

		Ok(cluster)
	}

	async fn launch_cluster(self, id: ClusterId, uuid: Option<uuid::Uuid>) -> LauncherResult<()> {
		let mut cluster = api::cluster::dao::get_cluster_by_id(id)
			.await?
			.ok_or_else(|| anyhow::anyhow!("cluster with id {} not found", id))?;

		let uuid = match uuid {
			Some(uuid) => uuid,
			None => api::credentials::get_default_user()
				.await?
				.ok_or_else(|| anyhow::anyhow!("no default user set"))?,
		};

		let user = api::credentials::get_user(uuid)
			.await?
			.ok_or_else(|| anyhow::anyhow!("user with uuid {} not found", uuid))?;

		// let user = api::credentials::get_fake_user();

		let _ = api::game::launch::launch_minecraft(&mut cluster, user, None).await?;

		Ok(())
	}

	async fn update_cluster_by_id(
		self,
		id: ClusterId,
		request: ClusterUpdate,
	) -> LauncherResult<()> {
		api::cluster::dao::update_cluster_by_id(
			id,
			|mut active_model: ClusterPartial| async move {
				if let Some(name) = request.name {
					active_model.name = Set(name);
				}

				// ok i know this is so wrong but im not a rust guy
				if let Some(icon_url) = request.icon_url {
					api::cluster::dao::set_icon_by_id(id, &icon_url).await?;
				}

				if let Some(setting_profile_name) = request.setting_profile_name {
					active_model.setting_profile_name = Set(Some(setting_profile_name))
				}

				Ok(active_model)
			},
		)
		.await?;

		Ok(())
	}

	async fn get_screenshots(self, id: ClusterId) -> LauncherResult<Vec<String>> {
		let cluster = api::cluster::dao::get_cluster_by_id(id)
			.await?
			.ok_or_else(|| anyhow::anyhow!("cluster with id {} not found", id))?;

		api::cluster::content::get_screenshots(&cluster).await
	}

	async fn get_worlds(self, id: ClusterId) -> LauncherResult<Vec<String>> {
		let cluster = api::cluster::dao::get_cluster_by_id(id)
			.await?
			.ok_or_else(|| anyhow::anyhow!("cluster with id {} not found", id))?;

		api::cluster::content::get_worlds(&cluster).await
	}

	async fn get_logs(self, id: ClusterId) -> LauncherResult<Vec<String>> {
		let cluster = api::cluster::dao::get_cluster_by_id(id)
			.await?
			.ok_or_else(|| anyhow::anyhow!("cluster with id {} not found", id))?;

		api::cluster::content::get_logs(&cluster).await
	}

	async fn get_log_by_name(self, id: ClusterId, name: String) -> LauncherResult<Option<String>> {
		let cluster = api::cluster::dao::get_cluster_by_id(id)
			.await?
			.ok_or_else(|| anyhow::anyhow!("cluster with id {} not found", id))?;

		api::cluster::content::get_log_by_name(&cluster, &name).await
	}
	// endregion: clusters

	// MARK: Impl: processes
	// region: processes
	async fn get_running_processes(self) -> LauncherResult<Vec<Process>> {
		api::processes::get_running_processes().await
	}

	async fn get_running_processes_by_cluster_id(
		self,
		cluster_id: ClusterId,
	) -> LauncherResult<Vec<Process>> {
		api::processes::get_running_processes_by_cluster_id(cluster_id).await
	}

	async fn is_cluster_running(self, cluster_id: ClusterId) -> LauncherResult<bool> {
		api::processes::is_cluster_running(cluster_id).await
	}

	async fn kill_process(self, pid: u32) -> LauncherResult<()> {
		api::processes::kill_process(pid).await
	}
	// endregion: processes

	// MARK: Impl: profiles
	// region: profiles
	async fn get_global_profile(self) -> SettingsProfile {
		api::setting_profiles::get_global_profile().await
	}

	async fn get_profile_or_default(self, name: Option<String>) -> LauncherResult<SettingsProfile> {
		api::setting_profiles::dao::get_profile_or_default(name.as_ref()).await
	}

	async fn create_settings_profile(self, name: String) -> LauncherResult<SettingsProfile> {
		let mut profile = get_global_profile().await;
		profile.name = name;

		api::setting_profiles::dao::insert_profile(profile.into()).await
	}

	// please kill this with fire
	async fn update_cluster_profile(
		self,
		name: String,
		profile: ProfileUpdate,
	) -> LauncherResult<SettingsProfile> {
		let profile = api::setting_profiles::dao::update_profile_by_name(
			&name,
			|mut active_model: SettingsProfilePartial| async move {
				if let Some(res) = profile.res {
					active_model.res = Set(Some(res));
				}

				if let Some(force_fullscreen) = profile.force_fullscreen {
					active_model.force_fullscreen = Set(Some(force_fullscreen));
				}

				if let Some(mem_max) = profile.mem_max {
					active_model.mem_max = Set(Some(mem_max));
				}

				if let Some(launch_args) = profile.launch_args {
					active_model.launch_args = Set(Some(launch_args));
				}

				if let Some(launch_env) = profile.launch_env {
					active_model.launch_env = Set(Some(launch_env));
				}

				if let Some(hook_pre) = profile.hook_pre {
					active_model.hook_pre = Set(Some(hook_pre));
				}

				if let Some(hook_wrapper) = profile.hook_wrapper {
					active_model.hook_wrapper = Set(Some(hook_wrapper));
				}

				if let Some(hook_post) = profile.hook_post {
					active_model.hook_post = Set(Some(hook_post));
				}

				Ok(active_model)
			},
		)
		.await?;

		Ok(profile)
	}
	// endregion: profiles

	// MARK: Impl: metadata
	// region: metadata
	async fn get_loaders_for_version(self, mc_version: String) -> LauncherResult<Vec<GameLoader>> {
		api::game::metadata::get_loaders_for_version(&mc_version).await
	}

	async fn get_game_versions(self) -> LauncherResult<Vec<Version>> {
		api::game::metadata::get_game_versions().await
	}
	// endregion: metadata

	// MARK: Impl: users
	// region: users
	async fn get_users(self) -> LauncherResult<Vec<MinecraftCredentials>> {
		api::credentials::get_users().await
	}

	async fn get_user(self, uuid: uuid::Uuid) -> LauncherResult<Option<MinecraftCredentials>> {
		api::credentials::get_user(uuid).await
	}

	async fn remove_user(self, uuid: uuid::Uuid) -> LauncherResult<()> {
		api::credentials::remove_user(uuid).await
	}

	async fn get_default_user(
		self,
		fallback: Option<bool>,
	) -> LauncherResult<Option<MinecraftCredentials>> {
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

	async fn open_msa_login<R: Runtime>(
		self,
		app_handle: AppHandle<R>,
	) -> LauncherResult<Option<MinecraftCredentials>> {
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
				&& let Some((_, code)) = win.url()?.query_pairs().find(|x| x.0 == "code")
			{
				win.close()?;
				let value = api::credentials::finish(&code.clone(), flow).await?;

				return Ok(Some(value));
			}

			tokio::time::sleep(std::time::Duration::from_millis(50)).await;
		}

		win.close()?;

		Ok(None)
	}

	async fn get_users_from_author(
		self,
		provider: Provider,
		author: PackageAuthor,
	) -> LauncherResult<Vec<ManagedUser>> {
		provider.get_users_from_author(author).await
	}
	// endregion: users

	// MARK: Impl: settings
	// region: settings
	async fn read_settings(self) -> LauncherResult<Settings> {
		let state = State::get().await?;
		let settings = state.settings.read().await;

		Ok(settings.clone())
	}

	async fn write_settings(self, setting: Settings) -> LauncherResult<()> {
		let state = State::get().await?;
		let mut settings = state.settings.write().await;

		*settings = setting;

		settings.save().await?;

		Ok(())
	}
	// endregion: settings

	// MARK: Impl: package
	// region: package
	async fn search_packages(
		self,
		provider: Provider,
		query: SearchQuery,
	) -> LauncherResult<Paginated<SearchResult>> {
		provider.search(&query).await
	}

	async fn get_package(self, provider: Provider, slug: String) -> LauncherResult<ManagedPackage> {
		provider.get(&slug).await
	}

	async fn get_package_body(
		self,
		provider: Provider,
		body: ManagedPackageBody,
	) -> LauncherResult<String> {
		provider.get_body(&body).await
	}

	async fn get_multiple_packages(
		self,
		provider: Provider,
		slugs: Vec<String>,
	) -> LauncherResult<Vec<ManagedPackage>> {
		provider.get_multiple(&slugs).await
	}

	async fn get_package_versions(
		self,
		provider: Provider,
		slug: String,
		mc_version: Option<String>,
		loader: Option<GameLoader>,
		offset: usize,
		limit: usize,
	) -> LauncherResult<Paginated<ManagedVersion>> {
		provider
			.get_versions_paginated(&slug, mc_version, loader, offset, limit)
			.await
	}

	async fn download_package(
		self,
		provider: Provider,
		package_id: String,
		version_id: String,
		cluster_id: ClusterId,
		skip_compatibility: Option<bool>,
	) -> LauncherResult<packages::Model> {
		let cluster = api::cluster::dao::get_cluster_by_id(cluster_id)
			.await?
			.ok_or_else(|| anyhow::anyhow!("cluster with id {} not found", cluster_id))?;

		let package = provider.get(&package_id).await?;

		let versions = provider.get_versions(&[version_id]).await?;
		let version = versions
			.into_iter()
			.next()
			.ok_or_else(|| anyhow::anyhow!("Version not found"))?;

		let model = api::packages::download_package(&package, &version, None, None).await?;

		api::packages::link_package(&model, &cluster, skip_compatibility).await?;

		Ok(model)
	}

	async fn download_external_package(
		self,
		package: ExternalPackage,
		cluster_id: ClusterId,
		force: Option<bool>,
		skip_compatibility: Option<bool>,
	) -> LauncherResult<Option<packages::Model>> {
		let cluster = api::cluster::dao::get_cluster_by_id(cluster_id)
			.await?
			.ok_or_else(|| anyhow::anyhow!("cluster with id {} not found", cluster_id))?;

		let model = api::packages::download_external_package(
			&package,
			&cluster,
			force,
			skip_compatibility,
			None,
		)
		.await?;

		Ok(model)
	}

	async fn get_linked_packages(
		self,
		cluster_id: ClusterId,
	) -> LauncherResult<Vec<packages::Model>> {
		let cluster = api::cluster::dao::get_cluster_by_id(cluster_id)
			.await?
			.ok_or_else(|| anyhow::anyhow!("cluster with id {} not found", cluster_id))?;

		let model = api::packages::dao::get_linked_packages(&cluster).await?;

		Ok(model)
	}

	async fn remove_package(
		self,
		cluster_id: ClusterId,
		package_hash: String,
	) -> LauncherResult<()> {
		api::packages::remove_package(cluster_id, package_hash).await
	}
	// endregion: package

	// MARK: Impl: modpack
	// region: modpack
	async fn install_modpack(
		self,
		modpack: ModpackArchive,
		cluster_id: ClusterId,
	) -> LauncherResult<()> {
		let cluster = api::cluster::dao::get_cluster_by_id(cluster_id)
			.await?
			.ok_or_else(|| anyhow::anyhow!("cluster with id {} not found", cluster_id))?;

		modpack
			.format
			.install_modpack_archive(&modpack, &cluster, None, None)
			.await
	}
	// endregion: modpack

	// MARK: Impl: minecraft
	// region: minecraft
	async fn fetch_player_profile(
		self,
		uuid: String,
	) -> LauncherResult<crate::utils::minecraft::MojangPlayerProfile> {
		crate::utils::minecraft::fetch_player_profile(&uuid).await
	}

	async fn fetch_logged_in_profile(
		self,
		access_token: String,
	) -> LauncherResult<crate::utils::minecraft::MojangFullPlayerProfile> {
		crate::utils::minecraft::fetch_logged_in_profile(&access_token).await
	}

	async fn upload_skin_bytes(
		self,
		access_token: String,
		skin_data: Vec<u8>,
		image_format: String,
		skin_variant: crate::utils::minecraft::SkinVariant,
	) -> LauncherResult<crate::utils::minecraft::MojangSkin> {
		crate::utils::minecraft::upload_skin_bytes(
			&access_token,
			skin_data,
			&image_format,
			skin_variant,
		)
		.await
	}

	async fn change_skin(
		self,
		access_token: String,
		skin_url: String,
		skin_variant: crate::utils::minecraft::SkinVariant,
	) -> LauncherResult<crate::utils::minecraft::MojangSkin> {
		crate::utils::minecraft::change_skin(&access_token, &skin_url, skin_variant).await
	}

	async fn change_cape(
		self,
		access_token: String,
		cape_uuid: String,
	) -> LauncherResult<crate::utils::minecraft::MojangFullPlayerProfile> {
		crate::utils::minecraft::change_cape(&access_token, &cape_uuid).await
	}

	async fn remove_cape(
		self,
		access_token: String,
	) -> LauncherResult<crate::utils::minecraft::MojangFullPlayerProfile> {
		crate::utils::minecraft::remove_cape(&access_token).await
	}

	async fn convert_username_uuid(
		self,
		username_uuid: String,
	) -> LauncherResult<crate::utils::minecraft::MowojangProfile> {
		crate::utils::minecraft::convert_username_uuid(&username_uuid).await
	}
	// endregion: minecraft

	// MARK: Impl: discord RPC
	// region: discord RPC
	async fn set_discord_rpc_message(self, message: String) -> LauncherResult<()> {
		let state = State::get().await?;
		if let Some(discord) = &state.rpc {
			if !state.settings.read().await.discord_enabled {
				discord.clear_activity().await;
				return Ok(());
			}
			discord.set_message(&message, None).await;
		}

		Ok(())
	}
	// endregion: discord RPC

	// MARK: Impl: Other
	// region: other
	async fn open(self, input: String) -> LauncherResult<()> {
		opener::open(input)?;
		Ok(())
	}
	// endregion: other
}
