use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

use crate::{send_warning, utils::io, LauncherResult};

use super::Dirs;

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
	pub global_game_settings: SettingsProfile,
	pub allow_parallel_running_clusters: bool,
	pub enable_gamemode: bool,
	pub discord_enabled: bool,
	pub settings_version: u32,
}

impl Default for Settings {
	fn default() -> Self {
		Self {
			global_game_settings: SettingsProfile {
				name: "Global".into(),
				..Default::default()
			},
			allow_parallel_running_clusters: false,
			discord_enabled: false,
			enable_gamemode: false,
			settings_version: 2,
		}
	}
}

impl Settings {
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

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Default, Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SettingsProfile {
	pub name: String,
	pub java_id: Option<usize>,
	pub res_w: Option<usize>,
	pub res_h: Option<usize>,
	pub force_fullscreen: Option<bool>,
	pub mem_max: Option<usize>,
	pub launch_args: Option<String>,
	pub launch_env: Option<String>,
	pub hook_pre: Option<String>,
	pub hook_wrapper: Option<String>,
	pub hook_post: Option<String>,
}