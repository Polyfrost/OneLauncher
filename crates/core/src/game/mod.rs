//! **OneLauncher Game**
//!
//! Manages, installs, and launches the core Minecraft game.

use crate::prelude::JavaVersion;
use crate::processor;
use crate::proxy::send::{init_or_edit_ingress, send_ingress};
use crate::proxy::{IngressId, IngressType};
use crate::store::{
	self as st, Cluster, ClusterStage, MinecraftCredentials, ProcessorChild, State,
};
use crate::utils::io::{self, IOError};

use chrono::Utc;
use interpulse as ip;
use interpulse::api::minecraft::{RuleAction, VersionInfo};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Command;
use uuid::Uuid;

mod arguments;
pub mod metadata;

macro_rules! data_entry {
    ($dest:expr; $($name:literal: client => $client:expr, server => $server:expr;)+) => {
        $(std::collections::HashMap::insert(
            $dest,
            String::from($name),
            interpulse::api::modded::SidedDataEntry {
                client: String::from($client),
                server: String::from($server),
            },
        );)+
    }
}

#[tracing::instrument(skip(cluster))]
#[onelauncher_debug::debugger]
pub async fn install_minecraft(
	cluster: &Cluster,
	old_ingress: Option<IngressId>,
	repair: bool,
) -> crate::Result<()> {
	let should_sync = old_ingress.is_some();
	let ingress = init_or_edit_ingress(
		old_ingress,
		IngressType::DownloadLoader {
			cluster_name: cluster.meta.name.clone(),
			cluster_path: cluster.get_full_path().await?,
		},
		100.0,
		"downloading minecraft loader",
	)
	.await?;

	crate::api::cluster::edit(&cluster.cluster_path(), |clus| {
		clus.stage = ClusterStage::Downloading;

		async { Ok(()) }
	})
	.await?;

	State::sync().await?;

	if should_sync {
		Cluster::sync_packages(cluster.cluster_path(), true);
	}

	let state = State::get().await?;
	let instance_path = &io::canonicalize(cluster.get_full_path().await?)?;
	let metadata = state.metadata.read().await;

	let version_manifest = metadata.minecraft.to_owned();
    let versions = version_manifest
        .ok_or(anyhow::anyhow!(
            "couldn't get minecraft manifest"
        ))?
        .versions;

	let version_idx = versions
		.iter()
		.position(|g| g.id == cluster.meta.mc_version)
		.ok_or(anyhow::anyhow!(
			"invalid minecraft game version {}",
			cluster.meta.mc_version
		))?;

	let version = &versions[version_idx];
	let updated = version_idx
		<= versions
			.iter()
			.position(|g| g.id == "22w16a")
			.unwrap_or(0); // LWJGL patching
	let version_jar = cluster
		.meta
		.loader_version
		.as_ref()
		.map_or(version.id.clone(), |g| {
			format!("{}-{}", version.id.clone(), g.id.clone())
		});
	let mut version_info = metadata::download_version_info(
		&state,
		version,
		cluster.meta.loader_version.as_ref(),
		Some(repair),
		Some(&ingress),
	)
	.await?;
	// TODO: java checking and installation
	let key = version_info
		.java_version
		.as_ref()
		.map(|it| it.major_version)
		.unwrap_or(8);
	let (java_version, set_java) =
		if let Some(java_version) = java_version_from_cluster(cluster, &version_info).await? {
			(std::path::PathBuf::from(java_version.path), false)
		} else {
			(crate::api::java::install_java(key).await?, true)
		};

	let java_version = crate::api::java::check_java(java_version.clone())
		.await?
		.ok_or_else(|| anyhow::anyhow!("java path validation failed: {:?}", java_version))?;

	if set_java {
		{
			let mut settings = state.settings.write().await;
			settings
				.java_versions
				.insert(format!("JAVA_{key}"), java_version.clone());
		}
		State::sync().await?;
	}

	metadata::download_minecraft(
		&state,
		&version_info,
		&ingress,
		&java_version.arch,
		repair,
		updated,
	)
	.await?;

	if let Some(processors) = &version_info.processors {
		let client = state
			.directories
			.version_dir(&version_jar)
			.await
			.join(format!("{version_jar}.jar"));
		let libraries = state.directories.libraries_dir().await;
		if let Some(ref mut data) = version_info.data {
			data_entry! {
				data;
				"SIDE":
					client => "client",
					server => "";
				"MINECRAFT_JAR":
					client => client.to_string_lossy(),
					server => "";
				"MINECRAFT_VERSION":
					client => cluster.meta.mc_version.clone(),
					server => "";
				"ROOT":
					client => instance_path.to_string_lossy(),
					server => "";
				"LIBRARY_DIR":
					client => libraries.to_string_lossy(),
					server => "";
			}

			send_ingress(&ingress, 0.0, Some("running forge processors")).await?;
			let total_length = processors.len();
			for (index, processor) in processors.iter().enumerate() {
				if let Some(sides) = &processor.sides {
					if !sides.contains(&String::from("client")) {
						continue;
					}
				}

				let cp = ref_owned!(cp = processor.classpath.clone() => {
					cp.push(processor.jar.clone())
				});

				let child = Command::new(&java_version.path)
					.arg("-cp")
					.arg(arguments::get_classpath_library(
						&libraries,
						&cp,
						&java_version.arch,
					)?)
					.arg(
						arguments::main_class(arguments::get_library(
							&libraries,
							&processor.jar,
							false,
						)?)
						.await?
						.ok_or_else(|| {
							anyhow::anyhow!(
								"failed to find processor main class for {}",
								processor.jar
							)
						})?,
					)
					.args(arguments::processor_arguments(
						&libraries,
						&processor.args,
						data,
					)?)
					.output()
					.await
					.map_err(|e| IOError::with_path(e, &java_version.path))
					.map_err(|err| anyhow::anyhow!("failed to run processor: {err}"))?;

				if !child.status.success() {
					return Err(anyhow::anyhow!(
						"error occured while running processor: {}",
						String::from_utf8_lossy(&child.stderr)
					)
					.into());
				}

				send_ingress(
					&ingress,
					30.0 / total_length as f64,
					Some(&format!(
						"running forge processor {}/{}",
						index, total_length
					)),
				)
				.await?;
			}
		}
	}

	crate::api::cluster::edit(&cluster.cluster_path(), |clus| {
		clus.stage = ClusterStage::Installed;

		async { Ok(()) }
	})
	.await?;
	State::sync().await?;
	send_ingress(&ingress, 1.0, Some("installed minecraft successfully")).await?;

	Ok(())
}

