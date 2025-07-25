use onelauncher_core::error::{DaoError, LauncherResult};
use tauri::Runtime;

#[taurpc::procedures(path = "onelauncher", export_to = "../frontend/src/bindings.gen.ts")]
pub trait OneLauncherApi {
	async fn return_error() -> LauncherResult<()>;

	async fn open_dev_tools<R: Runtime>(webview_window: tauri::WebviewWindow<R>);

	async fn set_window_style<R: Runtime>(
		webview_window: tauri::WebviewWindow<R>,
		decorations: bool,
	);
}

#[taurpc::ipc_type]
pub struct OneLauncherApiImpl;

#[taurpc::resolvers]
impl OneLauncherApi for OneLauncherApiImpl {
	async fn return_error(self) -> LauncherResult<()> {
		let err = DaoError::NotFound.into();
		Err(err)
	}

	async fn open_dev_tools<R: Runtime>(self, webview_window: tauri::WebviewWindow<R>) {
		#[cfg(feature = "devtools")]
		webview_window.open_devtools();
	}

	async fn set_window_style<R: Runtime>(
		self,
		webview_window: tauri::WebviewWindow<R>,
		decorations: bool,
	) {
		webview_window.set_decorations(decorations).ok();
	}
}
