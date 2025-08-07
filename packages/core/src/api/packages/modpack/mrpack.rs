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
use crate::api::packages::data::{ExternalPackage, PackageSide};
use crate::api::packages::modpack::data::{ModpackFile, ModpackManifest};
use crate::api::packages::modpack::{InstallableModpackFormatExt, ModpackFormatExt};
use crate::api::packages::provider::ProviderExt;
use crate::api::{self};
use crate::error::LauncherResult;
use crate::store::ingress::SubIngress;
use crate::utils::DatabaseModelExt;
use crate::utils::io::{self, IOError};

pub struct MrPackFormatImpl {
	archive: Option<PathBuf>,
	raw_manifest: MrPackManifest,
	manifest: OnceCell<ModpackManifest>,
	mc_version: String,
	loader: GameLoader,
	loader_version: String,
}

const MODRINTH_URL_PREFIX: &str = "https://cdn.modrinth.com/";

// TODO: move from_file fetching to manifest function

#[async_trait::async_trait]
impl ModpackFormatExt for MrPackFormatImpl {
	async fn from_file(
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

		let Some(this) = Self::from_bytes(Arc::new(manifest_file)).await? else {
			return Ok(None);
		};

		let mut this = this
			.as_any()
			.downcast::<Self>()
			.expect("downcast failed for MrPackFormatImpl");

		this.archive = Some(path);
		Ok(Some(this))
	}

	async fn from_bytes(
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
}

#[async_trait::async_trait]
impl InstallableModpackFormatExt for MrPackFormatImpl {
	fn as_any(self: Box<Self>) -> Box<dyn std::any::Any + Send + Sync>
	where
		Self: Sized,
	{
		self
	}

	async fn manifest(&self) -> LauncherResult<&ModpackManifest> {
		if let Some(manifest) = self.manifest.get() {
			return Ok(manifest);
		}

		let mut project_ids_to_fetch: Vec<String> = Vec::new();
		let mut version_ids_to_fetch: Vec<String> = Vec::new();
		let mut files: Vec<ModpackFile> = Vec::new();

		for file in &self.raw_manifest.files {
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

					project_ids_to_fetch.push(project_id.to_string());
					version_ids_to_fetch.push(version_id.to_string())
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

				files.push(ModpackFile::External(ExternalPackage {
					name,
					url: download_url,
					sha1: file.hashes.sha1.clone(),
					size: file.file_size,
					package_type,
				}));
			}
		}

		let managed_packages = Provider::Modrinth
			.get_multiple(&project_ids_to_fetch)
			.await?;
		let managed_versions = Provider::Modrinth
			.get_versions(&version_ids_to_fetch)
			.await?;

		files.extend(
			managed_packages
				.into_iter()
				.zip(managed_versions.into_iter())
				.map(|(package, version)| ModpackFile::Managed((package, version))),
		);

		let manifest = ModpackManifest {
			name: self.raw_manifest.name.clone(),
			version: self.raw_manifest.version_id.clone(),
			mc_version: self.mc_version.clone(),
			loader: self.loader.clone(),
			loader_version: self.loader_version.clone(),
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
			match file {
				ModpackFile::Managed((package, version)) => {
					match api::packages::download_package(package, version, None, None).await {
						Ok(model) => packages_to_link.push(model),
						Err(e) => errors.push(e),
					}
				}
				ModpackFile::External(package) => {
					if let Err(e) = api::packages::download_external_package(
						package,
						cluster,
						None,
						skip_compatibility,
						None,
					)
					.await
					{
						errors.push(e);
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

		if let Some(archive) = &self.archive {
			tracing::debug!(
				"extracting overrides from modpack archive: {}",
				archive.display()
			);
			let dest = cluster.path().await?;

			io::extract_zip_filtered(
				archive,
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
		}

		Ok(())
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrPackManifest {
	pub format_version: usize,
	pub version_id: String,
	pub name: String,
	pub files: Vec<MrPackFile>,
	pub dependencies: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrPackFile {
	pub path: String,
	pub hashes: MrPackFileHash,
	#[serde(default)]
	pub env: MrPackFileEnv,
	pub downloads: Vec<String>,
	pub file_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrPackFileHash {
	pub sha1: String,
	// pub sha512: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrPackFileEnv {
	pub client: PackageSide,
	pub server: PackageSide,
}