#[tracing::instrument(skip_all)]
#[onelauncher_debug::debugger]
#[allow(clippy::too_many_arguments)]
pub async fn launch_minecraft(
	cluster: &Cluster,
	java_args: &[String],
	env_args: &[(String, String)],
	mc_options: &[(String, String)],
	post_hook: Option<String>,
	credentials: &MinecraftCredentials,
	resolution: &st::Resolution,
	memory: &st::Memory,
	wrapper: &Option<String>,
) -> crate::Result<Arc<tokio::sync::RwLock<ProcessorChild>>> {
	if cluster.stage == ClusterStage::PackDownloading || cluster.stage == ClusterStage::Downloading
	{
		return Err(anyhow::anyhow!("cluster is still downloading").into());
	}

	if cluster.stage != ClusterStage::Installed {
		install_minecraft(cluster, None, false).await?;
	}

	let state = State::get().await?;
	let metadata = state.metadata.read().await;
	let instance_path = cluster.get_full_path().await?;
	let instance_path = &io::canonicalize(instance_path)?;

	let version_manifest = metadata.minecraft.to_owned();
    let versions = version_manifest
        .ok_or(anyhow::anyhow!("couldn't get minecraft manifest"))?
        .versions;

	let version_index = versions
		.iter()
		.position(|it| it.id == cluster.meta.mc_version)
		.ok_or(anyhow::anyhow!(
			"invalid game version {}",
			cluster.meta.mc_version
		))?;

	let version = &versions[version_index];
	let updated = version_index
		<= versions
			.iter()
			.position(|x| x.id == "22w16a")
			.unwrap_or(0);
	let version_jar = cluster
		.meta
		.loader_version
		.as_ref()
		.map_or(version.id.clone(), |it| {
			format!("{}-{}", version.id.clone(), it.id.clone())
		});
	let version_info = metadata::download_version_info(
		&state,
		version,
		cluster.meta.loader_version.as_ref(),
		None,
		None,
	)
	.await?;
	let java_version = java_version_from_cluster(cluster, &version_info)
		.await?
		.ok_or_else(|| anyhow::anyhow!("missing java installation"))?;
	let java_version = crate::api::java::check_java(java_version.path.clone().into())
		.await?
		.ok_or_else(|| anyhow::anyhow!("java path invalid: {}", java_version.path))?;
	let client_path = state
		.directories
		.version_dir(&version_jar)
		.await
		.join(format!("{version_jar}.jar"));
	let args = version_info.arguments.clone().unwrap_or_default();
	let mut command = match wrapper {
		Some(hook) => ref_owned!(it = Command::new(hook) => {it.arg(&java_version.path)}),
		None => Command::new(&java_version.path),
	};
	let env_args = Vec::from(env_args);
	let existing = processor::get_uuids_by_cluster_path(cluster.cluster_path()).await?;
	if let Some(uuid) = existing.first() {
		return Err(anyhow::anyhow!(
			"cluster {} is already running ({uuid})",
			cluster.cluster_path()
		)
		.into());
	}

	command
		.args(
			arguments::java_arguments(
				args.get(&ip::api::minecraft::ArgumentType::Jvm)
					.map(|x| x.as_slice()),
				&state.directories.version_natives_dir(&version_jar).await,
				&state.directories.libraries_dir().await,
				&arguments::classpaths(
					&state.directories.libraries_dir().await,
					version_info.libraries.as_slice(),
					&client_path,
					&java_version.arch,
					updated,
				)?,
				&version_jar,
				*memory,
				Vec::from(java_args),
				&java_version.arch,
			)?
			.into_iter()
			.collect::<Vec<_>>(),
		)
		.arg(version_info.main_class.clone())
		.args(
			arguments::minecraft_arguments(
				args.get(&ip::api::minecraft::ArgumentType::Game)
					.map(|a| a.as_slice()),
				version_info.minecraft_arguments.as_deref(),
				credentials,
				&version.id,
				&version_info.asset_index.id,
				instance_path,
				&state.directories.assets_dir().await,
				&version.type_,
				*resolution,
				&java_version.arch,
			)?
			.into_iter()
			.collect::<Vec<_>>(),
		)
		.current_dir(instance_path.clone());

	// when cargo makes the DYLD_LIBRARY_PATH it breaks Minecraft
	#[cfg(target_os = "macos")]
	if std::env::var("CARGO").is_ok() {
		command.env_remove("DYLD_FALLBACK_LIBRARY_PATH");
	}

	// remove preexisting Java options, as they should be set in the cluster settings.
	command.env_remove("_JAVA_OPTIONS");
	command.envs(env_args);

	// overrides `options.txt` with our settings: i can't believe it's not yaml
	use regex::Regex;

	if !mc_options.is_empty() {
		let options_path = instance_path.join("options.txt");
		let mut options_string = String::new();
		if options_path.exists() {
			options_string = io::read_to_string(&options_path).await?;
		}
		for (key, value) in mc_options {
			let re = Regex::new(&format!(r"(?m)^{}:.*$", regex::escape(key)))?;
			if !re.is_match(&options_string) {
				options_string.push_str(&format!("\n{}:{}", key, value));
			} else {
				let replaced_string = re
					.replace_all(&options_string, &format!("{}:{}", key, value))
					.to_string();
				options_string = replaced_string;
			}
		}

		io::write(&options_path, options_string).await?;
	}

	crate::api::cluster::edit(&cluster.cluster_path(), |clust| {
		clust.meta.played_at = Some(Utc::now());

		async { Ok(()) }
	})
	.await?;
	State::sync().await?;

	let mut censors = HashMap::new();
	let username = whoami::username();
	let realname = whoami::realname();
	censors.insert(format!("/{}/", username), "/{ENV_USERNAME}/".to_string());
	censors.insert(
		format!("\\{}\\", username),
		"\\{ENV_USERNAME}\\".to_string(),
	);
	censors.insert(format!("/{}/", realname), "/{ENV_REALNAME}/".to_string());
	censors.insert(
		format!("\\{}\\", realname),
		"\\{ENV_REALNAME}\\".to_string(),
	);
	censors.insert(
		credentials.access_token.clone(),
		"{MC_ACCESS_TOKEN}".to_string(),
	);
	censors.insert(credentials.username.clone(), "{MC_USERNAME}".to_string());
	censors.insert(
		credentials.id.as_simple().to_string(),
		"{MC_UUID}".to_string(),
	);
	censors.insert(
		credentials.id.as_hyphenated().to_string(),
		"{MC_UUID}".to_string(),
	);

	#[cfg(feature = "tauri")]
	{
		use crate::ProxyState;
		let window = ProxyState::get_main_window().await?;
		let settings = state.settings.read().await;
		if settings.hide_on_launch {
			window.minimize()?;
		}
	}

	if !*state.offline.read().await {
		let _ = state
			.discord_rpc
			.set_activity(&format!("Playing {}", cluster.meta.name), true)
			.await;
	}

	let mut state_processor = state.processor.write().await;
	state_processor
		.insert_process(
			Uuid::new_v4(),
			cluster.cluster_path(),
			command,
			post_hook,
			censors,
		)
		.await
}

