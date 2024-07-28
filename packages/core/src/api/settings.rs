//! Settings Management

use crate::proxy::send::{init_ingress, send_ingress};
use crate::store::{Clusters, Directories, Settings};
use crate::State;
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;

/// gets a [`Settings`] object state.
#[tracing::instrument]
pub async fn get() -> crate::Result<Settings> {
	let state = State::get().await?;
	let settings = state.settings.read().await;
	Ok(settings.clone())
}

/// sets a [`Settings`] object state.
#[tracing::instrument]
pub async fn set(settings: Settings) -> crate::Result<()> {
	let state = State::get().await?;

	if settings.config_dir != state.settings.read().await.config_dir {
		return Err(anyhow::anyhow!("ccannot change config directory as a setting").into());
	}

	let (io, fetch) = async {
		let read = state.settings.read().await;
		(
			settings.max_async_io_operations != read.max_async_io_operations,
			settings.max_async_fetches != read.max_async_fetches,
		)
	}
	.await;

	let discord_rpc = {
		let read = state.settings.read().await;
		settings.disable_discord != read.disable_discord
	};

	{
		*state.settings.write().await = settings;
	}

	if discord_rpc {
		state.discord_rpc.clear(true).await?;
	}

	if io {
		state.reset_io_semaphore().await;
	}

	if fetch {
		state.reset_fetch_semaphore().await;
	}

	State::sync().await?;
	Ok(())
}

/// sets the config and caches directory, this can have side effects.
pub async fn set_directory(new: PathBuf) -> crate::Result<()> {
	tracing::trace!("changing config directory to {}", new.display());

	if !new.is_dir() {
		return Err(
			anyhow::anyhow!("new config directory {} is not a folder!", new.display()).into(),
		);
	}

	if !is_writable(new.clone()).await? {
		return Err(
			anyhow::anyhow!("new config directory {} is not writeable", new.display()).into(),
		);
	}

	let ingress = init_ingress(
		crate::IngressType::SyncConfig {
			new_path: new.clone(),
		},
		100.0,
		"syncing and changing configuration directory",
	)
	.await?;

	tracing::trace!("changing config directory, overriding state write access");
	let mut state_write = State::get_and_write().await?;
	let old = state_write.directories.config_dir.read().await.clone();

	tracing::trace!("resetting file watcher after changing config directory");
	let fs_watcher = crate::utils::watcher::initialize_watcher().await?;
	state_write.watcher = RwLock::new(fs_watcher);

	tracing::trace!("collecting all config files to be transfered");
	let mut cfg_entries = crate::utils::io::read_dir(&old).await?;
	let across_filesystems = is_different_fs(&old, &new);
	let mut entries = vec![];
	let mut cleanable = vec![];

	while let Some(e) = cfg_entries
		.next_entry()
		.await
		.map_err(|err| crate::utils::io::IOError::with_path(err, &old))?
	{
		let epath = e.path();
		if let Some(file_name) = epath.file_name() {
			if file_name == crate::constants::CLUSTERS_FOLDER
				|| file_name == crate::constants::METADATA_FOLDER
			{
				if across_filesystems {
					entries.extend(crate::package::import::sub(&epath).await?);
					cleanable.push(epath.clone());
				} else {
					entries.push(epath.clone());
				}
			}
		}
	}

	tracing::trace!("transferring configuration files");
	let io_semaphore = &state_write.io_semaphore;
	let nentries = entries.len() as f64;
	for epath in entries {
		let relpath = epath
			.strip_prefix(&old)
			.map_err(|err| anyhow::anyhow!("failed to strip prefix {err}"))?;
		let newpath = new.join(relpath);
		if across_filesystems {
			crate::utils::http::copy(&epath, &newpath, io_semaphore).await?;
		} else {
			crate::utils::io::rename(epath.clone(), newpath.clone()).await?;
		}

		send_ingress(&ingress, 80.0 * (1.0 / nentries), None).await?;
	}

	tracing::trace!("re-setting configuration settings");
	let settings = {
		let mut settings = state_write.settings.write().await;
		settings.config_dir = Some(new.clone());

		tracing::trace!("updating java paths and keys");
		for j in settings.java_versions.keys() {
			if let Some(java) = settings.java_versions.get_mut(&j) {
				if let Ok(relpath) = PathBuf::from(java.path.clone()).strip_prefix(&old) {
					java.path = new.join(relpath).to_string_lossy().to_string();
				}
			}
		}

		tracing::trace!("re-syncing settings file");
		settings
			.sync(&state_write.directories.settings_file())
			.await?;
		settings.clone()
	};

	tracing::trace!("re-initializing the config directory");
	state_write.directories = Directories::initalize(&settings)?;
	let cleanable_nentries = cleanable.len();
	if cleanable_nentries > 0 {
		tracing::trace!("deleting old config files");
	}
	for ce in cleanable {
		crate::utils::io::remove_dir_all(ce).await?;
		send_ingress(&ingress, 10.0 * (1.0 / cleanable_nentries as f64), None).await?;
	}

	tracing::trace!("re-resetting file watching system");
	let mut file_watcher = crate::utils::watcher::initialize_watcher().await?;
	state_write.clusters =
		RwLock::new(Clusters::initialize(&state_write.directories, &mut file_watcher).await?);
	state_write.watcher = RwLock::new(file_watcher);
	send_ingress(&ingress, 10.0, None).await?;

	tracing::info!(
		"successfully switched configuration directory to {}",
		new.display()
	);

	Ok(())
}

/// checks if two paths are on different data storage devices
fn is_different_fs(a: &Path, b: &Path) -> bool {
	let roota = a.components().next();
	let rootb = b.components().next();
	roota != rootb
}

/// checks if a [`PathBuf`] is writable or locked.
pub async fn is_writable(path: PathBuf) -> crate::Result<bool> {
	let tmp = path.join(".tmp");
	match crate::utils::io::write(tmp.clone(), "test").await {
		Ok(_) => {
			crate::utils::io::remove_file(tmp).await?;
			Ok(true)
		}
		Err(e) => {
			tracing::error!("failed to write to config directory: {}", e);
			Ok(false)
		}
	}
}
