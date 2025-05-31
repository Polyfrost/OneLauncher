#[onelauncher_core::command]
#[allow(unused_parens)]
pub fn open_dev_tools(webview: tauri::WebviewWindow) {
	#[cfg(feature = "devtools")]
	webview.open_devtools();
}