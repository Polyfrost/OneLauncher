use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use freya::prelude::spawn_forever;
use oneclient_content::modpacks::{
    BlockedFile, MODPACK_BUNDLE_NAME, ModpackInstallReport, find_blocked_downloads,
    import_blocked_files,
};
use oneclient_content::packages::ResolvedAlternative;
use oneclient_core::clusters::{
    ExistingPack, LoaderPlan, ModpackCluster, ModpackSource, PreparedImport, PreparedModpack,
    create_modpack_instance,
    import_modpack_into_cluster, install_modpack_instance, prepare_modpack, prepare_modpack_import,
    remove_imported_modpack, repair_modpack_cluster, update_modpack_cluster,
};
use oneclient_db::models::ClusterId;
use oneclient_events::GroupedProgressSession;

use super::Actions;
use crate::i18n::{t, tf};
use crate::launcher::{self, off_ui};
use crate::notifications::{
    BlockedDownloads, ExistingPackView, FlaggedChoice, ModpackConfirm, ModpackImportView,
};
use crate::state::AppChannel;

const MAX_LISTED: usize = 3;

enum JobSlot {
    Idle,
    Preparing(Option<ClusterId>),
    AwaitingConfirm(Box<PreparedModpack>),
    AwaitingImport(Box<PreparedImport>),
    Running(ClusterId),
}

static JOB: Mutex<JobSlot> = Mutex::new(JobSlot::Idle);

static WAITING: Mutex<VecDeque<ModpackSource>> = Mutex::new(VecDeque::new());

fn waiting() -> MutexGuard<'static, VecDeque<ModpackSource>> {
    WAITING.lock().unwrap_or_else(PoisonError::into_inner)
}

fn with_slot<R>(change: impl FnOnce(&mut JobSlot) -> R) -> R {
    let mut slot = JOB.lock().unwrap_or_else(PoisonError::into_inner);
    change(&mut slot)
}

#[must_use]
pub(super) fn modpack_job_idle() -> bool {
    with_slot(|slot| matches!(slot, JobSlot::Idle))
}

fn busy_cluster() -> Option<ClusterId> {
    with_slot(|slot| match slot {
        JobSlot::Running(id) | JobSlot::Preparing(Some(id)) => Some(*id),
        JobSlot::AwaitingImport(import) => Some(import.cluster_id),
        _ => None,
    })
}

#[must_use]
pub fn modpack_job_running(cluster_id: ClusterId) -> bool {
    busy_cluster() == Some(cluster_id)
}

pub(super) async fn modpack_job_in_shared_dir(cluster_id: ClusterId) -> bool {
    let Some(busy) = busy_cluster().filter(|id| *id != cluster_id) else {
        return false;
    };
    let Ok(state) = launcher::state() else {
        return false;
    };
    let shares = |id| {
        let state = state.clone();
        async move {
            state
                .clusters
                .get(id)
                .await
                .is_ok_and(|cluster| !cluster.uses_dedicated_dir())
        }
    };
    shares(cluster_id).await && shares(busy).await
}

fn take_prepared() -> Option<PreparedModpack> {
    with_slot(|slot| match std::mem::replace(slot, JobSlot::Preparing(None)) {
        JobSlot::AwaitingConfirm(prepared) => Some(*prepared),
        other => {
            *slot = other;
            None
        }
    })
}

fn take_prepared_import() -> Option<PreparedImport> {
    with_slot(|slot| match std::mem::replace(slot, JobSlot::Preparing(None)) {
        JobSlot::AwaitingImport(prepared) => {
            *slot = JobSlot::Preparing(Some(prepared.cluster_id));
            Some(*prepared)
        }
        other => {
            *slot = other;
            None
        }
    })
}

enum ModpackJob {
    Install(Box<PreparedModpack>),
    Import {
        import: Box<PreparedImport>,
        existing: ExistingPack,
        skipped: HashSet<String>,
        alternatives: Vec<ResolvedAlternative>,
    },
    Update {
        cluster_id: ClusterId,
        version_id: String,
    },
    Repair {
        cluster_id: ClusterId,
    },
}

struct JobTexts {
    progress: &'static str,
    done: &'static str,
    problems: &'static str,
    failed: &'static str,
}

