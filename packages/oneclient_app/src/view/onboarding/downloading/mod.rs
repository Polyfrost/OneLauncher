use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use freya::animation::*;
use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::notification::{
    GroupedProgressEvent, GroupedProgressSession, Notification, NotificationService,
};
use oneclient_core::{BundleArchive, BundleFile, ImportTarget};
use oneclient_db::models::OverrideType;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::hooks::{
    BridgeDispatch, ClusterBundles, invalidate_cluster_queries, migration_detection,
    onboarding_bundles_items, try_default_account, use_current_account, use_dispatch,
    use_migration, use_onboarding_bundles, use_onboarding_selection, use_settings_snapshot,
};
use crate::routes::Route;
use crate::theme::colors;
use crate::view::onboarding::{matching_new_cluster_id, pkg_key};

mod backdrop;
mod progress;
mod summary;
mod tips;
pub use backdrop::LoadingBackdrop;
use progress::{ProgressView, failure_panel, progress_panel};
use summary::summary_view;
use tips::{tip_panel, use_onboarding_tip};

const FADE_DURATION_MS: u64 = 700;
const MAX_TASK_ROWS: usize = 6;

#[derive(Clone, PartialEq)]
struct ClusterPlan {
    cluster_id: i64,
    mc_version: String,
    /// `(bundle_name, package_id, override)` for every file whose fate differs
    /// from the bundle manifest's default.
    overrides: Vec<(String, String, OverrideType)>,
}

#[derive(Clone, PartialEq)]
struct InstallFailure {
    plan: ClusterPlan,
    reason: String,
}

#[derive(Clone, PartialEq)]
struct TaskLine {
    label: String,
    phase: &'static str,
    current: u64,
    total: u64,
}

#[derive(Clone, PartialEq, Default)]
struct GroupedAgg {
    children: HashMap<Uuid, TaskLine>,
    done_units: u64,
    carried: u64,
}

impl GroupedAgg {
    fn fraction(&self) -> f32 {
        let completed: u64 = self.done_units
            + self
                .children
                .values()
                .map(|t| t.current.min(t.total))
                .sum::<u64>();
        let total: u64 = self.done_units + self.children.values().map(|t| t.total).sum::<u64>();
        if total == 0 {
            0.0
        } else {
            (completed as f32 / total as f32).clamp(0.0, 1.0)
        }
    }

    fn task_list(&self) -> Vec<TaskLine> {
        let mut tasks: Vec<TaskLine> = self.children.values().cloned().collect();
        tasks.sort_by(|a, b| a.label.cmp(&b.label));
        tasks
    }

    fn downloaded_bytes(&self) -> u64 {
        self.carried
            + self.done_units
            + self
                .children
                .values()
                .map(|t| t.current.min(t.total))
                .sum::<u64>()
    }

    fn summary(&self) -> Option<String> {
        if self.children.is_empty() {
            return None;
        }

        let mut kinds: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut phases: BTreeMap<&'static str, usize> = BTreeMap::new();
        for task in self.children.values() {
            *kinds.entry(categorize(&task.label)).or_default() += 1;
            *phases.entry(task.phase).or_default() += 1;
        }

        let verb = phases
            .iter()
            .max_by_key(|(_, n)| **n)
            .map(|(p, _)| *p)
            .unwrap_or("Downloading");

        let mut ordered: Vec<(&str, usize)> = kinds.into_iter().collect();
        ordered.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        let parts: Vec<String> = ordered
            .into_iter()
            .take(3)
            .map(|(kind, count)| {
                if count > 1 {
                    format!("{kind} ×{count}")
                } else {
                    kind.to_string()
                }
            })
            .collect();

        Some(format!("{verb} {}", parts.join(", ")))
    }
}

#[derive(Clone, Copy, PartialEq, Default)]
struct Meter {
    speed_bps: f64,
}

