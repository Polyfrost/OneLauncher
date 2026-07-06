use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::profile::GameSettingsProfile;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ViewLayout {
	#[default]
	Grid,
	List,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
#[serde(default)]
pub struct ViewState {
	pub layout: ViewLayout,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub sort: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct LauncherSettings {
	pub settings_version: u32,
	pub log_debug: bool,
	pub auto_update: bool,
	pub enable_gamemode: bool,
	pub discord_enabled: bool,
	pub microsoft_login_use_browser: bool,
	pub max_concurrent_requests: usize,
	pub global_game_settings: GameSettingsProfile,
	pub allow_parallel_running_clusters: bool,
	pub dynamic_background_enabled: bool,
	pub view_states: BTreeMap<String, ViewState>,
	pub seen_onboarding: bool,
	pub seen_versions: Vec<String>,
	pub modrinth_api_key: Option<String>,
	pub curseforge_api_key: Option<String>,
	pub custom_api_endpoint: Option<String>,
	pub custom_meta_url_base: Option<String>,
}

impl LauncherSettings {
	pub fn view_state(&self, key: &str) -> ViewState {
		self.view_states.get(key).cloned().unwrap_or_default()
	}

	pub fn set_view_state(&mut self, key: impl Into<String>, state: ViewState) {
		self.view_states.insert(key.into(), state);
	}
}

impl Default for LauncherSettings {
	fn default() -> Self {
		Self {
			settings_version: 1,
			log_debug: false,
			auto_update: true,
			discord_enabled: true,
			microsoft_login_use_browser: true,
			enable_gamemode: false,
			max_concurrent_requests: 25,
			global_game_settings: GameSettingsProfile::default_global_profile(),
			allow_parallel_running_clusters: false,
			dynamic_background_enabled: true,
			view_states: BTreeMap::new(),
			seen_onboarding: false,
			seen_versions: Vec::new(),
			modrinth_api_key: None,
			curseforge_api_key: None,
			custom_api_endpoint: None,
			custom_meta_url_base: None,
		}
	}
}