impl ModpackJob {
    fn texts(&self) -> JobTexts {
        match self {
            Self::Install(_) => JobTexts {
                progress: t("oneclient.actions.modpacks.texts.install.progress"),
                done: t("oneclient.actions.modpacks.texts.install.done"),
                problems: t("oneclient.actions.modpacks.texts.install.problems"),
                failed: t("oneclient.actions.modpacks.texts.install.failed"),
            },
            Self::Import { .. } => JobTexts {
                progress: t("oneclient.actions.modpacks.texts.import.progress"),
                done: t("oneclient.actions.modpacks.texts.import.done"),
                problems: t("oneclient.actions.modpacks.texts.import.problems"),
                failed: t("oneclient.actions.modpacks.texts.import.failed"),
            },
            Self::Update { .. } => JobTexts {
                progress: t("oneclient.actions.modpacks.texts.update.progress"),
                done: t("oneclient.actions.modpacks.texts.update.done"),
                problems: t("oneclient.actions.modpacks.texts.update.problems"),
                failed: t("oneclient.actions.modpacks.texts.update.failed"),
            },
            Self::Repair { .. } => JobTexts {
                progress: t("oneclient.actions.modpacks.texts.repair.progress"),
                done: t("oneclient.actions.modpacks.texts.repair.done"),
                problems: t("oneclient.actions.modpacks.texts.repair.problems"),
                failed: t("oneclient.actions.modpacks.texts.repair.failed"),
            },
        }
    }

    fn done_body(&self, cluster_name: &str) -> String {
        match self {
            Self::Install(_) => tf("oneclient.actions.modpacks.done.install", &[&cluster_name]),
            Self::Import { import, .. } => tf(
                "oneclient.actions.modpacks.done.import",
                &[&import.pack_name(), &cluster_name],
            ),
            Self::Update { .. } => tf("oneclient.actions.modpacks.done.update", &[&cluster_name]),
            Self::Repair { .. } => tf("oneclient.actions.modpacks.done.repair", &[&cluster_name]),
        }
    }
}

fn confirm_view(prepared: &PreparedModpack) -> ModpackConfirm {
    let manifest = &prepared.manifest;
    let loader = match (&manifest.loader, &manifest.loader_version) {
        (loader, _) if !loader.is_modded() => t("oneclient.actions.modpacks.vanilla").to_string(),
        (loader, Some(version)) => format!("{loader} {version}"),
        (loader, None) => loader.to_string(),
    };
    let source = match &prepared.source {
        ModpackSource::Provider { provider, .. } => provider.to_string(),
        ModpackSource::File(path) => path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| t("oneclient.actions.modpacks.a_file").to_string()),
    };

    let (pack_name, version) = match &prepared.share {
        Some(shared) if shared.share.modpack.is_some() => (
            shared.share.name.clone(),
            format!("{} {}", manifest.name, manifest.version),
        ),
        Some(shared) => (
            shared.share.name.clone(),
            t("oneclient.actions.modpacks.shared_instance").to_string(),
        ),
        None if manifest.name.trim().is_empty() => {
            (prepared.instance_name.clone(), manifest.version.clone())
        }
        None => (manifest.name.clone(), manifest.version.clone()),
    };

    ModpackConfirm {
        notices: prepared.notices().to_vec(),
        pack_name,
        version,
        instance_name: prepared.instance_name.clone(),
        mc_version: manifest.mc_version.clone(),
        loader,
        source,
        summary: prepared.summary(),
        import: None,
        flagged: prepared.flagged.clone(),
    }
}

fn import_view(import: &PreparedImport) -> ModpackConfirm {
    let mut view = confirm_view(&import.prepared);
    view.pack_name = import.pack_name();
    view.instance_name = import.cluster_name.clone();
    let existing = import.existing().map(|pack| ExistingPackView {
        name: pack.name.clone(),
        version: pack.version.clone(),
    });
    view.import = Some(ModpackImportView {
        cluster_name: import.cluster_name.clone(),
        previous_version: import
            .previous
            .as_ref()
            .filter(|_| existing.is_none())
            .map(|previous| previous.version.clone()),
        existing,
        notes: import_notes(import),
        flagged: import.flagged.clone(),
    });
    view
}

