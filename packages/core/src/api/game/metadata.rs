use std::path::PathBuf;

use futures::{StreamExt, stream};
use interpulse::api::minecraft::{AssetsIndex, DownloadType, Library, Os, OsRule, Rule, RuleAction, Version, VersionInfo};
use interpulse::api::modded::LoaderVersion;
use regex::Regex;
use reqwest::Method;

use crate::api::ingress::{init_ingress, send_ingress_ref_opt};
use crate::error::{LauncherError, LauncherResult};
use crate::store::ingress::{IngressRef, IngressType};
use crate::store::{Core, Dirs};
use crate::utils::crypto::HashAlgorithm;
use crate::utils::ext::OsExt;
use crate::utils::http;
use crate::utils::io;

pub async fn download_minecraft_ingressed(
	version: &VersionInfo,
	java_arch: &str,
	force: Option<bool>
) -> LauncherResult<()> {
	let id = init_ingress(
		IngressType::MinecraftDownload,
		&format!("Downloading Minecraft {}", version.id),
		100.0,
	)
	.await?;

	let ingress_ref = IngressRef::new(&id, 100.0);

	download_minecraft(version, java_arch, Some(&ingress_ref), force).await
}

// MARK: Main
#[tracing::instrument(skip_all)]
pub async fn download_minecraft(
	version: &VersionInfo,
	java_arch: &str,
	ingress_ref: Option<&IngressRef<'_>>,
	force: Option<bool>
) -> LauncherResult<()> {

	const TASKS: f64 = 4.0;
	let ingress_ref = ingress_ref.map(|i| i.with_increment(i.increment_by / TASKS));
	let ingress_ref = ingress_ref.as_ref();

	let asset_index = download_assets_index(version, ingress_ref, force).await?;
	tokio::try_join! {
		download_assets(version.assets == "legacy", &asset_index, ingress_ref, force),
		download_client(version, ingress_ref, force),
		download_libraries(&version.id, &version.libraries, java_arch, ingress_ref, force),
	}?;

	Ok(())
}

// MARK: Version Info
#[tracing::instrument(skip_all)]
pub async fn download_version_info(
	version: &Version,
	loader: Option<&LoaderVersion>,
	ingress_ref: Option<&IngressRef<'_>>,
	force: Option<bool>,
) -> LauncherResult<VersionInfo> {
	tracing::debug!("loading version info for version {}", version.id);
	let version_id = loader.map_or(version.id.clone(), |it| format!("{}-{}", version.id, it.id));

	let ingress_ref = ingress_ref.map(|i| i.with_increment(i.increment_by / 3.33));
	let ingress_ref = ingress_ref.as_ref();

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
			http::fetch_json_advanced(Method::GET, &version.url, None, None, None, ingress_ref).await?;

		if let Some(loader) = loader {
			let partial: interpulse::api::modded::PartialVersionInfo =
				http::fetch_json_advanced(Method::GET, &loader.url, None, None, None, ingress_ref)
					.await?;

			info = interpulse::api::modded::merge_partial_version(partial, info);
		}

		info.id.clone_from(&version_id);

		io::create_dir_all(&path.parent().unwrap_or_else(|| panic!("couldn't get version path parent"))).await?;
		io::write(&path, &serde_json::to_vec(&info)?).await?;
		info
	};

	send_ingress_ref_opt(ingress_ref).await?;

	tracing::debug!("loaded minecraft version info for minecraft version {version_id}");
	Ok(result)
}

