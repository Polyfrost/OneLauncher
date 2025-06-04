use onelauncher_core::error::{DaoError, LauncherResult};
use tauri::Runtime;

#[taurpc::procedures(path = "oneclient", export_to = "../frontend/src/bindings.gen.ts")]
pub trait OneClientApi {
	async fn return_error() -> LauncherResult<()>;

	async fn open_dev_tools<R: Runtime>(webview_window: tauri::WebviewWindow<R>);
}

#[taurpc::ipc_type]
pub struct OneClientApiImpl;

#[taurpc::resolvers]
impl OneClientApi for OneClientApiImpl {

	async fn return_error(self) -> LauncherResult<()> {
		let err = DaoError::NotFound.into();
		Err(err)
	}

	async fn open_dev_tools<R: Runtime>(self, webview_window: tauri::WebviewWindow<R>) {
		#[cfg(feature = "devtools")]
		webview_window.open_devtools();
	}

}
