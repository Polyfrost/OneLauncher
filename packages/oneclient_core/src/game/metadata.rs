use std::path::PathBuf;

use futures_util::{StreamExt, stream};
use interfrost::api::minecraft::{AssetsIndex, DownloadType, Library, Os, Version, VersionInfo};
use interfrost::api::modded::LoaderVersion;
use reqwest::Method;
use tracing::instrument;

use crate::game::GameError;
use crate::game::download::{download_to_path, fetch_bytes_verified};
use crate::game::rules::validate_rules;
use crate::metadata::MetadataError;
use crate::metadata::MetadataStore;
use crate::os_ext::OsExt;
use crate::packages::domain::GameLoader;
use crate::notification::GroupedProgressSession;
use crate::paths;
use crate::state::LauncherServices;
use crate::{LauncherError, LauncherResult};

#[instrument(skip_all)]
pub async fn download_minecraft(
    services: &LauncherServices,
    progress: &GroupedProgressSession,
    version: &VersionInfo,
    java_arch: &str,
    minecraft_updated: bool,
    force: bool,
) -> LauncherResult<()> {
    let asset_index = download_assets_index(services, progress, version, force).await?;
    tokio::try_join!(
        download_assets(
            services,
            progress,
            version.assets == "legacy",
            asset_index,
            force
        ),
        download_client(services, progress, version, force),
        download_libraries(
            services,
            progress,
            version.id.clone(),
            version.libraries.clone(),
            java_arch,
            minecraft_updated,
            force,
        ),
    )?;

    Ok(())
}

#[instrument(skip(services, progress))]
pub async fn download_version_info(
    services: &LauncherServices,
    progress: Option<&GroupedProgressSession>,
    version: &Version,
    loader: Option<&LoaderVersion>,
    force: bool,
) -> LauncherResult<VersionInfo> {
    let version_id = loader
        .map(|it| format!("{}-{}", version.id, it.id))
        .unwrap_or_else(|| version.id.clone());

    let path = paths::versions_dir()?
        .join(&version_id)
        .join(format!("{version_id}.json"));

    let result = if path.exists() && !force {
        let data = polyio::read(&path).await?;
        serde_json::from_slice(&data)?
    } else {
        tracing::info!(
            version_id = %version_id,
            "downloading Minecraft version metadata"
        );

        let version_url = version.url.parse().map_err(LauncherError::UrlError)?;
        let requester = services.requester.clone();
        let mut info: VersionInfo = match progress {
            Some(progress) => {
                progress
                    .run_child(format!("Version metadata ({version_id})"), 1, |child| {
                        let requester = requester.clone();
                        async move {
                            child.set_progress(0, Some(1));
                            let result = requester
                                .send_json(Method::GET, version_url, None, &[])
                                .await
                                .map_err(LauncherError::from)?;
                            child.set_progress(1, Some(1));
                            Ok::<VersionInfo, LauncherError>(result)
                        }
                    })
                    .await?
            }
            None => requester
                .send_json(Method::GET, version_url, None, &[])
                .await
                .map_err(LauncherError::from)?,
        };

        if let Some(loader) = loader {
            let loader_url = loader.url.parse().map_err(LauncherError::UrlError)?;
            let requester = services.requester.clone();
            let partial: interfrost::api::modded::PartialVersionInfo = match progress {
                Some(progress) => {
                    progress
                        .run_child(format!("Loader metadata ({version_id})"), 1, |child| {
                            let requester = requester.clone();
                            async move {
                                child.set_progress(0, Some(1));
                                let result = requester
                                    .send_json(Method::GET, loader_url, None, &[])
                                    .await
                                    .map_err(LauncherError::from)?;
                                child.set_progress(1, Some(1));
                                Ok::<interfrost::api::modded::PartialVersionInfo, LauncherError>(
                                    result,
                                )
                            }
                        })
                        .await?
                }
                None => requester
                    .send_json(Method::GET, loader_url, None, &[])
                    .await
                    .map_err(LauncherError::from)?,
            };

            info = interfrost::api::modded::merge_partial_version(partial, info);

            for lib in &mut info.libraries {
                lib.name = lib.name.replace("${interpulse.gameVersion}", &version.id);
            }
        }

        info.id.clone_from(&version_id);

        if let Some(parent) = path.parent() {
            polyio::create_dir_all(parent).await?;
        }

        polyio::write(&path, &serde_json::to_vec(&info)?).await?;

        info
    };

    Ok(result)
}

