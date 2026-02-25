use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use onelauncher_entity::clusters;
use onelauncher_entity::loader::GameLoader;
use onelauncher_entity::package::{PackageType, Provider};
use serde::Deserialize;
use tokio::sync::OnceCell;

use crate::api::packages::data::{ExternalPackage, ManagedVersion, PackageOverrides};
use crate::api::packages::modpack::data::{
	ModpackArchive, ModpackFile, ModpackFileKind, ModpackManifest,
};
use crate::api::packages::modpack::mrpack::MrPackFile;
use crate::api::packages::modpack::{InstallableModpackFormatExt, ModpackFormatExt};
use crate::api::packages::provider::ProviderExt;
use crate::error::LauncherResult;
use crate::store::ingress::SubIngress;
use crate::utils::io::{self, IOError};

pub struct PolyMrPackFormatImpl {
	pub(super) archive: Option<PathBuf>,
	pub(super) raw_manifest: PolyMrPackManifest,
	pub(super) manifest: OnceCell<ModpackManifest>,
	pub(super) mc_version: String,
	pub(super) loader: GameLoader,
	pub(super) loader_version: String,
}

#[async_trait::async_trait]
impl ModpackFormatExt for PolyMrPackFormatImpl {
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
		let serialized: PolyMrPackManifest = match serde_json::from_slice(&bytes) {
			Ok(manifest) => manifest,
			Err(e) => {
				tracing::debug!("failed to deserialize modpack as polymrpack: {}", e);
				return Ok(None);
			}
		};

		let mut mc_version: Option<String> = None;
		let mut loader: Option<GameLoader> = None;
		let mut loader_version: Option<String> = None;

		for (key, value) in &serialized.dependencies {
			if key == "minecraft" {
				mc_version = Some(value.clone());
			} else {
				loader = GameLoader::from_str(key).ok();
				loader_version = Some(value.clone());
			}
		}

		if mc_version.is_none() {
			tracing::error!("polymrpack manifest does not contain a minecraft version");
			return Ok(None);
		}

		if loader.is_none() {
			tracing::error!("polymrpack manifest does not contain a valid game loader");
			return Ok(None);
		}

		if loader_version.is_none() {
			tracing::error!("polymrpack manifest does not contain a valid loader version");
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

		super::mrpack::download_and_link_packages(
			cluster,
			manifest,
			skip_compatibility,
			ingress.as_ref(),
		)
		.await?;
		super::mrpack::copy_overrides_folder(cluster, path, &ingress).await?;

		Ok(())
	}
}

#[async_trait::async_trait]
impl InstallableModpackFormatExt for PolyMrPackFormatImpl {
	fn as_any(self: Box<Self>) -> Box<dyn std::any::Any + Send + Sync>
	where
		Self: Sized,
	{
		self
	}

	fn kind(&self) -> super::ModpackFormat {
		super::ModpackFormat::PolyMrPack
	}

	async fn manifest(&self) -> LauncherResult<&ModpackManifest> {
		if let Some(manifest) = self.manifest.get() {
			return Ok(manifest);
		}

		let files = to_modpack_files(&self.raw_manifest.files)
			.await
			.map_err(|e| anyhow::anyhow!("failed to parse polymrpack files: {e}"))?;

		let manifest = ModpackManifest {
			name: self.raw_manifest.name.clone(),
			version: self.raw_manifest.version_id.clone(),
			mc_version: self.mc_version.clone(),
			loader: self.loader,
			loader_version: self.loader_version.clone(),
			enabled: self.raw_manifest.enabled,
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

		super::mrpack::download_and_link_packages(
			cluster,
			manifest,
			skip_compatibility,
			ingress.as_ref(),
		)
		.await?;

		if let Some(path) = self.archive.as_ref() {
			super::mrpack::copy_overrides_folder(cluster, path, &ingress).await?;
		}

		Ok(())
	}
}

#[allow(clippy::too_many_lines)]
async fn to_modpack_files(mrpack_files: &Vec<PolyMrPackFile>) -> LauncherResult<Vec<ModpackFile>> {
	#[derive(Clone)]
	struct FetchedPackage {
		version_id: String,
		enabled: bool,
		hidden: bool,
		overrides: Option<PackageOverrides>,
	}

	let mut to_fetch: HashMap<String, FetchedPackage> = HashMap::new();
	let mut files: Vec<ModpackFile> = Vec::new();

	for file in mrpack_files {
		let name = file
			.base
			.path
			.split('/')
			.next_back()
			.unwrap_or(&file.base.path)
			.to_string();

		if let Some(url) = file
			.base
			.downloads
			.iter()
			.find(|url| url.starts_with(super::mrpack::MODRINTH_URL_PREFIX))
		{
			// https://cdn.modrinth.com/data/<project_id>/versions/<version_id>/<file_name>
			let paths = url[super::mrpack::MODRINTH_URL_PREFIX.len()..]
				.split('/')
				.collect::<Vec<_>>();

			if paths.len() >= 4 {
				let project_id = paths[1];
				let version_id = paths[3];

				to_fetch.insert(
					project_id.to_string(),
					FetchedPackage {
						version_id: version_id.to_string(),
						enabled: file.enabled,
						hidden: file.hidden,
						overrides: file.base.overrides.clone(),
					},
				);
			} else {
				tracing::error!("invalid modrinth file URL: '{}'", url);
			}
		} else {
			let download_url = file
				.base
				.downloads
				.first()
				.cloned()
				.ok_or_else(|| {
					tracing::warn!("mrpack file '{}' does not contain a download URL", name);
				})
				.unwrap_or(String::new());

			// the path usually contains the folder name such as "mods" or "resourcepacks"
			// so we can use it to determine the package type
			let package_type = file
				.base
				.path
				.split('/')
				.next()
				.map_or(PackageType::Mod, PackageType::from);

			files.push(ModpackFile {
				enabled: file.enabled,
				hidden: file.hidden,
				kind: ModpackFileKind::External(ExternalPackage {
					name,
					url: download_url,
					sha1: file.base.hashes.sha1.clone(),
					size: file.base.file_size,
					package_type,
				}),
				overrides: file.base.overrides.clone(),
			});
		}
	}

	let managed_packages = Provider::Modrinth
		.get_multiple(&to_fetch.keys().cloned().collect::<Vec<_>>())
		.await?;
	let managed_versions = Provider::Modrinth
		.get_versions(
			&to_fetch
				.values()
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
			let fetched = to_fetch.get(&fetched_pkg.id).unwrap();
			files.push(ModpackFile {
				enabled: fetched.enabled,
				hidden: fetched.hidden,
				kind: ModpackFileKind::Managed(Box::new((fetched_pkg, version))),
				overrides: fetched.overrides.clone(),
			});
		} else {
			tracing::error!("no version found for managed package '{}'", fetched_pkg.id);
		}
	}

	Ok(files)
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PolyMrPackManifest {
	pub id: String,
	pub category: String,
	pub enabled: bool,
	// #[serde(rename = "polyFormat")]
	// pub poly_format_version: i32,
	pub update_url: Option<String>,

	pub format_version: usize,
	pub version_id: String,
	pub name: String,
	pub files: Vec<PolyMrPackFile>,
	pub dependencies: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PolyMrPackFile {
	#[serde(flatten)]
	pub base: MrPackFile,
	pub enabled: bool,
	pub hidden: bool,
}
