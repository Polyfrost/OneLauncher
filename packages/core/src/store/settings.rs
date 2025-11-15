use onelauncher_entity::setting_profiles;
use serde::{Deserialize, Serialize};

use crate::utils::io;
use crate::{LauncherResult, send_warning};

use super::Dirs;

#[onelauncher_macro::specta]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
	pub global_game_settings: setting_profiles::Model,
	pub allow_parallel_running_clusters: bool,
	pub enable_gamemode: bool,
	pub discord_enabled: bool,
	pub seen_onboarding: bool,
	pub max_concurrent_requests: usize,
	pub settings_version: u32,
	pub native_window_frame: bool,

	#[cfg(feature = "tauri")]
	pub show_tanstack_dev_tools: bool,
}

impl Default for Settings {
	fn default() -> Self {
		Self {
			global_game_settings: setting_profiles::Model::default_global_profile(),
			allow_parallel_running_clusters: false,
			discord_enabled: false,
			seen_onboarding: false,
			enable_gamemode: false,
			max_concurrent_requests: 25,
			settings_version: 1,
			native_window_frame: false,

			#[cfg(feature = "tauri")]
			show_tanstack_dev_tools: tauri::is_dev(),
		}
	}
}

impl Settings {
	#[tracing::instrument]
	pub async fn new() -> Self {
		match Self::read().await {
			Ok(settings) => settings,
			Err(err) => {
				send_warning!("Failed to read settings file: {}", err);
				Self::default()
			}
		}
	}

	async fn read() -> LauncherResult<Self> {
		let path = Dirs::get_settings_file().await?;
		let data = io::read(path).await?;
		Ok(serde_json::from_slice(&data)?)
	}

	pub async fn save(&self) -> LauncherResult<()> {
		let path = Dirs::get_settings_file().await?;
		let data = serde_json::to_string_pretty(self)?;
		io::write(path, data).await?;

		Ok(())
	}
}
