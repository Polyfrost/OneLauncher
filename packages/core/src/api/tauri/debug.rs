use tauri::Runtime;
use tauri_plugin_os::{arch, family, locale, platform, type_, version};

use crate::error::LauncherResult;

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
	async fn get_locale() -> Option<String>;

	#[taurpc(alias = "getType")]
	async fn get_type() -> String;

	#[taurpc(alias = "getPlatform")]
	async fn get_platform() -> String;

	#[taurpc(alias = "getVersion")]
	async fn get_version() -> String;

	#[taurpc(alias = "getCommitHash")]
	async fn get_commit_hash() -> LauncherResult<String>;

	#[taurpc(alias = "getBuildTimestamp")]
	async fn get_build_timestamp() -> LauncherResult<String>;
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

	async fn get_locale(self) -> Option<String> {
		locale()
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

	async fn get_commit_hash(self) -> LauncherResult<String> {
		if tauri::is_dev() {
			let hash = std::env::var("GIT_HASH").map_err(anyhow::Error::from)?;
			Ok(hash)
		} else {
			Ok("null".to_string())
		}
	}

	async fn get_build_timestamp(self) -> LauncherResult<String> {
		let timestamp = std::env::var("BUILD_TIMESTAMP").map_err(anyhow::Error::from)?;
		Ok(timestamp)
	}
}
