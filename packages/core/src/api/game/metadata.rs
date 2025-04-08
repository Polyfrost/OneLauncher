use futures::{StreamExt, stream};
use interpulse::api::minecraft::{AssetsIndex, Version, VersionInfo};
use interpulse::api::modded::LoaderVersion;
use reqwest::Method;

use crate::api::ingress::{init_ingress, send_ingress_ref_opt};
use crate::error::LauncherResult;
use crate::store::ingress::{IngressRef, IngressType};
use crate::store::{Core, Dirs};
use crate::utils::crypto::HashAlgorithm;
use crate::utils::http::{self, fetch_json_advanced};
use crate::utils::io;

pub async fn download_minecraft_ingressed(version: &VersionInfo, force: Option<bool>) -> LauncherResult<()> {
	let id = init_ingress(
		IngressType::MinecraftDownload,
		&format!("Downloading Minecraft {}", version.id),
		100.0,
	)
	.await?;

	let ingress_ref = IngressRef::new(&id, 100.0);

	download_minecraft(version, Some(&ingress_ref), force).await
}

// MARK: Main
#[tracing::instrument(skip_all)]
pub async fn download_minecraft(version: &VersionInfo, ingress_ref: Option<&IngressRef<'_>>, force: Option<bool>) -> LauncherResult<()> {

	const TASKS: f64 = 2.0;
	let ingress_ref = ingress_ref.map(|i| i.with_increment(i.increment_by / TASKS));
	let ingress_ref = ingress_ref.as_ref();

	let asset_index = download_assets_index(version, ingress_ref, force).await?;
	tokio::try_join! {
		download_assets(version.assets == "legacy", &asset_index, ingress_ref, force)
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
			fetch_json_advanced(Method::GET, &version.url, None, None, None, ingress_ref).await?;

		if let Some(loader) = loader {
			let partial: interpulse::api::modded::PartialVersionInfo =
				fetch_json_advanced(Method::GET, &loader.url, None, None, None, ingress_ref)
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
				"failed to read assets index from cache: {err:?}, downloading again",
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
#[tracing::instrument(skip_all)]
pub async fn download_assets(
	legacy: bool,
	assets_index: &AssetsIndex,
	ingress_ref: Option<&IngressRef<'_>>,
	force: Option<bool>,
) -> LauncherResult<()> {
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
			Ok(())
		}
	}))
	.buffer_unordered(Core::get().fetch_attempts.min(7))
	.collect::<Vec<_>>();

	requests.await;

	Ok(())
}
