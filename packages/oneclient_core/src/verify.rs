use std::path::PathBuf;
use std::sync::Arc;

use oneclient_cluster::ClusterError;
use oneclient_content::packages::PackageStore;
use oneclient_content::packages::store::artifact_absolute_path;
use oneclient_db::dao::artifact as artifact_dao;
use oneclient_events::{GroupedProgressSession, TaskCategory, TaskPhase};
use oneclient_common::paths;
use polyio::{normalize_hash, sha1_file};

use crate::clusters::prepare::prepare_cluster_locked;
use crate::game::{
    download_version_info, get_loader_version, resolve_minecraft_version, verify_game_files,
};
use crate::state::LauncherState;
use crate::LauncherResult;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ClusterVerifyReport {
    pub checked: usize,
    /// Mismatched files also deleted from disk
    pub corrupt: usize,
    pub missing: usize,
    pub repaired: usize,
    pub refetched: usize,
    /// Corrupt with no source to re-fetch from named so the user can replace them
    pub unrepairable: Vec<String>,
    /// Game files re-downloaded uncounted because the assets index was gone too
    pub reinstalled_game_files: bool,
}

impl ClusterVerifyReport {
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.corrupt == 0
            && self.missing == 0
            && self.unrepairable.is_empty()
            && !self.reinstalled_game_files
    }

    #[must_use]
    pub fn summary(&self) -> String {
        if self.is_clean() {
            if self.refetched > 0 {
                return format!(
                    "All {} files verified, {} refetched.",
                    self.checked, self.refetched
                );
            }
            return format!("All {} files verified.", self.checked);
        }

        if self.reinstalled_game_files && self.corrupt == 0 && self.missing == 0 {
            return "Game files were missing and have been re-downloaded.".to_string();
        }

        let mut parts = Vec::new();
        if self.corrupt > 0 {
            parts.push(format!("{} corrupt", self.corrupt));
        }
        if self.missing > 0 {
            parts.push(format!("{} missing", self.missing));
        }
        if self.repaired > 0 {
            parts.push(format!("{} repaired", self.repaired));
        }
        if self.refetched > 0 {
            parts.push(format!("{} refetched", self.refetched));
        }
        if !self.unrepairable.is_empty() {
            parts.push(format!("{} could not be replaced", self.unrepairable.len()));
        }

        format!("Checked {} files: {}.", self.checked, parts.join(", "))
    }
}

/// Deletes bad files and re-prepares rather than overwriting in place to reuse
/// the one well-tested download path instead of maintaining a second
#[tracing::instrument(skip(state), level = "debug")]
pub async fn verify_cluster_files(
    state: &Arc<LauncherState>,
    cluster_id: i64,
) -> LauncherResult<ClusterVerifyReport> {
    let cluster = state.clusters.get(cluster_id).await?;

    let progress = GroupedProgressSession::start(
        &state.services.events,
        format!("Verifying files - {}", cluster.mc_version),
    );

    let result = run_verify(state, cluster_id, &progress).await;
    progress.finish();

    match &result {
        Ok(report) => tracing::info!(
            cluster_id,
            checked = report.checked,
            corrupt = report.corrupt,
            missing = report.missing,
            repaired = report.repaired,
            "cluster verification finished"
        ),
        Err(err) => tracing::error!(cluster_id, "cluster verification failed: {err}"),
    }

    result
}

