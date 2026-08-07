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
use oneclient_events::{Choice, GroupedProgressSession, Prompt, TaskCategory, TaskPhase};
use oneclient_common::paths;
use crate::McCtx;
use crate::error::{McError, McResult};

/// Asset objects are tiny (median ~10 KiB) and latency-bound so throughput
/// scales with how many are in flight not with bandwidth
const ASSET_DOWNLOAD_CONCURRENCY: usize = 32;
/// Libraries are larger and fewer less fan-out is needed to saturate the link
const LIBRARY_DOWNLOAD_CONCURRENCY: usize = 16;

/// Computed up-front so the pre-download size estimate and the progress-bar
/// denominators come from the same numbers
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

/// Size not hash catches interrupted-download truncation without reading
/// ~5000 assets per launch
/// `expected` of 0 means the manifest gave no size
fn matches_expected_size(path: &Path, expected: u64) -> bool {
    match std::fs::metadata(path) {
        Ok(meta) => expected == 0 || meta.len() == expected,
        Err(_) => false,
    }
}

/// 0 when the entry carries no artifact metadata (maven-coordinate libraries
/// from loader manifests)
fn library_artifact_size(lib: &Library) -> u64 {
    lib.downloads
        .as_ref()
        .and_then(|downloads| downloads.artifact.as_ref())
        .map_or(0, |artifact| u64::from(artifact.size))
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

/// Stats thousands of files so it runs on the blocking pool and keeps the
/// download fan-outs from stat-ing inside a concurrency slot
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
            if !force && matches_expected_size(&path, u64::from(asset.size)) {
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
            // download path still reports it as a failure rather than skipping it
            let Ok(artifact_path) = interfrost::utils::get_path_from_artifact(&lib.name) else {
                plan.libraries.push(lib);
                continue;
            };

            if !force
                && matches_expected_size(&lib_dir.join(&artifact_path), library_artifact_size(&lib))
            {
                continue;
            }

            plan.library_bytes += library_artifact_size(&lib);
            plan.library_bytes += native_download_size(&lib, &java_arch);
            plan.libraries.push(lib);
        }

        if let Some((path, size)) = client {
            let present = path
                .map(|path| matches_expected_size(&path, size))
                .unwrap_or(false);
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct VerifyReport {
    pub checked: usize,
    /// Mismatched the manifest and were removed
    pub corrupt: usize,
    pub missing: usize,
}

impl VerifyReport {
    #[must_use]
    pub fn needs_repair(&self) -> bool {
        self.corrupt > 0 || self.missing > 0
    }
}

/// Expensive reads every asset and library so it belongs behind an explicit
/// user action not the launch path
/// Nothing is re-downloaded here corrupt files are deleted so the next prepare
/// refetches them
#[tracing::instrument(skip(progress, version, assets_index), fields(version_id = %version.id), level = "debug")]
pub async fn verify_game_files(
    progress: &GroupedProgressSession,
    version: &VersionInfo,
    assets_index: AssetsIndex,
    java_arch: &str,
    minecraft_updated: bool,
) -> McResult<VerifyReport> {
    let legacy = version.assets == "legacy";
    let asset_dir = if legacy {
        paths::legacy_assets_dir()?
    } else {
        paths::assets_object_dir()?
    };
    let lib_dir = paths::libraries_dir()?;

    let mut targets: Vec<(PathBuf, String)> = Vec::new();

    for (name, asset) in assets_index.objects {
        let path = asset_object_path(&asset_dir, &name, &asset.hash, legacy);
        targets.push((path, asset.hash));
    }

    for lib in &version.libraries {
        if let Some(rules) = &lib.rules
            && !validate_rules(rules, java_arch, minecraft_updated)
        {
            continue;
        }
        if !lib.downloadable {
            continue;
        }
        let Ok(artifact_path) = interfrost::utils::get_path_from_artifact(&lib.name) else {
            continue;
        };
        // Only libraries whose manifest entry carries a hash can be checked
        // maven-coordinate entries from loader manifests publish none
        if let Some(artifact) = lib
            .downloads
            .as_ref()
            .and_then(|downloads| downloads.artifact.as_ref())
            && !artifact.sha1.is_empty()
        {
            targets.push((lib_dir.join(&artifact_path), artifact.sha1.clone()));
        }
    }

    if let Some(client) = version.downloads.get(&DownloadType::Client) {
        let path = paths::versions_dir()?
            .join(&version.id)
            .join(format!("{}.jar", version.id));
        targets.push((path, client.sha1.clone()));
    }

    let total = targets.len() as u64;
    let child = progress.child("Verifying game files", total.max(1), TaskCategory::Assets);
    child.set_phase(TaskPhase::Verifying);

    let started = std::time::Instant::now();
    let child_for_progress = child.clone();

    // One blocking task for the whole sweep a spawn_blocking per small asset
    // would cost more than the hashing itself
    let report = tokio::task::spawn_blocking(move || {
        let mut report = VerifyReport::default();

        for (index, (path, expected)) in targets.iter().enumerate() {
            if !path.is_file() {
                report.missing += 1;
                continue;
            }

            match polyio::sha1_file_sync(path) {
                Ok(actual) if polyio::normalize_hash(&actual) == polyio::normalize_hash(expected) => {
                    report.checked += 1;
                }
                Ok(actual) => {
                    tracing::warn!(
                        path = %path.display(),
                        %actual,
                        %expected,
                        "corrupt game file; removing so it is re-downloaded"
                    );
                    report.checked += 1;
                    report.corrupt += 1;
                    if let Err(err) = std::fs::remove_file(path) {
                        tracing::error!(path = %path.display(), "could not remove: {err}");
                    }
                }
                Err(err) => {
                    // Unreadable is as good as corrupt removing it lets the
                    // download path replace it
                    tracing::warn!(path = %path.display(), "could not hash: {err}");
                    report.corrupt += 1;
                    let _ = std::fs::remove_file(path);
                }
            }

            // The child's sampling is per-call so throttle here or this emits
            // thousands of events
            if index % 64 == 0 {
                child_for_progress.set_progress(index as u64, Some(total.max(1)));
            }
        }

        report
    })
    .await
    .map_err(std::io::Error::other)?;

    child.finish();

    tracing::info!(
        checked = report.checked,
        corrupt = report.corrupt,
        missing = report.missing,
        elapsed_ms = started.elapsed().as_millis() as u64,
        "verified game files"
    );

    Ok(report)
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

    // Reserve each category's real total before any child appears so the bars
    // start at the right denominator instead of growing towards it
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

    let (failed_assets, _client, failed_libraries) = tokio::try_join!(
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

    confirm_incomplete_install(ctx, failed_assets, failed_libraries).await?;

    tracing::info!(
        version = %version.id,
        elapsed_ms = started.elapsed().as_millis() as u64,
        "minecraft download complete"
    );

    Ok(())
}

enum IncompleteInstall {
    LaunchAnyway,
}

/// Download failures are counted not propagated so one unreachable file does
/// not abandon the other four thousand
/// They already survived every retry but it stays the user's call refusing to
/// launch would strand offline players
#[tracing::instrument(skip(ctx), level = "debug")]
pub async fn confirm_incomplete_install(
    ctx: &McCtx,
    failed_assets: usize,
    failed_libraries: usize,
) -> McResult<()> {
    let failed = failed_assets + failed_libraries;
    if failed == 0 {
        return Ok(());
    }

    tracing::warn!(
        failed_assets,
        failed_libraries,
        "some files could not be downloaded; asking the user how to proceed"
    );

    let body = match (failed_assets, failed_libraries) {
        (0, libraries) => format!(
            "{libraries} librar{} could not be downloaded after several attempts. \
             The game will most likely fail to start without {}.",
            if libraries == 1 { "y" } else { "ies" },
            if libraries == 1 { "it" } else { "them" },
        ),
        (assets, 0) => format!(
            "{assets} game asset{} could not be downloaded after several attempts. \
             Sounds or textures may be missing.",
            if assets == 1 { "" } else { "s" },
        ),
        (assets, libraries) => format!(
            "{assets} asset(s) and {libraries} librar{} could not be downloaded after \
             several attempts. The game may fail to start or be missing content.",
            if libraries == 1 { "y" } else { "ies" },
        ),
    };

    let answer = ctx
        .events
        .ask(
            Prompt::new("Some files could not be downloaded", body)
                .option(
                    Choice::new("launch", "Launch anyway"),
                    IncompleteInstall::LaunchAnyway,
                )
                .dismiss("Cancel"),
        )
        .await;

    match answer {
        Ok(Some(chosen)) => match chosen.value {
            IncompleteInstall::LaunchAnyway => {
                tracing::warn!(failed, "user chose to launch with files missing");
                Ok(())
            }
        },
        // Dismissed or undeliverable nobody agreed to launch a broken install
        // and a headless caller must not silently opt in
        Ok(None) => Err(McError::IncompleteInstallCancelled { failed }),
        Err(err) => {
            tracing::warn!("could not ask about the incomplete install: {err}");
            Err(McError::IncompleteInstallCancelled { failed })
        }
    }
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

    // Objects are sharded into 256 fixed subdirectories creating them once
    // here saves a blocking-pool round trip on every one of the ~5000 files
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

    // Size not hash the jar is only published by rename after its hash
    // matched so a wrong-but-right-length jar is unreachable
    let expected_size = u64::from(client.size);
    if !force
        && let Ok(meta) = polyio::stat(&path).await
        && (expected_size == 0 || meta.len() == expected_size)
    {
        return Ok(path);
    }

    if path.exists() {
        tracing::warn!("client jar is the wrong size, redownloading");
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
        if !matches_expected_size(&lib_dir.join(&rel), library_artifact_size(lib)) {
            tracing::warn!(library = %lib.name, "missing or truncated classpath library; will repair");
            return Ok(true);
        }
    }
    Ok(false)
}

/// Takes its paths rather than resolving them so it can be tested without the
/// process-global launcher directory
fn assets_tree_missing(index: &Path, objects: &Path) -> bool {
    if !index.is_file() {
        tracing::warn!(path = %index.display(), "assets index is missing; will repair");
        return true;
    }

    if !objects.is_dir() {
        tracing::warn!(path = %objects.display(), "assets directory is missing; will repair");
        return true;
    }

    false
}

/// A `Ready` cluster skips preparation so nothing else re-checks its files
/// this turns a deleted assets tree into an automatic repair
/// Coarse checks only per-object verification belongs to `download_minecraft`'s plan
#[tracing::instrument(skip(version_info), level = "debug")]
pub fn game_files_missing(
    version_info: &VersionInfo,
    java_arch: &str,
    minecraft_updated: bool,
) -> McResult<bool> {
    let index = paths::assets_index_dir()?.join(format!("{}.json", version_info.asset_index.id));
    let objects = if version_info.assets == "legacy" {
        paths::legacy_assets_dir()?
    } else {
        paths::assets_object_dir()?
    };

    if assets_tree_missing(&index, &objects) {
        return Ok(true);
    }

    if let Some(client) = version_info.downloads.get(&DownloadType::Client) {
        let jar = paths::versions_dir()?
            .join(&version_info.id)
            .join(format!("{}.jar", version_info.id));
        if !matches_expected_size(&jar, u64::from(client.size)) {
            tracing::warn!(path = %jar.display(), "client jar is missing or truncated; will repair");
            return Ok(true);
        }
    }

    libraries_missing(version_info, java_arch, minecraft_updated)
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
            // Rules and the on-disk check already ran in `plan_downloads`
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

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "oneclient-install-{tag}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn install_assets(dir: &Path) -> (PathBuf, PathBuf) {
        let indexes = dir.join("assets").join("indexes");
        let objects = dir.join("assets").join("objects");
        std::fs::create_dir_all(&indexes).unwrap();
        std::fs::create_dir_all(&objects).unwrap();

        let index = indexes.join("17.json");
        std::fs::write(&index, b"{}").unwrap();

        (index, objects)
    }

    #[test]
    fn a_complete_assets_tree_needs_no_repair() {
        let dir = scratch("assets-ok");
        let (index, objects) = install_assets(&dir);

        assert!(!assets_tree_missing(&index, &objects));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_deleted_assets_directory_asks_for_a_repair() {
        let dir = scratch("assets-deleted");
        let (index, objects) = install_assets(&dir);
        std::fs::remove_dir_all(dir.join("assets")).unwrap();

        assert!(assets_tree_missing(&index, &objects));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_deleted_assets_index_asks_for_a_repair() {
        let dir = scratch("assets-no-index");
        let (index, objects) = install_assets(&dir);
        std::fs::remove_file(&index).unwrap();

        assert!(assets_tree_missing(&index, &objects));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_index_that_is_a_directory_is_not_mistaken_for_a_file() {
        // The launcher has produced this shape before so the check must be
        // `is_file` not `exists`
        let dir = scratch("assets-index-dir");
        let (index, objects) = install_assets(&dir);
        std::fs::remove_file(&index).unwrap();
        std::fs::create_dir(&index).unwrap();

        assert!(assets_tree_missing(&index, &objects));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn size_check_accepts_a_complete_file() {
        let dir = scratch("size-ok");
        let file = dir.join("object.bin");
        std::fs::write(&file, b"0123456789").unwrap();

        assert!(matches_expected_size(&file, 10));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn size_check_rejects_a_truncated_file() {
        let dir = scratch("size-short");
        let file = dir.join("object.bin");
        std::fs::write(&file, b"0123").unwrap();

        assert!(!matches_expected_size(&file, 10));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn size_check_rejects_a_missing_file() {
        let dir = scratch("size-missing");

        assert!(!matches_expected_size(&dir.join("nope.bin"), 10));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The bus is drained by the test so `ask` has somebody to talk to no
    /// request is ever sent through the client
    fn test_ctx() -> (McCtx, tokio::sync::mpsc::UnboundedReceiver<oneclient_events::Event>) {
        let (events, rx) = oneclient_events::EventBus::channel();
        let net = oneclient_net::RequestClient::new(oneclient_net::NetConfig::default())
            .expect("a client");
        (McCtx::new(net, events), rx)
    }

    #[tokio::test]
    async fn a_complete_install_asks_nothing() {
        let (ctx, mut rx) = test_ctx();

        confirm_incomplete_install(&ctx, 0, 0)
            .await
            .expect("nothing failed, so nothing to confirm");

        assert!(
            rx.try_recv().is_err(),
            "a clean install must not interrupt the user"
        );
    }

    #[tokio::test]
    async fn launching_anyway_is_allowed() {
        let (ctx, mut rx) = test_ctx();

        let asking = tokio::spawn(async move { confirm_incomplete_install(&ctx, 3, 0).await });

        let Some(oneclient_events::Event::Notification(
            oneclient_events::Notification::Prompt(request),
        )) = rx.recv().await
        else {
            panic!("expected a prompt");
        };
        assert!(request.body.contains("3 game assets"), "{}", request.body);
        request
            .reply
            .send(Some(oneclient_events::Answer::new("launch")))
            .unwrap();

        asking.await.unwrap().expect("the user opted in");
    }

    #[tokio::test]
    async fn dismissing_the_prompt_cancels_the_launch() {
        let (ctx, mut rx) = test_ctx();

        let asking = tokio::spawn(async move { confirm_incomplete_install(&ctx, 0, 2).await });

        let Some(oneclient_events::Event::Notification(
            oneclient_events::Notification::Prompt(request),
        )) = rx.recv().await
        else {
            panic!("expected a prompt");
        };
        assert!(
            request.body.contains("fail to start"),
            "{}",
            request.body
        );
        request.reply.send(None).unwrap();

        let err = asking.await.unwrap().expect_err("dismissal must cancel");
        assert!(
            matches!(err, McError::IncompleteInstallCancelled { failed: 2 }),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn a_bus_with_nobody_listening_cancels_rather_than_launching() {
        let (ctx, rx) = test_ctx();
        drop(rx);

        let err = confirm_incomplete_install(&ctx, 1, 1)
            .await
            .expect_err("an unanswerable prompt must not launch the game");
        assert!(
            matches!(err, McError::IncompleteInstallCancelled { failed: 2 }),
            "{err:?}"
        );
    }

    #[test]
    fn size_check_falls_back_to_presence_without_a_manifest_size() {
        let dir = scratch("size-unknown");
        let file = dir.join("object.bin");
        std::fs::write(&file, b"anything").unwrap();

        // Maven-coordinate libraries carry no artifact metadata
        assert!(matches_expected_size(&file, 0));
        assert!(!matches_expected_size(&dir.join("nope.bin"), 0));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
