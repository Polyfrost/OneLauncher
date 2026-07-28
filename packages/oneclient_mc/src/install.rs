use std::path::{Path, PathBuf};

use futures_util::{StreamExt, stream};
use interfrost::api::minecraft::{
    Asset, AssetsIndex, DownloadType, Library, Os, Version, VersionInfo,
};
use interfrost::api::modded::LoaderVersion;
use reqwest::Method;

use crate::download::{download_to_path, fetch_bytes_verified};
use crate::rules::validate_rules;

use crate::manifest::MetadataStore;
use oneclient_common::os_ext::OsExt;
use oneclient_common::domain::GameLoader;
use oneclient_events::{GroupedProgressSession, TaskCategory};
use oneclient_common::paths;
use crate::McCtx;
use crate::error::{McError, McResult};

/// Asset objects are tiny (median ~10 KiB) and latency-bound, so throughput
/// scales with how many are in flight, not with bandwidth.
const ASSET_DOWNLOAD_CONCURRENCY: usize = 32;
/// Libraries are larger and fewer; less fan-out is needed to saturate the link.
const LIBRARY_DOWNLOAD_CONCURRENCY: usize = 16;

/// Everything a version still needs, with what is already on disk subtracted.
///
/// Computed once, up-front, so the size shown before a download starts and the
/// denominators the progress bars fill against come from the same numbers.
/// Without it the estimate assumes a full download while the bars discover
/// their own total as files are added, so neither matches reality.
#[derive(Debug, Default)]
pub struct DownloadPlan {
    pub assets: Vec<(String, Asset)>,
    pub asset_bytes: u64,
    pub libraries: Vec<Library>,
    pub library_bytes: u64,
    pub client: bool,
    pub client_bytes: u64,
}

impl DownloadPlan {
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.asset_bytes + self.library_bytes + self.client_bytes
    }
}

fn asset_object_path(dir: &Path, name: &str, hash: &str, legacy: bool) -> PathBuf {
    if legacy {
        dir.join(name.replace('/', std::path::MAIN_SEPARATOR_STR))
    } else {
        dir.join(&hash[0..2]).join(hash)
    }
}

fn native_download_size(lib: &Library, java_arch: &str) -> u64 {
    let Some((os_key, classifiers)) = lib.natives.as_ref().and_then(|natives| {
        Some((
            natives.get(&Os::native_arch(java_arch))?,
            lib.downloads.as_ref()?.classifiers.as_ref()?,
        ))
    }) else {
        return 0;
    };

    let parsed = os_key.replace("${arch}", oneclient_common::constants::ARCH_WIDTH);
    classifiers
        .get(&parsed)
        .map_or(0, |native| u64::from(native.size))
}

/// Stats every candidate file, so it runs on the blocking pool. An asset index
/// is thousands of entries. Doing it here also means the download fan-outs no
/// longer stat each file from inside a concurrency slot.
#[tracing::instrument(skip_all, level = "debug")]
pub async fn plan_downloads(
    version: &VersionInfo,
    assets_index: AssetsIndex,
    java_arch: &str,
    minecraft_updated: bool,
    force: bool,
) -> McResult<DownloadPlan> {
    let legacy = version.assets == "legacy";
    let asset_dir = if legacy {
        paths::legacy_assets_dir()?
    } else {
        paths::assets_object_dir()?
    };
    let lib_dir = paths::libraries_dir()?;

    let client = version.downloads.get(&DownloadType::Client).map(|client| {
        (
            paths::versions_dir()
                .map(|dir| dir.join(&version.id).join(format!("{}.jar", version.id))),
            u64::from(client.size),
        )
    });

    let libraries = version.libraries.clone();
    let java_arch = java_arch.to_string();

    tokio::task::spawn_blocking(move || {
        let mut plan = DownloadPlan::default();

        for (name, asset) in assets_index.objects {
            let path = asset_object_path(&asset_dir, &name, &asset.hash, legacy);
            if !force && path.exists() {
                continue;
            }
            plan.asset_bytes += u64::from(asset.size);
            plan.assets.push((name, asset));
        }

        for lib in libraries {
            if let Some(rules) = &lib.rules
                && !validate_rules(rules, &java_arch, minecraft_updated)
            {
                continue;
            }
            if !lib.downloadable {
                continue;
            }

            // A library whose coordinates don't resolve is kept in the plan so the
            // download path still reports it as a failure rather than skipping it.
            let Ok(artifact_path) = interfrost::utils::get_path_from_artifact(&lib.name) else {
                plan.libraries.push(lib);
                continue;
            };

            if !force && lib_dir.join(&artifact_path).exists() {
                continue;
            }

            plan.library_bytes += lib
                .downloads
                .as_ref()
                .and_then(|downloads| downloads.artifact.as_ref())
                .map_or(0, |artifact| u64::from(artifact.size));
            plan.library_bytes += native_download_size(&lib, &java_arch);
            plan.libraries.push(lib);
        }

        if let Some((path, size)) = client {
            let present = path.map(|path| path.exists()).unwrap_or(false);
            if force || !present {
                plan.client = true;
                plan.client_bytes = size;
            }
        }

        plan
    })
    .await
    .map_err(std::io::Error::other)
    .map_err(Into::into)
}

