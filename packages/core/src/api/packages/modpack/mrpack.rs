use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use onelauncher_entity::clusters;
use onelauncher_entity::loader::GameLoader;
use onelauncher_entity::package::{PackageType, Provider};
use serde::Deserialize;
use tokio::sync::OnceCell;

use crate::api::cluster::ClusterError;
use crate::api::packages::data::{ExternalPackage, ManagedVersion, PackageOverrides, PackageSide};
use crate::api::packages::modpack::data::{
	ModpackArchive, ModpackFile, ModpackFileKind, ModpackManifest,
};
use crate::api::packages::modpack::{InstallableModpackFormatExt, ModpackFormatExt};
use crate::api::packages::provider::ProviderExt;
use crate::api::{self};
use crate::error::LauncherResult;
use crate::store::ingress::SubIngress;
use crate::utils::DatabaseModelExt;
use crate::utils::io::{self, IOError};
use tokio_util::compat::TokioAsyncWriteCompatExt;

pub struct MrPackFormatImpl {
	pub(super) archive: Option<PathBuf>,
	pub(super) raw_manifest: MrPackManifest,
	pub(super) manifest: OnceCell<ModpackManifest>,
	pub(super) mc_version: String,
	pub(super) loader: GameLoader,
	pub(super) loader_version: String,
}

pub(super) const MODRINTH_URL_PREFIX: &str = "https://cdn.modrinth.com/";

#[async_trait::async_trait]
impl ModpackFormatExt for MrPackFormatImpl {
	async fn from_path(
		path: std::path::PathBuf,
	) -> LauncherResult<Option<Box<dyn InstallableModpackFormatExt>>>
	where
		Self: Sized,
	{
		let zip_file = tokio::fs::File::open(path.clone())
			.await
			.map_err(IOError::from)?;
		let buf_reader = tokio::io::BufReader::new(zip_file);

		let manifest_file = io::try_read_zip_entry_bytes(buf_reader, "modrinth.index.json").await?;

		let Some(this) = Self::from_manifest_bytes(Arc::new(manifest_file)).await? else {
			return Ok(None);
		};

		let mut this = this
			.as_any()
			.downcast::<Self>()
			.expect("downcast failed for MrPackFormatImpl");

		this.archive = Some(path);
		Ok(Some(this))
	}

	async fn from_manifest_bytes(
		bytes: Arc<Vec<u8>>,
	) -> LauncherResult<Option<Box<dyn InstallableModpackFormatExt>>>
	where
		Self: Sized,
	{
		let serialized: MrPackManifest = match serde_json::from_slice(&bytes) {
			Ok(manifest) => manifest,
			Err(e) => {
				tracing::debug!("failed to deserialize modpack as mrpack: {}", e);
				return Ok(None);
			}
		};

		let mut mc_version: Option<String> = None;
		let mut loader: Option<GameLoader> = None;
		let mut loader_version: Option<String> = None;

		for (key, value) in serialized.dependencies.iter() {
			if key == "minecraft" {
				mc_version = Some(value.clone());
			} else {
				loader = GameLoader::from_str(key).ok();
				loader_version = Some(value.clone());
			}
		}

		if mc_version.is_none() {
			tracing::error!("mrpack manifest does not contain a minecraft version");
			return Ok(None);
		}

		if loader.is_none() {
			tracing::error!("mrpack manifest does not contain a valid game loader");
			return Ok(None);
		}

		if loader_version.is_none() {
			tracing::error!("mrpack manifest does not contain a valid loader version");
			return Ok(None);
		}

		Ok(Some(Box::new(Self {
			archive: None,
			raw_manifest: serialized,
			manifest: OnceCell::new(),
			mc_version: mc_version.unwrap(),
			loader: loader.unwrap(),
			loader_version: loader_version.unwrap(),
		})))
	}

	async fn install_modpack_archive(
		modpack_archive: &ModpackArchive,
		cluster: &clusters::Model,
		skip_compatibility: Option<bool>,
		ingress: Option<SubIngress<'_>>,
	) -> LauncherResult<()>
	where
		Self: Sized,
	{
		let ModpackArchive { manifest, path, .. } = modpack_archive;

		download_and_link_packages(cluster, &manifest, skip_compatibility, &ingress).await?;
		copy_overrides_folder(cluster, &path, &ingress).await?;

		Ok(())
	}
}