fn categorize(label: &str) -> &'static str {
    if label.starts_with("Assets index") {
        "asset index"
    } else if label.starts_with("Asset") {
        "assets"
    } else if label.starts_with("Library") {
        "libraries"
    } else if label.starts_with("Natives") {
        "natives"
    } else if label.starts_with("Client") {
        "game client"
    } else if label.starts_with("Version metadata") || label.starts_with("Loader metadata") {
        "metadata"
    } else if label.starts_with("Mod") {
        "mods"
    } else {
        "content"
    }
}

#[derive(Clone, PartialEq, Default)]
struct DownloadStage {
    index: usize,
    total: usize,
    label: String,
}

enum InstallUiEvent {
    Progress(usize, usize),
    Stage(DownloadStage),
    Activity(String),
    TotalEstimate(u64),
    Finished(Vec<InstallFailure>),
}

#[derive(Clone, Copy)]
struct InstallHandles {
    progress: State<(usize, usize)>,
    agg: State<GroupedAgg>,
    stage: State<DownloadStage>,
    activity: State<String>,
    started_at: State<Option<Instant>>,
    meter: State<Meter>,
    total_estimate: State<u64>,
    failures: State<Vec<InstallFailure>>,
    running: State<bool>,
    complete: State<bool>,
}

#[derive(PartialEq)]
pub struct OnboardingDownloading;

impl Component for OnboardingDownloading {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let bundles_query = use_onboarding_bundles();
        let migration_query = use_migration();
        let selection = use_onboarding_selection();
        let settings = use_settings_snapshot().settings;
        let account_query = use_current_account();

        let selected_state = selection.selected;
        let language_state = selection.language;
        let reduce_motion_state = selection.reduce_motion;
        let predownload_state = selection.predownload;
        let mut setup_started = selection.setup_started;

        let mut confirmed = use_state(|| false);
        let mut started = use_state(|| false);
        let progress = use_state(|| (0usize, 0usize));
        let agg = use_state(GroupedAgg::default);
        let stage = use_state(DownloadStage::default);
        let activity = use_state(String::new);
        let started_at = use_state(|| None::<Instant>);
        let meter = use_state(Meter::default);
        let total_estimate = use_state(|| 0u64);
        let failures = use_state(Vec::<InstallFailure>::new);
        let running = use_state(|| false);
        let complete = use_state(|| false);
        let mut leaving = use_state(|| false);

        let handles = InstallHandles {
            progress,
            agg,
            stage,
            activity,
            started_at,
            meter,
            total_estimate,
            failures,
            running,
            complete,
        };

        use_side_effect(move || {
            if !*confirmed.read() || *started.peek() {
                return;
            }
            let Some(items) = onboarding_bundles_items(&bundles_query) else {
                return;
            };
            let selected = selected_state.peek().clone();
            let plans = build_plans(&items, &selected);
            let predownload = *predownload_state.peek();
            started.set(true);
            run_install_batch(plans, predownload, handles);
        });

        let dispatch_finish = dispatch.clone();
        use_side_effect(move || {
            if !*complete.read() || !failures.read().is_empty() || *leaving.peek() {
                return;
            }

            leaving.set(true);
            finish_onboarding(dispatch_finish.clone(), &bundles_query);
        });

        let (done, total) = *progress.read();
        let stage_now = stage.read().clone();
        let agg_now = agg.read().clone();
        let activity_now = activity.read().clone();
        let meter_now = *meter.read();
        let total_estimate_now = *total_estimate.read();
        let elapsed_secs = (*started_at.read()).map(|s| s.elapsed().as_secs());
        let predownload = *predownload_state.read();
        let is_running = *running.read();
        let is_complete = *complete.read();

        let global = if is_complete {
            100.0
        } else if total == 0 {
            0.0
        } else if total_estimate_now > 0 {
            (agg_now.downloaded_bytes() as f32 / total_estimate_now as f32 * 100.0).clamp(0.0, 99.0)
        } else {
            ((done as f32 + agg_now.fraction()) / total as f32 * 100.0).clamp(0.0, 100.0)
        };

        let has_failures = !failures.read().is_empty();
        let heading = if has_failures && is_complete {
            "Download issues found"
        } else if predownload {
            "Downloading..."
        } else {
            "Finishing up..."
        };

