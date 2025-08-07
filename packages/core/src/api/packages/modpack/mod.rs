use std::path::PathBuf;
use std::sync::Arc;

use onelauncher_entity::package::PackageType;
use onelauncher_entity::{clusters, packages};
use sea_orm::ActiveValue::Set;

use crate::api::packages::PackageError;
use crate::api::packages::modpack::mrpack::MrPackFormatImpl;
use crate::api::{self};
use crate::error::LauncherResult;
use crate::store::ingress::SubIngress;

pub mod data;
mod mrpack;

#[cfg(test)]
mod tests;

#[onelauncher_macro::specta]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModpackFormat {
	CurseForge,
	MrPack,
}

#[async_trait::async_trait]
pub trait ModpackFormatExt {
	async fn from_file(
		path: PathBuf,
	) -> LauncherResult<Option<Box<dyn InstallableModpackFormatExt>>>
	where
		Self: Sized;

	async fn from_bytes(
		bytes: Arc<Vec<u8>>,
	) -> LauncherResult<Option<Box<dyn InstallableModpackFormatExt>>>
	where
		Self: Sized;
}

#[async_trait::async_trait]
pub trait InstallableModpackFormatExt: Send + Sync + std::any::Any {
	async fn manifest(&self) -> LauncherResult<&data::ModpackManifest>;
	async fn install_to(
		&self,
		cluster: &clusters::Model,
		skip_compatibility: Option<bool>,
		ingress: Option<SubIngress<'_>>,
	) -> LauncherResult<()>;

	fn as_any(self: Box<Self>) -> Box<dyn std::any::Any + Send + Sync>;
}

type LoaderFn<Arg> = fn(
	Arg,
) -> std::pin::Pin<
	Box<dyn Future<Output = LauncherResult<Option<Box<dyn InstallableModpackFormatExt>>>> + Send>,
>;

impl ModpackFormat {
	async fn from_file(path: PathBuf) -> LauncherResult<Box<dyn InstallableModpackFormatExt>> {
		let pipeline: Vec<LoaderFn<PathBuf>> = vec![MrPackFormatImpl::from_file];

		for stage in pipeline {
			if let Some(format) = stage(path.clone()).await? {
				return Ok(format);
			}
		}

		Err(PackageError::UnsupportedModpackFormat.into())
	}

	async fn from_bytes(
		bytes: Arc<Vec<u8>>,
	) -> LauncherResult<Box<dyn InstallableModpackFormatExt>> {
		let pipeline: Vec<LoaderFn<Arc<Vec<u8>>>> = vec![MrPackFormatImpl::from_bytes];

		for stage in pipeline {
			if let Some(format) = stage(Arc::clone(&bytes)).await? {
				return Ok(format);
			}
		}

		Err(PackageError::UnsupportedModpackFormat.into())
	}
}

/// Installs a modpack to a cluster.
pub async fn install_managed_modpack(
	package_model: &packages::Model,
	modpack: &Box<dyn InstallableModpackFormatExt>,
	cluster: &mut clusters::Model,
	skip_compatibility: Option<bool>,
	ingress: Option<SubIngress<'_>>,
) -> LauncherResult<()> {
	if package_model.package_type != PackageType::ModPack {
		return Err(PackageError::IsNotModPack.into());
	}

	modpack
		.install_to(cluster, skip_compatibility, ingress)
		.await?;

	api::cluster::dao::update_cluster(cluster, async |mut c| {
		c.linked_modpack_hash = Set(Some(package_model.hash.clone()));
		Ok(c)
	})
	.await?;

	Ok(())
}
