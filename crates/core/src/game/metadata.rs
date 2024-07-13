//! Downloads Minecraft metadata to be launched

use futures::prelude::*;
use interpulse as ip;
use tokio::sync::OnceCell;

use interpulse::api::minecraft::{
	Asset, AssetsIndex, Library, Os, Version as GameVersion, VersionInfo as GameVersionInfo,
};
use interpulse::api::modded::LoaderVersion;

use crate::proxy::send::send_ingress;
use crate::proxy::utils::ingress_try_for_each;
use crate::proxy::IngressId;
use crate::store::State;
use crate::utils::http::*;
use crate::utils::io;
use crate::utils::platform::OsExt;

#[tracing::instrument(skip(st, version))]
pub async fn download_minecraft(
	st: &State,
	version: &GameVersionInfo,
	ingress: &IngressId,
	java_arch: &str,
	force: bool,
	updated: bool,
) -> crate::Result<()> {
	tracing::info!("downloading minecraft version {}", version.id);
	let assets_index = download_assets_index(st, version, Some(ingress), force).await?;
	let processors = if version
		.processors
		.as_ref()
		.map(|p| !p.is_empty())
		.unwrap_or(false)
	{
		25.0
	} else {
		40.0
	};

	tokio::try_join! {
		download_client(st, version, Some(ingress), force),
		download_assets(st, version.assets == "legacy", &assets_index, Some(ingress), processors, force),
		download_libraries(st, version.libraries.as_slice(), &version.id, Some(ingress), processors, java_arch, force, updated)
	}?;

	tracing::info!("downloaded minecraft version {}", version.id);
	Ok(())
}

#[tracing::instrument(skip_all, fields(version = version.id.as_str(), loader = ?loader))]
#[onelauncher_debug::debugger]
pub async fn download_version_info(
	st: &State,
	version: &GameVersion,
	loader: Option<&LoaderVersion>,
	force: Option<bool>,
	ingress: Option<&IngressId>,
) -> crate::Result<GameVersionInfo> {
	let version_id = loader.map_or(version.id.clone(), |it| format!("{}-{}", version.id, it.id));
	tracing::debug!("loading minecraft version info for minecraft version {version_id}");
	let path = st
		.directories
		.version_dir(&version_id)
		.await
		.join(format!("{version_id}.json"));
	let result = if path.exists() && !force.unwrap_or(false) {
		io::read(path)
			.err_into::<crate::Error>()
			.await
			.and_then(|ref it| Ok(serde_json::from_slice(it)?))
	} else {
		tracing::info!(
			"downloading minecraft version info for minecraft version {}",
			&version_id
		);
		let mut info = ip::api::minecraft::fetch_version_info(version).await?;

		if let Some(loader) = loader {
			let partial = ip::api::modded::fetch_partial_version(&loader.url).await?;
			info = ip::api::modded::merge_partial_version(partial, info);
		}

		info.id = version_id.clone();

		write(&path, &serde_json::to_vec(&info)?, &st.io_semaphore).await?;
		Ok(info)
	}?;

	if let Some(ingress) = ingress {
		send_ingress(ingress, 5.0, None).await?;
	}

	tracing::debug!("loaded minecraft version info for minecraft version {version_id}");
	Ok(result)
}

#[tracing::instrument(skip_all)]
#[onelauncher_debug::debugger]
pub async fn download_assets_index(
	st: &State,
	version: &GameVersionInfo,
	ingress: Option<&IngressId>,
	force: bool,
) -> crate::Result<AssetsIndex> {
	tracing::debug!("loading assets index");
	let path = st
		.directories
		.index_dir()
		.await
		.join(format!("{}.json", &version.asset_index.id));

	let result = if path.exists() && !force {
		io::read(path)
			.err_into::<crate::Error>()
			.await
			.and_then(|ref it| Ok(serde_json::from_slice(it)?))
	} else {
		let index = ip::api::minecraft::fetch_assets_index(version).await?;
		write(&path, &serde_json::to_vec(&index)?, &st.io_semaphore).await?;
		tracing::info!("downloaded assets index");
		Ok(index)
	}?;

	if let Some(ingress) = ingress {
		send_ingress(ingress, 5.0, None).await?;
	}

	tracing::debug!("loaded assets index");
	Ok(result)
}

#[tracing::instrument(skip(st, index))]
#[onelauncher_debug::debugger]
pub async fn download_assets(
	st: &State,
	legacy: bool,
	index: &AssetsIndex,
	ingress: Option<&IngressId>,
	ingress_amount: f64,
	force: bool,
) -> crate::Result<()> {
	tracing::debug!("loading minecraft assets");
	let num_futs = index.objects.len();
	let assets = stream::iter(index.objects.iter()).map(Ok::<(&String, &Asset), crate::Error>);

	ingress_try_for_each(
		assets,
		None,
		ingress,
		ingress_amount,
		num_futs,
		None,
		|(name, asset)| async move {
			let hash = &asset.hash;
			let resources = st.directories.object_dir(hash).await;
			let url = format!(
				"https://resources.download.minecraft.net/{sub_hash}/{hash}",
				sub_hash = &hash[..2]
			);
			let fetch_cell = OnceCell::<bytes::Bytes>::new();
			tokio::try_join! {
				async {
					if !resources.exists() || force {
						let resource = fetch_cell.get_or_try_init(|| fetch(&url, Some(hash), &st.fetch_semaphore)).await?;
						write(&resources, resource, &st.io_semaphore).await?;
						tracing::trace!("fetched asset resource with hash {hash}");
					}
					Ok::<_, crate::Error>(())
				},
				async {
					let resources = st.directories.legacy_assets_dir().await.join(
						name.replace('/', &String::from(std::path::MAIN_SEPARATOR))
					);

					if legacy && !resources.exists() || force {
						let resource = fetch_cell.get_or_try_init(|| fetch(&url, Some(hash), &st.fetch_semaphore)).await?;
						write(&resources, resource, &st.io_semaphore).await?;
						tracing::trace!("fetched legacy asset resource with hash {hash}");
					}
					Ok::<_, crate::Error>(())
				},
			}?;

			tracing::trace!("loaded asset resource with hash {hash}");
			Ok(())
		},
	)
	.await?;

	tracing::debug!("loaded minecraft assets");
	Ok(())
}