#[tracing::instrument(skip_all)]
pub async fn download_minecraft(
    ctx: &McCtx,
    progress: &GroupedProgressSession,
    version: &VersionInfo,
    java_arch: &str,
    minecraft_updated: bool,
    force: bool,
) -> McResult<()> {
    let started = std::time::Instant::now();
    let asset_index = download_assets_index(ctx, progress, version, force).await?;
    let plan = plan_downloads(version, asset_index, java_arch, minecraft_updated, force).await?;

    tracing::info!(
        version = %version.id,
        assets = plan.assets.len(),
        libraries = plan.libraries.len(),
        bytes = plan.total_bytes(),
        "planned minecraft download"
    );

    // Reserve each category's real total before any child appears, so the bars
    // start at the right denominator instead of growing towards it.
    progress.expect(
        TaskCategory::Assets,
        plan.assets.len() as u64,
        plan.asset_bytes,
    );
    progress.expect(
        TaskCategory::Libraries,
        plan.libraries.len() as u64,
        plan.library_bytes,
    );
    progress.expect(TaskCategory::Client, u64::from(plan.client), plan.client_bytes);

    let DownloadPlan {
        assets, libraries, ..
    } = plan;

    tokio::try_join!(
        download_assets(
            ctx,
            progress,
            version.assets == "legacy",
            assets,
        ),
        download_client(ctx, progress, version, force),
        download_libraries(
            ctx,
            progress,
            version.id.clone(),
            libraries,
            java_arch,
        ),
    )?;

    tracing::info!(
        version = %version.id,
        elapsed_ms = started.elapsed().as_millis() as u64,
        "minecraft download complete"
    );

    Ok(())
}

