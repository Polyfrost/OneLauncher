use std::collections::HashMap;

use onelauncher_core::api::packages::modpack::data::ModpackArchive;
use onelauncher_core::entity::loader::GameLoader;
use onelauncher_core::error::LauncherResult;
use onelauncher_core::send_error;
use onelauncher_core::store::Dirs;
use onelauncher_core::utils::{http, io};
use reqwest::Method;
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
		let mut bundles_lock = self.bundles.write().await;

		if let Some(entry) = bundles_lock.get(mc_version) {
			if let Some(bundles) = entry.get(&loader) {
				return Ok(bundles.clone());
			}
		}

		let mut found = Vec::new();

		// for (version, loaders) in &manifest.versions {
		// 	if version != mc_version {
		// 		continue;
		// 	}

		// 	let Some(paths) = loaders.get(&loader.get_format_name()) else {
		// 		continue;
		// 	};

		// 	// we will be first checking the disk cache, if that fails we fetch from remote
		// 	for path in paths {
		// 		let url = format!("{}{}", crate::constants::META_URL_BASE, path);
		// 		let res = http::request(Method::HEAD, &url, None).await?;

		// 		// res.

		// 		let name = path.split('/').last().unwrap_or(&format!("bundle-{}-{}-{}.mrpack"));
		// 	}

		// }

		Ok(found.clone())
	}

	/// Fetches the bundles manifest from remote, falling back to a saved copy on disk if available
	#[tracing::instrument]
	pub async fn fetch_cached() -> BundleManifest {
		let url = format!("{}/bundles.json", crate::constants::META_URL_BASE);
		let manifest_path = Self::dir().await.join("bundles.json");

		match http::fetch_json::<BundleManifest>(Method::GET, &url, None, None).await {
			Ok(manifest) => {
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