        let is_leaving = *leaving.read();
        let fade = use_animation_with_dependencies(&is_leaving, |conf, _| {
            conf.on_creation(OnCreation::Run);
            conf.on_change(OnChange::Rerun);
            AnimNum::new(0., 1.)
                .time(FADE_DURATION_MS)
                .ease(Ease::Out)
                .function(Function::Cubic)
        });
        let opacity = if is_leaving {
            1.0 - fade.get().value()
        } else {
            1.0
        };

        let tip = use_onboarding_tip();

        if !*confirmed.read() {
            let items = onboarding_bundles_items(&bundles_query).unwrap_or_default();
            let selected = selected_state.read().clone();
            let language = language_state.read().clone();
            let reduce_motion = *reduce_motion_state.read();
            let account_name = try_default_account(&account_query)
                .map(|account| account.username.clone())
                .unwrap_or_else(|| "Not signed in".to_string());

            let import_dispatch = dispatch.clone();
            let import_folder = selection.import_folder;
            let import_dedicated = selection.import_dedicated;
            let import_detection = migration_detection(&migration_query);
            let import_items = items.clone();

            let migration_summary = import_detection.as_ref().map(|detection| {
                let source_name = detection.source.display_name().to_string();
                match import_folder.read().clone() {
                    Some(folder) => {
                        let dedicated = *import_dedicated.read()
                            && detection
                                .instances
                                .iter()
                                .find(|c| c.folder_name == folder)
                                .and_then(|inst| matching_new_cluster_id(inst, &items))
                                .is_some();
                        let target = if dedicated {
                            "This version only"
                        } else {
                            "Shared game directory"
                        };
                        (source_name, folder, target.to_string())
                    }
                    None => (source_name, "No files imported".to_string(), String::new()),
                }
            });

            return summary_view(
                &items,
                &selected,
                &language,
                reduce_motion,
                settings.dynamic_background_enabled,
                account_name,
                migration_summary,
                predownload_state,
                move |_| {
                    if let (Some(detection), Some(folder)) =
                        (import_detection.as_ref(), import_folder.peek().clone())
                    {
                        let target = if *import_dedicated.peek() {
                            detection
                                .instances
                                .iter()
                                .find(|c| c.folder_name == folder)
                                .and_then(|inst| matching_new_cluster_id(inst, &import_items))
                                .map(|new_cluster_id| ImportTarget::Dedicated { new_cluster_id })
                                .unwrap_or(ImportTarget::Shared)
                        } else {
                            ImportTarget::Shared
                        };
                        import_dispatch.import_launcher(detection.source, folder, target);
                    }

                    setup_started.set(true);
                    confirmed.set(true);
                },
            )
            .into_element();
        }

        let dispatch_continue = dispatch.clone();
        let continue_query = bundles_query;
        let mut continue_leaving = leaving;

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .content(Content::Flex)
            .padding(Gaps::new(16., 48., 48., 48.))
            .opacity(opacity)
            .child(
                label()
                    .text(heading)
                    .font_size(36.)
                    .font_weight(FontWeight::BOLD)
                    .color(colors::fg_primary()),
            )
            .maybe_child((has_failures && is_complete).then(|| {
                failure_panel(
                    &failures.read(),
                    done,
                    total,
                    is_running,
                    move |_| {
                        let retry_plans: Vec<ClusterPlan> =
                            failures.peek().iter().map(|f| f.plan.clone()).collect();
                        if retry_plans.is_empty() || *running.peek() {
                            return;
                        }
                        let predownload = *predownload_state.peek();
                        run_install_batch(retry_plans, predownload, handles);
                    },
                    move |_| {
                        if *continue_leaving.peek() {
                            return;
                        }
                        continue_leaving.set(true);
                        finish_onboarding(dispatch_continue.clone(), &continue_query);
                    },
                )
                .into_element()
            }))
            .child(rect().width(Size::fill()).height(Size::flex(1.0)))
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .content(Content::Flex)
                    .cross_align(Alignment::End)
                    .child(progress_panel(ProgressView {
                        global,
                        stage: &stage_now,
                        agg: &agg_now,
                        activity: &activity_now,
                        speed_bps: meter_now.speed_bps,
                        total_estimate: total_estimate_now,
                        elapsed_secs,
                        done,
                        total,
                        predownload,
                        running: is_running,
                    }))
                    .child(rect().width(Size::flex(1.0)))
                    .child(tip_panel(tip)),
            )
            .into_element()
    }
}

