use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use freya::prelude::spawn_forever;
use oneclient_common::domain::ProviderId;
use oneclient_content::modpacks::SHARE_EXTENSION;
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

static IMPORTING: AtomicBool = AtomicBool::new(false);

/// Holds [`IMPORTING`] for as long as it lives, so a panic mid-import (which
/// `off_ui` re-raises) or a dropped task cannot leave every later import
/// refused until the app restarts
struct ImportingGuard;

impl ImportingGuard {
    fn acquire() -> Option<Self> {
        (!IMPORTING.swap(true, Ordering::AcqRel)).then_some(Self)
    }
}

impl Drop for ImportingGuard {
    fn drop(&mut self) {
        IMPORTING.store(false, Ordering::Release);
    }
}

#[must_use]
pub(super) fn import_running() -> bool {
    IMPORTING.load(Ordering::Acquire)
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

    fn from_archive(archive: InstanceArchive) -> Self {
        Self {
            workdir: Some(archive.workdir),
            ..Self::as_is(archive.instance)
        }
    }
}

fn discard_workdirs(jobs: &[ExternalImportJob]) {
    let workdirs: Vec<PathBuf> = jobs.iter().filter_map(|job| job.workdir.clone()).collect();
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
            self.notify("An install is already running")
                .body("Wait for it to finish, then try again.")
                .error()
                .send();
            discard_workdirs(&jobs);
            return;
        }
        let Some(guard) = ImportingGuard::acquire() else {
            self.notify("An import is already running")
                .body("Wait for it to finish, then try again.")
                .error()
                .send();
            discard_workdirs(&jobs);
            return;
        };

        let actions = self.clone();
        spawn_forever(async move {
            let _guard = guard;
            let Ok(state) = launcher::state() else {
                return;
            };
            let events = state.services.events.clone();

            let mut imported: Vec<ExternalImportReport> = Vec::new();
            let mut unavailable: Vec<String> = Vec::new();
            let mut blocked_shown = false;
            let total = jobs.len();
            for (position, job) in jobs.into_iter().enumerate() {
                let ExternalImportJob {
                    instance,
                    choices,
                    alternatives,
                    workdir,
                } = job;
                let name = instance.name.clone();
                let outcome = off_ui({
                    let state = state.clone();
                    async move {
                        let title = if total > 1 {
                            format!("Importing {} ({} of {total})", instance.name, position + 1)
                        } else {
                            format!("Importing {}", instance.name)
                        };
                        let session = GroupedProgressSession::start(&state.services.events, title);
                        let outcome =
                            import_external_instance(&state, &instance, &choices, Some(&session))
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
                if let Some(workdir) = workdir {
                    off_ui(async move { discard_instance_archive(&workdir).await }).await;
                }

                events.signal(oneclient_events::Signal::ClustersChanged);
                crate::hooks::invalidate_cluster_queries().await;

                match outcome {
                    Ok(mut report) => {
                        if !report.blocked.is_empty() && !blocked_shown {
                            blocked_shown = true;
                            actions.open_blocked_downloads(BlockedDownloads {
                                cluster_id: report.cluster_id,
                                cluster_name: report.cluster_name.clone(),
                                files: std::mem::take(&mut report.blocked),
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

            notify_summary(&actions, &imported, &unavailable);
            if let [only] = imported.as_slice() {
                actions.open_cluster_page(only.cluster_id);
            }
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
            let instance = off_ui(async move { is_instance_archive(&probe).await }).await;
            if instance {
                actions.import_instance_archive(path);
            } else {
                actions.install_modpack(ModpackSource::File(path));
            }
        });
    }

    pub fn import_instance_archive(&self, path: PathBuf) {
        if import_running() || !super::modpacks::modpack_job_idle() {
            self.notify("An import is already running")
                .body("Wait for it to finish, then try again.")
                .error()
                .send();
            return;
        }

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
                    actions
                        .import_external_instances(vec![ExternalImportJob::from_archive(archive)]);
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
    use super::{ImportingGuard, import_running, listed};

    #[test]
    fn a_running_import_is_visible_to_the_other_jobs() {
        assert!(!import_running());
        let guard = ImportingGuard::acquire().expect("nothing is importing yet");
        assert!(import_running());
        assert!(ImportingGuard::acquire().is_none());
        drop(guard);
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
