use std::path::PathBuf;

use futures::{StreamExt, stream};
use interpulse::api::minecraft::{AssetsIndex, DownloadType, Library, Os, Version, VersionInfo};
use interpulse::api::modded::LoaderVersion;
use onelauncher_entity::loader::GameLoader;
use reqwest::Method;

use crate::api::ingress::{init_ingress, IngressSendExt};
use crate::error::{LauncherError, LauncherResult};
use crate::store::ingress::{IngressType, SubIngress, SubIngressExt};
use crate::store::metadata::MetadataError;
use crate::store::{Core, Dirs, State};
use crate::utils::crypto::HashAlgorithm;
use crate::utils::os_ext::OsExt;
use crate::utils::{http, io};

pub async fn download_minecraft_ingressed(
	version: &VersionInfo,
	java_arch: &str,
	force: Option<bool>,
) -> LauncherResult<()> {
	let id = init_ingress(
		IngressType::MinecraftDownload,
		&format!("downloading Minecraft {}", version.id),
		100.0,
	)
	.await?;

	download_minecraft(version, java_arch, Some(&SubIngress::new(&id, 100.0)), force).await
}

// MARK: Main
#[tracing::instrument(skip_all)]
pub async fn download_minecraft(
	version: &VersionInfo,
	java_arch: &str,
	sub_ingress: Option<&SubIngress<'_>>,
	force: Option<bool>,
) -> LauncherResult<()> {

	let asset_index = download_assets_index(version, sub_ingress, force).await?;
	tokio::try_join! {
		download_assets(version.assets == "legacy", asset_index, sub_ingress, force),
		download_client(version, sub_ingress, force),
		download_libraries(version.id.clone(), version.libraries.clone(), java_arch.to_string(), sub_ingress, force),
	}?;

	Ok(())
}

// MARK: Version Info
#[tracing::instrument(skip(ingress, force))]
pub async fn download_version_info(
	version: &Version,
	loader: Option<&LoaderVersion>,
	ingress: Option<&SubIngress<'_>>,
	force: Option<bool>,
) -> LauncherResult<VersionInfo> {
	const TASKS: f64 = 3.33;

	tracing::debug!("loading version info for version {}", version.id);
	let version_id = loader.map_or(version.id.clone(), |it| format!("{}-{}", version.id, it.id));

	ingress.set_ingress_message("fetching version info").await?;

	tracing::debug!("loading minecraft version info for minecraft version {version_id}");

	let path = Dirs::get_versions_dir()
		.await?
		.join(&version_id)
		.join(format!("{version_id}.json"));

	let result = if path.exists() && !force.unwrap_or(false) {
		let data = io::read(path).await?;
		serde_json::from_slice(&data)?
	} else {
		tracing::info!(
			"downloading minecraft version info for minecraft version {}",
			&version_id
		);

		let mut info =
			http::fetch_json_advanced(Method::GET, &version.url, None, None, None, ingress)
				.await?;

		if let Some(loader) = loader {
			let partial: interpulse::api::modded::PartialVersionInfo =
				http::fetch_json_advanced(Method::GET, &loader.url, None, None, None, ingress)
					.await?;

			info = interpulse::api::modded::merge_partial_version(partial, info);
		}

		info.id.clone_from(&version_id);

		io::create_dir_all(
			&path
				.parent()
				.unwrap_or_else(|| panic!("couldn't get version path parent")),
		)
		.await?;
		io::write(&path, &serde_json::to_vec(&info)?).await?;
		info
	};

	ingress.send_ingress(ingress.ingress_total().map(|i| i / TASKS).unwrap_or_default()).await?;

	tracing::debug!("loaded minecraft version info for minecraft version {version_id}");
	Ok(result)
}

// MARK: Assets Index
#[tracing::instrument(skip_all)]
pub async fn download_assets_index(
	version: &VersionInfo,
	ingress: Option<&SubIngress<'_>>,
	force: Option<bool>,
) -> LauncherResult<AssetsIndex> {
	tracing::debug!("loading assets index for version {}", version.id);
	let path = Dirs::get_assets_index_dir()
		.await?
		.join(format!("{}.json", version.asset_index.id));

	let ingress_step = ingress.ingress_total().map(|i| i / 2.0).unwrap_or_default();
	ingress.set_ingress_message("fetching assets index").await?;

	if path.exists() && !force.unwrap_or(false) {
		match io::read_json::<AssetsIndex>(&path).await {
			Ok(data) => {
				ingress.send_ingress(ingress_step).await?;
				return Ok(data);
			}
			Err(err) => {
				tracing::error!("failed to read assets index from cache: {err:?}, downloading...",);
			}
		}
	}

	let data = serde_json::from_slice(
		&http::download(
			Method::GET,
			&version.asset_index.url,
			path,
			None,
			ingress.map(|i| SubIngress::from_sub(i, ingress_step)).as_ref(),
		)
		.await?,
	)?;

	tracing::debug!("loaded assets index for version {}", version.id);

	Ok(data)
}