#[instrument(skip_all)]
pub async fn download_assets_index(
    services: &LauncherServices,
    progress: &GroupedProgressSession,
    version: &VersionInfo,
    force: bool,
) -> LauncherResult<AssetsIndex> {
    let path = paths::assets_index_dir()?.join(format!("{}.json", version.asset_index.id));

    if path.exists() && !force {
        if let Ok(data) = polyio::read_json::<AssetsIndex>(&path).await {
            return Ok(data);
        }
        tracing::warn!("cached assets index is invalid, redownloading");
    }

    download_to_path(
        &services.requester,
        &services.notifier,
        progress,
        format!("Assets index ({})", version.asset_index.id),
        &version.asset_index.url,
        &path,
        Some(version.asset_index.sha1.as_str()),
    )
    .await?;

    polyio::read_json(&path).await.map_err(Into::into)
}

#[instrument(skip_all)]
pub async fn download_assets(
    services: &LauncherServices,
    progress: &GroupedProgressSession,
    legacy: bool,
    assets_index: AssetsIndex,
    force: bool,
) -> LauncherResult<usize> {
    let dir = if legacy {
        paths::legacy_assets_dir()?
    } else {
        paths::assets_object_dir()?
    };

    polyio::create_dir_all(&dir).await?;

    let requester = services.requester.clone();
    let notifier = services.notifier.clone();
    let progress = progress.clone();
    let requests = stream::iter(assets_index.objects.into_iter().map(|(name, asset)| {
        let dir = dir.clone();
        let requester = requester.clone();
        let notifier = notifier.clone();
        let progress = progress.clone();

        async move {
            let hash = &asset.hash;
            let subhash = &hash[0..2];
            let path = if legacy {
                dir.join(name.replace('/', std::path::MAIN_SEPARATOR_STR))
            } else {
                dir.join(subhash).join(hash)
            };

            if path.exists() && !force {
                return Ok::<(), LauncherError>(());
            }

            let url = format!("https://resources.download.minecraft.net/{subhash}/{hash}");
            download_to_path(
                &requester,
                &notifier,
                &progress,
                format!("Asset {name}"),
                &url,
                &path,
                Some(hash),
            )
            .await
        }
    }))
    .buffer_unordered(7)
    .collect::<Vec<_>>();

    let mut failed = 0;
    for res in requests.await {
        if let Err(err) = res {
            tracing::error!("failed to download asset: {err:?}");
            failed += 1;
        }
    }

    Ok(failed)
}

pub async fn download_client(
    services: &LauncherServices,
    progress: &GroupedProgressSession,
    version: &VersionInfo,
    force: bool,
) -> LauncherResult<PathBuf> {
    let client = version
        .downloads
        .get(&DownloadType::Client)
        .ok_or_else(|| GameError::NoClientDownload(version.id.clone()))?;

    let path = paths::versions_dir()?
        .join(&version.id)
        .join(format!("{}.jar", version.id));

    if path.exists()
        && !force
        && let Ok(actual) = crate::crypto::sha1_file(&path).await
    {
        if crate::crypto::normalize_hash(&actual) == crate::crypto::normalize_hash(&client.sha1) {
            return Ok(path);
        }
        tracing::warn!("client hash mismatch, redownloading");
    }

    download_to_path(
        &services.requester,
        &services.notifier,
        progress,
        format!("Client {}", version.id),
        &client.url,
        &path,
        Some(&client.sha1),
    )
    .await?;
    Ok(path)
}

pub fn libraries_missing(
    version_info: &VersionInfo,
    java_arch: &str,
    minecraft_updated: bool,
) -> LauncherResult<bool> {
    let lib_dir = paths::libraries_dir()?;
    for lib in &version_info.libraries {
        if let Some(rules) = &lib.rules
            && !validate_rules(rules, java_arch, minecraft_updated)
        {
            continue;
        }
        if !lib.include_in_classpath {
            continue;
        }
        let Ok(rel) = interfrost::utils::get_path_from_artifact(&lib.name) else {
            continue;
        };
        if !lib_dir.join(&rel).exists() {
            tracing::warn!(library = %lib.name, "missing classpath library; will repair");
            return Ok(true);
        }
    }
    Ok(false)
}

