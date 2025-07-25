use std::collections::HashMap;
use std::str::FromStr;

use interpulse::api::minecraft::VersionManifest as VanillaManifest;
use interpulse::api::modded::Manifest as ModdedManifest;
use onelauncher_entity::loader::GameLoader;
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::constants;
use crate::error::{LauncherError, LauncherResult};
use crate::utils::http::fetch_json;
use crate::utils::io;

use super::Dirs;

#[allow(clippy::unsafe_derive_deserialize)]
#[derive(Debug, Default)]
pub struct Metadata {
	initialized: bool,
	inner: MetadataInner,
	version_loader_cache: HashMap<String, Vec<GameLoader>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct MetadataInner {
	minecraft: Option<VanillaManifest>,
	forge: Option<ModdedManifest>,
	neo: Option<ModdedManifest>, // TODO(metadata): change to neoforge
	fabric: Option<ModdedManifest>,
	quilt: Option<ModdedManifest>,
	// legacyfabric: Option<ModdedManifest>,
}

#[onelauncher_macro::error]
#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
	#[error("failed to fetch metadata")]
	FetchError,
	#[error("loader {0} does not use a modded manifest")]
	NotModdedManifest(GameLoader),
	#[error("loader {0} does not use a vanilla manifest")]
	NotVanillaManifest(GameLoader),
	#[error("failed to parse metadata: {0}")]
	ParseError(
		#[from]
		#[skip]
		serde_json::Error,
	),
	#[error("no matching loader found")]
	NoMatchingLoader,
	#[error("no matching version found")]
	NoMatchingVersion,
}

impl Metadata {
	#[must_use]
	pub const fn initialized(&self) -> bool {
		self.initialized
	}

	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	#[tracing::instrument(skip(self))]
	pub async fn get_vanilla_or_fetch(&mut self) -> LauncherResult<&VanillaManifest> {
		if !self.initialized() {
			self.initialize().await?;
		}

		self.get_vanilla()
	}

	#[tracing::instrument(skip(self))]
	pub async fn get_modded_or_fetch(
		&mut self,
		loader: &GameLoader,
	) -> LauncherResult<&ModdedManifest> {
		if !loader.is_modded() {
			return Err(MetadataError::NotModdedManifest(*loader).into());
		}

		if !self.initialized() {
			self.initialize().await?;
		}

		self.get_modded(loader)
	}

	pub fn get_vanilla(&self) -> LauncherResult<&VanillaManifest> {
		self.inner
			.minecraft
			.as_ref()
			.ok_or_else(|| MetadataError::FetchError.into())
	}

	pub fn get_modded(&self, loader: &GameLoader) -> LauncherResult<&ModdedManifest> {
		if !loader.is_modded() {
			return Err(MetadataError::NotModdedManifest(*loader).into());
		}

		match loader {
			GameLoader::Forge => self.inner.forge.as_ref(),
			GameLoader::NeoForge => self.inner.neo.as_ref(),
			GameLoader::Fabric => self.inner.fabric.as_ref(),
			GameLoader::Quilt => self.inner.quilt.as_ref(),
			_ => None,
		}
		.ok_or_else(|| MetadataError::FetchError.into())
	}

	#[tracing::instrument(skip_all)]
	pub async fn initialize(&mut self) -> LauncherResult<()> {
		let path = Dirs::get_caches_dir().await?.join("metadata.json");
		let mut save_file = false;

		let mut metadata = Self::default();

		if let Ok(metadata_json) = io::read_json::<MetadataInner>(&path).await {
			metadata.inner = metadata_json;

			if metadata.refetch_errored().await > 0 {
				save_file = true;
			}
		} else {
			metadata.fetch_all().await;
			save_file = true;
		}

		if save_file {
			io::create_dir_all(&path.parent().expect("couldn't get metadata.json parent")).await?;
			io::write_json(&path, &metadata.inner).await?;
		}

		*self = metadata;
		self.initialized = true;

		Ok(())
	}