fn import_notes(import: &PreparedImport) -> Vec<(String, bool)> {
    let loader = import.prepared.manifest.loader;
    let mut notes = Vec::new();
    match &import.loader {
        LoaderPlan::Raise { from, to } => notes.push((
            tf(
                "oneclient.actions.modpacks.notes.loader_raise",
                &[&loader, &from, &to],
            ),
            false,
        )),
        LoaderPlan::Behind { current, needed } => notes.push((
            tf(
                "oneclient.actions.modpacks.notes.loader_behind",
                &[&loader, &needed, &import.cluster_name, &current],
            ),
            true,
        )),
        LoaderPlan::Keep => {}
    }
    if import.shared_game_dir {
        notes.push((
            tf(
                "oneclient.actions.modpacks.notes.shared_game_dir",
                &[&import.cluster_name],
            ),
            false,
        ));
    }
    notes
}

fn split_choices(
    import: &PreparedImport,
    choices: &HashMap<String, FlaggedChoice>,
) -> (HashSet<String>, Vec<ResolvedAlternative>) {
    let mut skipped = HashSet::new();
    let mut alternatives = Vec::new();
    for flagged in &import.flagged {
        match choices
            .get(&flagged.package_id)
            .copied()
            .unwrap_or_default()
        {
            FlaggedChoice::Keep => {}
            FlaggedChoice::Skip => {
                skipped.insert(flagged.package_id.clone());
            }
            FlaggedChoice::Replace(index) => {
                skipped.insert(flagged.package_id.clone());
                if let Some(alternative) = flagged.alternatives.get(index)
                    && alternative.version_id.is_some()
                    && !alternatives.contains(alternative)
                {
                    alternatives.push(alternative.clone());
                }
            }
        }
    }
    (skipped, alternatives)
}

impl Actions {
    fn claim_modpack_job(&self, target: Option<ClusterId>) -> bool {
        if super::external_import::import_running() {
            self.notify(t("oneclient.actions.modpacks.busy.import"))
                .body(t("oneclient.actions.modpacks.busy.body"))
                .error()
                .send();
            return false;
        }
        let claimed = with_slot(|slot| match slot {
            JobSlot::Idle => {
                *slot = JobSlot::Preparing(target);
                true
            }
            _ => false,
        });
        if !claimed {
            self.notify(t("oneclient.actions.modpacks.busy.install"))
                .body(t("oneclient.actions.modpacks.busy.body"))
                .error()
                .send();
            return false;
        }
        self.station
            .clone()
            .write_channel(AppChannel::Installs)
            .installs
            .modpack_busy = true;
        true
    }

    fn lock_modpack_cluster(&self, cluster_id: ClusterId) {
        with_slot(|slot| *slot = JobSlot::Running(cluster_id));
        self.station
            .clone()
            .write_channel(AppChannel::Installs)
            .installs
            .modpack_cluster = Some(cluster_id);
    }

    fn release_modpack_job(&self) {
        with_slot(|slot| *slot = JobSlot::Idle);
        {
            let mut station = self.station;
            let mut app = station.write_channel(AppChannel::Installs);
            app.installs.modpack_busy = false;
            app.installs.modpack_project = None;
            app.installs.modpack_cluster = None;
        }
        self.resume_imports();
    }

    pub(super) fn resume_imports(&self) {
        if super::external_import::import_running() || !modpack_job_idle() {
            return;
        }
        let launcher_jobs = super::external_import::take_waiting();
        if !launcher_jobs.is_empty() {
            self.import_external_instances(launcher_jobs);
            return;
        }
        let next = waiting().pop_front();
        if let Some(next) = next {
            self.start_modpack(next, true);
        }
    }

    pub fn install_modpack(&self, source: ModpackSource) {
        if matches!(source, ModpackSource::File(_))
            && (super::external_import::import_running() || !modpack_job_idle())
        {
            waiting().push_back(source);
            self.notify(t("oneclient.actions.modpacks.queued.title"))
                .body(t("oneclient.actions.modpacks.queued.body"))
                .send();
            return;
        }
        self.start_modpack(source, false);
    }