fn lib_short(name: &str) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    match parts.as_slice() {
        [_group, artifact, version, ..] => format!("{artifact} {version}"),
        _ => name.to_string(),
    }
}

pub async fn download_libraries(
    services: &LauncherServices,
    progress: &GroupedProgressSession,
    version: String,
    libraries: Vec<Library>,
    java_arch: &str,
    minecraft_updated: bool,
    force: bool,
) -> LauncherResult<usize> {
    let lib_dir = paths::libraries_dir()?;
    let natives_dest = paths::natives_dir()?.join(&version);
    let java_arch = java_arch.to_string();

    polyio::create_dir_all(&lib_dir).await?;
    polyio::create_dir_all(&natives_dest).await?;

    let requests = stream::iter(libraries.into_iter().map(|lib| {
        let lib_dir = lib_dir.clone();
        let natives_dest = natives_dest.clone();
        let java_arch = java_arch.clone();
        let requester = services.requester.clone();
        let notifier = services.notifier.clone();
        let progress = progress.clone();

        async move {
            if let Some(rules) = &lib.rules
                && !validate_rules(rules, &java_arch, minecraft_updated)
            {
                return Ok::<(), LauncherError>(());
            }

            if !lib.downloadable {
                return Ok(());
            }

            let artifact_path = interfrost::utils::get_path_from_artifact(&lib.name)
                .map_err(|_| GameError::LibraryPath(lib.name.clone()))?;
            let path = lib_dir.join(&artifact_path);

            if path.exists() && !force {
                return Ok(());
            }

            tokio::try_join!(
                async {
                    if let Some(interfrost::api::minecraft::LibraryDownloads {
                        artifact: Some(ref artifact),
                        ..
                    }) = lib.downloads
                        && !artifact.url.is_empty()
                    {
                        download_to_path(
                            &requester,
                            &notifier,
                            &progress,
                            format!("Library {}", lib_short(&lib.name)),
                            &artifact.url,
                            &path,
                            Some(&artifact.sha1),
                        )
                        .await?;
                        return Ok::<_, LauncherError>(());
                    }

                    let url = [
                        lib.url
                            .as_deref()
                            .unwrap_or("https://libraries.minecraft.net/"),
                        &artifact_path,
                    ]
                    .concat();
                    download_to_path(
                        &requester,
                        &notifier,
                        &progress,
                        format!("Library {}", lib_short(&lib.name)),
                        &url,
                        &path,
                        None,
                    )
                    .await?;
                    Ok(())
                },
                async {
                    if let Some((os_key, classifiers)) = lib.natives.as_ref().and_then(|natives| {
                        Some((
                            natives.get(&Os::native_arch(&java_arch))?,
                            lib.downloads.as_ref()?.classifiers.as_ref()?,
                        ))
                    }) {
                        let parsed = os_key.replace("${arch}", crate::constants::ARCH_WIDTH);
                        if let Some(native) = classifiers.get(&parsed) {
                            let data = fetch_bytes_verified(
                                &requester,
                                &notifier,
                                &progress,
                                format!("Natives {}", lib_short(&lib.name)),
                                &native.url,
                                &native.sha1,
                            )
                            .await?;

                            let extract = progress.child(format!("Natives {}", lib_short(&lib.name)), 1);
                            extract.set_phase(crate::notification::TaskPhase::Extracting);
                            polyio::unzip_bytes_filtered(
                                data,
                                Some(|name: &str| !name.starts_with("META-INF")),
                                &natives_dest,
                            )
                            .await?;
                            extract.finish();
                        }
                    }

                    Ok(())
                }
            )?;

            Ok(())
        }
    }))
    .buffer_unordered(7)
    .collect::<Vec<_>>();

    let mut failed = 0;
    for res in requests.await {
        if let Err(err) = res {
            tracing::error!("failed to download library: {err:?}");
            failed += 1;
        }
    }

    Ok(failed)
}

