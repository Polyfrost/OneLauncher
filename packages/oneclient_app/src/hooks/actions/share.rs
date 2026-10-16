use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use freya::prelude::spawn_forever;
use oneclient_cluster::ClusterManager;
use oneclient_content::modpacks::{SHARE_EXTENSION, ShareExportReport};
use oneclient_core::clusters::{ExportMode, export_cluster};
use oneclient_db::models::ClusterId;
use oneclient_events::GroupedProgressSession;

use super::Actions;
use crate::components::IconType;
use crate::launcher::{self, off_ui};

const MAX_LISTED: usize = 3;

static EXPORTING: AtomicBool = AtomicBool::new(false);

struct ExportSlot;

impl ExportSlot {
    fn claim() -> Option<Self> {
        (!EXPORTING.swap(true, Ordering::AcqRel)).then_some(Self)
    }
}

impl Drop for ExportSlot {
    fn drop(&mut self) {
        EXPORTING.store(false, Ordering::Release);
    }
}

impl Actions {
    pub fn export_cluster(&self, cluster_id: ClusterId, mode: ExportMode) {
        let Some(slot) = ExportSlot::claim() else {
            self.notify("An export is already running")
                .body("Wait for it to finish, then try again.")
                .error()
                .send();
            return;
        };
        let actions = self.clone();
        spawn_forever(async move {
            let _slot = slot;
            let Ok(state) = launcher::state() else { return };

            let cluster = match state.clusters.get(cluster_id).await {
                Ok(cluster) => cluster,
                Err(err) => {
                    actions.report_export_failure(&err.to_string());
                    return;
                }
            };

            let Some(dest) = pick_export_file(&cluster.name).await else {
                return;
            };

            let exported = off_ui({
                let state = state.clone();
                let dest = dest.clone();
                let title = format!("Exporting {}", cluster.name);
                async move {
                    let session = GroupedProgressSession::start(&state.services.events, title);
                    let exported =
                        export_cluster(&state, cluster_id, &dest, mode, Some(&session)).await;
                    if exported.is_ok() {
                        session.finish();
                    } else {
                        session.abort();
                    }
                    exported
                }
            })
            .await;

            match exported {
                Ok(report) => actions.report_export(&cluster.name, &dest, &report),
                Err(err) => actions.report_export_failure(&err.to_string()),
            }
        });
    }

    fn report_export(
        &self,
        cluster_name: &str,
        dest: &std::path::Path,
        report: &ShareExportReport,
    ) {
        let file = dest
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| cluster_name.to_string());

        let mut body = format!("Saved {cluster_name} as {file}.");
        if !report.embedded.is_empty() {
            body.push_str(&format!(
                " {} not on Modrinth or CurseForge went inside the file.",
                count_label(report.embedded.len(), "file", "files")
            ));
        }
        if !report.missing.is_empty() {
            body.push_str(&format!(
                " Left out {} that could not be found: {}.",
                count_label(report.missing.len(), "file", "files"),
                listed(&report.missing)
            ));
        }

        let notification = self
            .notify("Instance exported")
            .body(body)
            .icon(IconType::Users01);
        if report.missing.is_empty() {
            notification.send();
        } else {
            notification.error().send();
        }
    }

    fn report_export_failure(&self, reason: &str) {
        tracing::error!("could not export an instance: {reason}");
        self.notify("Couldn't export the instance")
            .body(reason.to_string())
            .error()
            .send();
    }
}

async fn pick_export_file(cluster_name: &str) -> Option<PathBuf> {
    let stem = ClusterManager::sanitize_name(cluster_name);
    let stem = if stem.is_empty() {
        "instance".to_string()
    } else {
        stem
    };

    let mut dialog = rfd::AsyncFileDialog::new()
        .set_title("Export instance")
        .set_file_name(format!("{stem}.{SHARE_EXTENSION}"))
        .add_filter("OneClient instance", &[SHARE_EXTENSION]);
    if let Some(dir) =
        directories::UserDirs::new().and_then(|dirs| dirs.download_dir().map(PathBuf::from))
    {
        dialog = dialog.set_directory(dir);
    }

    let mut path = dialog.save_file().await?.path().to_path_buf();
    if path.extension().and_then(|ext| ext.to_str()) != Some(SHARE_EXTENSION) {
        path.set_extension(SHARE_EXTENSION);
    }
    Some(path)
}

fn count_label(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

fn listed(names: &[String]) -> String {
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
    fn only_one_export_runs_at_a_time() {
        let first = ExportSlot::claim().expect("nothing is exporting yet");
        assert!(ExportSlot::claim().is_none());
        drop(first);
        assert!(ExportSlot::claim().is_some());
    }

    #[test]
    fn counts_read_naturally() {
        assert_eq!(count_label(1, "file", "files"), "1 file");
        assert_eq!(count_label(4, "file", "files"), "4 files");
    }

    #[test]
    fn long_lists_are_cut_short() {
        let names: Vec<String> = (1..=5).map(|i| format!("m{i}")).collect();
        assert_eq!(listed(&names[..2]), "m1, m2");
        assert_eq!(listed(&names), "m1, m2, m3 and 2 more");
    }
}
