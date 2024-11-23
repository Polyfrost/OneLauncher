//! Handles user-inputted settings and global values

use super::{Directories, JavaVersions};
use crate::constants::CURRENT_SETTINGS_FORMAT_VERSION;
use onelauncher_utils::io;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A global settings state for the launcher.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Settings {
	/// A `OneLauncher` theme managed by the `OneLauncher` GUI.
	#[serde(default = "theme_default")]
	pub theme: String,
	/// A global browser list view for the `OneLauncher` GUI.
	#[serde(default)]
	pub browser_list_view: BrowserListView,
	/// Does not ask for confirmation when closing the `OneLauncher` GUI
	#[serde(default)]
	pub hide_close_prompt: bool,
	/// Disables animations in the `OneLauncher` GUI
	#[serde(default)]
	pub disable_animations: bool,
	/// A global fullscreen Minecraft state.
	#[serde(default)]
	pub force_fullscreen: bool,
	/// Whether to allow launching the same cluster under the same account.
	#[serde(default)]
	pub allow_parallel_running_clusters: bool,
	/// Whether to launch Feral Gamemode on Linux systems.
	#[serde(default)]
	pub enable_gamemode: bool,
	/// A global default [`Resolution`] for Minecraft.
	pub resolution: Resolution,
	/// A global [`JavaVersions`] list and default version.
	pub java_versions: JavaVersions,
	/// A global [`Memory`] settings store for Java memory settings.
	pub memory: Memory,
	/// Global and default initialization hooks .
	pub init_hooks: InitHooks,
	/// Global and default custom Java arguments.
	pub custom_java_args: Vec<String>,
	/// Global and default custom environment variables.
	pub custom_env_args: Vec<(String, String)>,
	/// Global and default maximum [`Semaphore`] I/O operations.
	pub max_async_io_operations: usize,
	/// Global and default maximum [`Semaphore`] HTTP operations.
	pub max_async_fetches: usize,
	/// The [`CURRENT_FORMAT_VERSION`] of this settings file.
	pub version: u32,
	/// Whether or not to disable Discord IPC.
	#[serde(default)]
	pub disable_discord: bool,
	/// Whether or not to enable a debug mode in the launcher.
	#[serde(default)]
	pub debug_mode: bool,
	/// Whether or not to disable Plausible and Sentry analytics.
	#[serde(default)]
	pub disable_analytics: bool,
	/// The core global config directory stored as a [`PathBuf`].
	#[serde(default = "Directories::init_settings_dir")]
	pub config_dir: Option<PathBuf>,
	/// Whether or not to minimize the launcher upon a game launch.
	#[serde(default)]
	pub hide_on_launch: bool,
	/// Enable/disable custom window decorations.
	#[serde(default)]
	pub custom_frame: bool,
	/// Completed onboarding.
	#[serde(default)]
	pub onboarding_completed: bool,
}

fn theme_default() -> String {
	"dark".to_string()
}

impl Settings {
	/// Initializes the global settings state.
	#[tracing::instrument]
	pub async fn initialize(file: &Path) -> crate::Result<Self> {
		let mut recovered_corruption = false;

		let settings = if file.exists() {
			let read_settings = io::read(&file)
				.await
				.map_err(|err| anyhow::anyhow!("error reading settings file: {0}", err).into())
				.and_then(|it| serde_json::from_slice::<Self>(&it).map_err(crate::Error::from));

			if let Err(ref err) = read_settings {
				tracing::error!("failed to load settings file: {err}.");
				let backup = file.with_extension("json.bak");
				tracing::error!(
					"corrupted settings will be backed up as {}, and a new one will be created",
					backup.display()
				);
				let _ = io::rename(file, backup).await;
				recovered_corruption = true;
			}

			read_settings.ok()
		} else {
			None
		};

		if let Some(settings) = settings {
			Ok(settings)
		} else {
			let settings = Self {
				theme: "dark".to_string(),
				browser_list_view: BrowserListView::Grid,
				hide_close_prompt: true,
				disable_animations: false,
				force_fullscreen: false,
				allow_parallel_running_clusters: false,
				#[cfg(target_os = "linux")]
				enable_gamemode: true,
				#[cfg(not(target_os = "linux"))]
				enable_gamemode: false,
				resolution: Resolution::default(),
				java_versions: JavaVersions::new(),
				memory: Memory::default(),
				init_hooks: InitHooks::default(),
				custom_env_args: Vec::new(),
				custom_java_args: Vec::new(),
				max_async_fetches: 10,
				max_async_io_operations: 10,
				version: CURRENT_SETTINGS_FORMAT_VERSION,
				disable_analytics: false,
				disable_discord: false,
				debug_mode: false,
				config_dir: Directories::init_settings_dir(),
				hide_on_launch: false,
				custom_frame: true,
				onboarding_completed: false,
			};

			if recovered_corruption {
				settings.sync(file).await?;
			}

			Ok(settings)
		}
	}

	/// Synchronizes the current settings from a file.
	#[tracing::instrument(skip(self))]
	pub async fn sync(&self, to: &Path) -> crate::Result<()> {
		io::write(to, serde_json::to_vec(self)?).await?;
		Ok(())
	}
}

/// `OneLauncher` browser list type.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserListView {
	#[default]
	Grid,
	List,
}

/// Global memory settings across all clusters.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Memory {
	/// Maximum amount of Java memory available globally.
	pub maximum: u32,
	/// Minimum amount of Java memory available globally.
	pub minimum: u32,
}

impl Default for Memory {
	fn default() -> Self {
		Self {
			maximum: 2048,
			minimum: 256,
		}
	}
}

/// Global Minecraft resolution.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Resolution(pub u16, pub u16);

impl Default for Resolution {
	fn default() -> Self {
		Self(854, 480)
	}
}

/// Global initialization hooks for all Minecraft clusters.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct InitHooks {
	/// Pre-launch hook.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub pre: Option<String>,
	/// Wrapper hook for the runtime of the game instance.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub wrapper: Option<String>,
	/// Post-launch hook.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub post: Option<String>,
}