#[tracing::instrument(skip(ctx, progress), level = "debug")]
pub async fn download_version_info(
    ctx: &McCtx,
    progress: Option<&GroupedProgressSession>,
    version: &Version,
    loader: Option<&LoaderVersion>,
    force: bool,
) -> McResult<VersionInfo> {
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
        tracing::debug!(
            version_id = %version_id,
            "downloading Minecraft version metadata"
        );

        let version_url = version.url.parse().map_err(McError::Url)?;
        let requester = ctx.net.clone();
        let mut info: VersionInfo = match progress {
            Some(progress) => {
                progress
                    .run_child(
                        format!("Version metadata ({version_id})"),
                        1,
                        TaskCategory::Metadata,
                        |child| {
                        let requester = requester.clone();
                        async move {
                            child.set_progress(0, Some(1));
                            let result = requester
                                .send_json(Method::GET, version_url, None, &[])
                                .await
                                .map_err(McError::from)?;
                            child.set_progress(1, Some(1));
                            Ok::<VersionInfo, McError>(result)
                        }
                    })
                    .await?
            }
            None => requester
                .send_json(Method::GET, version_url, None, &[])
                .await
                .map_err(McError::from)?,
        };

        if let Some(loader) = loader {
            let loader_url = loader.url.parse().map_err(McError::Url)?;
            let requester = ctx.net.clone();
            let partial: interfrost::api::modded::PartialVersionInfo = match progress {
                Some(progress) => {
                    progress
                        .run_child(
                            format!("Loader metadata ({version_id})"),
                            1,
                            TaskCategory::Metadata,
                            |child| {
                            let requester = requester.clone();
                            async move {
                                child.set_progress(0, Some(1));
                                let result = requester
                                    .send_json(Method::GET, loader_url, None, &[])
                                    .await
                                    .map_err(McError::from)?;
                                child.set_progress(1, Some(1));
                                Ok::<interfrost::api::modded::PartialVersionInfo, McError>(
                                    result,
                                )
                            }
                        })
                        .await?
                }
                None => requester
                    .send_json(Method::GET, loader_url, None, &[])
                    .await
                    .map_err(McError::from)?,
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

#[tracing::instrument(skip_all, level = "debug")]
pub async fn download_assets_index(
    ctx: &McCtx,
    progress: &GroupedProgressSession,
    version: &VersionInfo,
    force: bool,
) -> McResult<AssetsIndex> {
    let path = paths::assets_index_dir()?.join(format!("{}.json", version.asset_index.id));

    if path.exists() && !force {
        if let Ok(data) = polyio::read_json::<AssetsIndex>(&path).await {
            return Ok(data);
        }
        tracing::warn!("cached assets index is invalid, redownloading");
    }

    download_to_path(
        &ctx.net,
        &ctx.events,
        progress,
        format!("Assets index ({})", version.asset_index.id),
        TaskCategory::Assets,
        u64::from(version.asset_index.size),
        &version.asset_index.url,
        &path,
        Some(version.asset_index.sha1.as_str()),
    )
    .await?;

    polyio::read_json(&path).await.map_err(Into::into)
}

#[tracing::instrument(skip_all, level = "debug")]
pub async fn download_assets(
    ctx: &McCtx,
    progress: &GroupedProgressSession,
    legacy: bool,
    assets: Vec<(String, Asset)>,
) -> McResult<usize> {
    if assets.is_empty() {
        return Ok(0);
    }

    let dir = if legacy {
        paths::legacy_assets_dir()?
    } else {
        paths::assets_object_dir()?
    };

    polyio::create_dir_all(&dir).await?;

    // Objects are sharded into 256 fixed subdirectories; creating them once
    // here saves a blocking-pool round trip on every one of the ~5000 files.
    if !legacy {
        let dir = dir.clone();
        tokio::task::spawn_blocking(move || {
            for subhash in 0u16..256 {
                let _ = std::fs::create_dir_all(dir.join(format!("{subhash:02x}")));
            }
        })
        .await
        .map_err(std::io::Error::other)?;
    }

    let started = std::time::Instant::now();
    let count = assets.len();
    let requester = ctx.net.clone();
    let events = ctx.events.clone();
    let progress = progress.clone();
    let requests = stream::iter(assets.into_iter().map(|(name, asset)| {
        let dir = dir.clone();
        let requester = requester.clone();
        let events = events.clone();
        let progress = progress.clone();

        async move {
            let hash = &asset.hash;
            let subhash = &hash[0..2];
            let path = asset_object_path(&dir, &name, hash, legacy);

            let url = format!("https://resources.download.minecraft.net/{subhash}/{hash}");
            download_to_path(
                &requester,
                &events,
                &progress,
                format!("Asset {name}"),
                TaskCategory::Assets,
                u64::from(asset.size),
                &url,
                &path,
                Some(hash),
            )
            .await
        }
    }))
    .buffer_unordered(ASSET_DOWNLOAD_CONCURRENCY)
    .collect::<Vec<_>>();

    let mut failed = 0;
    for res in requests.await {
        if let Err(err) = res {
            tracing::error!("failed to download asset: {err:?}");
            failed += 1;
        }
    }

    tracing::info!(
        count,
        failed,
        elapsed_ms = started.elapsed().as_millis() as u64,
        "assets ready"
    );

    Ok(failed)
}

#[tracing::instrument(skip(ctx, progress, version), fields(version_id = %version.id, force), level = "debug")]
pub async fn download_client(
    ctx: &McCtx,
    progress: &GroupedProgressSession,
    version: &VersionInfo,
    force: bool,
) -> McResult<PathBuf> {
    let client = version
        .downloads
        .get(&DownloadType::Client)
        .ok_or_else(|| McError::NoClientDownload(version.id.clone()))?;

    let path = paths::versions_dir()?
        .join(&version.id)
        .join(format!("{}.jar", version.id));

    if path.exists()
        && !force
        && let Ok(actual) = polyio::sha1_file(&path).await
    {
        if polyio::normalize_hash(&actual) == polyio::normalize_hash(&client.sha1) {
            return Ok(path);
        }
        tracing::warn!("client hash mismatch, redownloading");
    }

    download_to_path(
        &ctx.net,
        &ctx.events,
        progress,
        format!("Client {}", version.id),
        TaskCategory::Client,
        u64::from(client.size),
        &client.url,
        &path,
        Some(&client.sha1),
    )
    .await?;
    Ok(path)
}

#[tracing::instrument(skip(version_info), level = "debug")]
pub fn libraries_missing(
    version_info: &VersionInfo,
    java_arch: &str,
    minecraft_updated: bool,
) -> McResult<bool> {
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

#[tracing::instrument(skip(ctx, progress, libraries), level = "debug")]
pub async fn download_libraries(
    ctx: &McCtx,
    progress: &GroupedProgressSession,
    version: String,
    libraries: Vec<Library>,
    java_arch: &str,
) -> McResult<usize> {
    if libraries.is_empty() {
        return Ok(0);
    }

    let lib_dir = paths::libraries_dir()?;
    let natives_dest = paths::natives_dir()?.join(&version);
    let java_arch = java_arch.to_string();

    polyio::create_dir_all(&lib_dir).await?;
    polyio::create_dir_all(&natives_dest).await?;

    let started = std::time::Instant::now();
    let count = libraries.len();
    let requests = stream::iter(libraries.into_iter().map(|lib| {
        let lib_dir = lib_dir.clone();
        let natives_dest = natives_dest.clone();
        let java_arch = java_arch.clone();
        let requester = ctx.net.clone();
        let events = ctx.events.clone();
        let progress = progress.clone();

        async move {
            // Rule evaluation and the on-disk check already happened in
            // `plan_downloads`; everything here is known to need fetching.
            let artifact_path = interfrost::utils::get_path_from_artifact(&lib.name)
                .map_err(|_| McError::LibraryPath(lib.name.clone()))?;
            let path = lib_dir.join(&artifact_path);

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
                            &events,
                            &progress,
                            format!("Library {}", lib_short(&lib.name)),
                            TaskCategory::Libraries,
                            u64::from(artifact.size),
                            &artifact.url,
                            &path,
                            Some(&artifact.sha1),
                        )
                        .await?;
                        return Ok::<_, McError>(());
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
                        &events,
                        &progress,
                        format!("Library {}", lib_short(&lib.name)),
                        TaskCategory::Libraries,
                        0,
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
                        let parsed = os_key.replace("${arch}", oneclient_common::constants::ARCH_WIDTH);
                        if let Some(native) = classifiers.get(&parsed) {
                            let data = fetch_bytes_verified(
                                &requester,
                                &events,
                                &progress,
                                format!("Natives {}", lib_short(&lib.name)),
                                TaskCategory::Natives,
                                u64::from(native.size),
                                &native.url,
                                &native.sha1,
                            )
                            .await?;

                            let extract = progress.child(
                                format!("Natives {}", lib_short(&lib.name)),
                                1,
                                TaskCategory::Natives,
                            );
                            extract.set_phase(oneclient_events::TaskPhase::Extracting);
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

            Ok::<(), McError>(())
        }
    }))
    .buffer_unordered(LIBRARY_DOWNLOAD_CONCURRENCY)
    .collect::<Vec<_>>();

    let mut failed = 0;
    for res in requests.await {
        if let Err(err) = res {
            tracing::error!("failed to download library: {err:?}");
            failed += 1;
        }
    }

    tracing::info!(
        count,
        failed,
        elapsed_ms = started.elapsed().as_millis() as u64,
        "libraries ready"
    );

    Ok(failed)
}

#[tracing::instrument(skip(metadata, ctx), level = "debug")]
pub async fn get_loader_versions(
    metadata: &mut MetadataStore,
    ctx: &McCtx,
    mc_version: &str,
    loader: GameLoader,
) -> McResult<Vec<String>> {
    if loader == GameLoader::Vanilla {
        return Ok(Vec::new());
    }

    let manifest = metadata.get_modded_or_fetch(ctx, loader).await?;
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

#[tracing::instrument(skip(metadata, ctx), level = "debug")]
pub async fn get_loader_version(
    metadata: &mut MetadataStore,
    ctx: &McCtx,
    mc_version: &str,
    loader: GameLoader,
    loader_version: Option<&str>,
) -> McResult<Option<LoaderVersion>> {
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

    let mut manifest = metadata.get_modded_or_fetch(ctx, loader).await?;
    let (mut saw_matching, mut resolved) = resolve_from_manifest(manifest);
    if resolved.is_some() {
        return Ok(resolved);
    }

    if !saw_matching || loader_version.is_some() {
        metadata.fetch_all(ctx).await;
        manifest = metadata.get_modded(loader)?;
        (saw_matching, resolved) = resolve_from_manifest(manifest);
        if resolved.is_some() {
            return Ok(resolved);
        }
    }

    if let Some(requested) = loader_version {
        if !saw_matching {
            return Err(McError::NoMatchingVersion);
        }
        return Err(McError::RequestedLoaderVersionNotFound {
            requested: requested.to_string(),
        });
    }

    if saw_matching {
        Err(McError::NoMatchingLoader)
    } else {
        Err(McError::NoMatchingVersion)
    }
}

#[tracing::instrument(skip(metadata, ctx), level = "debug")]
pub async fn resolve_minecraft_version(
    metadata: &mut MetadataStore,
    ctx: &McCtx,
    mc_version: &str,
) -> McResult<(Version, usize, bool)> {
    let mut manifest = metadata.get_vanilla_or_fetch(ctx).await?;
    let mut version_index = manifest.versions.iter().position(|it| it.id == mc_version);

    if version_index.is_none() {
        metadata.fetch_all(ctx).await;
        manifest = metadata.get_vanilla()?;
        version_index = manifest.versions.iter().position(|it| it.id == mc_version);
    }

    let version_index = version_index.ok_or(McError::NoMatchingVersion)?;
    let versions = &manifest.versions;

    Ok((
        versions[version_index].clone(),
        version_index,
        is_version_updated(version_index, versions),
    ))
}

#[tracing::instrument(skip(metadata, ctx), level = "debug")]
pub async fn get_game_versions(
    metadata: &mut MetadataStore,
    ctx: &McCtx,
) -> McResult<Vec<Version>> {
    let manifest = metadata.get_vanilla_or_fetch(ctx).await?;
    Ok(manifest.versions.clone())
}

#[tracing::instrument(skip(metadata, ctx), level = "debug")]
pub async fn get_loaders_for_version(
    metadata: &mut MetadataStore,
    ctx: &McCtx,
    mc_version: &str,
) -> McResult<Vec<GameLoader>> {
    metadata.get_loaders_for_version(ctx, mc_version).await
}

#[must_use]
pub fn is_version_updated(version_index: usize, versions: &[Version]) -> bool {
    version_index <= versions.iter().position(|x| x.id == "22w16a").unwrap_or(0)
}
