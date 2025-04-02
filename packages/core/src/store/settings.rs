use onelauncher_entity::setting_profiles;
use serde::{Deserialize, Serialize};

use crate::{constants, send_warning, utils::io, LauncherResult};

use super::Dirs;

#[onelauncher_macro::specta]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
	pub global_game_settings: setting_profiles::Model,
	pub allow_parallel_running_clusters: bool,
	pub enable_gamemode: bool,
	pub discord_enabled: bool,
	pub max_concurrent_requests: usize,
	pub settings_version: u32,
}

impl Default for Settings {
	fn default() -> Self {
		Self {
			global_game_settings: default_global_game_settings(),
			allow_parallel_running_clusters: false,
			discord_enabled: false,
			enable_gamemode: false,
			max_concurrent_requests: 25,
			settings_version: constants::CURRENT_SETTINGS_FORMAT_VERSION,
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
			},
		}
	}

	async fn read() -> LauncherResult<Self> {
		let path = Dirs::get().await?.settings_file();
		let data = io::read(path).await?;
		Ok(serde_json::from_slice(&data)?)
	}

	pub async fn save(&self) -> LauncherResult<()> {
		let path = Dirs::get().await?.settings_file();
		let data = serde_json::to_string_pretty(self)?;
		io::write(path, data).await?;

		Ok(())
	}
}

fn default_global_game_settings() -> setting_profiles::Model {
	setting_profiles::Model {
		name: "Global".into(),
		force_fullscreen: Some(false),
		hook_post: None,
		hook_pre: None,
		hook_wrapper: None,
		java_id: None,
		launch_args: None,
		launch_env: None,
		mem_max: Some(3072),
		res_h: None,
		res_w: None,
	}
}