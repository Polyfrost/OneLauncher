//! Handles fetching metadata via interpulse.

use interpulse::api::minecraft::VersionManifest as MinecraftManifest;
use interpulse::api::modded::Manifest as ModdedManifest;
use serde::{Deserialize, Serialize};

use crate::constants::METADATA_API_URL;
use crate::utils::http::{fetch, read_json, write, FetchSemaphore, IoSemaphore};
use crate::State;

use super::Directories;

/// A structure of manifests and metadata fetching utilities.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Metadata {
	/// The [`MinecraftManifest`] associated with core Minecraft versions.
	pub minecraft: Option<MinecraftManifest>,
	/// The [`ModdedManifest`] associated with core Fabric versions.
	pub fabric: Option<ModdedManifest>,
	/// The [`ModdedManifest`] associated with core Quilt versions.
	pub quilt: Option<ModdedManifest>,
	/// The [`ModdedManifest`] associated with core `NeoForge` versions.
	pub neoforge: Option<ModdedManifest>,
	/// The [`ModdedManifest`] associated with core Forge versions.
	pub forge: Option<ModdedManifest>,
	// /// The [`ModdedManifest`] associated with core Legacy Fabric versions.
	// pub legacy_fabric: Option<ModdedManifest>,
}

impl Metadata {
	/// Get the formatted manifest for a specific [`Metadata`] type.
	fn get_manifest(name: &str, version: usize) -> String {
		format!("{METADATA_API_URL}/{name}/v{version}/manifest.json")
	}

	/// Fetch all available metadata types that are currently [`None`] and get a new [`Metadata`] structure.
	/// Useful for when reading from cache and a certain object not existing
	pub async fn fetch_errored(&mut self, semaphore: &FetchSemaphore) -> crate::Result<bool> {
		let mut did_change = false;
		if self.minecraft.is_none() {
			self.minecraft = Self::fetch_minecraft(semaphore).await;

			if self.minecraft.is_some() {
				did_change = true;
			}
		}

		if self.fabric.is_none() {
			self.fabric = Self::fetch_fabric(semaphore).await;

			if self.fabric.is_some() {
				did_change = true;
			}
		}

		if self.quilt.is_none() {
			self.quilt = Self::fetch_quilt(semaphore).await;

			if self.quilt.is_some() {
				did_change = true;
			}
		}

		if self.neoforge.is_none() {
			self.neoforge = Self::fetch_neoforge(semaphore).await;

			if self.neoforge.is_some() {
				did_change = true;
			}
		}

		if self.forge.is_none() {
			self.forge = Self::fetch_forge(semaphore).await;

			if self.forge.is_some() {
				did_change = true;
			}
		}

		Ok(did_change)
	}

	/// Fetch all available metadata types and get a new [`Metadata`] structure.
	pub async fn fetch(semaphore: &FetchSemaphore) -> crate::Result<Self> {
		let (minecraft, fabric, quilt, neoforge, forge) = tokio::join! {
			Self::fetch_minecraft(semaphore),
			Self::fetch_fabric(semaphore),
			Self::fetch_quilt(semaphore),
			Self::fetch_neoforge(semaphore),
			Self::fetch_forge(semaphore)
		};

		Ok(Self {
			minecraft,
			fabric,
			quilt,
			neoforge,
			forge,
		})
	}

	async fn fetch_minecraft(semaphore: &FetchSemaphore) -> Option<MinecraftManifest> {
		let url = Self::get_manifest(
			"minecraft",
			interpulse::api::minecraft::CURRENT_FORMAT_VERSION,
		);
		fetch_version_manifest(Some(&url), semaphore).await.ok()
	}

	async fn fetch_fabric(semaphore: &FetchSemaphore) -> Option<ModdedManifest> {
		let url = Self::get_manifest(
			"fabric",
			interpulse::api::modded::CURRENT_FABRIC_FORMAT_VERSION,
		);
		fetch_modded_manifest(&url, semaphore).await.ok()
	}

	async fn fetch_quilt(semaphore: &FetchSemaphore) -> Option<ModdedManifest> {
		let url = Self::get_manifest(
			"quilt",
			interpulse::api::modded::CURRENT_QUILT_FORMAT_VERSION,
		);
		fetch_modded_manifest(&url, semaphore).await.ok()
	}

