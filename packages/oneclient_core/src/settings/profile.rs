use serde::{Deserialize, Serialize};

use oneclient_db::models::SettingProfileRow;

pub const GLOBAL_PROFILE_NAME: &str = "Global";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameSettingsProfile {
	pub name: String,
	pub java_path: Option<String>,
	pub resolution: Option<Resolution>,
	pub force_fullscreen: Option<bool>,
	pub mem_max: Option<u32>,
	pub launch_args: Option<String>,
	pub launch_env: Option<String>,
	pub hook_pre: Option<String>,
	pub hook_wrapper: Option<String>,
	pub hook_post: Option<String>,
	pub os_extra: Option<SettingsOsExtra>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Resolution {
	pub width: u32,
	pub height: u32,
}

impl Default for Resolution {
	fn default() -> Self {
		Self::new(854, 480)
	}
}

impl Resolution {
	pub const fn new(width: u32, height: u32) -> Self {
		Self { width, height }
	}
}

cfg_select! {
	target_os = "windows" => {
		#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
		pub struct SettingsOsExtra {}
	}
	target_os = "macos" => {
		#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
		pub struct SettingsOsExtra {}
	}
	not(any(target_os = "windows", target_os = "macos")) => {
		#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
		pub struct SettingsOsExtra {
			pub enable_gamemode: Option<bool>,
		}

		impl Default for SettingsOsExtra {
			fn default() -> Self {
				Self {
					enable_gamemode: Some(true),
				}
			}
		}
	}
}

impl GameSettingsProfile {
	pub fn default_global_profile() -> Self {
		Self {
			name: GLOBAL_PROFILE_NAME.into(),
			java_path: None,
			resolution: None,
			force_fullscreen: Some(false),
			mem_max: Some(4096),
			launch_args: None,
			launch_env: None,
			hook_pre: None,
			hook_wrapper: None,
			hook_post: None,
			os_extra: Some(SettingsOsExtra::default()),
		}
	}

	pub fn is_global(&self) -> bool {
		self.name == GLOBAL_PROFILE_NAME
	}

	pub fn merge_global(&mut self, global: &Self) {
		if self.java_path.is_none() {
			self.java_path = global.java_path.clone();
		}
		if self.resolution.is_none() {
			self.resolution = global.resolution;
		}
		if self.force_fullscreen.is_none() {
			self.force_fullscreen = global.force_fullscreen;
		}
		if self.mem_max.is_none() {
			self.mem_max = global.mem_max;
		}
		if self.launch_args.is_none() {
			self.launch_args = global.launch_args.clone();
		}
		if self.launch_env.is_none() {
			self.launch_env = global.launch_env.clone();
		}
		if self.hook_pre.is_none() {
			self.hook_pre = global.hook_pre.clone();
		}
		if self.hook_wrapper.is_none() {
			self.hook_wrapper = global.hook_wrapper.clone();
		}
		if self.hook_post.is_none() {
			self.hook_post = global.hook_post.clone();
		}
		if self.os_extra.is_none() {
			self.os_extra = global.os_extra.clone();
		}
	}

	pub fn from_row(row: SettingProfileRow) -> crate::LauncherResult<Self> {
		Ok(Self {
			name: row.name,
			java_path: row.java_path,
			resolution: row
				.resolution
				.map(|json| serde_json::from_str(&json))
				.transpose()?,
			force_fullscreen: row.force_fullscreen.map(|v| v != 0),
			mem_max: row.mem_max.map(|v| v as u32),
			launch_args: row.launch_args,
			launch_env: row.launch_env,
			hook_pre: row.hook_pre,
			hook_wrapper: row.hook_wrapper,
			hook_post: row.hook_post,
			os_extra: row
				.os_extra
				.map(|json| serde_json::from_str(&json))
				.transpose()?,
		})
	}

	pub fn into_row(&self) -> crate::LauncherResult<SettingProfileRow> {
		Ok(SettingProfileRow {
			name: self.name.clone(),
			java_path: self.java_path.clone(),
			resolution: self
				.resolution
				.map(|res| serde_json::to_string(&res))
				.transpose()?,
			force_fullscreen: self.force_fullscreen.map(i64::from),
			mem_max: self.mem_max.map(i64::from),
			launch_args: self.launch_args.clone(),
			launch_env: self.launch_env.clone(),
			hook_pre: self.hook_pre.clone(),
			hook_wrapper: self.hook_wrapper.clone(),
			hook_post: self.hook_post.clone(),
			os_extra: self
				.os_extra
				.as_ref()
				.map(serde_json::to_string)
				.transpose()?,
		})
	}
}
