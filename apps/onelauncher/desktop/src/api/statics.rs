use onelauncher_core::{constants::{NATIVE_ARCH, TARGET_OS}, store::Core};
use serde::Serialize;
use specta::Type;

#[derive(Serialize, Type)]
pub struct ProgramInfo {
	launcher_version: String,
	webview_version: String,
	tauri_version: String,
	dev_build: bool,
	platform: String,
	arch: String,
}

#[must_use]
pub fn get_program_info() -> ProgramInfo {

	ProgramInfo {
		launcher_version: Core::get().launcher_version.clone(),
		webview_version: tauri::webview_version().unwrap_or_else(|_| "UNKNOWN".into()),
		tauri_version: tauri::VERSION.into(),
		dev_build: tauri::is_dev(),
		platform: TARGET_OS.into(),
		arch: NATIVE_ARCH.into(),
	}
}