    fn start_modpack(&self, source: ModpackSource, queued: bool) {
        if !self.claim_modpack_job(None) {
            return;
        }
        let from_browser = match &source {
            ModpackSource::Provider {
                provider,
                project_id,
                ..
            } => {
                self.station
                    .clone()
                    .write_channel(AppChannel::Installs)
                    .installs
                    .modpack_project = Some((*provider, project_id.clone()));
                true
            }
            ModpackSource::File(_) => false,
        };

        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else {
                actions.release_modpack_job();
                return;
            };

            let prepared = off_ui({
                let state = state.clone();
                async move {
                    let session = GroupedProgressSession::start(
                        &state.services.events,
                        t("oneclient.actions.modpacks.reading"),
                    );
                    let prepared = prepare_modpack(&state, &source, Some(&session)).await;
                    session.finish();
                    prepared
                }
            })
            .await;

            match prepared {
                Ok(prepared) if from_browser && prepared.flagged.is_none() => {
                    actions.run_modpack_job(ModpackJob::Install(Box::new(prepared)));
                }
                Ok(prepared) => {
                    let mut view = confirm_view(&prepared);
                    if queued {
                        view.notices
                            .insert(0, t("oneclient.actions.modpacks.queued_notice").to_string());
                    }
                    with_slot(|slot| *slot = JobSlot::AwaitingConfirm(Box::new(prepared)));
                    actions.with_engine(move |app| app.notifications.open_modpack_confirm(view));
                }
                Err(err) => {
                    tracing::warn!(error = %err, "could not read the modpack");
                    actions
                        .notify(t("oneclient.actions.modpacks.read_failed"))
                        .body(err.to_string())
                        .error()
                        .send();
                    actions.release_modpack_job();
                }
            }
        });
    }

    pub fn confirm_modpack(&self) {
        self.with_engine(|app| app.notifications.close_modpack_confirm());
        if let Some(prepared) = take_prepared() {
            self.run_modpack_job(ModpackJob::Install(Box::new(prepared)));
        }
    }

    pub fn import_modpack(&self, cluster_id: ClusterId, source: ModpackSource) {
        if busy_cluster() == Some(cluster_id) {
            return;
        }
        if self.refuse_while_running(cluster_id) || !self.claim_modpack_job(Some(cluster_id)) {
            return;
        }
        if let ModpackSource::Provider {
            provider,
            project_id,
            ..
        } = &source
        {
            self.station
                .clone()
                .write_channel(AppChannel::Installs)
                .installs
                .modpack_project = Some((*provider, project_id.clone()));
        }

        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else {
                actions.release_modpack_job();
                return;
            };

            let prepared = off_ui({
                let state = state.clone();
                async move {
                    let session = GroupedProgressSession::start(
                        &state.services.events,
                        t("oneclient.actions.modpacks.reading"),
                    );
                    let prepared =
                        prepare_modpack_import(&state, cluster_id, &source, Some(&session)).await;
                    session.finish();
                    prepared
                }
            })
            .await;

            match prepared {
                Ok(prepared) => {
                    let view = import_view(&prepared);
                    with_slot(|slot| *slot = JobSlot::AwaitingImport(Box::new(prepared)));
                    actions.with_engine(move |app| app.notifications.open_modpack_confirm(view));
                }
                Err(err) => {
                    tracing::warn!(cluster_id, error = %err, "could not prepare the modpack import");
                    actions
                        .notify(t("oneclient.actions.modpacks.texts.import.failed"))
                        .body(err.to_string())
                        .error()
                        .send();
                    actions.release_modpack_job();
                }
            }
        });
    }

    pub fn confirm_modpack_import(
        &self,
        choices: HashMap<String, FlaggedChoice>,
        existing: ExistingPack,
    ) {
        let pending = with_slot(|slot| match slot {
            JobSlot::AwaitingImport(import) => Some(import.cluster_id),
            _ => None,
        });
        if let Some(cluster_id) = pending
            && self.refuse_while_running(cluster_id)
        {
            return;
        }

        self.with_engine(|app| app.notifications.close_modpack_confirm());
        if let Some(import) = take_prepared_import() {
            let (skipped, alternatives) = split_choices(&import, &choices);
            self.run_modpack_job(ModpackJob::Import {
                import: Box::new(import),
                existing,
                skipped,
                alternatives,
            });
        }
    }

    pub fn remove_imported_modpack(
        &self,
        cluster_id: ClusterId,
        bundle_name: String,
        name: String,
    ) {
        if self.refuse_while_running(cluster_id) || !self.claim_modpack_job(Some(cluster_id)) {
            return;
        }
        self.lock_modpack_cluster(cluster_id);

        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else {
                actions.release_modpack_job();
                return;
            };

            let removed = off_ui({
                let state = state.clone();
                async move { remove_imported_modpack(&state, cluster_id, &bundle_name).await }
            })
            .await;

            crate::hooks::invalidate_cluster_queries().await;
            match removed {
                Ok(failed) if failed.is_empty() => {
                    actions
                        .notify(t("oneclient.actions.modpacks.removed.title"))
                        .body(tf("oneclient.actions.modpacks.removed.body", &[&name]))
                        .send();
                }
                Ok(failed) => {
                    actions
                        .notify(t("oneclient.actions.modpacks.partly_removed.title"))
                        .body(tf(
                            "oneclient.actions.modpacks.partly_removed.body",
                            &[&failed.len(), &name],
                        ))
                        .error()
                        .send();
                }
                Err(err) => {
                    tracing::warn!(cluster_id, error = %err, "could not remove the modpack");
                    actions
                        .notify(t("oneclient.actions.modpacks.remove_failed"))
                        .body(err.to_string())
                        .error()
                        .send();
                }
            }
            actions.release_modpack_job();
        });
    }

    pub fn cancel_modpack(&self) {
        self.with_engine(|app| app.notifications.close_modpack_confirm());
        let waiting = with_slot(|slot| {
            matches!(slot, JobSlot::AwaitingConfirm(_) | JobSlot::AwaitingImport(_))
        });
        if waiting {
            self.release_modpack_job();
        }
    }

    pub fn update_modpack(&self, cluster_id: ClusterId, version_id: String) {
        if self.refuse_while_running(cluster_id) || !self.claim_modpack_job(Some(cluster_id)) {
            return;
        }
        self.run_modpack_job(ModpackJob::Update {
            cluster_id,
            version_id,
        });
    }

    pub fn repair_modpack(&self, cluster_id: ClusterId) {
        if self.refuse_while_running(cluster_id) || !self.claim_modpack_job(Some(cluster_id)) {
            return;
        }
        self.run_modpack_job(ModpackJob::Repair { cluster_id });
    }

    fn refuse_while_running(&self, cluster_id: ClusterId) -> bool {
        if super::share::exporting(cluster_id) {
            self.notify(t("oneclient.actions.modpacks.blocked.exporting.title"))
                .body(t("oneclient.actions.modpacks.blocked.exporting.body"))
                .error()
                .send();
            return true;
        }
        if oneclient_core::clusters::is_resetting(cluster_id) {
            self.notify(t("oneclient.actions.modpacks.blocked.resetting.title"))
                .body(t("oneclient.actions.modpacks.blocked.resetting.body"))
                .error()
                .send();
            return true;
        }
        if self.station.peek().installs.cluster_installing(cluster_id) {
            self.notify(t("oneclient.actions.modpacks.blocked.installing.title"))
                .body(t("oneclient.actions.modpacks.blocked.installing.body"))
                .error()
                .send();
            return true;
        }
        if self.refuse_during_update_check(cluster_id) {
            return true;
        }
        let running = self.station.peek().game.is_launch_pending(cluster_id)
            || launcher::state().is_ok_and(|state| state.games.is_active(cluster_id));
        if running {
            self.notify(t("oneclient.actions.modpacks.blocked.running.title"))
                .body(t("oneclient.actions.modpacks.blocked.running.body"))
                .error()
                .send();
        }
        running
    }

    fn run_modpack_job(&self, job: ModpackJob) {
        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else {
                actions.release_modpack_job();
                return;
            };
            let events = state.services.events.clone();
            let texts = job.texts();

            let cluster_id = match &job {
                ModpackJob::Install(prepared) => {
                    let created = off_ui({
                        let state = state.clone();
                        let prepared = prepared.clone();
                        async move { create_modpack_instance(&state, &prepared).await }
                    })
                    .await;
                    match created {
                        Ok(cluster) => {
                            events.signal(oneclient_events::Signal::ClustersChanged);
                            cluster.id
                        }
                        Err(err) => {
                            tracing::warn!(error = %err, "{}", texts.failed);
                            events
                                .notify(texts.failed)
                                .body(err.to_string())
                                .error()
                                .send();
                            actions.release_modpack_job();
                            return;
                        }
                    }
                }
                ModpackJob::Import { import, .. } => import.cluster_id,
                ModpackJob::Update { cluster_id, .. } | ModpackJob::Repair { cluster_id } => {
                    *cluster_id
                }
            };
            actions.lock_modpack_cluster(cluster_id);

            let (job, outcome) = off_ui({
                let state = state.clone();
                async move {
                    let session =
                        GroupedProgressSession::start(&state.services.events, job.texts().progress);
                    let outcome = match &job {
                        ModpackJob::Install(prepared) => {
                            install_modpack_instance(&state, cluster_id, prepared, Some(&session))
                                .await
                        }
                        ModpackJob::Import {
                            import,
                            existing,
                            skipped,
                            ..
                        } => {
                            import_modpack_into_cluster(
                                &state,
                                import,
                                skipped,
                                *existing,
                                Some(&session),
                            )
                            .await
                        }
                        ModpackJob::Update { version_id, .. } => {
                            update_modpack_cluster(&state, cluster_id, version_id, Some(&session))
                                .await
                        }
                        ModpackJob::Repair { .. } => {
                            repair_modpack_cluster(&state, cluster_id, Some(&session)).await
                        }
                    };
                    session.finish();
                    (job, outcome)
                }
            })
            .await;

            match outcome {
                Ok(ModpackCluster { cluster, report }) => {
                    events.signal(oneclient_events::Signal::ClustersChanged);
                    crate::hooks::invalidate_cluster_queries().await;
                    notify_outcome(&actions, &job, &cluster.name, &report);

                    let opens_page = matches!(job, ModpackJob::Install(_));
                    let bundle_name = match &job {
                        ModpackJob::Import {
                            import, existing, ..
                        } => import.target_bundle_name(*existing).to_string(),
                        _ => MODPACK_BUNDLE_NAME.to_string(),
                    };
                    if !report.blocked.is_empty() {
                        actions.open_blocked_downloads(BlockedDownloads {
                            cluster_id: cluster.id,
                            cluster_name: cluster.name.clone(),
                            bundle_name,
                            files: report.blocked,
                            added: Default::default(),
                            open_when_done: opens_page,
                        });
                    } else if opens_page && report.failed.is_empty() {
                        actions.open_cluster_page(cluster.id);
                    }
                }
                Err(err) => {
                    tracing::warn!(error = %err, "{}", texts.failed);
                    if !matches!(job, ModpackJob::Install(_)) {
                        events.signal(oneclient_events::Signal::ClustersChanged);
                        crate::hooks::invalidate_cluster_queries().await;
                    }
                    let body = match &job {
                        ModpackJob::Install(_) => {
                            let removed = off_ui({
                                let state = state.clone();
                                async move { state.clusters.delete(cluster_id, true).await }
                            })
                            .await;
                            if let Err(cleanup) = &removed {
                                tracing::warn!(cluster_id, error = %cleanup, "could not remove the unfinished instance");
                            }
                            events.signal(oneclient_events::Signal::ClustersChanged);
                            crate::hooks::invalidate_cluster_queries().await;
                            match removed {
                                Ok(()) => {
                                    tf("oneclient.actions.modpacks.failed.install_removed", &[&err])
                                }
                                Err(_) => {
                                    tf("oneclient.actions.modpacks.failed.install_kept", &[&err])
                                }
                            }
                        }
                        ModpackJob::Import {
                            import,
                            existing,
                            alternatives,
                            ..
                        } => {
                            let picked: Vec<String> = alternatives
                                .iter()
                                .filter(|alternative| alternative.version_id.is_some())
                                .map(|alternative| alternative.name.clone())
                                .collect();
                            let replacements = if picked.is_empty() {
                                String::new()
                            } else {
                                format!(
                                    " {}",
                                    tf(
                                        "oneclient.actions.modpacks.failed.replacements",
                                        &[&listed(picked.into_iter())],
                                    )
                                )
                            };
                            let updating = import.previous.is_some()
                                && import.target_bundle_name(*existing) == import.bundle_name;
                            if updating {
                                tf(
                                    "oneclient.actions.modpacks.failed.import_updating",
                                    &[&err, &replacements],
                                )
                            } else {
                                tf(
                                    "oneclient.actions.modpacks.failed.import_new",
                                    &[&err, &replacements],
                                )
                            }
                        }
                        _ => tf("oneclient.actions.modpacks.failed.other", &[&err]),
                    };
                    events.notify(texts.failed).body(body).error().send();
                    actions.release_modpack_job();
                    return;
                }
            }

            if let ModpackJob::Import { alternatives, .. } = job {
                for alternative in alternatives {
                    let Some(version_id) = alternative.version_id else {
                        continue;
                    };
                    actions.start_install(
                        cluster_id,
                        alternative.provider,
                        alternative.project_id,
                        version_id,
                        None,
                        false,
                    );
                }
            }
            actions.release_modpack_job();
        });
    }

    pub fn open_cluster_page(&self, cluster_id: ClusterId) {
        self.with_engine(move |state| state.notifications.request_open_cluster(cluster_id));
    }

    pub fn take_open_cluster(&self) -> Option<ClusterId> {
        let mut cluster_id = None;
        self.with_engine(|state| cluster_id = state.notifications.take_open_cluster());
        cluster_id
    }

    pub fn open_blocked_downloads(&self, blocked: BlockedDownloads) {
        self.with_engine(move |state| state.notifications.open_blocked_downloads(blocked));
    }

    pub fn close_blocked_downloads(&self) {
        let mut remaining = None;
        self.with_engine(|state| remaining = state.notifications.take_blocked_downloads());

        let Some(blocked) = remaining else {
            return;
        };
        let missing = blocked.remaining();
        if missing.is_empty() {
            if blocked.open_when_done {
                self.open_cluster_page(blocked.cluster_id);
            }
            return;
        }

        self.notify(t("oneclient.actions.modpacks.blocked_downloads.title"))
            .body(tf(
                "oneclient.actions.modpacks.blocked_downloads.body",
                &[
                    &blocked.cluster_name,
                    &listed(missing.iter().map(|file| file.project_name.clone())),
                ],
            ))
            .error()
            .send();
    }

    pub fn scan_blocked_downloads(
        &self,
        cluster_id: ClusterId,
        bundle_name: String,
        locations: Vec<PathBuf>,
        files: Vec<BlockedFile>,
    ) {
        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };

            let imported = off_ui({
                let state = state.clone();
                async move {
                    let found = find_blocked_downloads(&locations, &files).await;
                    if found.is_empty() {
                        return Ok(Vec::new());
                    }
                    import_blocked_files(
                        cluster_id,
                        &bundle_name,
                        &found,
                        &state.services.content(),
                    )
                    .await
                    .map(|_| found.into_iter().map(|(_, file)| file).collect::<Vec<_>>())
                }
            })
            .await;

            match imported {
                Ok(found) if found.is_empty() => {}
                Ok(found) => {
                    let sha1s: Vec<String> = found.iter().map(|file| file.sha1.clone()).collect();
                    actions.with_engine(|app| {
                        app.notifications
                            .resolve_blocked_downloads(cluster_id, &sha1s);
                    });
                    crate::hooks::invalidate_cluster_queries().await;
                }
                Err(err) => {
                    tracing::warn!(cluster_id, error = %err, "could not add a manual download");
                    actions
                        .notify(t("oneclient.actions.modpacks.blocked_downloads.add_failed"))
                        .body(err.to_string())
                        .error()
                        .send();
                }
            }
        });
    }
}