#[tracing::instrument(skip(st, libraries))]
#[onelauncher_debug::debugger]
#[allow(clippy::too_many_arguments)]
pub async fn download_libraries(
	st: &State,
	libraries: &[Library],
	version: &str,
	ingress: Option<&IngressId>,
	ingress_amount: f64,
	java_arch: &str,
	force: bool,
	updated: bool,
) -> crate::Result<()> {
	tracing::debug!("loading minecraft libraries");

	tokio::try_join! {
		io::create_dir_all(st.directories.libraries_dir().await),
		io::create_dir_all(st.directories.version_natives_dir(version).await)
	}?;

	let num_files = libraries.len();
	ingress_try_for_each(
        stream::iter(libraries.iter())
            .map(Ok::<&Library, crate::Error>),
        None,
        ingress,
        ingress_amount,
        num_files,
        None,
        |lib| async move {
            if let Some(rules) = &lib.rules {
                if !super::rules(rules, java_arch, updated) {
                    tracing::trace!("skipped library {} due to rule", &lib.name);
                    return Ok(());
                }
            }

            tokio::try_join! {
                async {
                    let artifact_path = ip::utils::get_path_from_artifact(&lib.name)?;
                    let path = st.directories.libraries_dir().await.join(&artifact_path);

                    match lib.downloads {
                        _ if path.exists() && !force => Ok(()),
                        Some(ip::api::minecraft::LibraryDownloads {
                            artifact: Some(ref artifact),
                            ..
                        }) => {
                            let bytes = fetch(&artifact.url, Some(&artifact.sha1), &st.fetch_semaphore).await?;
                            write(&path, &bytes, &st.io_semaphore).await?;
                            tracing::trace!("downloaded library {} into path {:?}", &lib.name, &path);
                            Ok::<_, crate::Error>(())
                        }
                        _ => {
                            let url = [lib.url.as_deref().unwrap_or("https://libraries.minecraft.net/"), &artifact_path].concat();
                            let bytes = fetch(&url, None, &st.fetch_semaphore).await?;
                            write(&path, &bytes, &st.io_semaphore).await?;
                            tracing::trace!("downloaded library {} into path {:?}", &lib.name, &path);
                            Ok::<_, crate::Error>(())
                        }
                    }
                },
                async {
                    if let Some((os_key, classifiers)) = None.or_else(|| Some((
                        lib.natives.as_ref()?.get(&Os::native_arch(java_arch))?,
                        lib.downloads.as_ref()?.classifiers.as_ref()?
                    ))) {
                        let parsed = os_key.replace("${arch}", crate::utils::platform::ARCH_WIDTH);
                        if let Some(native) = classifiers.get(&parsed) {
                            let data = fetch(&native.url, Some(&native.sha1), &st.fetch_semaphore).await?;
                            let reader = std::io::Cursor::new(&data);
                            if let Ok(mut archive) = zip::ZipArchive::new(reader) {
                                match archive.extract(st.directories.version_natives_dir(version).await) {
                                    Ok(_) => tracing::debug!("downloaded native {}", &lib.name),
                                    Err(err) => tracing::error!("failed to download native {}: {}", &lib.name, err)
                                }
                            } else {
                                tracing::error!("failed to extract native {}", &lib.name)
                            }
                        }
                    }

                    Ok(())
                }
            }?;

            tracing::debug!("loaded minecraft library {}", lib.name);
            Ok(())
        }
    ).await?;

	tracing::debug!("downloaded all minecraft libraries");
	Ok(())
}

#[tracing::instrument(skip_all)]
#[onelauncher_debug::debugger]
pub async fn download_client(
	st: &State,
	version: &GameVersionInfo,
	ingress: Option<&IngressId>,
	force: bool,
) -> crate::Result<()> {
	let version_id = &version.id;
	tracing::debug!("loading minecraft client for minecraft version {version_id}");

	let client = version
		.downloads
		.get(&ip::api::minecraft::DownloadType::Client)
		.ok_or(anyhow::anyhow!(
			"no client downloads exist for {version_id}"
		))?;

	let path = st
		.directories
		.version_dir(version_id)
		.await
		.join(format!("{version_id}.jar"));

	if !path.exists() || force {
		let result = fetch(&client.url, Some(&client.sha1), &st.fetch_semaphore).await?;

		write(&path, &result, &st.io_semaphore).await?;
		tracing::trace!("fetched minecraft client version {version_id}");
	}

	if let Some(ingress) = ingress {
		send_ingress(ingress, 9.0, None).await?;
	}

	tracing::debug!("downloaded client for minecraft version {version_id}");
	Ok(())
}
