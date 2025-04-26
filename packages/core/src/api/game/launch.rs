use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use discord_rich_presence::activity::Timestamps;
use merge::Merge;
use onelauncher_entity::prelude::model::*;
use tokio::process::Command;
use tokio::sync::RwLock;

use crate::api::cluster::{prepare_cluster, update_playtime, ClusterError};
use crate::api::game::{arguments, log, metadata};
use crate::api::java::{self, JavaError};
use crate::api::setting_profiles::dao::get_profile_by_name;
use crate::api::setting_profiles::get_global_profile;
use crate::error::LauncherResult;
use crate::store::credentials::MinecraftCredentials;
use crate::store::processes::Process;
use crate::store::{Dirs, State};
use crate::utils::io;

#[tracing::instrument(skip_all)]
pub async fn launch_minecraft(
	cluster: &mut Cluster,
	creds: MinecraftCredentials,
	force: Option<bool>,
) -> LauncherResult<Arc<RwLock<Process>>> {
	if cluster.stage.is_downloading() {
		return Err(ClusterError::ClusterDownloading.into());
	}

	prepare_cluster(cluster, force).await?;

	let mut settings = get_global_profile().await;
	if let Some(name) = &cluster.setting_profile_name {
		if let Some(profile) = get_profile_by_name(name).await? {
			settings.merge(profile);
		}
	}

	let state = State::get().await?;
	let dirs = Dirs::get().await?;

	if !state.settings.read().await.allow_parallel_running_clusters {
		let running = state.processes.has_running(cluster.id).await;
		if running {
			return Err(ClusterError::ClusterAlreadyRunning.into());
		}
	}

	let cwd = &io::canonicalize(dirs.clusters_dir().join(cluster.folder_name.clone()))?;

	let metadata = state.metadata.read().await;
	let versions = &metadata.get_vanilla()?.versions;

	let version_index = versions
		.iter()
		.position(|it| it.id == cluster.mc_version)
		.ok_or_else(|| anyhow::anyhow!("invalid game version {}", cluster.mc_version))?;

	let version = versions[version_index].clone();
	let updated = version_index <= versions.iter().position(|x| x.id == "22w16a").unwrap_or(0);

	let loader_version = metadata::get_loader_version(
		&cluster.mc_version,
		cluster.mc_loader,
		cluster.mc_loader_version.as_deref(),
	)
	.await?;

	let version_name = loader_version
		.as_ref()
		.map_or(version.id.clone(), |it| {
			format!("{}-{}", version.id, it.id)
		});

	let version_info =
		metadata::download_version_info(&version, loader_version.as_ref(), None, None).await?;

	let Some(java) = java::get_recommended_java(&version_info, Some(&settings)).await? else {
		return Err(JavaError::MissingJava.into());
	};

	let java_path = PathBuf::from(&java.absolute_path);
	java::check_java_runtime(&java_path, false).await?;

	let client_jar_path = dirs
		.versions_dir()
		.join(&version_name)
		.join(format!("{version_name}.jar"));

	let mut command = if let Some(wrapper) = &settings.hook_wrapper {
		let mut cmd = Command::new(wrapper);
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
				args.get(&interpulse::api::minecraft::ArgumentType::Jvm)
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
				args.get(&interpulse::api::minecraft::ArgumentType::Game)
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
			io::read_to_string(&options_path).await?
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

	if let Some(discord) = &state.rpc {
		discord.set_message(
			&format!("Playing {}", cluster.name),
			Some(Timestamps::new().start(Utc::now().timestamp())),
		).await;
	}

	state.processes.spawn(cluster.id, cwd.clone(), creds, censors, &settings, command).await
}
