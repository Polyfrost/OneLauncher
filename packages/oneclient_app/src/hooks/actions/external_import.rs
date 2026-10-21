use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use freya::prelude::spawn_forever;
use oneclient_common::domain::ProviderId;
use oneclient_content::ContentError;
use oneclient_content::modpacks::{SHARE_EXTENSION, detect_format};
use oneclient_content::packages::PackageError;
use oneclient_core::clusters::ModpackSource;
use oneclient_core::{
    ExternalImportReport, ExternalInstance, ImportChoices, InstanceArchive, alternative_version_id,
    discard_instance_archive, import_external_instance, is_instance_archive, open_instance_archive,
};
use oneclient_events::{GroupedProgressSession, TaskCategory, TaskPhase};

use super::Actions;
use crate::launcher::{self, off_ui};
use crate::notifications::BlockedDownloads;

const MAX_LISTED: usize = 3;

static QUEUE: Mutex<Option<Vec<ExternalImportJob>>> = Mutex::new(None);

fn queue() -> MutexGuard<'static, Option<Vec<ExternalImportJob>>> {
    QUEUE.lock().unwrap_or_else(PoisonError::into_inner)
}

enum Admission {
    Start(Vec<ExternalImportJob>),
    Queued,
}

fn admit(jobs: Vec<ExternalImportJob>) -> Admission {
    let mut queue = queue();
    match queue.as_mut() {
        Some(pending) => {
            pending.extend(jobs);
            Admission::Queued
        }
        None => {
            *queue = Some(Vec::new());
            Admission::Start(jobs)
        }
    }
}

fn next_batch() -> Option<Vec<ExternalImportJob>> {
    let mut queue = queue();
    let pending = queue.as_mut()?;
    if pending.is_empty() {
        *queue = None;
        return None;
    }
    Some(std::mem::take(pending))
}

static WAITING_FOR_INSTALL: Mutex<Vec<ExternalImportJob>> = Mutex::new(Vec::new());

pub(super) fn take_waiting() -> Vec<ExternalImportJob> {
    std::mem::take(
        &mut *WAITING_FOR_INSTALL
            .lock()
            .unwrap_or_else(PoisonError::into_inner),
    )
}

fn queued_count() -> usize {
    queue().as_ref().map_or(0, Vec::len)
}

struct RunningImport {
    actions: Actions,
    workdirs: HashSet<PathBuf>,
    finished: bool,
}

impl RunningImport {
    fn hold(&mut self, jobs: &[ExternalImportJob]) {
        self.workdirs
            .extend(jobs.iter().filter_map(|job| job.workdir.clone()));
    }
}

impl Drop for RunningImport {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        let dropped = queue().take().unwrap_or_default();
        self.hold(&dropped);
        discard_workdir_set(std::mem::take(&mut self.workdirs));
        self.actions
            .notify("Import stopped")
            .body("Something went wrong, so the remaining imports were cancelled. Try importing them again.")
            .error()
            .send();
        self.actions.resume_imports();
    }
}

#[must_use]
pub(super) fn import_running() -> bool {
    queue().is_some()
}

#[derive(Debug, Clone)]
pub struct ExternalImportJob {
    pub instance: ExternalInstance,
    pub choices: ImportChoices,
    pub alternatives: Vec<AlternativePick>,
    pub workdir: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct AlternativePick {
    pub provider: ProviderId,
    pub project_id: String,
    pub name: String,
}

impl ExternalImportJob {
    pub fn as_is(instance: ExternalInstance) -> Self {
        Self {
            instance,
            choices: ImportChoices::default(),
            alternatives: Vec::new(),
            workdir: None,
        }
    }

