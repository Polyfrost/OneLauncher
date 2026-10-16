use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use freya::prelude::spawn_forever;
use oneclient_content::modpacks::{
    BlockedFile, ModpackInstallReport, find_blocked_downloads, import_blocked_files,
};
use oneclient_core::clusters::{
    ModpackCluster, ModpackSource, PreparedModpack, create_modpack_instance,
    install_modpack_instance, prepare_modpack, repair_modpack_cluster, update_modpack_cluster,
};
use oneclient_db::models::ClusterId;
use oneclient_events::GroupedProgressSession;

use super::Actions;
use crate::launcher::{self, off_ui};
use crate::notifications::{BlockedDownloads, ModpackConfirm};
use crate::state::AppChannel;

const MAX_LISTED: usize = 3;

enum JobSlot {
    Idle,
    Preparing,
    AwaitingConfirm(Box<PreparedModpack>),
    Running(ClusterId),
}

static JOB: Mutex<JobSlot> = Mutex::new(JobSlot::Idle);

fn with_slot<R>(change: impl FnOnce(&mut JobSlot) -> R) -> R {
    let mut slot = JOB.lock().unwrap_or_else(PoisonError::into_inner);
    change(&mut slot)
}

#[must_use]
pub fn modpack_job_running(cluster_id: ClusterId) -> bool {
    with_slot(|slot| matches!(slot, JobSlot::Running(id) if *id == cluster_id))
}

fn take_prepared() -> Option<PreparedModpack> {
    with_slot(|slot| match std::mem::replace(slot, JobSlot::Preparing) {
        JobSlot::AwaitingConfirm(prepared) => Some(*prepared),
        other => {
            *slot = other;
            None
        }
    })
}

enum ModpackJob {
    Install(Box<PreparedModpack>),
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
                progress: "Installing modpack",
                done: "Modpack installed",
                problems: "Modpack installed with problems",
                failed: "Modpack install failed",
            },
            Self::Update { .. } => JobTexts {
                progress: "Updating modpack",
                done: "Modpack updated",
                problems: "Modpack updated with problems",
                failed: "Modpack update failed",
            },
            Self::Repair { .. } => JobTexts {
                progress: "Reinstalling modpack files",
                done: "Modpack files reinstalled",
                problems: "Some modpack files are still missing",
                failed: "Could not reinstall the modpack files",
            },
        }
    }

    fn done_body(&self, cluster_name: &str) -> String {
        match self {
            Self::Install(_) => format!("{cluster_name} is ready to play."),
            Self::Update { .. } => format!("{cluster_name} is on the new version."),
            Self::Repair { .. } => format!("{cluster_name} has everything the pack lists."),
        }
    }
}

fn confirm_view(prepared: &PreparedModpack) -> ModpackConfirm {
    let manifest = &prepared.manifest;
    let loader = match (&manifest.loader, &manifest.loader_version) {
        (loader, _) if !loader.is_modded() => "Vanilla".to_string(),
        (loader, Some(version)) => format!("{loader} {version}"),
        (loader, None) => loader.to_string(),
    };
    let source = match &prepared.source {
        ModpackSource::Provider { provider, .. } => provider.to_string(),
        ModpackSource::File(path) => path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "a file".to_string()),
    };

    ModpackConfirm {
        pack_name: if manifest.name.trim().is_empty() {
            prepared.instance_name.clone()
        } else {
            manifest.name.clone()
        },
        version: manifest.version.clone(),
        instance_name: prepared.instance_name.clone(),
        mc_version: manifest.mc_version.clone(),
        loader,
        source,
        summary: manifest.summary(),
    }
}

impl Actions {
    fn claim_modpack_job(&self) -> bool {
        let claimed = with_slot(|slot| match slot {
            JobSlot::Idle => {
                *slot = JobSlot::Preparing;
                true
            }
            _ => false,
        });
        if !claimed {
            self.notify("A modpack is already being installed")
                .body("Wait for it to finish, then try again.")
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
        let mut station = self.station;
        let mut app = station.write_channel(AppChannel::Installs);
        app.installs.modpack_busy = false;
        app.installs.modpack_project = None;
        app.installs.modpack_cluster = None;
    }

    pub fn install_modpack(&self, source: ModpackSource) {
        if !self.claim_modpack_job() {
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
                    let session =
                        GroupedProgressSession::start(&state.services.events, "Reading modpack");
                    let prepared = prepare_modpack(&state, &source).await;
                    session.finish();
                    prepared
                }
            })
            .await;

