use data::{ManagedPackage, ManagedVersion};
use onelauncher_entity::icon::Icon;
use onelauncher_entity::package::PackageType;
use onelauncher_entity::{clusters, packages};
use reqwest::Method;
use serde::Serialize;

use crate::error::{LauncherError, LauncherResult};
use crate::store::Dirs;
use crate::store::ingress::SubIngress;
use crate::utils::crypto::HashAlgorithm;
use crate::utils::io::IOError;
use crate::utils::{http, icon, io};

pub mod dao;
pub mod data;
pub mod provider;

#[cfg(test)]
mod tests;

#[onelauncher_macro::specta]
#[derive(Debug, thiserror::Error, Serialize)]
pub enum PackageError {
	#[error("no file marked for download")]
	NoPrimaryFile,
	#[error("package is a modpack, it must be handled differently")]
	IsModPack,
	#[error(transparent)]
	Incompatible(#[from] IncompatiblePackageType),
}

#[onelauncher_macro::specta]
#[derive(Debug, thiserror::Error, Serialize)]
pub enum IncompatiblePackageType {
	#[error("package is not compatible with the current version")]
	McVersion,
	#[error("package is not compatible with the current loader")]
	Loader,
}

/// Downloads a package and registers it in the database from the given provider.
#[tracing::instrument(level = "debug", skip_all)]
pub async fn download_package(
	package: &ManagedPackage,
	version: &ManagedVersion,
	ingress: Option<SubIngress<'_>>,
) -> LauncherResult<packages::Model> {
	// Modpacks are downloaded and managed differently.
	if package.package_type == PackageType::ModPack {
		return Err(PackageError::IsModPack.into());
	}

	tracing::debug!(
		"downloading package '{}' version '{}' from '{}'",
		package.name,
		version.version_id,
		package.provider
	);

	// Check if the package is already downloaded

	let primary_file = version
		.files
		.iter()
		.find(|f| f.primary)
		.ok_or(PackageError::NoPrimaryFile)?;

	if let Some(model) = dao::get_package_by_hash(primary_file.sha1.clone()).await? {
		tracing::debug!("package is already downloaded");
		return Ok(model);
	}

	let dir = Dirs::get_package_dir(&package.package_type, &package.provider, &package.id).await?;

	io::create_dir_all(&dir).await?;

	let dest = join_package_file(dir, &version.version_id, primary_file.file_name.as_str());

	http::download_advanced(
		Method::GET,
		&primary_file.url,
		dest,
		None,
		None,
		Some((HashAlgorithm::Sha1, primary_file.sha1.as_str())),
		ingress.as_ref(),
	)
	.await?;

	let icon = if let Some(icon_url) = &package.icon_url {
		if let Some(icon) = Icon::try_from_url(url::Url::parse(icon_url)?) {
			Some(icon::cache_icon(&icon).await?)
		} else {
			None
		}
	} else {
		None
	};

	let model = packages::Model {
		hash: primary_file.sha1.clone(),
		display_name: package.name.clone(),
		display_version: version.display_version.clone(),
		file_name: primary_file.file_name.clone(),
		version_id: version.version_id.clone(),
		published_at: version.published,
		provider: package.provider.clone(),
		icon,
		package_id: package.id.clone(),
		mc_loader: version.loaders.clone().into(),
		mc_versions: version.mc_versions.clone().into(),
		package_type: package.package_type.clone(),
	};

	dao::insert_package(model.into()).await
}

/// Links a package to a cluster on the file system and in database.
/// * `skip_compatibility` - Checks whether the `mc_loader` and `mc_version` is compatible. Default is false
#[tracing::instrument(level = "debug", skip_all)]
pub async fn link_package(
	package: &packages::Model,
	cluster: &clusters::Model,
	skip_compatibility: Option<bool>,
) -> LauncherResult<()> {
	if package.package_type == PackageType::ModPack {
		return Err(PackageError::IsModPack.into());
	}

	tracing::debug!(
		"linking package '{}' version '{}' to cluster '{}'",
		package.display_name,
		package.display_version,
		cluster.name
	);

	tracing::trace!("checking if package is already linked to cluster");
	if dao::is_package_linked_to_cluster(package, cluster).await? {
		tracing::debug!("package is already linked to cluster");
		return Ok(());
	}

	tracing::trace!("checking compatibility of package with cluster");
	if !skip_compatibility.unwrap_or(false) {
		if !package.mc_loader.iter().any(|v| cluster.mc_loader.compatible_with(v)) {
			return Err(LauncherError::from(PackageError::from(IncompatiblePackageType::Loader)));
		}

		if !package.mc_versions.iter().any(|v| cluster.mc_version.contains(v)) {
			return Err(LauncherError::from(PackageError::from(IncompatiblePackageType::McVersion)));
		}
	}

	let dirs = Dirs::get().await?;
	let cluster_dir = dirs.clusters_dir().join(cluster.folder_name.clone());

	tracing::trace!("hard linking package to cluster");
	let src_pkg = path_from_model(dirs, package);
	let dest_dir = cluster_dir.join(package.package_type.folder_name());
	io::create_dir_all(&dest_dir).await?;

	let dest_pkg = dest_dir.join(package.file_name.as_str());
	tokio::fs::hard_link(src_pkg, dest_pkg).await.map_err(IOError::from)?;

	dao::link_package_to_cluster(package, cluster).await?;

	tracing::debug!(
		"linked package '{}' version '{}' to cluster '{}'",
		package.display_name,
		package.display_version,
		cluster.name
	);
	Ok(())
}

fn path_from_model(dirs: &Dirs, model: &packages::Model) -> std::path::PathBuf {
	join_package_file(dirs.package_dir(&model.package_type, &model.provider, &model.package_id), &model.version_id, model.file_name.as_str())
}

fn join_package_file(path: impl AsRef<std::path::Path>, version_id: &str, file_name: &str) -> std::path::PathBuf {
	path.as_ref().join(format!("{version_id}-{file_name}"))
}