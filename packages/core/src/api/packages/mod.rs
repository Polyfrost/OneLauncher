use data::{ManagedPackage, ManagedVersion};
use onelauncher_entity::icon::Icon;
use onelauncher_entity::package::Provider;
use onelauncher_entity::{clusters, packages};
use reqwest::Method;

use crate::api::cluster::dao::get_cluster_by_id;
use crate::api::packages::data::{ManagedPackageBody, PackageAuthor};
use crate::error::{LauncherError, LauncherResult};
use crate::send_error;
use crate::store::Dirs;
use crate::store::ingress::SubIngress;
use crate::utils::crypto::HashAlgorithm;
use crate::utils::io::IOError;
use crate::utils::{DatabaseModelExt, http, icon, io};

pub mod bundle_dao;
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

	let update_existing_package =
		if let Some(model) = dao::get_package_by_hash(primary_file.sha1.clone()).await? {
			if !force.unwrap_or(false) {
				let cached_path = model.path().await?;
				if cached_path.exists() {
					tracing::debug!("package is already downloaded");
					return Ok(model);
				}

				tracing::warn!(
					"package '{}' was found in database but cached file is missing at '{}'; re-downloading",
					model.hash,
					cached_path.display()
				);
			}
			true
		} else {
			false
		};

	let mut model = packages::Model {
		hash: primary_file.sha1.clone(),
		display_name: package.name.clone(),
		display_version: version.display_version.clone(),
		file_name: primary_file.file_name.clone(),
		version_id: version.version_id.clone(),
		published_at: version.published,
		provider: package.provider,
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

	if let Some(icon_url) = &package.icon_url
		&& let Some(icon) = Icon::try_from_url(&url::Url::parse(icon_url)?)
	{
		model.icon = Some(icon::cache_icon(&icon).await?);
	}

	persist_downloaded_package(model, update_existing_package).await
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
	let update_existing_package = if let Some(model) =
		dao::get_package_by_hash(package.sha1.clone()).await?
	{
		if !force.unwrap_or(false) {
			let cached_path = model.path().await?;
			if cached_path.exists() {
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

			tracing::warn!(
				"external package '{}' was found in database but cached file is missing at '{}'; re-downloading",
				model.hash,
				cached_path.display()
			);
		}
		true
	} else {
		false
	};

	let model = packages::Model {
		hash: package.sha1.clone(),
		display_name: package.name.clone(),
		display_version: "Unknown".to_string(),
		file_name: package.name.clone(),
		version_id: package.sha1.clone(),
		published_at: chrono::Utc::now(),
		provider: Provider::Local,
		icon: None,
		package_id: package.sha1.clone(),
		mc_loader: vec![].into(),
		mc_versions: vec![].into(),
		package_type: package.package_type.clone(),
	};

	http::download_advanced(
		Method::GET,
		&package.url,
		model.path().await?,
		None,
		None,
		Some((HashAlgorithm::Sha1, &package.sha1)),
		ingress.as_ref(),
	)
	.await?;

	let inserted_model = persist_downloaded_package(model, update_existing_package).await?;

	Ok(Some(inserted_model))
}

async fn persist_downloaded_package(
	model: packages::Model,
	update_existing_package: bool,
) -> LauncherResult<packages::Model> {
	if !update_existing_package {
		return dao::insert_package(model.into()).await;
	}

	let hash = model.hash.clone();
	let file_name = model.file_name.clone();
	let version_id = model.version_id.clone();
	let published_at = model.published_at;
	let display_name = model.display_name.clone();
	let display_version = model.display_version.clone();
	let package_type = model.package_type.clone();
	let provider = model.provider;
	let package_id = model.package_id.clone();
	let mc_versions = model.mc_versions.clone();
	let mc_loader = model.mc_loader.clone();
	let icon = model.icon.clone();

	dao::update_package_by_hash(hash, async |mut active| {
		active.file_name = sea_orm::ActiveValue::Set(file_name);
		active.version_id = sea_orm::ActiveValue::Set(version_id);
		active.published_at = sea_orm::ActiveValue::Set(published_at);
		active.display_name = sea_orm::ActiveValue::Set(display_name);
		active.display_version = sea_orm::ActiveValue::Set(display_version);
		active.package_type = sea_orm::ActiveValue::Set(package_type);
		active.provider = sea_orm::ActiveValue::Set(provider);
		active.package_id = sea_orm::ActiveValue::Set(package_id);
		active.mc_versions = sea_orm::ActiveValue::Set(mc_versions);
		active.mc_loader = sea_orm::ActiveValue::Set(mc_loader);
		active.icon = sea_orm::ActiveValue::Set(icon);
		Ok(active)
	})
	.await
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
	if !skip_compatibility.unwrap_or(false) && package.provider != Provider::Local {
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
				if p.provider == Provider::Local {
					return true;
				}
				p.mc_loader
					.iter()
					.any(|v| cluster.mc_loader.compatible_with(v))
					&& p.mc_versions.iter().any(|v| cluster.mc_version.contains(v))
			})
			.cloned()
			.collect::<Vec<_>>()
	};

	for package in &compatible_packages {
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
			.join(self.package_type.folder_name())
			.join(self.provider.name())
			.join(&self.package_id)
			.join(&self.version_id)
			.join(&self.file_name))
	}
}