async fn run_verify(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    progress: &GroupedProgressSession,
) -> LauncherResult<ClusterVerifyReport> {
    let cluster = state.clusters.get(cluster_id).await?;
    let mc_version = oneclient_common::version::normalize_mc_version_input(&cluster.mc_version);

    // Scoped so the metadata lock is released before the repair pass which takes it too
    let (version_info, minecraft_updated) = {
        let mut metadata = state.metadata.lock().await;
        let (version, _index, minecraft_updated) =
            resolve_minecraft_version(&mut metadata, &state.services.mc(), &mc_version)
                .await
                .map_err(|_| ClusterError::InvalidVersion(cluster.mc_version.clone()))?;

        let loader_version = get_loader_version(
            &mut metadata,
            &state.services.mc(),
            &mc_version,
            cluster.mc_loader,
            cluster.mc_loader_version.as_deref(),
        )
        .await?;

        let info = download_version_info(
            &state.services.mc(),
            Some(progress),
            &version,
            loader_version.as_ref(),
            false,
        )
        .await?;

        (info, minecraft_updated)
    };

    let mut report = ClusterVerifyReport::default();

    // Absent or corrupt index means nothing to verify assets against so skip
    // rather than fail prepare fetches it and the assets below
    let assets_index = cached_assets_index(&version_info).await;

    // Library rules are per-architecture use the runtime this cluster launches
    // with or a different library set than the launch path installs is verified
    let java_arch = {
        let global = state.settings.read().global_game_settings.clone();
        let profile = state.clusters.resolve_settings(&global, &cluster).await?;
        state
            .java
            .runtime_for_profile(profile.java_path.as_deref())
            .await
            .ok()
            .flatten()
            .map_or_else(|| std::env::consts::ARCH.to_string(), |rt| rt.os_arch)
    };

    // Counted apart from content which repairs itself as it goes one bucket
    // would count the content ones twice
    let mut game_broken = 0;

    // The index lives in the assets directory so its absence usually means that
    // whole directory is gone hand the work to prepare
    let reinstalled_game_files = assets_index.is_none();

    let mut unverifiable: Vec<PathBuf> = Vec::new();

    if let Some(assets_index) = assets_index {
        let (game, stale) = verify_game_files(
            progress,
            &version_info,
            assets_index,
            &java_arch,
            minecraft_updated,
        )
        .await?;

        report.checked += game.checked;
        report.corrupt += game.corrupt;
        report.missing += game.missing;
        game_broken = game.corrupt + game.missing;
        unverifiable = stale;
    } else {
        tracing::warn!("assets index is missing; re-downloading the game files wholesale");
    }

    verify_cluster_content(state, cluster_id, progress, &mut report).await?;

    // Runs `prepare` directly not the launch path which skips clusters already
    // marked `Ready` and so never heals one with deleted files
    if game_broken > 0 || !unverifiable.is_empty() || reinstalled_game_files {
        let staged = stage_aside(unverifiable).await;
        let prepared =
            prepare_cluster_locked(state, cluster_id, false, true, true, Some(progress)).await;
        report.refetched = settle_staged(staged).await;
        prepared?;
        report.repaired += game_broken;
    }

    report.reinstalled_game_files = reinstalled_game_files;

    Ok(report)
}

fn staged_path(path: &std::path::Path) -> Option<PathBuf> {
    let mut aside = path.file_name()?.to_os_string();
    aside.push(".stale");

    Some(path.with_file_name(aside))
}

async fn reclaim_orphans(paths: &[PathBuf]) {
    for path in paths {
        let Some(aside) = staged_path(path) else {
            continue;
        };

        if polyio::stat(&aside).await.is_err() {
            continue;
        }

        let recovered = if polyio::stat(path).await.is_ok() {
            polyio::remove_file(&aside).await
        } else {
            polyio::rename(&aside, path).await
        };

        match recovered {
            Ok(()) => {
                tracing::info!(path = %path.display(), "reclaimed a copy left by an interrupted verify");
            }
            Err(err) => {
                tracing::warn!(path = %aside.display(), "could not reclaim a staged copy: {err}");
            }
        }
    }
}

async fn stage_aside(paths: Vec<PathBuf>) -> Vec<(PathBuf, PathBuf)> {
    reclaim_orphans(&paths).await;

    let mut staged = Vec::with_capacity(paths.len());

    for path in paths {
        let Some(aside) = staged_path(&path) else {
            continue;
        };

        match polyio::rename(&path, &aside).await {
            Ok(()) => staged.push((path, aside)),
            Err(err) => {
                tracing::warn!(path = %path.display(), "could not stage for a refetch: {err}");
            }
        }
    }

    staged
}

async fn settle_staged(staged: Vec<(PathBuf, PathBuf)>) -> usize {
    let mut refetched = 0;

    for (path, aside) in staged {
        if polyio::stat(&path).await.is_ok() {
            refetched += 1;
            if let Err(err) = polyio::remove_file(&aside).await {
                tracing::warn!(path = %aside.display(), "could not drop the staged copy: {err}");
            }
        } else if let Err(err) = polyio::rename(&aside, &path).await {
            tracing::error!(path = %path.display(), "could not restore after a failed refetch: {err}");
        }
    }

    refetched
}