#[async_trait::async_trait]
impl InstallableModpackFormatExt for MrPackFormatImpl {
	fn as_any(self: Box<Self>) -> Box<dyn std::any::Any + Send + Sync>
	where
		Self: Sized,
	{
		self
	}

	fn kind(&self) -> super::ModpackFormat {
		super::ModpackFormat::MrPack
	}

	async fn manifest(&self) -> LauncherResult<&ModpackManifest> {
		if let Some(manifest) = self.manifest.get() {
			return Ok(manifest);
		}

		let files = to_modpack_files(&self.raw_manifest.files).await?;

		let manifest = ModpackManifest {
			name: self.raw_manifest.name.clone(),
			version: self.raw_manifest.version_id.clone(),
			mc_version: self.mc_version.clone(),
			loader: self.loader.clone(),
			loader_version: self.loader_version.clone(),
			enabled: false,
			files,
		};

		self.manifest
			.set(manifest)
			.expect("failed to cache inner modpack manifest");
		Ok(self.manifest.get().unwrap())
	}

	async fn install_to(
		&self,
		cluster: &clusters::Model,
		skip_compatibility: Option<bool>,
		ingress: Option<SubIngress<'_>>,
	) -> LauncherResult<()> {
		let manifest = self.manifest().await?;

		download_and_link_packages(cluster, &manifest, skip_compatibility, &ingress).await?;

		if let Some(path) = self.archive.as_ref() {
			copy_overrides_folder(cluster, path, &ingress).await?;
		}

		Ok(())
	}
}

pub(super) async fn download_and_link_packages(
	cluster: &clusters::Model,
	manifest: &ModpackManifest,
	skip_compatibility: Option<bool>,
	_ingress: &Option<SubIngress<'_>>,
) -> LauncherResult<()> {
	if cluster.mc_version != manifest.mc_version {
		return Err(ClusterError::MismatchedVersion(
			manifest.mc_version.clone(),
			cluster.mc_version.clone(),
		)
		.into());
	}

	if cluster.mc_loader != manifest.loader {
		return Err(ClusterError::MismatchedLoader(
			manifest.loader.clone(),
			cluster.mc_loader.clone(),
		)
		.into());
	}

	// TODO: Implement loader version checking
	// if cluster.mc_loader_version.is_some_and(|v| v )

	let mut errors = Vec::new();
	let mut packages_to_link = Vec::new();

	// TODO: Ingress
	for file in manifest.files.iter() {
		if file.enabled == false {
			continue;
		}

		match &file.kind {
			ModpackFileKind::Managed((package, version)) => {
				match api::packages::download_package(package, version, None, None).await {
					Ok(model) => packages_to_link.push(model),
					Err(e) => errors.push(e),
				}
			}
			ModpackFileKind::External(package) => {
				match api::packages::download_external_package(
					package,
					cluster,
					None,
					skip_compatibility,
					None,
				)
				.await
				{
					Ok(Some(model)) => packages_to_link.push(model),
					Ok(None) => {}
					Err(e) => errors.push(e),
				}
			}
		}
	}

	let linked = api::packages::link_many_packages_to_cluster(
		&packages_to_link,
		cluster,
		skip_compatibility,
	)
	.await?;
	if linked < packages_to_link.len() as u64 {
		tracing::warn!("not all packages could be linked to the cluster, some errors occurred");
	}
	Ok(())
}

pub async fn copy_overrides_folder(
	cluster: &clusters::Model,
	archive_path: &PathBuf,
	_ingress: &Option<SubIngress<'_>>,
) -> LauncherResult<()> {
	tracing::debug!(
		"extracting overrides from modpack archive: {}",
		archive_path.display()
	);
	let dest = cluster.path().await?;

	io::extract_zip_filtered(
		archive_path,
		dest,
		Some(|entry: &async_zip::StoredZipEntry| {
			entry
				.filename()
				.as_str()
				.is_ok_and(|s| s.starts_with("overrides/"))
		}),
		Some(|name: &str| name.trim_start_matches("overrides/").to_string()),
	)
	.await?;

	Ok(())
}

/// Like [`copy_overrides_folder`], but skips files that already exist on disk.
/// Used during bundle updates so that user customizations are preserved.
pub async fn copy_overrides_folder_no_overwrite(
	cluster: &clusters::Model,
	archive_path: &PathBuf,
) -> LauncherResult<()> {
	let dest = cluster.path().await?;
	extract_overrides_no_overwrite(archive_path, &dest).await
}