fn notify_outcome(
    actions: &Actions,
    job: &ModpackJob,
    cluster_name: &str,
    report: &ModpackInstallReport,
) {
    let texts = job.texts();
    let kept = match report.kept.len() {
        0 => String::new(),
        1 => format!(" {}", tf("oneclient.actions.modpacks.kept.one", &[&1])),
        n => format!(" {}", tf("oneclient.actions.modpacks.kept.other", &[&n])),
    };
    let duplicates = if report.disabled_duplicates.is_empty() {
        String::new()
    } else {
        format!(
            " {}",
            tf(
                "oneclient.actions.modpacks.duplicates",
                &[&listed(report.disabled_duplicates.iter().cloned())],
            )
        )
    };
    let notices: String = report
        .notices
        .iter()
        .map(|notice| format!(" {notice}"))
        .collect();
    if report.failed.is_empty() && report.notices.is_empty() {
        actions
            .notify(texts.done)
            .body(format!("{}{kept}{duplicates}", job.done_body(cluster_name)))
            .send();
        return;
    }
    if report.failed.is_empty() {
        actions
            .notify(texts.problems)
            .body(format!(
                "{}{kept}{duplicates}{notices}",
                job.done_body(cluster_name)
            ))
            .error()
            .send();
        return;
    }

    actions
        .notify(texts.problems)
        .body(tf(
            "oneclient.actions.modpacks.outcome.failed",
            &[
                &cluster_name,
                &listed(report.failed.iter().cloned()),
                &notices,
            ],
        ))
        .error()
        .send();
}

