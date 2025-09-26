use std::path::PathBuf;

use crate::error::LauncherResult;
use crate::store::Dirs;

#[taurpc::procedures(path = "folders")]
pub trait TauriLauncherFoldersApi {
	#[taurpc(alias = "fromCluster")]
	async fn from_cluster(folder_name: String) -> LauncherResult<PathBuf>;

	#[taurpc(alias = "openCluster")]
	async fn open_cluster(folder_name: String) -> LauncherResult<()>;
}

#[taurpc::ipc_type]
pub struct TauriLauncherFoldersApiImpl;

#[taurpc::resolvers]
impl TauriLauncherFoldersApi for TauriLauncherFoldersApiImpl {
	async fn from_cluster(self, folder_name: String) -> LauncherResult<PathBuf> {
		Ok(Dirs::get_clusters_dir().await?.join(folder_name))
	}

	async fn open_cluster(self, folder_name: String) -> LauncherResult<()> {
		let path = self.from_cluster(folder_name).await?;
		Ok(opener::open(path)?)
	}
}