fn build_plans(
    items: &[ClusterBundles],
    selected: &std::collections::HashSet<String>,
) -> Vec<ClusterPlan> {
    items
        .iter()
        .map(|cb| {
            let mut overrides = Vec::new();
            for archive in &cb.archives {
                overrides.extend(archive_overrides(cb.cluster.id, archive, selected));
            }
            ClusterPlan {
                cluster_id: cb.cluster.id,
                mc_version: cb.cluster.mc_version.clone(),
                overrides,
            }
        })
        .collect()
}

fn archive_overrides(
    cluster_id: i64,
    archive: &BundleArchive,
    selected: &std::collections::HashSet<String>,
) -> Vec<(String, String, OverrideType)> {
    let bundle_name = &archive.manifest.name;
    let wants = |file: &BundleFile| {
        selected.contains(&pkg_key(cluster_id, bundle_name, &file.kind.package_id()))
    };

    // Hidden files are dependencies the user never sees, so they follow the
    // bundle: kept if anything from it was taken (including a single opted-in
    // extra, which would otherwise lose its dependencies), dropped if not.
    let bundle_taken = archive
        .manifest
        .files
        .iter()
        .filter(|f| !f.hidden)
        .any(wants);

    let mut overrides = Vec::new();
    for file in &archive.manifest.files {
        let wanted = if file.hidden {
            bundle_taken
        } else {
            wants(file)
        };

        let override_type = match (wanted, file.enabled) {
            // Matches the manifest default; nothing to record.
            (true, true) | (false, false) => continue,
            // Opting into a mod the bundle ships turned off.
            (true, false) => OverrideType::Enabled,
            // Declined: don't fetch it at all.
            (false, true) => OverrideType::Removed,
        };

        overrides.push((bundle_name.clone(), file.kind.package_id(), override_type));
    }
    overrides
}

const GAME_SIZE_GUESS: u64 = 180_000_000;
const JRE_SIZE_GUESS: u64 = 45_000_000;

fn rough_download_estimate(
    items: &[ClusterBundles],
    selected: &std::collections::HashSet<String>,
) -> u64 {
    let mut total: u64 = 0;
    for cb in items {
        for archive in &cb.archives {
            let dropped: std::collections::HashSet<String> =
                archive_overrides(cb.cluster.id, archive, selected)
                    .into_iter()
                    .filter(|(_, _, ty)| *ty == OverrideType::Removed)
                    .map(|(_, pid, _)| pid)
                    .collect();
            let opted_in: std::collections::HashSet<String> =
                archive_overrides(cb.cluster.id, archive, selected)
                    .into_iter()
                    .filter(|(_, _, ty)| *ty == OverrideType::Enabled)
                    .map(|(_, pid, _)| pid)
                    .collect();

            for file in &archive.manifest.files {
                let pid = file.kind.package_id();
                let installing = if opted_in.contains(&pid) {
                    true
                } else if dropped.contains(&pid) {
                    false
                } else {
                    file.enabled
                };
                if installing {
                    total += file.size;
                }
            }
        }
        total += GAME_SIZE_GUESS + JRE_SIZE_GUESS;
    }
    total
}