pub async fn get_loader_versions(
    metadata: &mut MetadataStore,
    services: &LauncherServices,
    mc_version: &str,
    loader: GameLoader,
) -> LauncherResult<Vec<String>> {
    if loader == GameLoader::Vanilla {
        return Ok(Vec::new());
    }

    let manifest = metadata.get_modded_or_fetch(services, loader).await?;
    for entry in &manifest.game_versions {
        let id = entry
            .id
            .replace("${interpulse.gameVersion}", mc_version)
            .replace(interfrost::api::modded::DUMMY_REPLACE_STRING, mc_version);
        if id == mc_version {
            return Ok(entry.loaders.iter().map(|l| l.id.clone()).collect());
        }
    }
    Ok(Vec::new())
}

pub async fn get_loader_version(
    metadata: &mut MetadataStore,
    services: &LauncherServices,
    mc_version: &str,
    loader: GameLoader,
    loader_version: Option<&str>,
) -> LauncherResult<Option<LoaderVersion>> {
    if loader == GameLoader::Vanilla {
        return Ok(None);
    }

    let resolve_from_manifest = |manifest: &interfrost::api::modded::Manifest| {
        let mut saw_matching_game_version = false;

        for entry in &manifest.game_versions {
            if entry
                .id
                .replace("${interpulse.gameVersion}", mc_version)
                .replace(interfrost::api::modded::DUMMY_REPLACE_STRING, mc_version)
                != mc_version
            {
                continue;
            }

            saw_matching_game_version = true;

            if let Some(requested) = loader_version {
                if let Some(found) = entry
                    .loaders
                    .iter()
                    .find(|loader_entry| loader_entry.id == requested)
                {
                    return (saw_matching_game_version, Some(found.clone()));
                }
                continue;
            }

            if let Some(found) = entry
                .loaders
                .iter()
                .find(|l| l.stable)
                .or_else(|| entry.loaders.first())
            {
                return (saw_matching_game_version, Some(found.clone()));
            }
        }

        (saw_matching_game_version, None)
    };

    let mut manifest = metadata.get_modded_or_fetch(services, loader).await?;
    let (mut saw_matching, mut resolved) = resolve_from_manifest(manifest);
    if resolved.is_some() {
        return Ok(resolved);
    }

    if !saw_matching || loader_version.is_some() {
        metadata.fetch_all(services).await;
        manifest = metadata.get_modded(loader)?;
        (saw_matching, resolved) = resolve_from_manifest(manifest);
        if resolved.is_some() {
            return Ok(resolved);
        }
    }

    if let Some(requested) = loader_version {
        if !saw_matching {
            return Err(MetadataError::NoMatchingVersion.into());
        }
        return Err(MetadataError::RequestedLoaderVersionNotFound {
            requested: requested.to_string(),
        }
        .into());
    }

    if saw_matching {
        Err(MetadataError::NoMatchingLoader.into())
    } else {
        Err(MetadataError::NoMatchingVersion.into())
    }
}

pub async fn resolve_minecraft_version(
    metadata: &mut MetadataStore,
    services: &LauncherServices,
    mc_version: &str,
) -> LauncherResult<(Version, usize, bool)> {
    let mut manifest = metadata.get_vanilla_or_fetch(services).await?;
    let mut version_index = manifest.versions.iter().position(|it| it.id == mc_version);

    if version_index.is_none() {
        metadata.fetch_all(services).await;
        manifest = metadata.get_vanilla()?;
        version_index = manifest.versions.iter().position(|it| it.id == mc_version);
    }

    let version_index = version_index.ok_or(MetadataError::NoMatchingVersion)?;
    let versions = &manifest.versions;

    Ok((
        versions[version_index].clone(),
        version_index,
        is_version_updated(version_index, versions),
    ))
}

pub async fn get_game_versions(
    metadata: &mut MetadataStore,
    services: &LauncherServices,
) -> LauncherResult<Vec<Version>> {
    let manifest = metadata.get_vanilla_or_fetch(services).await?;
    Ok(manifest.versions.clone())
}

pub async fn get_loaders_for_version(
    metadata: &mut MetadataStore,
    services: &LauncherServices,
    mc_version: &str,
) -> LauncherResult<Vec<GameLoader>> {
    metadata.get_loaders_for_version(services, mc_version).await
}

#[must_use]
pub fn is_version_updated(version_index: usize, versions: &[Version]) -> bool {
    version_index <= versions.iter().position(|x| x.id == "22w16a").unwrap_or(0)
}
