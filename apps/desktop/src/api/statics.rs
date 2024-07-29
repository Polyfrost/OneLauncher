use serde::Serialize;
use specta::Type;
use tauri_specta::StaticCollection;
use onelauncher::constants::*;

pub fn get_static_collection() -> StaticCollection {
	StaticCollection::default()
		.register("PROGRAM_INFO", get_program_info())
        .to_owned()
}

#[derive(Serialize, Type)]
pub struct ProgramInfo {
	launcher_version: String,
	webview_version: String,
	tauri_version: String,
	dev_build: bool,
	platform: String,
	arch: String,
}

fn get_program_info() -> ProgramInfo {
	let webview_version = tauri::webview_version().unwrap_or("UNKNOWN".into());
	let tauri_version = tauri::VERSION;
	let dev_build = tauri::is_dev();

	ProgramInfo {
		launcher_version: VERSION.into(),
		webview_version,
		tauri_version: tauri_version.into(),
		dev_build,
		platform: TARGET_OS.into(),
		arch: NATIVE_ARCH.into(),
	}
}