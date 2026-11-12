use std::sync::Arc;

use anyhow::{Context, bail};
use clap::Subcommand;
use oneclient_core::{BisectAnswer, BisectRole, BisectStatus, Cluster, LauncherState};
use serde::{Deserialize, Serialize};

use super::find_cluster;
use crate::Output;
use crate::output::{accent, bold, dim, table, warn};

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum BisectCommand {
    /// Show the search and what to do next
    Status { cluster: String },
    /// Turn half the mods off at a time to find the one causing a problem
    Start { cluster: String },
    /// The problem happened on the last launch
    Broken { cluster: String },
    /// Everything worked on the last launch
    Works { cluster: String },
    /// The last launch crashed because a mod that was off is missing, keep it on and launch again
    KeepOn {
        cluster: String,
        /// Mods to keep on, the one the crash names when left out
        mods: Vec<String>,
        /// Every mod already ruled out
        #[arg(long, conflicts_with = "mods")]
        cleared: bool,
    },
    /// Ask about the last launch next time instead
    Skip { cluster: String },
    /// Go back one launch, in case the last answer was wrong
    Undo { cluster: String },
    /// Put the mods back and start over with the mods the cluster has now
    Restart { cluster: String },
    /// Stop searching and turn every mod back on
    Stop {
        cluster: String,
        /// Leave the mods that were found turned off
        #[arg(long)]
        disable_found: bool,
    },
}

pub(super) async fn run(
    state: &Arc<LauncherState>,
    command: BisectCommand,
    out: &Output,
) -> anyhow::Result<()> {
    match command {
        BisectCommand::Status { cluster } => {
            let cluster = find_cluster(state, &cluster).await?;
            match oneclient_core::bisect_status(state, cluster.id).await? {
                Some(status) => print(state, &cluster, &status, out),
                None => out.line(format!(
                    "No problem mod search is running for {}, start one with {}",
                    cluster.name,
                    accent(format!("oneclient_cli bisect start {}", cluster.id))
                )),
            }
            Ok(())
        }
        BisectCommand::Start { cluster } => {
            let cluster = find_cluster(state, &cluster).await?;
            let events = &state.services.events;
            let progress_id = uuid::Uuid::new_v4();
            events.progress(progress_id, "Starting mod diagnostics", 0, 0);
            let status = oneclient_core::start_bisect(state, cluster.id, |label, done, total| {
                events.progress(progress_id, label, done, total)
            })
            .await;
            events.progress(progress_id, "Starting mod diagnostics", 1, 1);
            print(state, &cluster, &status?, out);
            Ok(())
        }
        BisectCommand::Broken { cluster } => answer(state, &cluster, true, out).await,
        BisectCommand::Works { cluster } => answer(state, &cluster, false, out).await,
        BisectCommand::KeepOn {
            cluster,
            mods,
            cleared,
        } => {
            let (cluster, status) = running(state, &cluster).await?;
            let status = if cleared || !mods.is_empty() {
                let hashes = if cleared {
                    status
                        .mods
                        .iter()
                        .filter(|m| m.role == BisectRole::Cleared)
                        .map(|m| m.hash.clone())
                        .collect()
                } else {
                    mods.iter()
                        .map(|query| off_mod(&status, query))
                        .collect::<anyhow::Result<Vec<_>>>()?
                };
                if hashes.is_empty() {
                    bail!("no mod has been cleared yet, name the mods to keep on instead");
                }
                oneclient_core::keep_bisect_mods_on(state, cluster.id, &hashes).await?
            } else if status.suggestion.is_some() {
                oneclient_core::accept_bisect_suggestion(state, cluster.id).await?
            } else {
                bail!(
                    "the crash did not name a missing mod, list the mods to keep on or pass --cleared"
                );
            };
            print(state, &cluster, &status, out);
            Ok(())
        }
        BisectCommand::Skip { cluster } => {
            let (cluster, _) = running(state, &cluster).await?;
            oneclient_core::skip_bisect_answer(state, cluster.id).await?;
            out.line("The next launch counts instead");
            Ok(())
        }
        BisectCommand::Undo { cluster } => {
            let (cluster, _) = running(state, &cluster).await?;
            let status = oneclient_core::undo_bisect_answer(state, cluster.id).await?;
            print(state, &cluster, &status, out);
            Ok(())
        }
        BisectCommand::Restart { cluster } => {
            let (cluster, _) = running(state, &cluster).await?;
            let status = oneclient_core::restart_bisect(state, cluster.id).await?;
            print(state, &cluster, &status, out);
            Ok(())
        }
        BisectCommand::Stop {
            cluster,
            disable_found,
        } => {
            let (cluster, status) = running(state, &cluster).await?;
            let disable: Vec<String> = if disable_found {
                if !status.confirmed {
                    bail!("nothing has been confirmed as the problem yet");
                }
                status.culprits().iter().map(|m| m.hash.clone()).collect()
            } else {
                Vec::new()
            };
            oneclient_core::finish_bisect(state, cluster.id, &disable).await?;
            out.line(format!("Turned {}'s mods back on", cluster.name));
            for found in status.culprits().iter().filter(|_| disable_found) {
                out.line(format!("Left {} off", bold(&found.name)));
            }
            Ok(())
        }
    }
}