// MARK: Assets Index
#[tracing::instrument(skip_all)]
pub async fn download_assets_index(
	version: &VersionInfo,
	ingress_ref: Option<&IngressRef<'_>>,
	force: Option<bool>,
) -> LauncherResult<AssetsIndex> {
	tracing::debug!("loading assets index for version {}", version.id);
	let path = Dirs::get_assets_index_dir()
		.await?
		.join(version.asset_index.id.clone());

	let ingress_ref = ingress_ref.map(|i| i.with_increment(i.increment_by / 2.0));
	let ingress_ref = ingress_ref.as_ref();
	send_ingress_ref_opt(ingress_ref).await?;

	if path.exists() && !force.unwrap_or(false) {
		match io::read_json::<AssetsIndex>(&path).await {
			Ok(data) => {
				send_ingress_ref_opt(ingress_ref).await?;
				return Ok(data);
			}
			Err(err) => tracing::error!(
				"failed to read assets index from cache: {err:?}, downloading...",
			),
		}
	}

	let data = serde_json::from_slice(&http::download(
		Method::GET,
		&version.asset_index.url,
		path,
		None,
		ingress_ref,
	)
	.await?)?;

	tracing::debug!("loaded assets index for version {}", version.id);

	Ok(data)
}

// MARK: Assets
/// Downloads the assets for a given asset index, returns the number of failed downloads
#[tracing::instrument(skip_all)]
pub async fn download_assets(
	legacy: bool,
	assets_index: &AssetsIndex,
	ingress_ref: Option<&IngressRef<'_>>,
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

	let ingress_ref = ingress_ref.map(|i| i.with_increment(i.increment_by / len as f64));
	let ingress_ref = ingress_ref.as_ref();

	let requests = stream::iter(assets_index.objects.iter().map(|(name, asset)| async {
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
				ingress_ref,
			)
			.await.map(|_| ())
		} else {
			// TODO: Possibly check hash? (not sure if this is a good idea here)
			Ok(())
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
	ingress_ref: Option<&IngressRef<'_>>,
	force: Option<bool>,
) -> LauncherResult<PathBuf> {
	tracing::debug!("loading client for version {}", version.id);
	let client = version.downloads
		.get(&DownloadType::Client)
		.ok_or_else(|| anyhow::anyhow!("no client downloads exist for {}", &version.id))?;

	let path = Dirs::get_versions_dir().await?.join(version.id.clone()).join(format!("{}.jar", version.id));
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

	let ingress_ref = ingress_ref.map(|i| i.with_increment(i.increment_by / 2.0));
	let ingress_ref = ingress_ref.as_ref();

	http::download(Method::GET, &client.url, &path, Some((HashAlgorithm::Sha1, &client.sha1)), ingress_ref).await?;

	Ok(path)
}

// MARK: Libraries
/// Downloads the libraries and returns the number of failed downloads
pub async fn download_libraries(
	version: &str,
	libraries: &[Library],
	java_arch: &str,
	ingress_ref: Option<&IngressRef<'_>>,
	force: Option<bool>,
) -> LauncherResult<usize> {
	tracing::debug!("loading libraries for version {}", version);

	let lib_dir = Dirs::get_libraries_dir().await?;
	let natives_dir = Dirs::get_natives_dir().await?;

	io::create_dir_all(&lib_dir).await?;
	io::create_dir_all(&natives_dir).await?;

	let num_files = libraries.len();

	let ingress_ref = ingress_ref.map(|i| i.with_increment(i.increment_by / num_files as f64));
	let ingress_ref = ingress_ref.as_ref();

	let requests = stream::iter(libraries.iter().map(|lib| async {
		if let Some(rules) = &lib.rules {
			if !validate_rules(rules, java_arch, lib.natives.is_some()) {
				tracing::debug!("skipping library {} due to rules", lib.name);
				return Ok::<(), LauncherError>(());
			}
		}

		if !lib.downloadable {
			tracing::debug!("skipping library {} due to downloadability", lib.name);
			return Ok(());
		}

		tokio::try_join! {
			async {
				let artifact_path = interpulse::utils::get_path_from_artifact(&lib.name)?;
				let path = lib_dir.join(&artifact_path);

				if path.exists() && !force.unwrap_or(false) {
					return Ok(());
				}

				if let Some(interpulse::api::minecraft::LibraryDownloads {
					artifact: Some(ref artifact), ..
				}) = lib.downloads {
					if !artifact.url.is_empty() {
						http::download(
							Method::GET,
							&artifact.url,
							&path,
							Some((HashAlgorithm::Sha1, &artifact.sha1)),
							ingress_ref,
						).await?;

						tracing::trace!("fetched library {} to path {:?}", &lib.name, &path);
						return Ok::<_, LauncherError>(());
					}
				}

				let url = [lib.url.as_deref().unwrap_or("https://libraries.minecraft.net/"), &artifact_path].concat();
				http::download(
					Method::GET,
					&url,
					&path,
					None,
					ingress_ref,
				).await?;

				tracing::trace!("fetched library {} to path {:?}", &lib.name, &path);
				Ok::<_, LauncherError>(())
			},
			async {
				if let Some((os_key, classifiers)) = None.or_else(|| Some((
					lib.natives.as_ref()?.get(&Os::native_arch(java_arch))?,
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
							ingress_ref,
						).await?;

						let dest = natives_dir.join(version);
						io::unzip_bytes(data.to_vec(), &dest).await?;
						tracing::trace!("extracted native {} to path {:?}", &lib.name, &dest);
					}
				}

				Ok(())
			}
		}?;

		Ok(())
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

// MARK: Rules
#[tracing::instrument(level = "debug")]
fn validate_rules(rules: &[Rule], java_version: &str, updated: bool) -> bool {
	let mut rule = rules
		.iter()
		.map(|r| validate_rule(r, java_version, updated))
		.collect::<Vec<Option<bool>>>();
	if rules
		.iter()
		.all(|r| matches!(r.action, RuleAction::Disallow))
	{
		rule.push(Some(true));
	}

	!(rule.iter().any(|r| r == &Some(false)) || rule.iter().all(Option::is_none))
}

/// Parses a Minecraft library feature or OS rule.
/// Is disallowed -> Don't include it
/// Is not allowed -> Don't include it
/// Is allowed -> Include it
#[tracing::instrument(level = "debug")]
fn validate_rule(rule: &Rule, java_version: &str, updated: bool) -> Option<bool> {
	let result = match rule {
		Rule {
			os: Some(os), ..
		} => validate_os_rule(os, java_version, updated),
		Rule {
			features: Some(features),
			..
		} => {
			!features.is_demo_user.unwrap_or(true)
				|| features.has_custom_resolution.unwrap_or(false)
				|| !features.has_quick_plays_support.unwrap_or(true)
				|| !features.is_quick_play_multiplayer.unwrap_or(true)
				|| !features.is_quick_play_realms.unwrap_or(true)
				|| !features.is_quick_play_singleplayer.unwrap_or(true)
		}
		_ => return Some(true),
	};

	match rule.action {
		RuleAction::Allow => {
			if result {
				Some(true)
			} else {
				Some(false)
			}
		}
		RuleAction::Disallow => {
			if result {
				Some(false)
			} else {
				None
			}
		}
	}
}

#[must_use]
fn validate_os_rule(rule: &OsRule, java_arch: &str, updated: bool) -> bool {
	let mut rule_match = true;

	if let Some(ref arch) = rule.arch {
		rule_match &= !matches!(arch.as_str(), "x86" | "arm");
	}

	if let Some(name) = &rule.name {
		if updated && (name != &Os::LinuxArm64 || name != &Os::LinuxArm32) {
			rule_match &= &Os::native() == name || &Os::native_arch(java_arch) == name;
		} else {
			rule_match &= &Os::native_arch(java_arch) == name;
		}
	}

	if let Some(version) = &rule.version {
		if let Ok(regex) = Regex::new(version.as_str()) {
			rule_match &= regex.is_match(&sysinfo::System::os_version().unwrap_or_default());
		}
	}

	rule_match
}