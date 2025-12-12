use tauri::Runtime;
use tauri_plugin_os::{arch, family, locale, platform, type_, version};

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

	#[taurpc(alias = "getOsVersion")]
	async fn get_version() -> String;

	#[taurpc(alias = "getGitCommitHash")]
	async fn get_git_commit_hash() -> String;

	#[taurpc(alias = "getBuildTimestamp")]
	async fn get_build_timestamp() -> String;

	#[taurpc(alias = "getPackageVersion")]
	async fn get_package_version() -> String;
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

	async fn get_git_commit_hash(self) -> String {
		crate::build::COMMIT_HASH.to_string()
	}

	async fn get_build_timestamp(self) -> String {
		crate::build::BUILD_TIME.to_string()
	}

	async fn get_package_version(self) -> String {
		crate::build::PKG_VERSION.to_string()
	}
}