async fn running(
    state: &Arc<LauncherState>,
    query: &str,
) -> anyhow::Result<(Cluster, BisectStatus)> {
    let cluster = find_cluster(state, query).await?;
    let status = oneclient_core::bisect_status(state, cluster.id)
        .await?
        .with_context(|| format!("no problem mod search is running for {}", cluster.name))?;
    Ok((cluster, status))
}

async fn answer(
    state: &Arc<LauncherState>,
    query: &str,
    broken: bool,
    out: &Output,
) -> anyhow::Result<()> {
    let (cluster, status) = running(state, query).await?;
    if !status.awaiting_answer() {
        bail!(
            "launch {} first, then answer once the game has closed",
            cluster.name
        );
    }

    // Answering means the crash was the problem being looked for, not a mod
    // left off this round
    if status.suggestion.is_some() {
        oneclient_core::decline_bisect_suggestion(state, cluster.id).await?;
    }
    let status = if status.awaiting_confirmation() {
        oneclient_core::confirm_bisect(state, cluster.id, broken).await?
    } else {
        let answer = if broken {
            BisectAnswer::Broken
        } else {
            BisectAnswer::Works
        };
        oneclient_core::answer_bisect(state, cluster.id, answer).await?
    };
    print(state, &cluster, &status, out);
    Ok(())
}

fn off_mod(status: &BisectStatus, query: &str) -> anyhow::Result<String> {
    let lower = query.to_lowercase();
    let off: Vec<_> = status.off_this_round().collect();
    off.iter()
        .find(|m| m.name.to_lowercase() == lower || m.hash.starts_with(query))
        .or_else(|| {
            match off
                .iter()
                .filter(|m| m.name.to_lowercase().contains(&lower))
                .collect::<Vec<_>>()[..]
            {
                [only] => Some(only),
                _ => None,
            }
        })
        .map(|m| m.hash.clone())
        .with_context(|| {
            format!(
                "no single mod that is off this round matches \"{query}\", these are: {}",
                off.iter()
                    .map(|m| m.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
}

fn print(state: &LauncherState, cluster: &Cluster, status: &BisectStatus, out: &Output) {
    let game_active = state.games.is_active(cluster.id);
    let (title, body) = status.summary(game_active);
    out.line(bold(title));
    out.line(body);
    if let Some(note) = status.retry_note.as_ref().filter(|_| !game_active) {
        out.line(warn(note));
    }
    if status.mods_changed() && !status.finishing {
        out.line(warn(status.changed_note()));
    }

    let rows: Vec<Vec<String>> = status
        .mods
        .iter()
        .map(|m| {
            let role = match m.role {
                BisectRole::Testing | BisectRole::Found => bold(m.role.label()),
                _ => dim(m.role.label()),
            };
            vec![m.name.clone(), role]
        })
        .collect();
    out.line(table(&["MOD", "ROLE"], &rows));

    let cli = |args: &str| accent(format!("oneclient_cli bisect {args} {}", cluster.id));
    if game_active {
        return;
    }
    let next = if status.finishing {
        format!("Run {} to try putting them back again", cli("stop"))
    } else if let Some(suggestion) = status
        .suggestion
        .as_ref()
        .filter(|_| status.awaiting_answer())
    {
        format!(
            "{} was off and the crash looks like it is missing. Run {} to keep it on and launch again, or {} if this is the problem",
            suggestion.missing.join(", "),
            cli("keep-on"),
            cli("broken")
        )
    } else if status.awaiting_answer() {
        format!(
            "Did the problem happen? Run {} or {}. If it crashed because a mod is missing, {} or {}. {} asks next time",
            cli("broken"),
            cli("works"),
            cli("keep-on <mods>"),
            cli("keep-on --cleared"),
            cli("skip")
        )
    } else if status.confirmed {
        format!(
            "Run {} to turn the found mods off and the rest back on",
            cli("stop --disable-found")
        )
    } else {
        format!(
            "Launch it with {}, then answer once the game has closed",
            accent(format!("oneclient_cli launch {}", cluster.id))
        )
    };
    out.line(dim(next));
}