/// Extracts the `overrides/` folder from a zip archive into `dest_path`,
/// but **skips** any file that already exists on disk.
pub async fn extract_overrides_no_overwrite(
	archive_path: &PathBuf,
	dest_path: &std::path::Path,
) -> LauncherResult<()> {
	tracing::debug!(
		"extracting overrides (skip existing) from modpack archive: {}",
		archive_path.display()
	);

	let reader = async_zip::tokio::read::fs::ZipFileReader::new(archive_path)
		.await
		.map_err(io::IOError::from)?;
	let entries = reader.file().entries();

	for index in 0..entries.len() {
		let entry = entries.get(index).expect("expected more zip entries");

		let name = match entry.filename().as_str() {
			Ok(s) if s.starts_with("overrides/") => s.trim_start_matches("overrides/").to_string(),
			_ => continue,
		};

		if name.is_empty() {
			continue;
		}

		let name = io::sanitize_path(name);
		let path = dest_path.join(&name);
		let is_dir = entry.dir().map_err(io::IOError::from)?;

		if is_dir {
			io::create_dir_all(&path).await?;
		} else {
			// Skip files that already exist — preserve user customizations
			if path.exists() {
				tracing::trace!(
					path = %path.display(),
					"Skipping override file (already exists)"
				);
				continue;
			}

			if let Some(parent) = path.parent() {
				io::create_dir_all(parent).await?;
			}

			let file = tokio::fs::File::create(&path)
				.await
				.map_err(io::IOError::from)?;
			let writer = tokio::io::BufWriter::new(file);
			let entry_reader = reader
				.reader_without_entry(index)
				.await
				.map_err(io::IOError::from)?;

			futures_lite::io::copy(entry_reader, &mut writer.compat_write())
				.await
				.map_err(io::IOError::from)?;
		}
	}

	Ok(())
}