// MARK: Assets
/// Downloads the assets for a given asset index, returns the number of failed downloads
#[tracing::instrument(skip_all)]
pub async fn download_assets<'a>(
	legacy: bool,
	assets_index: AssetsIndex,
	ingress: Option<&SubIngress<'_>>,
	force: Option<bool>,
) -> LauncherResult<usize> {
	tracing::debug!("loading asssets");
	let len = assets_index.objects.len();

	let dir = if legacy {
		Dirs::get_legacy_assets_dir().await
	} else {
		Dirs::get_assets_object_dir().await
	}?;

	io::create_dir_all(&dir).await?;

	ingress.set_ingress_message("fetching assets").await?;

	let requests = stream::iter(assets_index.objects.into_iter().map(|(name, asset)| {
		let dir = dir.clone();
		async move {
			let hash = &asset.hash;
			let subhash = &hash[0..2];
			let path = if legacy {
				dir.join(name.replace('/', std::path::MAIN_SEPARATOR_STR))
			} else {
				dir.join(subhash).join(hash)
			};

			if !path.exists() || force.unwrap_or(false) {
				http::download_advanced(
					Method::GET,
					&format!("https://resources.download.minecraft.net/{subhash}/{hash}"),
					path,
					None,
					None,
					Some((HashAlgorithm::Sha1, hash)),
					ingress.ingress_sub(|total| total / len as f64).as_ref(),
				)
				.await
				.map(|_| ())
			} else {
				// TODO: Possibly check hash? (not sure if this is a good idea here)
				Ok(())
			}
		}
	}))
	.buffer_unordered(Core::get().fetch_attempts.min(7))
	.collect::<Vec<_>>();

	let mut failed = 0;
	for res in requests.await {
		if let Err(err) = res {
			tracing::error!("failed to download asset: {err:?}");
			failed += 1;
		}
	}

	tracing::debug!("loaded assets");
	Ok(failed)
}

// MARK: Client
pub async fn download_client(
	version: &VersionInfo,
	ingress: Option<&SubIngress<'_>>,
	force: Option<bool>,
) -> LauncherResult<PathBuf> {
	tracing::debug!("loading client for version {}", version.id);
	let client = version
		.downloads
		.get(&DownloadType::Client)
		.ok_or_else(|| anyhow::anyhow!("no client downloads exist for {}", &version.id))?;

	let path = Dirs::get_versions_dir()
		.await?
		.join(version.id.clone())
		.join(format!("{}.jar", version.id));

	if path.exists() && !force.unwrap_or(false) {
		let calculated_hash = HashAlgorithm::Sha1
			.hash_file(&path)
			.await
			.map_err(|_| anyhow::anyhow!("failed to hash file"))?;

		if client.sha1 == calculated_hash {
			tracing::debug!("client already exists, skipping download");
			return Ok(path);
		}

		tracing::warn!("client hash mismatch, redownloading");
	}

	http::download(
		Method::GET,
		&client.url,
		&path,
		Some((HashAlgorithm::Sha1, &client.sha1)),
		ingress,
	)
	.await?;

	Ok(path)
}

