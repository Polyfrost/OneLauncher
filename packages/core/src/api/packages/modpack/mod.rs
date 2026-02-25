use std::path::PathBuf;
use std::sync::Arc;

use onelauncher_entity::package::PackageType;
use onelauncher_entity::{clusters, packages};
use sea_orm::ActiveValue::Set;
use serde::{Deserialize, Serialize};

use crate::api::packages::PackageError;
use crate::api::packages::modpack::data::ModpackArchive;
use crate::api::packages::modpack::mrpack::MrPackFormatImpl;
use crate::api::packages::modpack::polymrpack::PolyMrPackFormatImpl;
use crate::api::{self};
use crate::error::LauncherResult;
use crate::store::ingress::SubIngress;

pub mod data;
pub mod mrpack;
mod polymrpack;

#[cfg(test)]
mod tests;

#[onelauncher_macro::specta]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModpackFormat {
	CurseForge,
	MrPack,
	PolyMrPack,
}

const FORMAT_PIPELINE_PATH: &[LoaderFn<PathBuf>] = &[
	PolyMrPackFormatImpl::from_path, // has to be first because of it being a **VALID** mrpack with extra stuff
	MrPackFormatImpl::from_path,
];

const FORMAT_PIPELINE_BYTES: &[LoaderFn<Arc<Vec<u8>>>] = &[
	PolyMrPackFormatImpl::from_manifest_bytes, // has to be first because of it being a **VALID** mrpack with extra stuff
	MrPackFormatImpl::from_manifest_bytes,
];

type LoaderFn<Arg> = fn(
	Arg,
) -> std::pin::Pin<
	Box<dyn Future<Output = LauncherResult<Option<Box<dyn InstallableModpackFormatExt>>>> + Send>,
>;

#[async_trait::async_trait]
pub trait ModpackFormatExt {
	async fn from_path(
		path: PathBuf,
	) -> LauncherResult<Option<Box<dyn InstallableModpackFormatExt>>>
	where
		Self: Sized;

	async fn from_manifest_bytes(
		bytes: Arc<Vec<u8>>,
	) -> LauncherResult<Option<Box<dyn InstallableModpackFormatExt>>>
	where
		Self: Sized;

	async fn install_modpack_archive(
		modpack_archive: &ModpackArchive,
		cluster: &clusters::Model,
		skip_compatibility: Option<bool>,
		ingress: Option<SubIngress<'_>>,
	) -> LauncherResult<()>
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

	fn kind(&self) -> ModpackFormat;
}

impl ModpackFormat {
	pub async fn from_file(
		path: &std::path::Path,
	) -> LauncherResult<Box<dyn InstallableModpackFormatExt>> {
		for stage in FORMAT_PIPELINE_PATH {
			if let Some(format) = stage(path.to_path_buf()).await? {
				return Ok(format);
			}
		}

		Err(PackageError::UnsupportedModpackFormat.into())
	}

	pub async fn from_manifest_bytes(
		bytes: Arc<Vec<u8>>,
	) -> LauncherResult<Box<dyn InstallableModpackFormatExt>> {
		for stage in FORMAT_PIPELINE_BYTES {
			if let Some(format) = stage(Arc::clone(&bytes)).await? {
				return Ok(format);
			}
		}

		Err(PackageError::UnsupportedModpackFormat.into())
	}

	pub async fn install_modpack_archive(
		&self,
		modpack_archive: &ModpackArchive,
		cluster: &clusters::Model,
		skip_compatibility: Option<bool>,
		ingress: Option<SubIngress<'_>>,
	) -> LauncherResult<()> {
		match self {
			Self::CurseForge => unimplemented!(),
			Self::MrPack => MrPackFormatImpl::install_modpack_archive(
				modpack_archive,
				cluster,
				skip_compatibility,
				ingress,
			),
			Self::PolyMrPack => PolyMrPackFormatImpl::install_modpack_archive(
				modpack_archive,
				cluster,
				skip_compatibility,
				ingress,
			),
		}
		.await
	}
}

/// Installs a modpack to a cluster.
pub async fn install_managed_modpack(
	package_model: &packages::Model,
	modpack: &dyn InstallableModpackFormatExt,
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
