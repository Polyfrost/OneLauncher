mod mrpack;

use std::path::Path;


pub use mrpack::{install_mrpack_to_cluster, MrpackInstaller};

use crate::ctx::ContentCtx;
use crate::error::ContentResult;

#[tracing::instrument(level = "debug", skip(archive_path, ctx))]
pub async fn install_modpack_archive(
	archive_path: impl AsRef<Path>,
	cluster_id: i64,
	ctx: &ContentCtx,
) -> ContentResult<()> {
	mrpack::install_mrpack_to_cluster(archive_path.as_ref().to_path_buf(), cluster_id, ctx).await
}