pub(crate) fn listed(names: impl Iterator<Item = String>) -> String {
    let names: Vec<String> = names.collect();
    let shown = names
        .iter()
        .take(MAX_LISTED)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    match names.len().saturating_sub(MAX_LISTED) {
        0 => shown,
        extra => tf("oneclient.actions.listed.more", &[&shown, &extra]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oneclient_core::clusters::FlaggedPackMod;

    #[test]
    fn long_lists_are_cut_short() {
        let names = |n: usize| (1..=n).map(|i| format!("m{i}"));
        assert_eq!(listed(names(2)), "m1, m2");
        assert_eq!(listed(names(3)), "m1, m2, m3");
        assert_eq!(listed(names(5)), "m1, m2, m3 and 2 more");
    }
    fn alternative(project_id: &str, version_id: Option<&str>) -> ResolvedAlternative {
        ResolvedAlternative {
            provider: oneclient_content::packages::ProviderId::Modrinth,
            project_id: project_id.to_string(),
            name: project_id.to_string(),
            version_id: version_id.map(str::to_string),
            version_number: None,
            icon_url: None,
        }
    }

    fn flagged(package_id: &str, alternatives: Vec<ResolvedAlternative>) -> FlaggedPackMod {
        FlaggedPackMod {
            package_id: package_id.to_string(),
            name: package_id.to_string(),
            explanation: None,
            alternatives,
        }
    }

    fn import(flagged: Vec<FlaggedPackMod>) -> PreparedImport {
        PreparedImport {
            prepared: PreparedModpack {
                source: ModpackSource::File(PathBuf::from("pack.mrpack")),
                artifact_hash: "hash".to_string(),
                archive_path: PathBuf::from("pack.mrpack"),
                manifest: oneclient_content::modpacks::ModpackManifest {
                    name: "Pack".to_string(),
                    version: "1.0".to_string(),
                    summary: None,
                    mc_version: "1.21.1".to_string(),
                    loader: oneclient_common::domain::GameLoader::Fabric,
                    loader_version: None,
                    contents: oneclient_content::modpacks::ModpackContents::default(),
                    override_prefixes: Vec::new(),
                },
                instance_name: "Pack".to_string(),
                flagged: None,
                share: None,
            },
            cluster_id: 1,
            cluster_name: "Cluster".to_string(),
            bundle_name: "modpack:file:pack".to_string(),
            release: None,
            project: None,
            previous: None,
            same_name: None,
            separate_bundle_name: "modpack:file:pack-2".to_string(),
            loader: LoaderPlan::Keep,
            shared_game_dir: false,
            flagged,
        }
    }

    #[test]
    fn flagged_choices_become_skips_and_replacements() {
        let swap = alternative("swap", Some("v1"));
        let import = import(vec![
            flagged("kept", Vec::new()),
            flagged("skipped", Vec::new()),
            flagged("defaulted", Vec::new()),
            flagged("replaced", vec![alternative("unresolved", None), swap.clone()]),
            flagged("replaced-again", vec![swap.clone()]),
            flagged("replaced-missing", vec![swap.clone()]),
            flagged("replaced-unresolved", vec![alternative("unresolved", None)]),
        ]);
        let choices = HashMap::from([
            ("kept".to_string(), FlaggedChoice::Keep),
            ("skipped".to_string(), FlaggedChoice::Skip),
            ("replaced".to_string(), FlaggedChoice::Replace(1)),
            ("replaced-again".to_string(), FlaggedChoice::Replace(0)),
            ("replaced-missing".to_string(), FlaggedChoice::Replace(7)),
            ("replaced-unresolved".to_string(), FlaggedChoice::Replace(0)),
        ]);

        let (skipped, alternatives) = split_choices(&import, &choices);

        let mut skipped: Vec<String> = skipped.into_iter().collect();
        skipped.sort();
        assert_eq!(
            skipped,
            [
                "defaulted",
                "replaced",
                "replaced-again",
                "replaced-missing",
                "replaced-unresolved",
                "skipped"
            ]
        );
        assert_eq!(alternatives, vec![swap]);
    }
}
