use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, bail};
use clap::Subcommand;
use oneclient_core::{Cluster, LauncherState};
use serde::{Deserialize, Serialize};

use super::{clusters_changed, find_cluster};
use crate::Output;
use crate::output::{bold, bytes, dim, table, when};

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum WorldCommand {
    /// List a cluster's worlds
    #[command(visible_alias = "ls")]
    List {
        cluster: String,
        /// Also add up how much space each world takes
        #[arg(long)]
        size: bool,
    },
    /// Move a world to the trash
    #[command(visible_alias = "rm")]
    Delete {
        cluster: String,
        world: String,
        /// Do not ask for confirmation
        #[arg(short, long)]
        yes: bool,
    },
    /// Open a world's folder in the file manager
    Open { cluster: String, world: String },
    /// List a world's data packs
    Datapacks { cluster: String, world: String },
    /// Copy data pack files into a world
    AddDatapack {
        cluster: String,
        world: String,
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// Move one of a world's data packs to the trash
    RemoveDatapack {
        cluster: String,
        world: String,
        name: String,
    },
}

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum ScreenshotCommand {
    /// List a cluster's screenshots
    #[command(visible_alias = "ls")]
    List { cluster: String },
    /// Open a screenshot, or the screenshots folder
    Open {
        cluster: String,
        name: Option<String>,
    },
    /// Move screenshots to the trash
    #[command(visible_alias = "rm")]
    Delete {
        cluster: String,
        #[arg(required = true)]
        names: Vec<String>,
    },
}

fn world(cluster: &Cluster, name: &str) -> anyhow::Result<oneclient_core::WorldInfo> {
    let worlds = oneclient_core::list_cluster_worlds(cluster)?;
    let lower = name.to_lowercase();
    worlds
        .iter()
        .find(|world| world.folder_name == name)
        .or_else(|| {
            worlds
                .iter()
                .find(|world| world.folder_name.to_lowercase() == lower)
        })
        .cloned()
        .with_context(|| format!("{} has no world \"{name}\", see `world list`", cluster.name))
}

pub(super) async fn run(
    state: &Arc<LauncherState>,
    command: WorldCommand,
    out: &Output,
) -> anyhow::Result<()> {
    match command {
        WorldCommand::List { cluster, size } => {
            let cluster = find_cluster(state, &cluster).await?;
            let mut worlds = oneclient_core::list_cluster_worlds(&cluster)?;
            if worlds.is_empty() {
                out.line(format!("{} has no worlds yet", cluster.name));
                return Ok(());
            }
            worlds.sort_by_key(|world| std::cmp::Reverse(world.last_played));
            let mut rows = Vec::new();
            for world in &worlds {
                let mut row = vec![bold(&world.folder_name), when(world.last_played)];
                if size {
                    let cluster = cluster.clone();
                    let name = world.folder_name.clone();
                    let total = tokio::task::spawn_blocking(move || {
                        oneclient_core::world_size(&cluster, &name)
                    })
                    .await??;
                    row.push(bytes(total));
                }
                rows.push(row);
            }
            let headers: &[&str] = if size {
                &["WORLD", "LAST PLAYED", "SIZE"]
            } else {
                &["WORLD", "LAST PLAYED"]
            };
            out.line(table(headers, &rows));
            Ok(())
        }
        WorldCommand::Delete {
            cluster,
            world: name,
            ..
        } => {
            let cluster = find_cluster(state, &cluster).await?;
            if state.games.is_active(cluster.id) {
                bail!(
                    "close {} first, its worlds cannot change while it runs",
                    cluster.name
                );
            }
            let world = world(&cluster, &name)?;
            let folder = world.folder_name.clone();
            let target = cluster.clone();
            tokio::task::spawn_blocking(move || oneclient_core::delete_world(&target, &folder))
                .await??;
            clusters_changed(state);
            out.line(format!("Moved {} to the trash", bold(&world.folder_name)));
            Ok(())
        }
        WorldCommand::Open {
            cluster,
            world: name,
        } => {
            let cluster = find_cluster(state, &cluster).await?;
            let world = world(&cluster, &name)?;
            oneclient_protocol::open::path(&world.path)?;
            out.line(format!("Opened {}", world.path.display()));
            Ok(())
        }
        WorldCommand::Datapacks {
            cluster,
            world: name,
        } => {
            let cluster = find_cluster(state, &cluster).await?;
            let world = world(&cluster, &name)?;
            let packs = oneclient_core::list_world_datapacks(&cluster, &world.folder_name).await?;
            if packs.is_empty() {
                out.line(format!("{} has no data packs", world.folder_name));
                return Ok(());
            }
            let rows: Vec<Vec<String>> = packs
                .iter()
                .map(|pack| {
                    vec![
                        bold(&pack.file_name),
                        bytes(pack.size_bytes),
                        dim(pack.description.clone().unwrap_or_default()),
                    ]
                })
                .collect();
            out.line(table(&["DATA PACK", "SIZE", "DESCRIPTION"], &rows));
            Ok(())
        }
        WorldCommand::AddDatapack {
            cluster,
            world: name,
            files,
        } => {
            let cluster = find_cluster(state, &cluster).await?;
            let world = world(&cluster, &name)?;
            oneclient_core::add_world_datapacks(&cluster, &world.folder_name, &files).await?;
            clusters_changed(state);
            out.line(format!(
                "Added {} data pack{} to {}",
                files.len(),
                if files.len() == 1 { "" } else { "s" },
                bold(&world.folder_name)
            ));
            Ok(())
        }
        WorldCommand::RemoveDatapack {
            cluster,
            world: name,
            name: pack,
        } => {
            let cluster = find_cluster(state, &cluster).await?;
            let world = world(&cluster, &name)?;
            oneclient_core::delete_world_datapack(&cluster, &world.folder_name, &pack)?;
            clusters_changed(state);
            out.line(format!("Moved {} to the trash", bold(&pack)));
            Ok(())
        }
    }
}

pub(super) async fn screenshots(
    state: &Arc<LauncherState>,
    command: ScreenshotCommand,
    out: &Output,
) -> anyhow::Result<()> {
    let find = |cluster: &Cluster, name: &str| {
        oneclient_core::list_cluster_screenshots(cluster)?
            .into_iter()
            .find(|shot| shot.name == name || shot.path.ends_with(name))
            .with_context(|| format!("{} has no screenshot {name}", cluster.name))
    };

    match command {
        ScreenshotCommand::List { cluster } => {
            let cluster = find_cluster(state, &cluster).await?;
            let mut shots = oneclient_core::list_cluster_screenshots(&cluster)?;
            if shots.is_empty() {
                out.line(format!("{} has no screenshots", cluster.name));
                return Ok(());
            }
            shots.sort_by_key(|shot| std::cmp::Reverse(shot.created));
            let rows: Vec<Vec<String>> = shots
                .iter()
                .map(|shot| {
                    vec![
                        bold(&shot.name),
                        when(shot.created),
                        shot.resolution
                            .map(|(w, h)| format!("{w}x{h}"))
                            .unwrap_or_default(),
                        bytes(shot.size_bytes),
                    ]
                })
                .collect();
            out.line(table(&["NAME", "TAKEN", "RESOLUTION", "SIZE"], &rows));
            Ok(())
        }
        ScreenshotCommand::Open { cluster, name } => {
            let cluster = find_cluster(state, &cluster).await?;
            let path = match name {
                Some(name) => find(&cluster, &name)?.path,
                None => cluster.game_dir()?.join("screenshots"),
            };
            oneclient_protocol::open::path(&path)?;
            out.line(format!("Opened {}", path.display()));
            Ok(())
        }
        ScreenshotCommand::Delete { cluster, names } => {
            let cluster = find_cluster(state, &cluster).await?;
            for name in names {
                let shot = find(&cluster, &name)?;
                oneclient_core::delete_screenshot(&shot.path)?;
                out.line(format!("Moved {} to the trash", bold(&shot.name)));
            }
            clusters_changed(state);
            Ok(())
        }
    }
}
