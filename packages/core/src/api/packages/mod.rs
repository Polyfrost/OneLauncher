use data::{ManagedPackage, ManagedVersion};
use onelauncher_entity::icon::Icon;
use onelauncher_entity::package::Provider;
use onelauncher_entity::{clusters, packages};
use reqwest::Method;

use crate::api::packages::data::{ManagedPackageBody, PackageAuthor};
use crate::error::{LauncherError, LauncherResult};
use crate::store::Dirs;
use crate::store::ingress::SubIngress;
use crate::utils::crypto::HashAlgorithm;
use crate::utils::io::IOError;
use crate::utils::{DatabaseModelExt, http, icon, io};

pub mod categories;
pub mod dao;
pub mod data;
pub mod modpack;
pub mod provider;

#[onelauncher_macro::error]
#[derive(Debug, thiserror::Error)]
pub enum PackageError {
	#[error("no file marked for download")]
	NoPrimaryFile,
	#[error("package is not a modpack")]
	IsNotModPack,
	#[error(transparent)]
	Incompatible(#[from] IncompatiblePackageType),
	#[error("missing API key for provider '{0}'")]
	MissingApiKey(Provider),
	#[error("unsupported package body type '{0}'")]
	UnsupportedBodyType(ManagedPackageBody),
	#[error("unsupported author type '{0}'")]
	UnsupportedAuthorType(PackageAuthor),
	#[error("unsupported modpack format")]
	UnsupportedModpackFormat,
}

#[onelauncher_macro::error]
#[derive(Debug, thiserror::Error)]
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
	force: Option<bool>,
	ingress: Option<SubIngress<'_>>,
) -> LauncherResult<packages::Model> {
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

	if !force.unwrap_or(false)
		&& let Some(model) = dao::get_package_by_hash(primary_file.sha1.clone()).await?
	{
		tracing::debug!("package is already downloaded");
		return Ok(model);
	}

	let mut model = packages::Model {
		hash: primary_file.sha1.clone(),
		display_name: package.name.clone(),
		display_version: version.display_version.clone(),
		file_name: primary_file.file_name.clone(),
		version_id: version.version_id.clone(),
		published_at: version.published,
		provider: package.provider.clone(),
		icon: None,
		package_id: package.id.clone(),
		mc_loader: version.loaders.clone().into(),
		mc_versions: version.mc_versions.clone().into(),
		package_type: package.package_type.clone(),
	};

	http::download_advanced(
		Method::GET,
		&primary_file.url,
		model.path().await?,
		None,
		None,
		Some((HashAlgorithm::Sha1, primary_file.sha1.as_str())),
		ingress.as_ref(),
	)
	.await?;

	if let Some(icon_url) = &package.icon_url {
		if let Some(icon) = Icon::try_from_url(&url::Url::parse(icon_url)?) {
			model.icon = Some(icon::cache_icon(&icon).await?);
		}
	}

	dao::insert_package(model.into()).await
}

/// Downloads a package and conditionally returns the database entry if it's hash was already registered.
pub async fn download_external_package(
	package: &data::ExternalPackage,
	cluster: &clusters::Model,
	force: Option<bool>,
	skip_compatibility: Option<bool>,
	ingress: Option<SubIngress<'_>>,
) -> LauncherResult<Option<packages::Model>> {
	tracing::debug!(
		"downloading external package '{}' from '{}'",
		package.name,
		package.url
	);

	// check if already downloaded
	if !force.unwrap_or(false)
		&& let Some(model) = dao::get_package_by_hash(package.sha1.clone()).await?
	{
		tracing::debug!(
			"external package is already downloaded as '{}'",
			model.display_name
		);

		// considering that this function downloads packages to a cluster,
		// we'll hard-link the already downloaded package to the cluster
		// as to keep functionality consistent
		link_package(&model, cluster, skip_compatibility).await?;

		return Ok(Some(model));
	}

	let dir = Dirs::get_clusters_dir()
		.await?
		.join(cluster.folder_name.clone())
		.join(package.package_type.folder_name());

	http::download_advanced(
		Method::GET,
		&package.url,
		dir,
		None,
		None,
		Some((HashAlgorithm::Sha1, &package.sha1)),
		ingress.as_ref(),
	)
	.await?;

	Ok(None)
}

/// Links a package to a cluster on the file system and in database.
/// * `skip_compatibility` - Checks whether the `mc_loader` and `mc_version` is compatible. Default is false
#[tracing::instrument(level = "debug", skip_all)]
pub async fn link_package(
	package: &packages::Model,
	cluster: &clusters::Model,
	skip_compatibility: Option<bool>,
) -> LauncherResult<()> {
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
		if !package
			.mc_loader
			.iter()
			.any(|v| cluster.mc_loader.compatible_with(v))
		{
			return Err(LauncherError::from(PackageError::from(
				IncompatiblePackageType::Loader,
			)));
		}

		if !package
			.mc_versions
			.iter()
			.any(|v| cluster.mc_version.contains(v))
		{
			return Err(LauncherError::from(PackageError::from(
				IncompatiblePackageType::McVersion,
			)));
		}
	}

	hard_link(package, cluster).await?;

	dao::link_package_to_cluster(package, cluster).await?;

	tracing::debug!(
		"linked package '{}' version '{}' to cluster '{}'",
		package.display_name,
		package.display_version,
		cluster.name
	);
	Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn link_many_packages_to_cluster(
	packages: &[packages::Model],
	cluster: &clusters::Model,
	skip_compatibility: Option<bool>,
) -> LauncherResult<u64> {
	tracing::debug!(
		"linking {} packages to cluster '{}'",
		packages.len(),
		cluster.name
	);

	let compatible_packages = if skip_compatibility.unwrap_or(false) {
		packages.to_vec()
	} else {
		packages
			.iter()
			.filter(|p| {
				p.mc_loader
					.iter()
					.any(|v| cluster.mc_loader.compatible_with(v))
					&& p.mc_versions.iter().any(|v| cluster.mc_version.contains(v))
			})
			.cloned()
			.collect::<Vec<_>>()
	};

	for package in compatible_packages.iter() {
		hard_link(package, cluster).await?;
	}

	dao::link_many_packages_to_cluster(packages, cluster).await?;

	Ok(0)
}

#[tracing::instrument(level = "debug", skip_all)]
async fn hard_link(package: &packages::Model, cluster: &clusters::Model) -> LauncherResult<()> {
	let cluster_dir = Dirs::get_clusters_dir()
		.await?
		.join(cluster.folder_name.clone());

	tracing::trace!(
		"hard linking package '{}' to cluster '{}'",
		package.display_name,
		cluster.name
	);
	let src_pkg = package.path().await?;
	let dest_dir = cluster_dir.join(package.package_type.folder_name());
	io::create_dir_all(&dest_dir).await?;

	let dest_pkg = dest_dir.join(package.file_name.as_str());
	tokio::fs::hard_link(src_pkg, dest_pkg)
		.await
		.map_err(IOError::from)?;

	Ok(())
}

#[async_trait::async_trait]
impl DatabaseModelExt for packages::Model {
	async fn path(&self) -> LauncherResult<std::path::PathBuf> {
		Ok(Dirs::get_packages_dir()
			.await?
			.join(&self.package_type.folder_name())
			.join(&self.provider.name())
			.join(&self.package_id)
			.join(&self.version_id)
			.join(&self.file_name))
	}
}
