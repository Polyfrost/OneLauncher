use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, anyhow, bail};
use clap::Subcommand;
use oneclient_common::paths;
use oneclient_core::LauncherState;
use serde::{Deserialize, Serialize};

use super::Host;
use crate::Output;
use crate::output::{accent, bold, bytes, dim, good, heading, table, warn};

const LISTED_CLUSTERS: usize = 10;

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum StorageCommand {
    /// Show how much space OneClient uses
    Report {
        #[arg(long)]
        json: bool,
    },
    /// Free space taken by unused downloads and old cluster leftovers
    Clean {
        /// Only cached downloads no cluster uses
        #[arg(long)]
        cache: bool,
        /// Only content left behind by old clusters and unused natives
        #[arg(long)]
        leftovers: bool,
    },
    /// Move the game data folder somewhere else
    Move {
        #[arg(required_unless_present = "default")]
        dir: Option<PathBuf>,
        /// Move it back to the default place
        #[arg(long, conflicts_with = "dir")]
        default: bool,
        /// Do not ask for confirmation
        #[arg(short, long)]
        yes: bool,
    },
    /// Delete what a move left behind in the old folder
    DiscardOld {
        /// Do not ask for confirmation
        #[arg(short, long)]
        yes: bool,
    },
    /// Open the data folder in the file manager
    Open,
}

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum RecoveryCommand {
    /// List the database snapshots taken before each upgrade
    Snapshots,
    /// Put a snapshot back in place of the current database
    Restore {
        /// Snapshot file name, the newest when left out
        snapshot: Option<String>,
        /// Do not ask for confirmation
        #[arg(short, long)]
        yes: bool,
    },
    /// Start over with an empty database, game files stay where they are
    Reset {
        /// Do not ask for confirmation
        #[arg(short, long)]
        yes: bool,
    },
}

pub(super) async fn run(
    host: Host,
    state: &Arc<LauncherState>,
    command: StorageCommand,
    out: &Output,
) -> anyhow::Result<()> {
    match command {
        StorageCommand::Report { json } => {
            let report = oneclient_core::storage::storage_report(state).await?;
            if json {
                out.line(serde_json::to_string_pretty(&report)?);
                return Ok(());
            }

            let mut lines = vec![
                format!("{} {}", heading("Storage"), bytes(report.total_bytes)),
                String::new(),
            ];
            let rows: Vec<Vec<String>> = report
                .categories
                .iter()
                .map(|entry| {
                    vec![
                        bold(&entry.label),
                        bytes(entry.bytes),
                        dim(entry.path.display()),
                    ]
                })
                .collect();
            lines.push(table(&["WHAT", "SIZE", "FOLDER"], &rows));

            let mut clusters = report.clusters.clone();
            clusters.sort_by_key(|entry| std::cmp::Reverse(entry.bytes));
            if !clusters.is_empty() {
                lines.push(String::new());
                let rows: Vec<Vec<String>> = clusters
                    .iter()
                    .take(LISTED_CLUSTERS)
                    .map(|entry| vec![entry.label.clone(), bytes(entry.bytes)])
                    .collect();
                lines.push(table(&["CLUSTER", "SIZE"], &rows));
            }

            let leftovers = report.legacy_cluster_content.bytes + report.unused_natives.bytes;
            lines.push(String::new());
            lines.push(format!(
                "{} {} in unused downloads, {} in leftovers, free them with `oneclient_cli storage clean`",
                dim("Reclaimable:"),
                bytes(report.unreferenced_cache.bytes),
                bytes(leftovers)
            ));
            if let Some(old) = oneclient_core::relocate::leftovers(state).await {
                lines.push(format!(
                    "{} {} in the old data folder {}, remove it with `oneclient_cli storage discard-old`",
                    dim("Old folder:"),
                    bytes(old.bytes),
                    accent(old.path.display())
                ));
            }
            out.line(lines.join("\n"));
            Ok(())
        }
        StorageCommand::Clean { cache, leftovers } => {
            let both = !cache && !leftovers;
            if cache || both {
                let freed = oneclient_core::storage::clean_unreferenced_cache(state).await?;
                out.line(format!("Freed {} of unused downloads", bytes(freed)));
            }
            if leftovers || both {
                let removed = oneclient_core::storage::clean_cluster_leftovers(state).await?;
                out.line(format!("Removed {removed} leftover files"));
            }
            super::clusters_changed(state);
            Ok(())
        }
        StorageCommand::Move { dir, .. } => {
            if host == Host::Gui {
                bail!("move the data folder from OneClient's launcher settings, or close it first");
            }
            let plan = match dir {
                Some(dir) => oneclient_core::relocate::plan(state, &dir).await,
                None => oneclient_core::relocate::plan_default(state).await,
            }
            .map_err(|reason| anyhow!(reason))?;

            out.line(format!(
                "Moving {} from {} to {}",
                bytes(plan.bytes),
                plan.from.display(),
                accent(plan.to.display())
            ));
            if let Some(warning) = &plan.warning {
                out.line(warn(warning));
            }

            let events = state.services.events.clone();
            let progress = uuid::Uuid::new_v4();
            let outcome = oneclient_core::relocate::relocate(state, &plan, |copied, total| {
                events.progress(progress, "Moving the data folder", copied, total);
            })
            .await
            .map_err(|reason| anyhow!(reason))?;

            out.line(format!(
                "{} Moved {} to {}",
                good("✓"),
                bytes(outcome.bytes),
                outcome.to.display()
            ));
            if outcome.skipped_links > 0 {
                out.line(dim(format!(
                    "{} links could not be carried over",
                    outcome.skipped_links
                )));
            }
            out.line(dim(
                "The old folder stays until you run `oneclient_cli storage discard-old`",
            ));
            Ok(())
        }
        StorageCommand::DiscardOld { .. } => {
            let freed = oneclient_core::relocate::discard_leftovers(state)
                .await
                .map_err(|reason| anyhow!(reason))?;
            out.line(format!("Freed {}", bytes(freed)));
            Ok(())
        }
        StorageCommand::Open => {
            let dir = paths::data_dir()?;
            oneclient_protocol::open::path(dir)?;
            out.line(format!("Opened {}", dir.display()));
            Ok(())
        }
    }
}