	/// Fetches the all missing metadata and returns the amount that were a success
	#[tracing::instrument(skip_all)]
	pub async fn refetch_errored(&mut self) -> u8 {
		let mut changed: u8 = 0;

		macro_rules! check {
			($var:tt, $is_modded:tt) => {
				if self.inner.$var.is_none() {
					match GameLoader::from_str(stringify!($var)) {
						Ok(loader) => match check!(_fetch, loader, $is_modded) {
							Ok(data) => {
								self.inner.$var = Some(data);
								changed += 1;
							}
							Err(err) => {
								tracing::error!("failed to fetch manifest for {}: {}", loader, err);
							}
						},
						Err(err) => tracing::error!("{err}"),
					};
				}
			};

			(_fetch, $loader:expr, false) => {
				fetch_vanilla_manifest($loader).await
			};

			(_fetch, $loader:expr, true) => {
				fetch_modded_manifest($loader).await
			};
		}

		check!(minecraft, false);
		check!(forge, true);
		check!(neo, true);
		check!(fabric, true);
		check!(quilt, true);

		changed
	}

	/// Force fetches all the manifests and sets them in the current instance
	#[tracing::instrument(skip_all)]
	pub async fn fetch_all(&mut self) {
		macro_rules! fetch_manifest {
			($loader:expr, false) => {
				fetch_vanilla_manifest($loader).await
			};

			($loader:expr, true) => {
				fetch_modded_manifest($loader).await
			};
		}

		macro_rules! task {
			($(($var:tt, $is_modded:tt)),*) => {
				let ($($var),*) = tokio::join!(
					$(
						async move {
							if let Ok(loader) = GameLoader::from_str(stringify!($var)) {
								fetch_manifest!(loader, $is_modded).map_err(|e| $crate::send_error!("couldn't fetch {loader} manifest: {e}")).ok()
							} else {
								tracing::error!("loader '{}' is not supported", stringify!($var));
								None
							}
						},
					)*
				);

				$(
					self.inner.$var = $var;
				)*
			};
		}

		task! {
			// (name, is_modded)
			(minecraft, false),
			(forge, true),
			(neo, true),
			(fabric, true),
			(quilt, true)
		};
	}

	/// Returns a list of loaders that are compatible with the given Minecraft version.
	///
	/// WARNING: Can be heavy in certain cases!
	pub async fn get_loaders_for_version(
		&mut self,
		mc_version: &str,
	) -> LauncherResult<Vec<GameLoader>> {
		if !self.initialized() {
			self.initialize().await?;
		}

		if let Some(hit) = self.version_loader_cache.get(mc_version) {
			return Ok(hit.clone());
		}

		let mut loaders = Vec::<GameLoader>::new();
		for loader in GameLoader::modded_loaders() {
			let manifest = match self.get_modded(loader) {
				Ok(manifest) => manifest,
				Err(LauncherError::MetadataError(MetadataError::NotModdedManifest(_))) => continue,
				Err(e) => return Err(e),
			};

			let found = manifest.game_versions.iter().find(|it| it.id == mc_version);
			if found.is_some() {
				loaders.push(*loader);
			}
		}

		self.version_loader_cache
			.insert(mc_version.to_owned(), loaders.clone());

		Ok(loaders)
	}
}

async fn fetch_vanilla_manifest(loader: GameLoader) -> LauncherResult<VanillaManifest> {
	fetch_manifest(loader).await
}

async fn fetch_modded_manifest(loader: GameLoader) -> LauncherResult<ModdedManifest> {
	fetch_manifest(loader).await
}

async fn fetch_manifest<T: DeserializeOwned>(loader: GameLoader) -> LauncherResult<T> {
	fetch_json::<T>(
		Method::GET,
		format!(
			"{}/{}/v{}/manifest.json",
			constants::METADATA_API_URL,
			loader.get_format_name(),
			loader.get_format_version()
		)
		.as_str(),
		None,
		None,
	)
	.await
}