async fn to_modpack_files(mrpack_files: &Vec<MrPackFile>) -> LauncherResult<Vec<ModpackFile>> {
	#[derive(Clone)]
	struct ToFetch {
		project_id: String,
		version_id: String,
		overrides: Option<PackageOverrides>,
	}

	let mut to_fetch: Vec<ToFetch> = Vec::new();
	let mut files: Vec<ModpackFile> = Vec::new();

	for file in mrpack_files {
		let name = file
			.path
			.split('/')
			.last()
			.unwrap_or(&file.path)
			.to_string();

		if let Some(url) = file
			.downloads
			.iter()
			.find(|url| url.starts_with(MODRINTH_URL_PREFIX))
		{
			// https://cdn.modrinth.com/data/<project_id>/versions/<version_id>/<file_name>
			let paths = url[MODRINTH_URL_PREFIX.len()..]
				.split('/')
				.collect::<Vec<_>>();

			if paths.len() >= 4 {
				let project_id = paths[1];
				let version_id = paths[3];

				to_fetch.push(ToFetch {
					project_id: project_id.to_string(),
					version_id: version_id.to_string(),
					overrides: file.overrides.clone(),
				});
			} else {
				tracing::error!("invalid modrinth file URL: '{}'", url);
			}
		} else {
			let download_url = file
				.downloads
				.first()
				.cloned()
				.ok_or_else(|| {
					tracing::warn!("mrpack file '{}' does not contain a download URL", name)
				})
				.unwrap_or(String::new());

			// the path usually contains the folder name such as "mods" or "resourcepacks"
			// so we can use it to determine the package type
			let package_type = file
				.path
				.split('/')
				.next()
				.and_then(|s| PackageType::try_from(s).ok())
				.unwrap_or(PackageType::Mod);

			files.push(ModpackFile {
				kind: ModpackFileKind::External(ExternalPackage {
					name,
					url: download_url,
					sha1: file.hashes.sha1.clone(),
					size: file.file_size,
					package_type,
				}),
				overrides: file.overrides.clone(),
				enabled: true,
			});
		}
	}

	let managed_packages = Provider::Modrinth
		.get_multiple(
			&to_fetch
				.iter()
				.map(|f| f.project_id.clone())
				.collect::<Vec<_>>(),
		)
		.await?;
	let managed_versions = Provider::Modrinth
		.get_versions(
			&to_fetch
				.iter()
				.map(|f| f.version_id.clone())
				.collect::<Vec<_>>(),
		)
		.await?;

	let mut version_map = managed_versions
		.into_iter()
		.map(|v| (v.project_id.clone(), v))
		.collect::<HashMap<String, ManagedVersion>>();

	for fetched_pkg in managed_packages {
		if let Some(version) = version_map.remove(&fetched_pkg.id) {
			let overrides = to_fetch
				.iter()
				.find(|f| f.project_id == fetched_pkg.id)
				.and_then(|f| f.overrides.clone());

			files.push(ModpackFile {
				kind: ModpackFileKind::Managed((fetched_pkg, version)),
				enabled: true,
				overrides,
			});
		} else {
			tracing::error!("no version found for managed package '{}'", fetched_pkg.id);
		}
	}

	Ok(files)
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MrPackManifest {
	pub format_version: usize,
	pub version_id: String,
	pub name: String,
	pub files: Vec<MrPackFile>,
	pub dependencies: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MrPackFile {
	pub path: String,
	pub hashes: MrPackFileHash,
	#[serde(default)]
	pub env: MrPackFileEnv,
	pub downloads: Vec<String>,
	pub file_size: usize,
	#[serde(default)]
	pub overrides: Option<PackageOverrides>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MrPackFileHash {
	pub sha1: String,
	// pub sha512: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MrPackFileEnv {
	pub client: PackageSide,
	pub server: PackageSide,
}

#[cfg(test)]
mod tests {
	use std::path::PathBuf;

	use async_zip::base::write::ZipFileWriter;
	use async_zip::{Compression, ZipEntryBuilder};
	use tokio_util::compat::TokioAsyncWriteCompatExt;

	use super::extract_overrides_no_overwrite;

	/// Helper: creates a .mrpack zip at `zip_path` with the given entries.
	/// Each entry is (filename_in_zip, content_bytes).
	async fn create_test_zip(zip_path: &std::path::Path, entries: &[(&str, &[u8])]) {
		let file = tokio::fs::File::create(zip_path).await.unwrap();
		let mut writer = ZipFileWriter::new(file.compat_write());

		for &(name, content) in entries {
			let entry = ZipEntryBuilder::new(name.into(), Compression::Stored);
			writer.write_entry_whole(entry, content).await.unwrap();
		}

		writer.close().await.unwrap();
	}

	#[tokio::test]
	async fn test_extract_overrides_no_overwrite_skips_existing() {
		let tmp = std::env::temp_dir().join(format!(
			"onelauncher_test_overrides_{}",
			std::time::SystemTime::now()
				.duration_since(std::time::UNIX_EPOCH)
				.unwrap()
				.as_nanos()
		));
		tokio::fs::create_dir_all(&tmp).await.unwrap();

		let zip_path = tmp.join("test.mrpack");
		let dest = tmp.join("cluster");
		tokio::fs::create_dir_all(&dest).await.unwrap();

		// Create a zip with:
		//   overrides/config.txt       -> "bundle content"
		//   overrides/new_file.txt     -> "new from bundle"
		//   overrides/sub/nested.txt   -> "nested content"
		//   mods/not_an_override.jar   -> "should be ignored"
		create_test_zip(
			&zip_path,
			&[
				("overrides/config.txt", b"bundle content"),
				("overrides/new_file.txt", b"new from bundle"),
				("overrides/sub/nested.txt", b"nested content"),
				("mods/not_an_override.jar", b"should be ignored"),
			],
		)
		.await;

		// Pre-create config.txt with custom content (simulating user edit)
		tokio::fs::write(dest.join("config.txt"), b"user customized")
			.await
			.unwrap();

		// Run
		extract_overrides_no_overwrite(&PathBuf::from(&zip_path), &dest)
			.await
			.unwrap();

		// 1) Existing file should NOT be overwritten
		let config = tokio::fs::read_to_string(dest.join("config.txt"))
			.await
			.unwrap();
		assert_eq!(
			config, "user customized",
			"existing file should be preserved, not overwritten"
		);

		// 2) New file should be extracted
		let new_file = tokio::fs::read_to_string(dest.join("new_file.txt"))
			.await
			.unwrap();
		assert_eq!(
			new_file, "new from bundle",
			"new file should be extracted from zip"
		);

		// 3) Nested file in subdirectory should be extracted
		let nested = tokio::fs::read_to_string(dest.join("sub/nested.txt"))
			.await
			.unwrap();
		assert_eq!(nested, "nested content", "nested file should be extracted");

		// 4) Non-override file should NOT be extracted
		assert!(
			!dest.join("mods/not_an_override.jar").exists(),
			"non-override file should not be extracted"
		);
	}
}
