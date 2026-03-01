use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Runtime;
use tauri_plugin_os::{arch, family, locale, platform, type_, version};
use tokio::fs;

#[onelauncher_macro::specta]
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DebugInfoData {
	pub in_dev: bool,
	pub platform: String,
	pub arch: String,
	pub family: String,
	pub locale: String,
	pub os_type: String,
	pub os_version: String,
	pub os_distro: String,
	pub commit_hash: String,
	pub build_timestamp: String,
	pub build_version: String,
}

#[onelauncher_macro::specta]
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct DebugInfoParsedLine {
	pub title: String,
	pub value: String,
}

impl DebugInfoParsedLine {
	pub fn new<T: Into<String>, U: Into<String>>(title: T, value: U) -> Self {
		Self {
			title: title.into(),
			value: value.into(),
		}
	}
}

#[taurpc::procedures(path = "debug")]
pub trait TauriLauncherDebugApi {
	#[taurpc(alias = "openDevTools")]
	async fn open_dev_tools<R: Runtime>(webview_window: tauri::WebviewWindow<R>);

	#[taurpc(alias = "isInDev")]
	async fn is_in_dev() -> bool;

	#[taurpc(alias = "getArch")]
	async fn get_arch() -> String;

	#[taurpc(alias = "getFamily")]
	async fn get_family() -> String;

	#[taurpc(alias = "getLocale")]
	async fn get_locale() -> String;

	#[taurpc(alias = "getType")]
	async fn get_type() -> String;

	#[taurpc(alias = "getPlatform")]
	async fn get_platform() -> String;

	#[taurpc(alias = "getOsVersion")]
	async fn get_version() -> String;

	#[taurpc(alias = "getOsDistro")]
	async fn get_distro() -> String;

	#[taurpc(alias = "getGitCommitHash")]
	async fn get_git_commit_hash() -> String;

	#[taurpc(alias = "getBuildTimestamp")]
	async fn get_build_timestamp() -> String;

	#[taurpc(alias = "getPackageVersion")]
	async fn get_package_version() -> String;

	#[taurpc(alias = "getFullDebugInfo")]
	async fn get_full_debug_info() -> DebugInfoData;

	#[taurpc(alias = "getFullDebugInfoParsed")]
	async fn get_full_debug_info_parsed() -> Vec<DebugInfoParsedLine>;

	#[taurpc(alias = "getFullDebugInfoParsedString")]
	async fn get_full_debug_info_parsed_string() -> String;
}

#[taurpc::ipc_type]
pub struct TauriLauncherDebugApiImpl;

#[taurpc::resolvers]
impl TauriLauncherDebugApi for TauriLauncherDebugApiImpl {
	async fn open_dev_tools<R: Runtime>(self, webview_window: tauri::WebviewWindow<R>) {
		webview_window.open_devtools();
	}

	async fn is_in_dev(self) -> bool {
		tauri::is_dev()
	}

	async fn get_arch(self) -> String {
		arch().to_string()
	}

	async fn get_family(self) -> String {
		family().to_string()
	}

	/// Returns the user's locale (like "en-AU"), or "UNKNOWN" if it couldn't be found.
	async fn get_locale(self) -> String {
		locale().unwrap_or("UNKNOWN".to_string())
	}

	async fn get_type(self) -> String {
		type_().to_string()
	}

	async fn get_platform(self) -> String {
		platform().to_string()
	}

	async fn get_version(self) -> String {
		version().to_string()
	}

	async fn get_distro(self) -> String {
		let platform = self.clone().get_platform().await;

		if platform == "linux" {
			if let Ok(contents) = fs::read_to_string("/etc/os-release").await {
				for line in contents.lines() {
					if let Some(value) = line.strip_prefix("PRETTY_NAME=") {
						return value.trim_matches('"').to_string();
					}
				}

				for line in contents.lines() {
					if let Some(value) = line.strip_prefix("NAME=") {
						return value.trim_matches('"').to_string();
					}
				}
			}

			"UNKNOWN".to_string()
		} else {
			platform
		}
	}

	async fn get_git_commit_hash(self) -> String {
		crate::build::COMMIT_HASH.to_string()
	}

	async fn get_build_timestamp(self) -> String {
		crate::build::BUILD_TIMESTAMP.to_string()
	}

	async fn get_package_version(self) -> String {
		crate::build::PKG_VERSION.to_string()
	}

	async fn get_full_debug_info(self) -> DebugInfoData {
		DebugInfoData {
			in_dev: self.clone().is_in_dev().await,
			platform: self.clone().get_platform().await,
			arch: self.clone().get_arch().await,
			family: self.clone().get_family().await,
			locale: self.clone().get_locale().await,
			os_type: self.clone().get_type().await,
			os_version: self.clone().get_version().await,
			os_distro: self.clone().get_distro().await,
			commit_hash: self.clone().get_git_commit_hash().await,
			build_timestamp: self.clone().get_build_timestamp().await,
			build_version: self.clone().get_package_version().await,
		}
	}

	async fn get_full_debug_info_parsed(self) -> Vec<DebugInfoParsedLine> {
		let info = self.clone().get_full_debug_info().await;

		vec![
			DebugInfoParsedLine::new("inDev", if info.in_dev { "yes" } else { "no" }),
			DebugInfoParsedLine::new("Platform", info.platform),
			DebugInfoParsedLine::new("Arch", info.arch),
			DebugInfoParsedLine::new("Family", info.family),
			DebugInfoParsedLine::new("Locale", info.locale),
			DebugInfoParsedLine::new("Os Type", info.os_type),
			DebugInfoParsedLine::new("Os Version", info.os_version),
			DebugInfoParsedLine::new("Os Distro", info.os_distro),
			DebugInfoParsedLine::new("Commit Hash", info.commit_hash),
			DebugInfoParsedLine::new("Build Timestamp", info.build_timestamp),
			DebugInfoParsedLine::new("Version", info.build_version),
		]
	}

	async fn get_full_debug_info_parsed_string(self) -> String {
		let debug_info = self.clone().get_full_debug_info_parsed().await;

		let timestamp = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.expect("Time went backwards")
			.as_secs();

		let mut lines = Vec::new();

		lines.push("## OneClient Debug Information".to_string());
		lines.push(format!(
			"**Data exported at:** <t:{timestamp}> (`{timestamp}`)"
		));

		for line_data in debug_info {
			if line_data.title == "Build Timestamp" {
				lines.push(format!(
					"**{}:** <t:{}> (`{}`)",
					line_data.title, line_data.value, line_data.value
				));
			} else {
				lines.push(format!("**{}:** `{}`", line_data.title, line_data.value));
			}
		}

		lines.join("\n")
	}
}