// MARK: Libraries
/// Downloads the libraries and returns the number of failed downloads
pub async fn download_libraries(
	version: String,
	libraries: Vec<Library>,
	java_arch: String,
	ingress: Option<&SubIngress<'_>>,
	force: Option<bool>,
) -> LauncherResult<usize> {
	tracing::debug!("loading libraries for version {}", version);

	let lib_dir = Dirs::get_libraries_dir().await?;
	let natives_dest = Dirs::get_natives_dir().await?.join(version);

	io::create_dir_all(&lib_dir).await?;
	io::create_dir_all(&natives_dest).await?;

	let num_files = libraries.len();
	let ingress = ingress.ingress_sub(|total| total / num_files as f64);
	let ingress = ingress.as_ref();
	ingress.set_ingress_message("fetching libraries").await?;

	let requests = stream::iter(libraries.into_iter().map(|lib| {
		let lib_dir = lib_dir.clone();
		let natives_dest = natives_dest.clone();
		let java_arch = java_arch.clone();

		async move {
			if let Some(rules) = &lib.rules
				&& !super::rules::validate_rules(rules, &java_arch, lib.natives.is_some()) {
					tracing::debug!("skipping library {} due to rules", lib.name);
					return Ok::<(), LauncherError>(());
				}

			if !lib.downloadable {
				tracing::debug!("skipping library {} due to downloadability", lib.name);
				return Ok(());
			}

			let artifact_path = interpulse::utils::get_path_from_artifact(&lib.name)?;
			let path = lib_dir.join(&artifact_path);

			if path.exists() && !force.unwrap_or(false) {
				tracing::debug!("library {} is installed, skipping", &lib.name);
				return Ok(());
			}

			tokio::try_join! {
				async {
					if let Some(interpulse::api::minecraft::LibraryDownloads {
						artifact: Some(ref artifact), ..
					}) = lib.downloads
						&& !artifact.url.is_empty() {
							http::download(
								Method::GET,
								&artifact.url,
								&path,
								Some((HashAlgorithm::Sha1, &artifact.sha1)),
								ingress,
							).await?;

							tracing::trace!("fetched library {} to path {:?}", &lib.name, &path);
							return Ok::<_, LauncherError>(());
						}

					let url = [lib.url.as_deref().unwrap_or("https://libraries.minecraft.net/"), &artifact_path].concat();
					http::download(
						Method::GET,
						&url,
						&path,
						None,
						ingress,
					).await?;

					tracing::trace!("fetched library {} to path {:?}", &lib.name, &path);
					Ok::<_, LauncherError>(())
				},
				async {
					if let Some((os_key, classifiers)) = None.or_else(|| Some((
						lib.natives.as_ref()?.get(&Os::native_arch(&java_arch))?,
						lib.downloads.as_ref()?.classifiers.as_ref()?
					))) {
						let parsed = os_key.replace("${arch}", crate::constants::ARCH_WIDTH);
						if let Some(native) = classifiers.get(&parsed) {
							tracing::trace!("found native library {}", &lib.name);

							let data = http::fetch_advanced(
								Method::GET,
								&native.url,
								None,
								Some((HashAlgorithm::Sha1, &native.sha1)),
								None,
								ingress,
							).await?;

							io::unzip_bytes_filtered(
								data.to_vec(),
								Some(|name: &str| !name.starts_with("META-INF")),
								&natives_dest
							).await?;

							tracing::trace!("extracted native {} to path {:?}", &lib.name, &natives_dest);
						}
					}

					Ok(())
				}
			}?;

			Ok(())
		}
	})).buffer_unordered(7).collect::<Vec<_>>();

	let mut failed = 0;
	for res in requests.await {
		if let Err(err) = res {
			tracing::error!("failed to download asset: {err:?}");
			failed += 1;
		}
	}

	Ok(failed)
}

// MARK: Loader Version
/// Gets the loader version for a given Minecraft version and loader
/// If `loader_version` is `None`, it will return the latest stable version else
/// it will return the specified version if found
#[tracing::instrument]
pub async fn get_loader_version(
	mc_version: &str,
	loader: GameLoader,
	loader_version: Option<&str>,
) -> LauncherResult<Option<LoaderVersion>> {
	if loader == GameLoader::Vanilla {
		return Ok(None);
	}

	let state = State::get().await?;
	let metadata = state.metadata.read().await;
	let manifest = metadata.get_modded(loader)?;

	let Some(loaders) = manifest.game_versions.iter().find(|it| {
		it.id
			.replace(interpulse::api::modded::DUMMY_REPLACE_STRING, mc_version)
			== mc_version
	}) else {
		return Err(MetadataError::NoMatchingVersion.into());
	};

	let loader_version = loaders
		.loaders
		.iter()
		.find(|it| loader_version.map_or(it.stable, |version| it.id == version))
		.or_else(|| loaders.loaders.first())
		.cloned()
		.ok_or(MetadataError::NoMatchingLoader)?;

	Ok(Some(loader_version))
}
