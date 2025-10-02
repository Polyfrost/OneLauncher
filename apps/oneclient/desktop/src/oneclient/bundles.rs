use std::collections::HashMap;
use std::path::PathBuf;

use onelauncher_core::api::packages::modpack::data::ModpackArchive;
use onelauncher_core::api::packages::modpack::{InstallableModpackFormatExt, ModpackFormat};
use onelauncher_core::entity::loader::GameLoader;
use onelauncher_core::error::LauncherResult;
use onelauncher_core::send_error;
use onelauncher_core::store::Dirs;
use onelauncher_core::utils::{http, io};
use reqwest::{Method, header};
use tokio::sync::{OnceCell, RwLock};

/// e.g.
/// ```json
/// {
/// 	"versions": {
/// 		"1.21.5": {
/// 			"fabric": ["/generated/hud-fabric-1.21.5.mrpack"],
/// 			"forge": ["/generated/hud-forge-1.21.5.mrpack"]
/// 		}
/// 	},
/// }
/// ```
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
struct BundleManifest {
	pub versions: HashMap<String, HashMap<String, Vec<String>>>,
}

static BUNDLES_STATE: OnceCell<BundlesManager> = OnceCell::const_new();

#[derive(Debug)]
pub struct BundlesManager {
	manifest: RwLock<BundleManifest>,
	bundles: RwLock<HashMap<String, HashMap<GameLoader, Vec<ModpackArchive>>>>,
}

impl BundlesManager {
	pub async fn get() -> &'static Self {
		BUNDLES_STATE
			.get_or_init(|| async {
				let manifest = Self::fetch_cached().await;

				Self {
					manifest: RwLock::new(manifest),
					bundles: RwLock::new(HashMap::new()),
				}
			})
			.await
	}

	#[tracing::instrument]
	pub async fn get_bundles_for(
		&self,
		mc_version: &str,
		loader: onelauncher_core::entity::loader::GameLoader,
	) -> LauncherResult<Vec<ModpackArchive>> {
		let manifest = self.manifest.read().await;
		let bundles_lock = self.bundles.read().await;

		if let Some(entry) = bundles_lock.get(mc_version) {
			if let Some(bundles) = entry.get(&loader) {
				return Ok(bundles.clone());
			}
		}

		// drop read lock as we're gonna acquire a write lock this time
		drop(bundles_lock);

		let mut bundles_lock = self.bundles.write().await;

		let mut found = Vec::new();

		for (version, loaders) in &manifest.versions {
			if version != mc_version {
				continue;
			}

			let Some(paths) = loaders.get(&loader.get_format_name()) else {
				continue;
			};

			// we will be first checking the disk cache, if that fails we fetch from remote
			for path in paths {
				let Some(file_name) = path.split('/').last() else {
					tracing::error!("no bundle name was found in path: {path}");
					continue;
				};

				let disk_path = BundlesManager::dir().await.join("bundles").join(file_name);

				let modpack = match download_and_load_bundle(path, &disk_path).await {
					Ok(modpack) => modpack,
					Err(e) => {
						tracing::error!("failed to load bundle from {path}: {e}");
						continue;
					}
				};

				let manifest = match modpack.manifest().await {
					Ok(manifest) => manifest,
					Err(e) => {
						tracing::error!("failed to load modpack manifest from {path}: {e}");
						continue;
					}
				}
				.clone();

				found.push(ModpackArchive {
					manifest,
					path: disk_path,
					format: modpack.kind(),
				});
			}
		}

		bundles_lock
			.entry(mc_version.to_string())
			.or_default()
			.insert(loader, found.clone());

		Ok(found.clone())
	}

	/// Fetches the bundles manifest from remote, falling back to a saved copy on disk if available
	#[tracing::instrument]
	pub async fn fetch_cached() -> BundleManifest {
		let url = format!("{}/bundles.json", crate::constants::META_URL_BASE);
		let manifest_path = Self::dir().await.join("bundles.json");

		match http::fetch_json::<BundleManifest>(Method::GET, &url, None, None).await {
			Ok(manifest) => {
				io::create_dir_all(manifest_path.parent().unwrap())
					.await
					.unwrap_or_else(|e| {
						tracing::error!("failed to create bundles dir: {e}");
					});

				if let Err(e) = io::write_json(&manifest_path, &manifest).await {
					send_error!("failed to cache bundles manifest to disk: {e}");
				}

				manifest
			}
			Err(e) if manifest_path.exists() => {
				tracing::debug!(
					"falling back to cached bundles manifest, due to error fetching remote: {e}"
				);

				match io::read_json::<BundleManifest>(&manifest_path).await {
					Ok(manifest) => manifest,
					Err(e) => {
						tracing::error!("failed to read cached bundles manifest: {e}");

						BundleManifest::default()
					}
				}
			}
			Err(e) => {
				tracing::error!("failed to fetch bundles manifest from remote: {e}");

				BundleManifest::default()
			}
		}
	}

	/// returns the directory for everything bundle related
	pub async fn dir() -> std::path::PathBuf {
		Dirs::get_caches_dir()
			.await
			.expect("failed to get caches dir")
			.join("oneclient")
			.join("bundles")
	}
}

#[tracing::instrument]
async fn download_and_load_bundle(
	url_path: &str,
	disk_path: &PathBuf,
) -> LauncherResult<Box<dyn InstallableModpackFormatExt>> {
	let url = format!("{}{}", crate::constants::META_URL_BASE, url_path);

	if disk_path.exists() {
		// we check if the remote file is different to the local file
		let res = http::request(Method::HEAD, &url).await?;

		if !res.status().is_success() {
			return Err(anyhow::anyhow!("failed to download bundle from remote: {}", url).into());
		}

		if res.headers().get(reqwest::header::CONTENT_LENGTH).is_none() {
			return Err(anyhow::anyhow!(
				"bundle at {url} missing content-length header, skipping..."
			)
			.into());
		}

		// TODO: check hash if provided in future, for now we check file size :(
		let content_length = res
			.headers()
			.get(header::CONTENT_LENGTH)
			.and_then(|v| v.to_str().ok())
			.and_then(|v| v.parse::<u64>().ok())
			.unwrap_or(0);

		let file_size = io::stat(disk_path).await.map(|m| m.len()).unwrap_or(0);

		tracing::debug!("bundle content length: {content_length}, local file size: {file_size}");
		if content_length == file_size {
			// file is up to date, load from disk
			return Ok(ModpackFormat::from_file(disk_path).await?);
		}
	}

	tracing::debug!("downloading bundle from remote: {url}");
	// if we are at this point, it means we either need to update or download
	http::download(Method::GET, &url, disk_path, None, None).await?;

	Ok(ModpackFormat::from_file(disk_path).await?)
}