fn run_install_batch(plans: Vec<ClusterPlan>, predownload: bool, handles: InstallHandles) {
    let InstallHandles {
        mut progress,
        mut agg,
        mut stage,
        mut activity,
        mut started_at,
        mut meter,
        mut total_estimate,
        mut failures,
        mut running,
        mut complete,
    } = handles;

    running.set(true);
    complete.set(false);
    failures.set(Vec::new());
    agg.set(GroupedAgg::default());
    meter.set(Meter::default());
    total_estimate.set(0);
    activity.set("Getting started...".to_string());
    started_at.set(Some(Instant::now()));
    let total = plans.len();
    progress.set((0, total));

    let (notif_tx, mut notif_rx) = mpsc::unbounded_channel::<Notification>();
    let notifier = NotificationService::new(notif_tx);

    let (ui_tx, mut ui_rx) = mpsc::unbounded_channel::<InstallUiEvent>();

    spawn(async move {
        let mut prev = 0u64;
        let mut speed = 0f64;

        loop {
            tokio::time::sleep(Duration::from_millis(1000)).await;

            let downloaded = agg.peek().downloaded_bytes();
            let delta = downloaded.saturating_sub(prev);
            let inst = delta as f64;

            speed = if speed <= 0.0 {
                inst
            } else {
                speed * 0.6 + inst * 0.4
            };

            prev = downloaded;
            meter.set(Meter { speed_bps: speed });

            if !*running.peek() {
                break;
            }
        }
    });

    spawn(async move {
        let mut local = GroupedAgg::default();
        loop {
            let Some(first) = notif_rx.recv().await else {
                break;
            };
            if let Notification::GroupedProgress(event) = first {
                apply_grouped(&mut local, event);
            }
            // Drain everything already queued in one pass.
            while let Ok(notification) = notif_rx.try_recv() {
                if let Notification::GroupedProgress(event) = notification {
                    apply_grouped(&mut local, event);
                }
            }
            agg.set(local.clone());
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    });

    spawn(async move {
        while let Some(event) = ui_rx.recv().await {
            match event {
                InstallUiEvent::Progress(done, total) => progress.set((done, total)),
                InstallUiEvent::Stage(next) => stage.set(next),
                InstallUiEvent::Activity(text) => activity.set(text),
                InstallUiEvent::TotalEstimate(bytes) => total_estimate.set(bytes),
                InstallUiEvent::Finished(failed) => {
                    agg.set(GroupedAgg::default());
                    activity.set("Wrapping up...".to_string());
                    invalidate_cluster_queries().await;
                    failures.set(failed);
                    running.set(false);
                    complete.set(true);
                    break;
                }
            }
        }
    });

    tokio::spawn(async move {
        let mut failed = Vec::new();

        if predownload {
            let _ = ui_tx.send(InstallUiEvent::Activity(
                "Calculating download size...".to_string(),
            ));
            if let Ok(state) = oneclient_core::LauncherState::get() {
                let mut sum = 0u64;
                for plan in &plans {
                    match oneclient_core::estimate_cluster_download(
                        &state,
                        plan.cluster_id,
                        state.bundles.as_ref(),
                    )
                    .await
                    {
                        Ok(bytes) => sum += bytes,
                        Err(err) => tracing::warn!(
                            cluster_id = plan.cluster_id,
                            "download size estimate failed: {err}"
                        ),
                    }
                    let _ = ui_tx.send(InstallUiEvent::TotalEstimate(sum));
                }
            }
        }

        for (index, plan) in plans.into_iter().enumerate() {
            let _ = ui_tx.send(InstallUiEvent::Stage(DownloadStage {
                index,
                total,
                label: plan.mc_version.clone(),
            }));

            if let Err(reason) = install_one(&plan, predownload, &notifier, &ui_tx).await {
                tracing::error!(
                    cluster_id = plan.cluster_id,
                    mc_version = %plan.mc_version,
                    "onboarding install failed: {reason}"
                );
                failed.push(InstallFailure { plan, reason });
            }
            let _ = ui_tx.send(InstallUiEvent::Progress(index + 1, total));
        }

        drop(notifier);
        let _ = ui_tx.send(InstallUiEvent::Finished(failed));
    });
}

async fn install_one(
    plan: &ClusterPlan,
    predownload: bool,
    notifier: &NotificationService,
    ui_tx: &mpsc::UnboundedSender<InstallUiEvent>,
) -> Result<(), String> {
    let state = oneclient_core::LauncherState::get().map_err(|err| err.to_string())?;

    if !plan.overrides.is_empty() {
        let _ = ui_tx.send(InstallUiEvent::Activity(
            "Saving your package choices...".to_string(),
        ));
    }
    oneclient_core::set_bundle_package_overrides(plan.cluster_id, &plan.overrides, &state.services)
        .await
        .map_err(|err| err.to_string())?;

    if !predownload {
        return Ok(());
    }

    let session =
        GroupedProgressSession::start(notifier, format!("Downloading {}", plan.mc_version));

    let _ = ui_tx.send(InstallUiEvent::Activity(format!(
        "Downloading Minecraft {}...",
        plan.mc_version
    )));

    let prepared = oneclient_core::ClusterManager::prepare(
        &state,
        plan.cluster_id,
        false,
        true,
        true,
        Some(&session),
    )
    .await;

    let bundles_result = if prepared.is_ok() {
        let _ = ui_tx.send(InstallUiEvent::Activity(format!(
            "Installing mods & content for {}...",
            plan.mc_version
        )));
        oneclient_core::install_cluster_bundles(
            plan.cluster_id,
            state.bundles.as_ref(),
            Some(&session),
            &state.services,
        )
        .await
    } else {
        Ok(())
    };

    session.finish();

    prepared.map_err(|err| err.to_string())?;
    bundles_result.map_err(|err| err.to_string())?;
    Ok(())
}

fn apply_grouped(agg: &mut GroupedAgg, event: GroupedProgressEvent) {
    match event {
        GroupedProgressEvent::Start { .. } => {
            agg.children.clear();
            agg.done_units = 0;
        }
        GroupedProgressEvent::AddChild {
            child_id,
            label,
            total,
            ..
        } => {
            agg.children.insert(
                child_id,
                TaskLine {
                    label,
                    phase: "Downloading",
                    current: 0,
                    total: total.max(1),
                },
            );
        }
        GroupedProgressEvent::UpdateChild {
            child_id,
            current,
            total,
            ..
        } => {
            if let Some(task) = agg.children.get_mut(&child_id) {
                task.current = current;
                task.total = total.max(1);
            }
        }
        GroupedProgressEvent::SetChildPhase {
            child_id, phase, ..
        } => {
            if let Some(task) = agg.children.get_mut(&child_id) {
                task.phase = phase.label();
            }
        }
        GroupedProgressEvent::FinishChild { child_id, .. } => {
            if let Some(task) = agg.children.remove(&child_id) {
                agg.done_units += task.total;
            }
        }
        GroupedProgressEvent::End { .. } => {
            agg.carried += agg.done_units
                + agg
                    .children
                    .values()
                    .map(|t| t.current.min(t.total))
                    .sum::<u64>();
            agg.children.clear();
            agg.done_units = 0;
        }
    }
}

fn finish_onboarding(
    dispatch: BridgeDispatch,
    bundles_query: &freya::query::UseQuery<crate::hooks::OnboardingBundlesQuery>,
) {
    let versions: Vec<String> = onboarding_bundles_items(bundles_query)
        .map(|items| {
            let mut versions: Vec<String> = items
                .iter()
                .map(|cb| cb.cluster.mc_version.clone())
                .collect();

            versions.sort();
            versions.dedup();
            versions
        })
        .unwrap_or_default();

    spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(FADE_DURATION_MS)).await;

        for version in versions {
            dispatch.record_seen_version(version);
        }

        dispatch.mark_onboarding_seen();
        let _ = RouterContext::get().replace(Route::Home {});
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::onboarding::test_support::{archive, cluster, file};
    use std::collections::HashSet;

    fn skyblock() -> BundleArchive {
        archive(
            "SkyBlock",
            false,
            vec![
                file("skyblock-main", true, false),
                file("skycubed", false, false),
                file("sb-dep", true, true),
            ],
        )
    }

    fn items(archives: Vec<BundleArchive>) -> Vec<ClusterBundles> {
        vec![ClusterBundles {
            cluster: cluster(1),
            archives,
        }]
    }

    fn keys(archive: &BundleArchive, pids: &[&str]) -> HashSet<String> {
        pids.iter()
            .map(|p| pkg_key(1, &archive.manifest.name, p))
            .collect()
    }

    #[test]
    fn declining_a_bundle_removes_its_visible_and_hidden_files() {
        let sb = skyblock();
        let plans = build_plans(&items(vec![sb.clone()]), &HashSet::new());

        let ov = &plans[0].overrides;
        // The hidden dependency must be dropped too, or it installs anyway.
        assert!(ov.contains(&(
            sb.manifest.name.clone(),
            "sb-dep".to_string(),
            OverrideType::Removed
        )));
        assert!(ov.contains(&(
            sb.manifest.name.clone(),
            "skyblock-main".to_string(),
            OverrideType::Removed
        )));
        assert!(!ov.iter().any(|(_, pid, _)| pid == "skycubed"));
    }

    #[test]
    fn accepting_a_bundle_records_nothing_for_its_defaults() {
        let sb = skyblock();
        let selected = keys(&sb, &["skyblock-main"]);
        let plans = build_plans(&items(vec![sb.clone()]), &selected);

        assert!(plans[0].overrides.is_empty());
    }

    #[test]
    fn opting_into_an_extra_writes_enabled() {
        let sb = skyblock();
        let selected = keys(&sb, &["skyblock-main", "skycubed"]);
        let plans = build_plans(&items(vec![sb.clone()]), &selected);

        assert_eq!(
            plans[0].overrides,
            vec![(
                sb.manifest.name.clone(),
                "skycubed".to_string(),
                OverrideType::Enabled
            )]
        );
    }

    #[test]
    fn an_extra_alone_still_keeps_hidden_dependencies() {
        let sb = skyblock();
        let selected = keys(&sb, &["skycubed"]);
        let plans = build_plans(&items(vec![sb.clone()]), &selected);
        let ov = &plans[0].overrides;

        assert!(ov.contains(&(
            sb.manifest.name.clone(),
            "skycubed".to_string(),
            OverrideType::Enabled
        )));
        assert!(!ov.iter().any(|(_, pid, _)| pid == "sb-dep"));
        assert!(ov.contains(&(
            sb.manifest.name.clone(),
            "skyblock-main".to_string(),
            OverrideType::Removed
        )));
    }

    #[test]
    fn rejections_are_removed_never_disabled() {
        let plans = build_plans(&items(vec![skyblock()]), &HashSet::new());

        assert!(
            !plans[0]
                .overrides
                .iter()
                .any(|(_, _, ty)| *ty == OverrideType::Disabled)
        );
    }

    #[test]
    fn per_cluster_choices_are_independent() {
        let sb = skyblock();
        let both = vec![
            ClusterBundles {
                cluster: cluster(1),
                archives: vec![sb.clone()],
            },
            ClusterBundles {
                cluster: cluster(2),
                archives: vec![sb.clone()],
            },
        ];
        let selected: HashSet<String> = [pkg_key(1, &sb.manifest.name, "skyblock-main")].into();
        let plans = build_plans(&both, &selected);

        assert!(plans[0].overrides.is_empty());
        assert!(
            plans[1]
                .overrides
                .iter()
                .any(|(_, pid, ty)| pid == "skyblock-main" && *ty == OverrideType::Removed)
        );
    }

    #[test]
    fn estimate_skips_declined_bundles_and_counts_opted_in_extras() {
        let sb = skyblock();
        let all = items(vec![sb.clone()]);
        let baseline = GAME_SIZE_GUESS + JRE_SIZE_GUESS;

        let declined = rough_download_estimate(&all, &HashSet::new());
        assert_eq!(declined, baseline, "declined bundle should cost nothing");

        let accepted = rough_download_estimate(&all, &keys(&sb, &["skyblock-main"]));
        assert_eq!(accepted, baseline + 2);

        let with_extra = rough_download_estimate(&all, &keys(&sb, &["skyblock-main", "skycubed"]));
        assert_eq!(with_extra, baseline + 3);
    }
}
