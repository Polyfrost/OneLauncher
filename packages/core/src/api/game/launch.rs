use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use chrono::Utc;
use discord_rich_presence::activity::Timestamps;
use merge::Merge;
use onelauncher_entity::prelude::model::*;
use tokio::process::Command;
use tokio::sync::RwLock;

use crate::api::cluster::{ClusterError, prepare_cluster, update_playtime};
use crate::api::game::{arguments, log, metadata};
use crate::api::java::{self, JavaError};
use crate::api::setting_profiles::dao::get_profile_by_name;
use crate::api::setting_profiles::get_global_profile;
use crate::error::LauncherResult;
use crate::store::credentials::MinecraftCredentials;
use crate::store::processes::Process;
use crate::store::{Dirs, State};
use crate::utils::io;
use onelauncher_entity::cluster_stage::ClusterStage;
use sea_orm::ActiveValue::Set;

/// Per-cluster lock map to avoid concurrent launch/prepare races.
static CLUSTER_LAUNCH_LOCKS: OnceLock<
	Mutex<std::collections::HashMap<i64, Arc<tokio::sync::Mutex<()>>>>,
> = OnceLock::new();

fn get_cluster_launch_lock(cluster_id: i64) -> Arc<tokio::sync::Mutex<()>> {
	let locks = CLUSTER_LAUNCH_LOCKS.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
	let mut map = locks.lock().unwrap();
	map.entry(cluster_id)
		.or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
		.clone()
}

