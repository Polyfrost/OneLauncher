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

/// What a verify pass found and what it managed to do about it.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ClusterVerifyReport {
    /// Files read and hashed, across game files and installed content.
    pub checked: usize,
    /// Files whose contents did not match and were removed.
    pub corrupt: usize,
    /// Files a manifest expected that were not on disk at all.
    pub missing: usize,
    /// Files that were successfully fetched again.
    pub repaired: usize,
    /// Content that was corrupt but has no source to re-download from —
    /// locally imported files, or a version the provider no longer serves.
    /// Named so the user can replace them by hand.
    pub unrepairable: Vec<String>,
    /// The game files were re-downloaded without being counted, because the
    /// assets index they would have been checked against was itself gone.
    /// Reported separately so a wholesale reinstall is never described as a
    /// clean sweep of the handful of files that could still be counted.
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

    /// A one-line summary for a notification.
    #[must_use]
    pub fn summary(&self) -> String {
        if self.is_clean() {
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
        if !self.unrepairable.is_empty() {
            parts.push(format!("{} could not be replaced", self.unrepairable.len()));
        }

        format!("Checked {} files: {}.", self.checked, parts.join(", "))
    }
}

/// Verifies every file a cluster installed, and re-downloads what is broken.
///
/// Runs in three passes. Game files are hashed against the Mojang manifest and
/// the bad ones deleted; installed content is hashed against the hash it is
/// indexed under and the bad ones deleted and re-fetched from the provider that
/// served them; then the ordinary prepare path runs, which re-downloads exactly
/// what is now missing using the same verified, retrying download every other
/// install goes through.
///
/// Deleting and re-preparing rather than overwriting in place is deliberate: it
/// reuses one well-tested download path instead of adding a second one that
/// would have to be kept in step with it.
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

    // Resolving the version needs the metadata lock, which prepare also takes.
    // Scope it so it is released before the repair pass below.
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

    // The index is needed to know which assets this version expects. Reading
    // the cached copy avoids a network round trip; if it is absent or itself
    // corrupt there is nothing to verify assets against, so skip that part
    // rather than fail — prepare will fetch it and the assets below.
    let assets_index = cached_assets_index(&version_info).await;

    // Library rules are evaluated per architecture, so checking against the
    // wrong one would verify a different set of libraries than the launch path
    // installs. Use the runtime this cluster actually launches with.
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

    // Counted apart from content: these are repaired by the prepare pass below,
    // whereas content repairs itself as it goes. Adding both into one bucket
    // would count the content ones twice.
    let mut game_broken = 0;

    // Without the index there is nothing to check the assets against — and the
    // index lives inside the assets directory, so its absence usually means
    // that whole directory is gone. That is precisely the case this repair
    // exists for, so hand the work to prepare rather than reporting a clean
    // sweep of the nothing that could be checked.
    let reinstalled_game_files = assets_index.is_none();

    if let Some(assets_index) = assets_index {
        let game = verify_game_files(
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
    } else {
        tracing::warn!("assets index is missing; re-downloading the game files wholesale");
    }

    verify_cluster_content(state, cluster_id, progress, &mut report).await?;

    // Everything corrupt is gone, so a normal prepare sees it as missing and
    // fetches it back through the verified, retrying download path. A file it
    // cannot get surfaces as an error rather than a silent gap, so reaching the
    // line after this means the game files are whole again.
    //
    // Note this runs `prepare` directly rather than going through the launch
    // path, which skips preparing a cluster already marked `Ready` — the very
    // reason a cluster with deleted files keeps failing to launch instead of
    // healing itself.
    if game_broken > 0 || reinstalled_game_files {
        prepare_cluster_locked(state, cluster_id, false, true, true, Some(progress)).await?;
        report.repaired += game_broken;
    }

    report.reinstalled_game_files = reinstalled_game_files;

    Ok(report)
}

/// Hashes the cluster's installed content against the hashes it is indexed
/// under, and re-fetches whatever no longer matches.
///
/// Content is cached content-addressed, so a file that does not hash to its own
/// key is unusable by definition: nothing can find it again, and materializing
/// it into the game folder would put a corrupt mod in front of the game.
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

/// Re-downloads one piece of content from the provider that served it.
///
/// Failures are recorded rather than propagated: one mod that a provider has
/// since delisted should not abandon verification of everything else. A locally
/// imported file has no provider at all, and the user is the only one who can
/// replace it — so it is named in the report instead of silently vanishing.
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

        // The cluster is pinned to one exact file, identified by the hash it is
        // indexed under. Re-downloading a different file from the same version
        // would silently change what is installed.
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
    fn a_wholesale_reinstall_is_not_reported_as_a_clean_sweep() {
        // The `metadata/assets` case: the index the assets would have been
        // checked against was gone too, so almost nothing could be counted.
        // Saying "All 2 files verified" after re-downloading the game would be
        // actively misleading.
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
        // A locally imported mod has no provider to fetch it from again, so the
        // user is the only one who can put it back. Saying nothing would leave
        // them with a cluster that is quietly missing a file.
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