	async fn fetch_neoforge(semaphore: &FetchSemaphore) -> Option<ModdedManifest> {
		let url = Self::get_manifest(
			"neoforge",
			interpulse::api::modded::CURRENT_NEOFORGE_FORMAT_VERSION,
		);
		fetch_modded_manifest(&url, semaphore).await.ok()
	}

	async fn fetch_forge(semaphore: &FetchSemaphore) -> Option<ModdedManifest> {
		let url = Self::get_manifest(
			"forge",
			interpulse::api::modded::CURRENT_FORGE_FORMAT_VERSION,
		);
		fetch_modded_manifest(&url, semaphore).await.ok()
	}

	/// Initialize the core Metadata manager.
	#[tracing::instrument(skip(io_semaphore, fetch_semaphore))]
	#[onelauncher_macros::memory]
	pub async fn initialize(
		dirs: &Directories,
		online: bool,
		io_semaphore: &IoSemaphore,
		fetch_semaphore: &FetchSemaphore,
	) -> crate::Result<Self> {
		let mut metadata = None;
		let path = dirs.caches_dir().await.join("metadata.json");
		let backup = dirs.caches_dir().await.join("metadata.json.bak");
		let mut should_write = false;

		if let Ok(mut metadata_json) = read_json::<Self>(&path, io_semaphore).await {
			should_write = metadata_json.fetch_errored(fetch_semaphore).await?;

			metadata = Some(metadata_json);
		} else if online {
			let res = async {
				let fetch_data = Self::fetch(fetch_semaphore).await?;
				should_write = true;

				metadata = Some(fetch_data);
				Ok::<(), crate::Error>(())
			}
			.await;

			match res {
				Ok(()) => {}
				Err(err) => {
					tracing::warn!("failed to fetch metadata: {err}");
				}
			}
		} else if let Ok(metadata_json) = read_json::<Self>(&backup, io_semaphore).await {
			metadata = Some(metadata_json);
			onelauncher_utils::io::copy(&backup, &path).await?;
		}

		if should_write {
			write(
				&path,
				&serde_json::to_vec(&metadata).unwrap_or_default(),
				io_semaphore,
			)
			.await?;
			write(
				&backup,
				&serde_json::to_vec(&metadata).unwrap_or_default(),
				io_semaphore,
			)
			.await?;
		}

		metadata.map_or_else(
			|| Err(anyhow::anyhow!("failed to fetch launcher metadata").into()),
			Ok,
		)
	}

	/// Update and backup all available metadata.
	pub async fn update() {
		let res = async {
			let state = State::get().await?;
			let fetch_data = Self::fetch(&state.fetch_semaphore).await?;

			let path = state.directories.caches_dir().await.join("metadata.json");
			let backup = state
				.directories
				.caches_dir()
				.await
				.join("metadata.json.bak");

			if path.exists() {
				onelauncher_utils::io::copy(&path, &backup).await?;
			}

			write(
				&path,
				&serde_json::to_vec(&fetch_data)?,
				&state.io_semaphore,
			)
			.await?;

			let mut old_metadata = state.metadata.write().await;
			*old_metadata = fetch_data;

			Ok::<(), crate::Error>(())
		}
		.await;

		match res {
			Ok(()) => {}
			Err(err) => {
				tracing::warn!("failed to update launcher metadata: {err}");
			}
		};
	}
}

async fn fetch_version_manifest(
	url: Option<&str>,
	semaphore: &FetchSemaphore,
) -> crate::Result<MinecraftManifest> {
	let url = url.unwrap_or(interpulse::api::minecraft::VERSION_MANIFEST_URL);
	Ok(serde_json::from_slice(
		&fetch(url, None, semaphore).await.inspect_err(|_| {
			tracing::error!("couldn't fetch version manifest at '{}'", url);
		})?,
	)?)
}

async fn fetch_modded_manifest(
	url: &str,
	semaphore: &FetchSemaphore,
) -> crate::Result<ModdedManifest> {
	Ok(serde_json::from_slice(
		&fetch(url, None, semaphore).await.inspect_err(|_| {
			tracing::error!("couldn't fetch modded manifest at '{}'", url);
		})?,
	)?)
}