#[tracing::instrument(skip_all)]
pub async fn launch_minecraft(
	cluster: &mut Cluster,
	creds: MinecraftCredentials,
	force: Option<bool>,
	search_for_java: Option<bool>,
) -> LauncherResult<Arc<RwLock<Process>>> {
	let cluster_launch_lock = get_cluster_launch_lock(cluster.id);
	let _launch_guard = cluster_launch_lock.lock().await;

	tracing::info!(
		cluster_id = %cluster.id,
		cluster_name = %cluster.name,
		mc_version = %cluster.mc_version,
		mc_loader = ?cluster.mc_loader,
		mc_loader_version = ?cluster.mc_loader_version,
		"launch requested"
	);

	if cluster.stage.is_downloading() {
		let state = State::get().await?;
		let mut has_active_prepare = state
			.ingress_processor
			.has_active_prepare_cluster(&cluster.name)
			.await;

		if has_active_prepare {
			tracing::info!(
				cluster_id = %cluster.id,
				cluster_name = %cluster.name,
				"cluster prepare is active; waiting for completion before launching"
			);

			let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(120);
			while tokio::time::Instant::now() < deadline {
				tokio::time::sleep(std::time::Duration::from_millis(300)).await;

				if let Some(fresh_cluster) =
					crate::api::cluster::dao::get_cluster_by_id(cluster.id).await?
				{
					*cluster = fresh_cluster;
				}

				has_active_prepare = state
					.ingress_processor
					.has_active_prepare_cluster(&cluster.name)
					.await;

				if !cluster.stage.is_downloading() || !has_active_prepare {
					break;
				}
			}
		}

		if has_active_prepare {
			return Err(ClusterError::ClusterDownloading.into());
		}

		tracing::warn!(
			cluster_id = %cluster.id,
			cluster_name = %cluster.name,
			"cluster was marked as Downloading without active prepare ingress; resetting stage"
		);

		crate::api::cluster::dao::update_cluster(cluster, async |mut active| {
			active.stage = Set(ClusterStage::NotReady);
			Ok(active)
		})
		.await?;
	}

	tracing::info!(cluster_id = %cluster.id, "preparing cluster before launch");
	prepare_cluster(cluster, force, search_for_java).await?;
	tracing::info!(cluster_id = %cluster.id, "cluster preparation completed");
	let global = get_global_profile().await;
	let settings = if let Some(name) = &cluster.setting_profile_name
		&& let Some(profile) = get_profile_by_name(name).await?
	{
		// Start from cluster profile so its values (e.g. mem_max) take precedence, then fill
		// in any None fields from global (merge uses overwrite_none).
		let mut s = profile;
		s.merge(global.clone());
		s
	} else {
		global
	};

	let state = State::get().await?;
	let dirs = Dirs::get().await?;

	if !state.settings.read().await.allow_parallel_running_clusters {
		let running = state.processes.has_running(cluster.id).await;
		if running {
			return Err(ClusterError::ClusterAlreadyRunning.into());
		}
	}

	let cwd = &io::canonicalize(dirs.clusters_dir().join(cluster.folder_name.clone()))?;

	let (version, updated) = {
		let metadata_store = state.metadata.read().await;
		let versions = &metadata_store.get_vanilla()?.versions;

		let version_index = versions
			.iter()
			.position(|it| it.id == cluster.mc_version)
			.ok_or_else(|| anyhow::anyhow!("invalid game version {}", cluster.mc_version))?;

		(
			versions[version_index].clone(),
			metadata::is_version_updated(version_index, versions),
		)
	};

	let loader_version = metadata::get_loader_version(
		&cluster.mc_version,
		cluster.mc_loader,
		cluster.mc_loader_version.as_deref(),
	)
	.await?;

	tracing::info!(
		cluster_id = %cluster.id,
		resolved_loader_version = ?loader_version.as_ref().map(|value| value.id.clone()),
		"resolved loader version for launch"
	);

	let version_name = loader_version.as_ref().map_or_else(
		|| version.id.clone(),
		|it| format!("{}-{}", version.id, it.id),
	);

	let version_info =
		metadata::download_version_info(&version, loader_version.as_ref(), None, None).await?;
	tracing::info!(
		cluster_id = %cluster.id,
		version_id = %version_info.id,
		"version metadata prepared for launch"
	);

	let Some(java) = java::get_recommended_java(&version_info, Some(&settings)).await? else {
		return Err(JavaError::MissingJava.into());
	};

	let java_path = PathBuf::from(&java.absolute_path);
	java::check_java_runtime(&java_path, false).await?;

	let client_jar_path = dirs
		.versions_dir()
		.join(&version_name)
		.join(format!("{version_name}.jar"));

	let mut command = if let Some(wrapper) = settings
		.hook_wrapper
		.as_deref()
		.and_then(|hook| (!hook.trim().is_empty()).then_some(hook))
	{
		let mut split = wrapper.split_whitespace();
		let mut cmd = Command::new(split.next().expect("hook wrapper should not be empty"));
		cmd.args(split);
		cmd.arg(&java.absolute_path);
		cmd
	} else {
		Command::new(&java.absolute_path)
	};

	let env_vars = settings.launch_env.iter().flat_map(|it| {
		it.split(' ').map(|it| {
			let mut split = it.split('=');
			let key = split.next().unwrap_or_default();
			let value = split.next().unwrap_or_default();
			(key.to_string(), value.to_string())
		})
	});

	#[cfg(target_os = "macos")]
	if std::env::var("CARGO").is_ok() {
		command.env_remove("DYLD_FALLBACK_LIBRARY_PATH");
	}

	command.env_remove("_JAVA_OPTIONS");
	command.envs(env_vars);

	let args = version_info.arguments.clone().unwrap_or_default();

	command
		.args(
			arguments::java_arguments(
				updated,
				args.get(&interfrost::api::minecraft::ArgumentType::Jvm)
					.map(Vec::as_slice),
				&dirs.natives_dir().join(&version_name),
				&dirs.libraries_dir(),
				&arguments::classpaths(
					&dirs.libraries_dir(),
					version_info.libraries.as_slice(),
					&client_jar_path,
					&java.arch,
					updated,
				)?,
				&version_name,
				settings.mem_max.unwrap_or(2048),
				settings.launch_args.clone().unwrap_or_default(),
				&java.arch,
			)?
			.into_iter(),
		)
		.arg(version_info.main_class.clone())
		.args(
			arguments::minecraft_arguments(
				updated,
				args.get(&interfrost::api::minecraft::ArgumentType::Game)
					.map(Vec::as_slice),
				version_info.minecraft_arguments.as_deref(),
				&creds,
				&version.id,
				&version_info.asset_index.id,
				cwd,
				&dirs.assets_dir(),
				version.type_,
				settings.res.unwrap_or_default(),
				&java.arch,
			)?
			.into_iter(),
		)
		.current_dir(cwd);

	// TODO: Make all options configurable?
	let mc_options = &[(
		String::from("fullscreen"),
		settings.force_fullscreen.unwrap_or(false).to_string(),
	)];

	if !mc_options.is_empty() {
		let options_path = cwd.join("options.txt");
		let mut options_string = if options_path.exists() {
			let bytes = io::read(&options_path).await?;
			let (cow, _, had_errors) = encoding_rs::UTF_8.decode(&bytes);
			if had_errors {
				let (cow, _, _) = encoding_rs::WINDOWS_1252.decode(&bytes);
				cow.into_owned()
			} else {
				cow.into_owned()
			}
		} else {
			String::new()
		};
		for (key, value) in mc_options {
			let re = regex::Regex::new(&format!(r"(?m)^{}:.*$", regex::escape(key)))?;
			if re.is_match(&options_string) {
				let replaced_string = re
					.replace_all(&options_string, &format!("{key}:{value}"))
					.to_string();
				options_string = replaced_string;
			} else {
				use std::fmt::Write;
				let _ = write!(options_string, "\n{key}:{value}");
			}
		}

		io::write(&options_path, options_string).await?;
	}

	update_playtime(cluster.id, 0).await?;

	let censors = log::create_censors(&creds);

	if state.ensure_rpc().await {
		let rpc = state.rpc.read().await;
		if let Some(discord) = rpc.as_ref() {
			discord
				.set_message(
					&format!("Playing {}", cluster.name),
					Some(Timestamps::new().start(Utc::now().timestamp())),
				)
				.await;
		}
	}

	tracing::info!(cluster_id = %cluster.id, "spawning minecraft process");

	state
		.processes
		.spawn(cluster.id, cwd.clone(), creds, censors, &settings, command)
		.await
}