    fn from_archive(archive: InstanceArchive) -> Vec<Self> {
        let InstanceArchive { instances, workdir } = archive;
        instances
            .into_iter()
            .map(|instance| {
                let mut job = Self::as_is(instance);
                job.choices.move_files = true;
                job.workdir = Some(workdir.clone());
                job
            })
            .collect()
    }
}

fn discard_workdir_set(workdirs: HashSet<PathBuf>) {
    if workdirs.is_empty() {
        return;
    }
    spawn_forever(async move {
        off_ui(async move {
            for workdir in workdirs {
                discard_instance_archive(&workdir).await;
            }
        })
        .await;
    });
}

impl Actions {
    /// Instances go one at a time: each can pull a whole modpack and hash
    /// hundreds of files, and running them side by side only fights over disk
    pub fn import_external_instances(&self, jobs: Vec<ExternalImportJob>) {
        if jobs.is_empty() {
            return;
        }
        if !super::modpacks::modpack_job_idle() {
            WAITING_FOR_INSTALL
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .extend(jobs);
            self.notify("Queued for import")
                .body("It will import as soon as the current install finishes.")
                .send();
            return;
        }
        let jobs = match admit(jobs) {
            Admission::Start(jobs) => jobs,
            Admission::Queued => {
                self.notify("Queued for import")
                    .body("It will import as soon as the current import finishes.")
                    .send();
                return;
            }
        };

        let actions = self.clone();
        spawn_forever(async move {
            let mut running = RunningImport {
                actions: actions.clone(),
                workdirs: HashSet::new(),
                finished: false,
            };
            running.hold(&jobs);
            let Ok(state) = launcher::state() else {
                return;
            };
            let events = state.services.events.clone();

            let mut imported: Vec<ExternalImportReport> = Vec::new();
            let mut unavailable: Vec<String> = Vec::new();
            let mut done = 0usize;
            let mut batch = jobs;
            loop {
                let batch_len = batch.len();
                for (index, job) in batch.into_iter().enumerate() {
                    let ExternalImportJob {
                        instance,
                        choices,
                        alternatives,
                        workdir: _,
                    } = job;
                    let name = instance.name.clone();
                    let total = done + (batch_len - index) + queued_count();
                    let position = done + 1;
                    done += 1;
                    let outcome = off_ui({
                        let state = state.clone();
                        async move {
                            let title = if total > 1 {
                                format!("Importing {} ({position} of {total})", instance.name)
                            } else {
                                format!("Importing {}", instance.name)
                            };
                            let session =
                                GroupedProgressSession::start(&state.services.events, title);
                            let outcome = import_external_instance(
                                &state,
                                &instance,
                                &choices,
                                Some(&session),
                            )
                            .await;
                            if outcome.is_ok() {
                                session.finish();
                            } else {
                                session.abort();
                            }
                            outcome
                        }
                    })
                    .await;

                    events.signal(oneclient_events::Signal::ClustersChanged);
                    crate::hooks::invalidate_cluster_queries().await;

                    match outcome {
                        Ok(report) => {
                            if !report.blocked.is_empty() {
                                actions.open_blocked_downloads(BlockedDownloads {
                                    cluster_id: report.cluster_id,
                                    cluster_name: report.cluster_name.clone(),
                                    files: report.blocked.clone(),
                                    bundle_name: oneclient_content::modpacks::MODPACK_BUNDLE_NAME
                                        .into(),
                                    added: Default::default(),
                                    open_when_done: false,
                                });
                            }
                            unavailable.extend(
                                actions
                                    .install_alternatives(report.cluster_id, alternatives)
                                    .await,
                            );
                            imported.push(report);
                        }
                        Err(err) => {
                            tracing::warn!(instance = %name, error = %err, "could not import instance");
                            actions
                                .notify(format!("Could not import {name}"))
                                .body(err.to_string())
                                .error()
                                .send();
                        }
                    }
                }

                match next_batch() {
                    Some(next) => {
                        running.hold(&next);
                        batch = next;
                    }
                    None => {
                        running.finished = true;
                        break;
                    }
                }
            }

            discard_workdir_set(std::mem::take(&mut running.workdirs));
            notify_summary(&actions, &imported, &unavailable);
            if let [only] = imported.as_slice() {
                actions.open_cluster_page(only.cluster_id);
            }
            actions.resume_imports();
        });
    }

