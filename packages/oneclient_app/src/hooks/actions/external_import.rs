use std::sync::atomic::{AtomicBool, Ordering};

use freya::prelude::spawn_forever;
use oneclient_core::{ExternalImportReport, ExternalInstance, import_external_instance};
use oneclient_events::GroupedProgressSession;

use super::Actions;
use crate::launcher::{self, off_ui};
use crate::notifications::BlockedDownloads;

const MAX_LISTED: usize = 3;

static IMPORTING: AtomicBool = AtomicBool::new(false);

impl Actions {
    /// Instances go one at a time: each can pull a whole modpack and hash
    /// hundreds of files, and running them side by side only fights over disk
    pub fn import_external_instances(&self, instances: Vec<ExternalInstance>) {
        if instances.is_empty() {
            return;
        }
        if IMPORTING.swap(true, Ordering::AcqRel) {
            self.notify("An import is already running")
                .body("Wait for it to finish, then try again.")
                .error()
                .send();
            return;
        }

        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else {
                IMPORTING.store(false, Ordering::Release);
                return;
            };
            let events = state.services.events.clone();

            let mut imported: Vec<ExternalImportReport> = Vec::new();
            let mut blocked_shown = false;
            for instance in instances {
                let name = instance.name.clone();
                let outcome = off_ui({
                    let state = state.clone();
                    async move {
                        let session = GroupedProgressSession::start(
                            &state.services.events,
                            format!("Importing {}", instance.name),
                        );
                        let outcome =
                            import_external_instance(&state, &instance, Some(&session)).await;
                        session.finish();
                        outcome
                    }
                })
                .await;

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

            notify_summary(&actions, &imported);
            if let [only] = imported.as_slice() {
                actions.open_cluster_page(only.cluster_id);
            }

            IMPORTING.store(false, Ordering::Release);
        });
    }
}

fn notify_summary(actions: &Actions, imported: &[ExternalImportReport]) {
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
    use super::listed;

    #[test]
    fn lists_read_naturally() {
        let names = |n: &[&str]| listed(n.iter().map(|s| s.to_string()));
        assert_eq!(names(&["A"]), "A");
        assert_eq!(names(&["A", "B"]), "A and B");
        assert_eq!(names(&["A", "B", "C"]), "A, B and C");
        assert_eq!(names(&["A", "B", "C", "D", "E"]), "A, B, C and 2 more");
    }
}
