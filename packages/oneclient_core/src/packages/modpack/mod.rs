mod mrpack;

use std::path::Path;


pub use mrpack::{install_mrpack_to_cluster, MrpackInstaller};

use crate::state::LauncherServices;
use crate::LauncherResult;

#[tracing::instrument(level = "debug", skip(archive_path, services))]
pub async fn install_modpack_archive(
	archive_path: impl AsRef<Path>,
	cluster_id: i64,
	services: &LauncherServices,
) -> LauncherResult<()> {
	mrpack::install_mrpack_to_cluster(archive_path.as_ref().to_path_buf(), cluster_id, services).await
}