pub async fn recover(command: RecoveryCommand, out: Output) {
    let code = match recovery(command, &out) {
        Ok(()) => 0,
        Err(err) => {
            out.error(format!("{} {err:#}", crate::output::bad("error:")));
            1
        }
    };
    out.exit(code);
}

fn recovery(command: RecoveryCommand, out: &Output) -> anyhow::Result<()> {
    let database = paths::database_file()?;
    let snapshots = oneclient_db::backup::list(&database);

    match command {
        RecoveryCommand::Snapshots => {
            if snapshots.is_empty() {
                out.line("No snapshots yet, one is taken before each database upgrade");
                return Ok(());
            }
            for snapshot in &snapshots {
                let size = std::fs::metadata(snapshot).map(|m| m.len()).unwrap_or(0);
                out.line(format!(
                    "{} {}",
                    bold(snapshot.file_name().unwrap_or_default().to_string_lossy()),
                    dim(bytes(size))
                ));
            }
            Ok(())
        }
        RecoveryCommand::Restore { snapshot, .. } => {
            let chosen = match snapshot {
                Some(name) => snapshots
                    .into_iter()
                    .find(|path| path.file_name().is_some_and(|file| file == name.as_str()))
                    .with_context(|| {
                        format!("no snapshot named {name}, see `recovery snapshots`")
                    })?,
                None => snapshots
                    .into_iter()
                    .next()
                    .context("there are no snapshots to restore")?,
            };
            let replaced = oneclient_db::backup::restore(&database, &chosen)?;
            out.line(format!(
                "{} Restored {}",
                good("✓"),
                chosen.file_name().unwrap_or_default().to_string_lossy()
            ));
            if let Some(replaced) = replaced {
                out.line(dim(format!(
                    "The previous database is kept at {}",
                    replaced.display()
                )));
            }
            Ok(())
        }
        RecoveryCommand::Reset { .. } => {
            let replaced = oneclient_db::backup::reset(&database)?;
            out.line(format!(
                "{} The database starts over the next time OneClient opens",
                good("✓")
            ));
            if let Some(replaced) = replaced {
                out.line(dim(format!(
                    "The previous database is kept at {}",
                    replaced.display()
                )));
            }
            Ok(())
        }
    }
}