    pub fn import_file(&self, path: PathBuf) {
        let is_mrpack = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case(SHARE_EXTENSION));
        if is_mrpack {
            self.install_modpack(ModpackSource::File(path));
            return;
        }

        let actions = self.clone();
        spawn_forever(async move {
            let probe = path.clone();
            let verdict = off_ui(async move {
                if is_instance_archive(&probe).await {
                    return Ok(true);
                }
                match detect_format(&probe).await {
                    Ok(_) => Ok(false),
                    Err(ContentError::Package(PackageError::UnsupportedModpackFormat)) => Err(None),
                    Err(err) => Err(Some(err.to_string())),
                }
            })
            .await;
            match verdict {
                Ok(true) => actions.import_instance_archive(path),
                Ok(false) => actions.install_modpack(ModpackSource::File(path)),
                Err(reason) => {
                    let file = path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "the file".to_string());
                    if let Some(reason) = &reason {
                        tracing::warn!(file, error = %reason, "could not read the dropped file");
                    }
                    actions
                        .notify(format!("Can't import {file}"))
                        .body(reason.unwrap_or_else(|| {
                            "This .zip isn't a OneClient, Prism Launcher, MultiMC or CurseForge export."
                                .to_string()
                        }))
                        .error()
                        .send();
                }
            }
        });
    }

    pub fn import_instance_archive(&self, path: PathBuf) {
        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else {
                return;
            };
            let file = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "the file".to_string());

            let opened = off_ui({
                let state = state.clone();
                let file = file.clone();
                async move {
                    let session = GroupedProgressSession::start(
                        &state.services.events,
                        format!("Reading {file}"),
                    );
                    let child = session.child("Unpacking the instance", 1, TaskCategory::Packages);
                    child.set_phase(TaskPhase::Importing);
                    let opened = open_instance_archive(&path).await;
                    child.finish();
                    if opened.is_ok() {
                        session.finish();
                    } else {
                        session.abort();
                    }
                    opened
                }
            })
            .await;

            match opened {
                Ok(archive) => {
                    actions.import_external_instances(ExternalImportJob::from_archive(archive));
                }
                Err(err) => {
                    tracing::warn!(file, error = %err, "could not open an instance export");
                    actions
                        .notify(format!("Could not open {file}"))
                        .body(err.to_string())
                        .error()
                        .send();
                }
            }
        });
    }

    async fn install_alternatives(
        &self,
        cluster_id: i64,
        alternatives: Vec<AlternativePick>,
    ) -> Vec<String> {
        let Ok(state) = launcher::state() else {
            return alternatives.into_iter().map(|pick| pick.name).collect();
        };

        let mut unavailable = Vec::new();
        for AlternativePick {
            provider,
            project_id,
            name,
        } in alternatives
        {
            let picked =
                off_ui({
                    let state = state.clone();
                    let project_id = project_id.clone();
                    async move {
                        alternative_version_id(&state, cluster_id, provider, &project_id).await
                    }
                })
                .await;
            match picked {
                Ok(Some(version_id)) => {
                    self.start_install(cluster_id, provider, project_id, version_id, None, true);
                }
                Ok(None) => unavailable.push(name),
                Err(err) => {
                    tracing::warn!(%project_id, error = %err, "could not pick a version of an alternative mod");
                    unavailable.push(name);
                }
            }
        }
        unavailable
    }
}