#[tracing::instrument]
pub fn rules(rules: &[ip::api::minecraft::Rule], java_version: &str, updated: bool) -> bool {
	let mut rule = rules
		.iter()
		.map(|r| rule(r, java_version, updated))
		.collect::<Vec<Option<bool>>>();
	if rules
		.iter()
		.all(|r| matches!(r.action, RuleAction::Disallow))
	{
		rule.push(Some(true))
	}

	!(rule.iter().any(|r| r == &Some(false)) || rule.iter().all(|r| r.is_none()))
}

#[tracing::instrument]
pub fn rule(rule: &ip::api::minecraft::Rule, java_version: &str, updated: bool) -> Option<bool> {
	use ip::api::minecraft::{Rule, RuleAction};

	let result = match rule {
		Rule {
			os: Some(ref os), ..
		} => crate::utils::platform::os_rule(os, java_version, updated),
		Rule {
			features: Some(ref features),
			..
		} => {
			!features.is_demo_user.unwrap_or(true)
				|| features.has_custom_resolution.unwrap_or(false)
				|| !features.has_quick_plays_support.unwrap_or(true)
				|| !features.is_quick_play_multiplayer.unwrap_or(true)
				|| !features.is_quick_play_realms.unwrap_or(true)
				|| !features.is_quick_play_singleplayer.unwrap_or(true)
		}
		_ => return Some(true),
	};

	match rule.action {
		RuleAction::Allow => {
			if result {
				Some(true)
			} else {
				None
			}
		}
		RuleAction::Disallow => {
			if result {
				Some(false)
			} else {
				None
			}
		}
	}
}

pub async fn java_version_from_cluster(
	cluster: &Cluster,
	version_info: &VersionInfo,
) -> crate::Result<Option<JavaVersion>> {
	if let Some(java) = cluster.java.clone().and_then(|x| x.custom_version) {
		Ok(Some(java))
	} else {
		let key = version_info
			.java_version
			.as_ref()
			.map(|x| x.major_version)
			.unwrap_or(8);

		let state = State::get().await?;
		let settings = state.settings.read().await;

		if let Some(j) = settings.java_versions.get(&format!("JAVA_{key}")) {
			return Ok(Some(j.clone()));
		}

		Ok(None)
	}
}
