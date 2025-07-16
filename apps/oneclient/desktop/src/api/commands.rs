use std::collections::HashMap;

use onelauncher_core::{entity::clusters, error::LauncherResult};
use tauri::Runtime;

#[taurpc::procedures(path = "oneclient", export_to = "../frontend/src/bindings.gen.ts")]
pub trait OneClientApi {
	#[taurpc(alias = "openDevTools")]
	async fn open_dev_tools<R: Runtime>(webview_window: tauri::WebviewWindow<R>);

	#[taurpc(alias = "getClustersGroupedByMajor")]
	async fn get_clusters_grouped_by_major() -> LauncherResult<HashMap<u32, Vec<clusters::Model>>>;
}

#[taurpc::ipc_type]
pub struct OneClientApiImpl;

#[taurpc::resolvers]
impl OneClientApi for OneClientApiImpl {
	async fn open_dev_tools<R: Runtime>(self, webview_window: tauri::WebviewWindow<R>) {
		#[cfg(feature = "devtools")]
		webview_window.open_devtools();
	}

	async fn get_clusters_grouped_by_major(self) -> LauncherResult<HashMap<u32, Vec<clusters::Model>>> {
		let clusters = onelauncher_core::api::cluster::dao::get_all_clusters().await?;

		let mut mapped: HashMap<u32, Vec<clusters::Model>> = HashMap::new();

		for cluster in clusters {
			// Assuming `mc_version` is a String and you have a function to parse major version
			let split = &mut cluster.mc_version.split('.');
			split.next();
			if let Some(major) = split.next() {
				// Convert the major version to a u32
				let major: u32 = match major.parse() {
					Ok(v) => v,
					Err(_) => continue, // Skip if parsing fails
				};

				mapped.entry(major)
					.or_insert_with(Vec::new)
					.push(cluster);
			}
		}

		Ok(mapped)
	}

}