fn notify_summary(actions: &Actions, imported: &[ExternalImportReport], unavailable: &[String]) {
    if imported.is_empty() {
        return;
    }

    let names = listed(imported.iter().map(|r| r.cluster_name.clone()));
    let mut body = if imported.len() == 1 {
        format!("{names} is ready to play.")
    } else {
        format!("{names} are ready to play.")
    };

    let failed: Vec<String> = imported
        .iter()
        .flat_map(|r| r.content_failed.iter().cloned())
        .collect();
    let skipped: usize = imported.iter().map(|r| r.mods_skipped).sum();
    let left_out: usize = imported.iter().map(|r| r.mods_left_out).sum();
    let blocked: usize = imported.iter().map(|r| r.blocked.len()).sum();

    if !failed.is_empty() {
        body.push_str(&format!(
            " Could not bring over {}.",
            listed(failed.into_iter())
        ));
    }
    if skipped > 0 {
        body.push_str(&format!(
            " {skipped} mod{} skipped: the instance has no mod loader.",
            if skipped == 1 { " was" } else { "s were" }
        ));
    }
    if left_out > 0 {
        body.push_str(&format!(
            " {left_out} mod{} left out.",
            if left_out == 1 { " was" } else { "s were" }
        ));
    }
    if !unavailable.is_empty() {
        body.push_str(&format!(
            " Could not find a version of {} to install instead.",
            listed(unavailable.iter().cloned())
        ));
    }
    if blocked > 0 {
        body.push_str(" Some modpack files still need a manual download from CurseForge.");
    }
    for report in imported {
        if !report.parts_skipped.is_empty() {
            body.push_str(&format!(
                " {} was imported without {}: jar mods and custom components aren't supported.",
                report.cluster_name,
                listed(report.parts_skipped.iter().cloned())
            ));
        }
        if let Some(major) = report.java_pinned {
            body.push_str(&format!(
                " {} uses Java {major}, as it did before.",
                report.cluster_name
            ));
        }
        if let Some(major) = report.jvm_args_dropped_for {
            body.push_str(&format!(
                " {}'s Java arguments were left out: they were written for Java {major}, \
                 which is not set up.",
                report.cluster_name
            ));
        }
    }
    let unreadable: Vec<String> = imported
        .iter()
        .flat_map(|r| r.files_skipped.iter().cloned())
        .collect();
    if !unreadable.is_empty() {
        tracing::warn!(files = ?unreadable, "import skipped files it could not read");
        body.push_str(&format!(
            " Could not copy {}; it may be open in another program.",
            listed(unreadable.into_iter())
        ));
    }

    let title = match imported.len() {
        1 => "Instance imported".to_string(),
        n => format!("{n} instances imported"),
    };
    actions.notify(title).body(body).send();
}

fn listed(names: impl Iterator<Item = String>) -> String {
    let names: Vec<String> = names.collect();
    let shown = names.iter().take(MAX_LISTED).cloned().collect::<Vec<_>>();
    let rest = names.len().saturating_sub(shown.len());
    match (shown.as_slice(), rest) {
        ([], _) => String::new(),
        ([one], 0) => one.clone(),
        (many, 0) => {
            let (last, head) = many.split_last().expect("non-empty");
            format!("{} and {last}", head.join(", "))
        }
        (many, rest) => format!("{} and {rest} more", many.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::{Admission, admit, import_running, listed, next_batch, queued_count};

    #[test]
    fn later_imports_wait_in_the_queue() {
        assert!(!import_running());
        assert!(matches!(admit(Vec::new()), Admission::Start(_)));
        assert!(import_running());
        assert!(matches!(admit(Vec::new()), Admission::Queued));
        assert_eq!(queued_count(), 0);
        assert!(next_batch().is_none());
        assert!(!import_running());
    }

    #[test]
    fn lists_read_naturally() {
        let names = |n: &[&str]| listed(n.iter().map(|s| s.to_string()));
        assert_eq!(names(&["A"]), "A");
        assert_eq!(names(&["A", "B"]), "A and B");
        assert_eq!(names(&["A", "B", "C"]), "A, B and C");
        assert_eq!(names(&["A", "B", "C", "D", "E"]), "A, B, C and 2 more");
    }
}
