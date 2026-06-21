// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
	// Must run before the async runtime spawns any threads and before the
	// webview initialises (see onelauncher_core::platform).
	onelauncher_core::platform::apply_startup_workarounds();

	tokio::runtime::Builder::new_multi_thread()
		.enable_all()
		.build()
		.expect("failed to build tokio runtime")
		.block_on(onelauncher_gui::run());
}