/// Removes a package from a cluster.
/// It is also deleted from the filesystem and database.
///
/// * `record_override` - When `true` and the package is tracked as a bundle package, a
///   `Removed` override is saved so future bundle syncs won't re-install it. Pass `false`
///   for system-initiated removals (bundle updates, bundle-driven removals) where the
///   removal is not a deliberate user choice.
pub async fn remove_package(
	cluster_id: i64,
	package_hash: String,
	record_override: bool,
) -> LauncherResult<()> {
	if let Some(package) = dao::get_package_by_hash(package_hash.clone()).await? {
		// Always look up bundle mapping: needed for recording a Removed override (user action)
		// and for cleaning up stale Disabled overrides (system action).
		let bundle_override_data: Option<(String, String)> = if let Ok(Some(bundle_mapping)) =
			bundle_dao::get_bundle_package(cluster_id, &package_hash).await
		{
			match (bundle_mapping.bundle_name, bundle_mapping.package_id) {
				(Some(bn), Some(pid)) => Some((bn, pid)),
				_ => None,
			}
		} else {
			None
		};

		dao::unlink_package_from_cluster(&package_hash, cluster_id).await?;

		// Remove the hard link from the cluster's folder
		if let Some(cluster) = get_cluster_by_id(cluster_id).await? {
			let cluster_dir = Dirs::get_clusters_dir()
				.await?
				.join(cluster.folder_name)
				.join(package.package_type.folder_name())
				.join(&package.file_name);

			if cluster_dir.exists() {
				let remove_result = if cluster_dir.is_dir() {
					io::remove_dir_all(&cluster_dir).await
				} else {
					io::remove_file(&cluster_dir).await
				};
				if let Err(err) = remove_result {
					send_error!("{}{}", "failed to remove file: ", err);
				}
			}
		}

		if !dao::is_package_used(&package_hash).await? {
			let path = package.path().await?;
			if path.exists()
				&& let Err(err) = io::remove_file(&path).await
			{
				send_error!("{}{}", "failed to remove file: ", err);
			}
			dao::delete_package_by_id(package_hash).await?;
		}

		// Act on bundle override data AFTER all filesystem/DB mutations succeed.
		if let Some((bundle_name, package_id)) = bundle_override_data {
			if record_override {
				// User-initiated removal: record intent to keep package out of future syncs.
				bundle_dao::save_bundle_override(
					cluster_id,
					&bundle_name,
					&package_id,
					onelauncher_entity::cluster_bundle_overrides::OverrideType::Removed,
				)
				.await?;
			} else {
				// System-initiated removal: clean up stale overrides for this specific
				// bundle package, but preserve overrides when a replacement hash for the
				// same bundle/package mapping was already linked and tracked.
				let replacement_exists =
					bundle_dao::has_bundle_package_mapping(cluster_id, &bundle_name, &package_id)
						.await?;
				if !replacement_exists {
					bundle_dao::remove_bundle_override(cluster_id, &bundle_name, &package_id)
						.await?;
				}
			}
		}
	}

	Ok(())
}

/// Toggles a package's enabled state in a cluster by renaming its hard link
/// between `.jar` and `.jar.disabled`. Returns the new enabled state.
pub async fn toggle_package(cluster_id: i64, package_hash: String) -> LauncherResult<bool> {
	let package = dao::get_package_by_hash(package_hash.clone())
		.await?
		.ok_or_else(|| anyhow::anyhow!("package with hash '{}' not found", package_hash))?;

	let cluster = get_cluster_by_id(cluster_id)
		.await?
		.ok_or_else(|| anyhow::anyhow!("cluster with id {} not found", cluster_id))?;

	let cluster_mods_dir = Dirs::get_clusters_dir()
		.await?
		.join(&cluster.folder_name)
		.join(package.package_type.folder_name());

	let file_name = &package.file_name;
	let is_disabled = file_name.ends_with(".disabled");

	let (new_file_name, enabled) = if is_disabled {
		(file_name.trim_end_matches(".disabled").to_string(), true)
	} else {
		(format!("{file_name}.disabled"), false)
	};

	// Read bundle mapping data before any mutations so the override can be written
	// after the filesystem rename succeeds (avoiding DB/disk desync on FS failure).
	let bundle_override_data: Option<(String, String)> = if let Ok(Some(bundle_mapping)) =
		bundle_dao::get_bundle_package(cluster_id, &package_hash).await
	{
		match (bundle_mapping.bundle_name, bundle_mapping.package_id) {
			(Some(bn), Some(pid)) => Some((bn, pid)),
			_ => None,
		}
	} else {
		None
	};

	// Rename the hard link in the cluster folder FIRST
	let current_path = cluster_mods_dir.join(file_name);
	let new_path = cluster_mods_dir.join(&new_file_name);
	if current_path.exists() {
		tokio::fs::rename(&current_path, &new_path)
			.await
			.map_err(IOError::from)?;
	}

	// Write the bundle override AFTER the filesystem rename succeeds
	if let Some((bundle_name, package_id)) = bundle_override_data {
		if enabled {
			bundle_dao::remove_bundle_override(cluster_id, &bundle_name, &package_id).await?;
		} else {
			bundle_dao::save_bundle_override(
				cluster_id,
				&bundle_name,
				&package_id,
				onelauncher_entity::cluster_bundle_overrides::OverrideType::Disabled,
			)
			.await?;
		}
	}

	// Rename the file in the central package store
	let old_store_path = package.path().await?;
	dao::update_package_by_hash(package_hash, async |mut model| {
		model.file_name = sea_orm::ActiveValue::Set(new_file_name.clone());
		Ok(model)
	})
	.await?;

	// Re-fetch to get the updated path
	let updated_package = dao::get_package_by_hash(package.hash.clone())
		.await?
		.ok_or_else(|| anyhow::anyhow!("package not found after update"))?;
	let new_store_path = updated_package.path().await?;

	if old_store_path.exists() && old_store_path != new_store_path {
		if let Some(parent) = new_store_path.parent() {
			io::create_dir_all(parent).await?;
		}
		tokio::fs::rename(&old_store_path, &new_store_path)
			.await
			.map_err(IOError::from)?;
	}

	Ok(enabled)
}