            match prepared {
                Ok(prepared) if from_browser => {
                    actions.run_modpack_job(ModpackJob::Install(Box::new(prepared)));
                }
                Ok(prepared) => {
                    let view = confirm_view(&prepared);
                    with_slot(|slot| *slot = JobSlot::AwaitingConfirm(Box::new(prepared)));
                    actions.with_engine(move |app| app.notifications.open_modpack_confirm(view));
                }
                Err(err) => {
                    tracing::warn!(error = %err, "could not read the modpack");
                    actions
                        .notify("Could not read the modpack")
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
        match take_prepared() {
            Some(prepared) => self.run_modpack_job(ModpackJob::Install(Box::new(prepared))),
            None => self.release_modpack_job(),
        }
    }

    pub fn cancel_modpack(&self) {
        self.with_engine(|app| app.notifications.close_modpack_confirm());
        self.release_modpack_job();
    }

    pub fn update_modpack(&self, cluster_id: ClusterId, version_id: String) {
        if self.refuse_while_running(cluster_id) || !self.claim_modpack_job() {
            return;
        }
        self.run_modpack_job(ModpackJob::Update {
            cluster_id,
            version_id,
        });
    }

    pub fn repair_modpack(&self, cluster_id: ClusterId) {
        if self.refuse_while_running(cluster_id) || !self.claim_modpack_job() {
            return;
        }
        self.run_modpack_job(ModpackJob::Repair { cluster_id });
    }

    fn refuse_while_running(&self, cluster_id: ClusterId) -> bool {
        if oneclient_core::clusters::is_resetting(cluster_id) {
            self.notify("This instance is being reset")
                .body("Wait for the reset to finish, then try again.")
                .error()
                .send();
            return true;
        }
        let running = launcher::state().is_ok_and(|state| state.games.is_active(cluster_id));
        if running {
            self.notify("Close the game first")
                .body("The modpack's files cannot be changed while Minecraft is running.")
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
                    if !report.blocked.is_empty() {
                        actions.open_blocked_downloads(BlockedDownloads {
                            cluster_id: cluster.id,
                            cluster_name: cluster.name.clone(),
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
                                Ok(()) => format!("{err} The unfinished instance was removed."),
                                Err(_) => format!(
                                    "{err} Use Reinstall Missing Files in the instance's settings to finish it."
                                ),
                            }
                        }
                        _ => format!(
                            "{err} Use Reinstall Missing Files in the instance's settings to try again."
                        ),
                    };
                    events.notify(texts.failed).body(body).error().send();
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

        self.notify("Some mods are still missing")
            .body(format!(
                "{} is missing {}. It may not start until they are added.",
                blocked.cluster_name,
                listed(missing.iter().map(|file| file.project_name.clone())),
            ))
            .error()
            .send();
    }

    pub fn scan_blocked_downloads(
        &self,
        cluster_id: ClusterId,
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
                    import_blocked_files(cluster_id, &found, &state.services.content())
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
                        .notify("Could not add a download")
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
    if report.failed.is_empty() {
        actions
            .notify(texts.done)
            .body(job.done_body(cluster_name))
            .send();
        return;
    }

    actions
        .notify(texts.problems)
        .body(format!(
            "{cluster_name} could not get {}.",
            listed(report.failed.iter().cloned())
        ))
        .error()
        .send();
}

fn listed(names: impl Iterator<Item = String>) -> String {
    let names: Vec<String> = names.collect();
    let shown = names
        .iter()
        .take(MAX_LISTED)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    match names.len().saturating_sub(MAX_LISTED) {
        0 => shown,
        extra => format!("{shown} and {extra} more"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_lists_are_cut_short() {
        let names = |n: usize| (1..=n).map(|i| format!("m{i}"));
        assert_eq!(listed(names(2)), "m1, m2");
        assert_eq!(listed(names(3)), "m1, m2, m3");
        assert_eq!(listed(names(5)), "m1, m2, m3 and 2 more");
    }
}
