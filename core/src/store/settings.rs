//! Handles user-inputted settings and global values

use super::{Directories, JavaVersions};
use crate::utils::io;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The current [`Settings`] format version, changed for breaking changes.
/// If updated, a config file migration logic **NEEDS** to be implemented.
const CURRENT_FORMAT_VERSION: u32 = 1;

/// A global settings state for the launcher.
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Settings {
	/// A OneLauncher [`Theme`] managed by the core GUI.
	pub theme: Theme,
    /// Does not ask for confirmation when closing the OneLauncher GUI
    #[serde(default)]
    pub hide_close_prompt: bool,
    /// Disables animations in the OneLauncher GUI
    #[serde(default)]
    pub disable_animations: bool,
	/// A global fullscreen Minecraft state.
	#[serde(default)]
	pub force_fullscreen: bool,
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
	/// Enable/disable advanced rendering and window decorations.
	#[serde(default)]
	pub rendering: bool,
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
				.and_then(|it| serde_json::from_slice::<Settings>(&it).map_err(crate::Error::from));

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
				theme: Theme::Dark,
                hide_close_prompt: false,
                disable_animations: false,
				force_fullscreen: false,
				resolution: Resolution::default(),
				java_versions: JavaVersions::new(),
				memory: Memory::default(),
				init_hooks: InitHooks::default(),
				custom_env_args: Vec::new(),
				custom_java_args: Vec::new(),
				max_async_fetches: 10,
				max_async_io_operations: 10,
				version: CURRENT_FORMAT_VERSION,
				disable_analytics: false,
				disable_discord: false,
				debug_mode: false,
				config_dir: Directories::init_settings_dir(),
				hide_on_launch: false,
				rendering: true,
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

/// A OneLauncher theme managed by the GUI.
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
	/// A default Dark theme.
	Dark,
	/// A default Light theme.
	Light,
	/// OLED Dark Theme.
	Contrast,
	/// Cute and colorful theme.
	Cat,
}

/// Global memory settings across all clusters.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
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
#[cfg_attr(feature = "tauri", derive(specta::Type))]
pub struct Resolution(pub u16, pub u16);

impl Default for Resolution {
	fn default() -> Self {
		Self(854, 480)
	}
}

/// Global initialization hooks for all Minecraft clusters.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
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