/// The content cache is content-addressed so a file that does not hash to its
/// own key is unusable nothing can find it again
async fn verify_cluster_content(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    progress: &GroupedProgressSession,
    report: &mut ClusterVerifyReport,
) -> LauncherResult<()> {
    let content = state.services.content();
    let linked = PackageStore::list_linked_artifacts(cluster_id, &content).await?;
    if linked.is_empty() {
        return Ok(());
    }

    let child = progress.child(
        "Verifying content",
        linked.len() as u64,
        TaskCategory::Packages,
    );
    child.set_phase(TaskPhase::Verifying);

    for (index, link) in linked.iter().enumerate() {
        child.set_progress(index as u64, Some(linked.len() as u64));

        let Some(artifact) = artifact_dao::get_artifact_by_hash(&state.services.db, &link.hash)
            .await?
        else {
            continue;
        };

        let Ok(path) = artifact_absolute_path(&artifact.path) else {
            continue;
        };

        if !path.is_file() {
            report.missing += 1;
            repair_content(state, link, report).await;
            continue;
        }

        let actual = match sha1_file(&path).await {
            Ok(actual) => actual,
            Err(err) => {
                tracing::warn!(path = %path.display(), "could not hash content: {err}");
                report.corrupt += 1;
                let _ = polyio::remove_file(&path).await;
                repair_content(state, link, report).await;
                continue;
            }
        };

        report.checked += 1;

        if normalize_hash(&actual) == normalize_hash(&link.hash) {
            continue;
        }

        tracing::warn!(
            file = %link.file_name,
            expected = %link.hash,
            %actual,
            "corrupt content file; removing"
        );
        report.corrupt += 1;
        let _ = polyio::remove_file(&path).await;
        repair_content(state, link, report).await;
    }

    child.finish();
    Ok(())
}

/// Failures are recorded not propagated one delisted mod should not abandon
/// verification of everything else
async fn repair_content(
    state: &Arc<LauncherState>,
    link: &oneclient_content::packages::LinkedArtifactInfo,
    report: &mut ClusterVerifyReport,
) {
    let content = state.services.content();

    let (Some(provider_id), Some(project_id), Some(version_id)) = (
        link.provider,
        link.project_id.as_deref(),
        link.version_id.as_deref(),
    ) else {
        tracing::warn!(file = %link.file_name, "no provider to re-download from");
        report.unrepairable.push(link.file_name.clone());
        return;
    };

    let outcome = async {
        let provider = content.providers.get(provider_id)?;
        let version = provider.get_version(project_id, version_id, &content).await?;

        // The cluster is pinned to one exact file by hash another file from the
        // same version would silently change what is installed
        let file = version
            .files
            .iter()
            .find(|file| normalize_hash(&file.sha1) == normalize_hash(&link.hash))
            .ok_or_else(|| oneclient_content::ContentError::InvalidData {
                reason: format!("{} is no longer served by {provider_id:?}", link.file_name),
            })?;

        oneclient_content::packages::store::download_version_file(
            provider_id,
            project_id,
            &version,
            link.content_type,
            file,
            true,
            None,
            &content,
        )
        .await
    }
    .await;

    match outcome {
        Ok(_) => {
            tracing::info!(file = %link.file_name, "re-downloaded corrupt content");
            report.repaired += 1;
        }
        Err(err) => {
            tracing::warn!(file = %link.file_name, "could not re-download: {err}");
            report.unrepairable.push(link.file_name.clone());
        }
    }
}

async fn cached_assets_index(
    info: &interfrost::api::minecraft::VersionInfo,
) -> Option<interfrost::api::minecraft::AssetsIndex> {
    let path = paths::assets_index_dir()
        .ok()?
        .join(format!("{}.json", info.asset_index.id));
    polyio::read_json(&path).await.ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clean_sweep_says_so() {
        let report = ClusterVerifyReport {
            checked: 5123,
            ..Default::default()
        };

        assert!(report.is_clean());
        assert_eq!(report.summary(), "All 5123 files verified.");
    }

    #[test]
    fn a_repaired_sweep_reports_what_it_did() {
        let report = ClusterVerifyReport {
            checked: 5123,
            corrupt: 3,
            missing: 1,
            repaired: 4,
            refetched: 0,
            unrepairable: Vec::new(),
            reinstalled_game_files: false,
        };

        assert!(!report.is_clean());
        assert_eq!(
            report.summary(),
            "Checked 5123 files: 3 corrupt, 1 missing, 4 repaired."
        );
    }

    #[test]
    fn hashless_loader_libraries_are_reported_as_refetched() {
        let report = ClusterVerifyReport {
            checked: 131,
            refetched: 8,
            ..Default::default()
        };

        assert!(report.is_clean());
        assert_eq!(report.summary(), "All 131 files verified, 8 refetched.");
    }

    #[test]
    fn a_wholesale_reinstall_is_not_reported_as_a_clean_sweep() {
        let report = ClusterVerifyReport {
            checked: 2,
            reinstalled_game_files: true,
            ..Default::default()
        };

        assert!(!report.is_clean());
        assert_eq!(
            report.summary(),
            "Game files were missing and have been re-downloaded."
        );
    }

    #[test]
    fn content_that_cannot_be_replaced_is_still_reported() {
        let report = ClusterVerifyReport {
            checked: 12,
            corrupt: 1,
            repaired: 0,
            unrepairable: vec!["my-local-mod.jar".to_string()],
            ..Default::default()
        };

        assert!(!report.is_clean());
        assert_eq!(
            report.summary(),
            "Checked 12 files: 1 corrupt, 1 could not be replaced."
        );
    }
}
