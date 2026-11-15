use std::io::SeekFrom;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use clap::{Args, Subcommand};
use oneclient_cluster::logs::{
    LogFileInfo, LogKind, LogLevel, ReadOptions, censor, cluster_output_log,
};
use oneclient_core::{Cluster, LauncherState};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use super::find_cluster;
use crate::output::{bad, bold, bytes, dim, table, warn, when};
use crate::{Level, Output};

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum LogsCommand {
    /// List a cluster's log files and crash reports
    #[command(visible_alias = "ls")]
    List { cluster: String },
    /// Show a log file in a pager
    Show(ShowLogArgs),
    /// Print the game output as it is written
    Follow { cluster: String },
    /// Delete a log file
    #[command(visible_alias = "rm")]
    Delete {
        cluster: String,
        file: String,
        /// Do not ask for confirmation
        #[arg(short, long)]
        yes: bool,
    },
    /// Upload a log to mclo.gs and print the link
    Upload {
        cluster: String,
        /// Log file name, the latest game output when left out
        file: Option<String>,
        /// When the log is too long, keep only its start instead of its start and end
        #[arg(long)]
        keep_start: bool,
    },
}

#[derive(Debug, Clone, Args, Serialize, Deserialize)]
pub struct ShowLogArgs {
    pub cluster: String,
    /// Log file name, the latest game output when left out
    pub file: Option<String>,
    /// Only lines at this level or above
    #[arg(long, value_enum)]
    pub level: Option<Level>,
    /// Only lines containing this text
    #[arg(short, long)]
    pub grep: Option<String>,
    /// Only the last N lines
    #[arg(short = 'n', long)]
    pub tail: Option<usize>,
    /// Print straight to the terminal
    #[arg(long)]
    pub no_pager: bool,
}

const FOLLOW_BACKLOG: usize = 20;

pub(super) async fn run(
    state: &Arc<LauncherState>,
    command: LogsCommand,
    out: &Output,
) -> anyhow::Result<()> {
    match command {
        LogsCommand::List { cluster } => {
            let cluster = find_cluster(state, &cluster).await?;
            let logs = oneclient_core::list_cluster_logs(&cluster)?;
            if logs.is_empty() {
                out.line(format!("{} has no logs yet", cluster.name));
                return Ok(());
            }
            let rows: Vec<Vec<String>> = logs
                .iter()
                .map(|log| {
                    vec![
                        bold(&log.name),
                        kind_label(log),
                        bytes(log.size_bytes),
                        when(log.modified),
                    ]
                })
                .collect();
            out.line(table(&["NAME", "KIND", "SIZE", "MODIFIED"], &rows));
            Ok(())
        }
        LogsCommand::Show(args) => show(state, args, out).await,
        LogsCommand::Follow { cluster } => {
            let cluster = find_cluster(state, &cluster).await?;
            follow_from(cluster, out.clone(), Some(FOLLOW_BACKLOG)).await
        }
        LogsCommand::Delete { cluster, file, .. } => {
            let cluster = find_cluster(state, &cluster).await?;
            let path = pick(&cluster, Some(&file))?;
            oneclient_core::delete_log_at(&path).await?;
            out.line(format!("Deleted {}", bold(&file)));
            Ok(())
        }
        LogsCommand::Upload {
            cluster,
            file,
            keep_start,
        } => {
            let cluster = find_cluster(state, &cluster).await?;
            let path = pick(&cluster, file.as_deref())?;
            let trim = if keep_start {
                oneclient_cluster::logs::LogTrim::KeepStart
            } else {
                oneclient_cluster::logs::LogTrim::KeepStartAndEnd
            };
            let uploaded =
                oneclient_core::upload_log_at(&state.services.requester, &path, trim).await?;
            if uploaded.truncated {
                out.line(dim(if keep_start {
                    "The log was too long, only its start was uploaded"
                } else {
                    "The log was too long, its middle was cut out"
                }));
            }
            out.line(uploaded.url);
            Ok(())
        }
    }
}

fn kind_label(log: &LogFileInfo) -> String {
    match log.kind {
        LogKind::Game { .. } => "game output".to_string(),
        LogKind::Minecraft => "minecraft".to_string(),
        LogKind::CrashReport => bad("crash report"),
        LogKind::Other => dim("other"),
    }
}

fn pick(cluster: &Cluster, name: Option<&str>) -> anyhow::Result<PathBuf> {
    let logs = oneclient_core::list_cluster_logs(cluster)?;
    match name {
        Some(name) => logs
            .into_iter()
            .find(|log| log.name == name || log.path.ends_with(name))
            .map(|log| log.path)
            .with_context(|| format!("{} has no log named {name}", cluster.name)),
        None => {
            let output = cluster_output_log(cluster)?;
            if output.is_file() {
                return Ok(output);
            }
            logs.into_iter()
                .next()
                .map(|log| log.path)
                .with_context(|| format!("{} has no logs yet", cluster.name))
        }
    }
}

async fn show(state: &Arc<LauncherState>, args: ShowLogArgs, out: &Output) -> anyhow::Result<()> {
    let cluster = find_cluster(state, &args.cluster).await?;
    let path = pick(&cluster, args.file.as_deref())?;
    let options = ReadOptions {
        level_filter: args.level.map(Into::into),
        search: args.grep,
        max_lines: args.tail,
    };

    let text = oneclient_core::read_log_at(&path, &options)
        .await?
        .into_iter()
        .map(|line| match line.level {
            LogLevel::Error | LogLevel::Fatal => bad(&line.text),
            LogLevel::Warn => warn(&line.text),
            LogLevel::Debug | LogLevel::Trace => dim(&line.text),
            _ => line.text,
        })
        .collect::<Vec<_>>()
        .join("\n");

    if args.no_pager {
        out.line(text);
    } else {
        out.page(text);
    }
    Ok(())
}

pub(super) async fn follow(cluster: Cluster, out: Output) -> anyhow::Result<()> {
    follow_from(cluster, out, None).await
}

async fn follow_from(cluster: Cluster, out: Output, backlog: Option<usize>) -> anyhow::Result<()> {
    let path = cluster_output_log(&cluster)?;
    let mut position = 0u64;

    if let Some(lines) = backlog
        && let Ok(content) = tokio::fs::read_to_string(&path).await
    {
        let tail: Vec<&str> = content.lines().rev().take(lines).collect();
        for line in tail.into_iter().rev() {
            out.line(censor(line));
        }
        position = content.len() as u64;
    }

    let mut partial = String::new();
    while !out.is_closed() && !crate::ipc::winding_down() {
        if let Ok(mut file) = tokio::fs::File::open(&path).await {
            let len = file.metadata().await.map(|meta| meta.len()).unwrap_or(0);
            if len < position {
                position = 0;
                partial.clear();
            }
            if len > position {
                file.seek(SeekFrom::Start(position)).await?;
                let mut chunk = Vec::new();
                file.read_to_end(&mut chunk).await?;
                position += chunk.len() as u64;
                partial.push_str(&String::from_utf8_lossy(&chunk));

                while let Some(end) = partial.find('\n') {
                    let line: String = partial.drain(..=end).collect();
                    out.line(censor(line.trim_end_matches(['\r', '\n'])));
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Ok(())
}
