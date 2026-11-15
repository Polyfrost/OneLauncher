use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, bail};
use clap::{Args, Subcommand, ValueEnum};
use oneclient_common::Patch;
use oneclient_common::domain::GameLoader;
use oneclient_content::modpacks::ShareExportReport;
use oneclient_content::packages::PackageStore;
use oneclient_core::busy::{self, Busy};
use oneclient_core::clusters::{
    ClusterKind, ClusterStage, CreateClusterOptions, ExportMode, ExportOptions, OfferLookup,
    SettingsChoice,
};
use oneclient_core::{Cluster, ClusterUpdate, LauncherState};
use oneclient_protocol::shortcut::{self, ShortcutRequest};
use serde::{Deserialize, Serialize};

use super::settings::GameSettingsCommand;
use super::{Host, clusters_changed, find_cluster};
use crate::output::{accent, bold, bytes, dim, duration, good, heading, table, warn, when};
use crate::{Loader, Output};

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum ClusterCommand {
    /// List clusters
    #[command(visible_alias = "ls")]
    List {
        #[arg(short, long, value_enum, default_value_t = ListFilter::All)]
        filter: ListFilter,
        /// Only clusters with this loader
        #[arg(short, long, value_enum)]
        loader: Option<Loader>,
        #[arg(long)]
        json: bool,
    },
    /// Show a cluster's details
    Info {
        cluster: String,
        #[arg(long)]
        json: bool,
    },
    /// Create a cluster
    Create(CreateArgs),
    /// Change a cluster's name, description, tags or cover
    Edit(EditArgs),
    /// Rename a cluster
    Rename { cluster: String, name: String },
    /// Delete a cluster
    #[command(visible_alias = "rm")]
    Delete {
        cluster: String,
        /// Keep the game folder on disk
        #[arg(long)]
        keep_files: bool,
        /// Do not ask for confirmation
        #[arg(short, long)]
        yes: bool,
    },
    /// Check every game file and download what is missing or damaged
    Repair { cluster: String },
    /// Put a cluster back to how it was when created, moving its changes aside
    Reset {
        cluster: String,
        /// Do not ask for confirmation
        #[arg(short, long)]
        yes: bool,
    },
    /// Open one of the cluster's folders in the file manager
    Open {
        cluster: String,
        #[arg(short, long, value_enum, default_value_t = Folder::Game)]
        folder: Folder,
    },
    /// Print one of the cluster's folders
    Path {
        cluster: String,
        #[arg(short, long, value_enum, default_value_t = Folder::Game)]
        folder: Folder,
    },
    /// Show or change a cluster's game settings
    Settings {
        cluster: String,
        #[command(subcommand)]
        action: Option<GameSettingsCommand>,
    },
    /// Give a cluster its own game folder, or put it back in the shared one
    Dedicated {
        cluster: String,
        #[arg(value_parser = super::settings::parse_bool, action = clap::ArgAction::Set)]
        enabled: bool,
    },
    /// Put a cluster in a shared game folder, the default one or a named group
    Share {
        cluster: String,
        /// Shared folder group to join, the default shared folder when left out
        #[arg(long)]
        group: Option<String>,
        /// Create the group if it does not exist yet
        #[arg(long, requires = "group")]
        create: bool,
    },
    /// List the named shared folder groups
    Groups,
    /// Delete a named shared folder group no cluster uses, with its worlds and settings
    DeleteGroup {
        group: String,
        /// Do not ask for confirmation
        #[arg(short, long)]
        yes: bool,
    },
    /// Show or set the renderer Minecraft 26.2 and later uses: default, opengl or vulkan
    Graphics {
        cluster: String,
        #[arg(value_parser = super::settings::parse_graphics_backend)]
        backend: Option<oneclient_common::domain::GraphicsBackend>,
    },
    /// Switch a cluster to another loader version
    LoaderVersion { cluster: String, version: String },
    /// Create a desktop shortcut that launches a cluster
    Shortcut {
        cluster: String,
        /// Folder to put it in, the desktop when left out
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// Check a cluster for loader, bundle and package updates now
    CheckUpdates { cluster: String },
    /// Save a cluster as a .mrpack that OneClient and other launchers can open
    Export {
        cluster: String,
        /// Where to write it, <name>.mrpack in the current folder when left out
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[command(flatten)]
        options: ExportArgs,
    },
    /// List the settings files an export or share can include
    SettingFiles { cluster: String },
    /// Copy settings, and with --worlds also worlds, from a shared folder into a cluster
    Sync {
        cluster: String,
        /// Shared folder group to copy from, the default shared folder when left out
        #[arg(long)]
        from: Option<String>,
        /// Also copy worlds the cluster does not have yet
        #[arg(long)]
        worlds: bool,
    },
    /// Copy the packages of another cluster into this one
    Migrate {
        cluster: String,
        /// Cluster to copy from
        #[arg(long)]
        from: String,
        /// Also copy the config folder
        #[arg(long)]
        configs: bool,
        /// Only show what would be copied
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
pub struct ExportArgs {
    /// Put every file in so launchers without OneClient's bundles can use it
    #[arg(long)]
    pub compatible: bool,
    /// Include the cluster's settings files, like options.txt and mod configs
    #[arg(long)]
    pub settings: bool,
    /// Include only this settings file, repeat for several, see `cluster setting-files`
    #[arg(long = "setting", value_name = "PATH")]
    pub setting_files: Vec<String>,
}

impl ExportArgs {
    pub(super) fn is_empty(&self) -> bool {
        !self.compatible && !self.settings && self.setting_files.is_empty()
    }

    pub(super) async fn options(
        &self,
        state: &Arc<LauncherState>,
        cluster: &Cluster,
    ) -> anyhow::Result<ExportOptions> {
        let mode = if self.compatible {
            ExportMode::Compatibility
        } else {
            ExportMode::OneClient
        };
        Ok(ExportOptions::new(mode).with_setting_choice(self.choice(state, cluster).await?))
    }

    pub(super) async fn choice(
        &self,
        state: &Arc<LauncherState>,
        cluster: &Cluster,
    ) -> anyhow::Result<SettingsChoice> {
        if self.setting_files.is_empty() {
            return Ok(if self.settings {
                SettingsChoice::All
            } else {
                SettingsChoice::None
            });
        }
        let known: Vec<String> = oneclient_core::clusters::list_setting_files(state, cluster.id)
            .await?
            .into_iter()
            .map(|file| file.path)
            .collect();
        if let Some(unknown) = self.setting_files.iter().find(|path| !known.contains(path)) {
            bail!(
                "{} has no settings file {unknown}, see `oneclient_cli cluster setting-files {}`",
                cluster.name,
                cluster.id
            );
        }
        Ok(SettingsChoice::Only(
            self.setting_files.iter().cloned().collect(),
        ))
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, Deserialize)]
pub enum ListFilter {
    All,
    Custom,
    Modpacks,
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, Deserialize)]
pub enum Kind {
    /// OneClient's curated setup with bundles
    Oneclient,
    Vanilla,
    Modded,
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, Deserialize)]
pub enum Folder {
    Game,
    Cluster,
    Mods,
    Resourcepacks,
    Shaderpacks,
    Saves,
    Screenshots,
    Logs,
    CrashReports,
    Config,
}

#[derive(Debug, Clone, Args, Serialize, Deserialize)]
pub struct CreateArgs {
    pub name: String,
    /// Minecraft version, e.g. 1.21.5
    #[arg(short = 'V', long = "version")]
    pub mc_version: String,
    #[arg(short, long, value_enum, default_value_t = Loader::Vanilla)]
    pub loader: Loader,
    /// Loader version, the latest when left out
    #[arg(long)]
    pub loader_version: Option<String>,
    /// Vanilla or modded by default, depending on the loader
    #[arg(short, long, value_enum)]
    pub kind: Option<Kind>,
    /// Bundles to take, for OneClient clusters, see `bundle available`
    #[arg(short, long = "bundle")]
    pub bundles: Vec<String>,
    #[arg(short, long)]
    pub description: Option<String>,
    #[arg(short, long = "tag")]
    pub tags: Vec<String>,
    /// Image to use as the cover
    #[arg(long)]
    pub cover: Option<PathBuf>,
    /// Give a OneClient cluster its own game folder instead of the shared one
    #[arg(long)]
    pub dedicated: bool,
    /// Download the game files now instead of on first launch
    #[arg(long)]
    pub install: bool,
}

#[derive(Debug, Clone, Args, Serialize, Deserialize)]
pub struct EditArgs {
    pub cluster: String,
    #[arg(short, long)]
    pub name: Option<String>,
    #[arg(short, long, conflicts_with = "clear_description")]
    pub description: Option<String>,
    #[arg(long)]
    pub clear_description: bool,
    /// Replaces the tags, repeat for several
    #[arg(short, long = "tag", conflicts_with = "clear_tags")]
    pub tags: Vec<String>,
    #[arg(long)]
    pub clear_tags: bool,
    /// Image to use as the cover
    #[arg(long, conflicts_with = "clear_cover")]
    pub cover: Option<PathBuf>,
    #[arg(long)]
    pub clear_cover: bool,
}

pub(super) async fn run(
    host: Host,
    state: &Arc<LauncherState>,
    command: ClusterCommand,
    out: &Output,
) -> anyhow::Result<()> {
    match command {
        ClusterCommand::List {
            filter,
            loader,
            json,
        } => list(state, filter, loader, json, out).await,
        ClusterCommand::Info { cluster, json } => {
            info(state, &find_cluster(state, &cluster).await?, json, out).await
        }
        ClusterCommand::Create(args) => create(state, args, out).await,
        ClusterCommand::Edit(args) => edit(state, args, out).await,
        ClusterCommand::Rename { cluster, name } => {
            edit(
                state,
                EditArgs {
                    cluster,
                    name: Some(name),
                    description: None,
                    clear_description: false,
                    tags: Vec::new(),
                    clear_tags: false,
                    cover: None,
                    clear_cover: false,
                },
                out,
            )
            .await
        }
        ClusterCommand::Delete {
            cluster,
            keep_files,
            ..
        } => {
            let cluster = idle_cluster(state, &cluster).await?;
            oneclient_core::clusters::release_imported_modpacks(state, cluster.id).await;
            state.clusters.delete(cluster.id, !keep_files).await?;
            oneclient_core::sync::forget_deleted_cluster(state, cluster.id).await;
            clusters_changed(state);
            out.line(format!("Deleted {}", bold(&cluster.name)));
            Ok(())
        }
        ClusterCommand::Repair { cluster } => {
            let cluster = idle_cluster(state, &cluster).await?;
            out.line(format!("Checking the files of {}", bold(&cluster.name)));
            let report = oneclient_core::verify_cluster_files(state, cluster.id).await?;
            clusters_changed(state);
            out.line(report.summary());
            Ok(())
        }
        ClusterCommand::Reset { cluster, .. } => {
            let cluster = find_cluster(state, &cluster).await?;
            let claim = oneclient_core::clusters::ResetClaim::take(cluster.id)
                .with_context(|| format!("{} is already being reset", cluster.name))?;
            let busy = if busy::modpack_job_running(cluster.id) {
                Some(Busy::ModpackJob)
            } else if busy::exporting(cluster.id) {
                Some(Busy::Exporting)
            } else if busy::is_launching(cluster.id) || state.games.is_active(cluster.id) {
                Some(Busy::Running)
            } else if state.games.is_syncing(cluster.id) {
                Some(Busy::Syncing)
            } else {
                None
            };
            if let Some(busy) = busy {
                bail!("{}: {busy}", cluster.name);
            }
            let session = oneclient_events::GroupedProgressSession::start(
                &state.services.events,
                format!("Resetting {}", cluster.name),
            );
            let outcome =
                oneclient_core::clusters::reset_cluster(state, claim, Some(&session)).await;
            session.finish();
            clusters_changed(state);
            let report = outcome?;
            if let Some(err) = &report.reinstall_error {
                bail!(
                    "its content could not be installed again: {err}, run reset again to finish{}",
                    report
                        .backup_dir
                        .as_ref()
                        .map(|dir| format!(", replaced files are in {}", dir.display()))
                        .unwrap_or_default()
                );
            }
            out.line(format!(
                "{} Reset {}{}",
                good("✓"),
                bold(&cluster.name),
                if report.kept_shared_configs {
                    ", config files in the shared game folder were kept"
                } else {
                    ""
                }
            ));
            if let Some(backup) = &report.backup_dir {
                out.line(dim(format!(
                    "Replaced files were moved to {}",
                    backup.display()
                )));
            }
            if !report.not_moved.is_empty() {
                out.error(warn(format!(
                    "Left in place: {}",
                    report.not_moved.join(", ")
                )));
            }
            Ok(())
        }
        ClusterCommand::Open { cluster, folder } => {
            let dir = folder_path(&find_cluster(state, &cluster).await?, folder)?;
            if !dir.exists() {
                bail!("{} does not exist yet", dir.display());
            }
            oneclient_protocol::open::path(&dir)?;
            out.line(format!("Opened {}", dir.display()));
            Ok(())
        }
        ClusterCommand::Path { cluster, folder } => {
            let dir = folder_path(&find_cluster(state, &cluster).await?, folder)?;
            out.line(dir.display().to_string());
            Ok(())
        }
        ClusterCommand::Settings { cluster, action } => {
            let cluster = find_cluster(state, &cluster).await?;
            super::settings::cluster(
                state,
                &cluster,
                action.unwrap_or(GameSettingsCommand::Show),
                out,
            )
            .await
        }
        ClusterCommand::Dedicated { cluster, enabled } => {
            let cluster = idle_cluster(state, &cluster).await?;
            if oneclient_core::clusters::has_imported_modpacks(state, cluster.id).await {
                bail!(
                    "remove the modpacks added to {} before changing its folder, see `modpack added`",
                    cluster.name
                );
            }
            let target = if enabled {
                oneclient_core::clusters::DirTarget::Dedicated
            } else {
                oneclient_core::clusters::DirTarget::Shared(None)
            };
            oneclient_core::game::switch_game_folder(state, cluster.id, target, false).await?;
            clusters_changed(state);
            out.line(if enabled {
                format!("{} now has its own game folder", bold(&cluster.name))
            } else {
                format!("{} now uses the shared game folder", bold(&cluster.name))
            });
            if enabled
                && let oneclient_core::clusters::DirTarget::Shared(group) = &cluster.dir_target
            {
                out.line(dim(format!(
                    "It starts empty, bring the shared settings and worlds along with `oneclient_cli cluster sync {}{} --worlds`",
                    cluster.id,
                    group
                        .as_deref()
                        .map(|group| format!(" --from {group}"))
                        .unwrap_or_default()
                )));
            }
            Ok(())
        }
        ClusterCommand::Share {
            cluster,
            group,
            create,
        } => {
            let cluster = idle_cluster(state, &cluster).await?;
            if cluster.is_isolated() {
                bail!("{} keeps its own folder and cannot share one", cluster.name);
            }
            if oneclient_core::clusters::has_imported_modpacks(state, cluster.id).await {
                bail!(
                    "remove the modpacks added to {} before changing its folder, see `modpack added`",
                    cluster.name
                );
            }
            let target = oneclient_core::clusters::DirTarget::Shared(group);
            oneclient_core::game::switch_game_folder(state, cluster.id, target, create).await?;
            clusters_changed(state);
            let cluster = state.clusters.get(cluster.id).await?;
            out.line(match cluster.shared_group() {
                Some(group) => format!(
                    "{} now uses the {} shared folder",
                    bold(&cluster.name),
                    accent(group)
                ),
                None => format!("{} now uses the shared game folder", bold(&cluster.name)),
            });
            Ok(())
        }
        ClusterCommand::Groups => {
            let groups = state.clusters.list_shared_groups().await?;
            if groups.is_empty() {
                out.line("No shared folder groups, create one with `cluster share <cluster> --group <name> --create`");
            }
            for group in groups {
                out.line(group);
            }
            Ok(())
        }
        ClusterCommand::DeleteGroup { group, .. } => {
            if state.games.any_syncing() {
                bail!("a cluster is syncing from a shared folder, wait for the sync to finish");
            }
            let found = state
                .clusters
                .find_shared_group(&group)
                .await?
                .with_context(|| format!("no shared folder group named {group}"))?;
            state.clusters.delete_shared_group(&found).await?;
            out.line(format!("Deleted the {} shared folder", bold(&found)));
            Ok(())
        }
        ClusterCommand::Graphics { cluster, backend } => {
            let cluster = find_cluster(state, &cluster).await?;
            if !oneclient_common::domain::GraphicsBackend::supported_on(&cluster.mc_version) {
                bail!(
                    "the graphics backend can only be picked on Minecraft 26.2 and later, {} is {}",
                    cluster.name,
                    cluster.mc_version
                );
            }
            let game_dir = cluster.game_dir()?;
            match backend {
                None => out.line(
                    match oneclient_core::game::read_graphics_backend(&game_dir).await {
                        Some(backend) => backend.to_string(),
                        None => "not set yet, launch the game once first".to_string(),
                    },
                ),
                Some(backend) => {
                    oneclient_core::game::write_graphics_backend(&game_dir, backend).await?;
                    out.line(format!(
                        "{} now renders with {}",
                        bold(&cluster.name),
                        accent(backend)
                    ));
                }
            }
            Ok(())
        }
        ClusterCommand::LoaderVersion { cluster, version } => {
            let cluster = idle_cluster(state, &cluster).await?;
            if !cluster.mc_loader.is_modded() {
                bail!("{} has no mod loader", cluster.name);
            }
            let available = {
                let mut metadata = state.metadata.lock().await;
                oneclient_core::get_loader_versions(
                    &mut metadata,
                    &state.services.mc(),
                    &cluster.mc_version,
                    cluster.mc_loader,
                )
                .await?
            };
            if !available.contains(&version) {
                bail!(
                    "{} {version} is not available for Minecraft {}, see `oneclient_cli versions {}`",
                    cluster.mc_loader,
                    cluster.mc_version,
                    cluster.mc_version
                );
            }
            state
                .clusters
                .update(
                    cluster.id,
                    ClusterUpdate::default().loader_version(&version),
                )
                .await?;
            state
                .clusters
                .set_stage(cluster.id, ClusterStage::NotReady)
                .await?;
            clusters_changed(state);
            out.line(format!(
                "{} now uses {} {version}, it downloads on the next launch",
                bold(&cluster.name),
                cluster.mc_loader
            ));
            Ok(())
        }
        ClusterCommand::Shortcut { cluster, dir } => {
            let cluster = find_cluster(state, &cluster).await?;
            let dir = dir
                .or_else(shortcut::default_dir)
                .context("no desktop folder found, pass --dir")?;
            let request = ShortcutRequest {
                cluster_name: cluster.name.clone(),
                folder_name: cluster.folder_name.clone(),
                dir,
                icon: cluster.modpack_icon_file().filter(|icon| icon.is_file()),
            };
            let path = match host {
                Host::Gui => tokio::task::spawn_blocking(move || shortcut::create(&request)).await,
                Host::Headless => {
                    let exe = app_exe()?;
                    tokio::task::spawn_blocking(move || shortcut::create_with(&request, &exe)).await
                }
            }??;
            out.line(format!("Created {}", accent(path.display())));
            Ok(())
        }
        ClusterCommand::Export {
            cluster,
            output,
            options,
        } => {
            let cluster = idle_cluster(state, &cluster).await?;
            let options = options.options(state, &cluster).await?;
            let _slot = claim_export(&cluster)?;
            let dest = match output {
                Some(path) if path.is_dir() => path.join(export_name(&cluster)),
                Some(path) => path.with_extension(oneclient_content::modpacks::SHARE_EXTENSION),
                None => std::env::current_dir()?.join(export_name(&cluster)),
            };
            let session = oneclient_events::GroupedProgressSession::start(
                &state.services.events,
                format!("Exporting {}", cluster.name),
            );
            let exported = oneclient_core::clusters::export_cluster_with(
                state,
                cluster.id,
                &dest,
                options,
                Some(&session),
            )
            .await;
            match exported {
                Ok(report) => {
                    session.finish();
                    out.line(format!(
                        "{} Saved {} as {}",
                        good("✓"),
                        bold(&cluster.name),
                        accent(dest.display())
                    ));
                    export_report(&report, out);
                    Ok(())
                }
                Err(err) => {
                    session.abort();
                    Err(err.into())
                }
            }
        }
        ClusterCommand::CheckUpdates { cluster } => {
            super::game::check_updates(state, &find_cluster(state, &cluster).await?, out).await
        }
        ClusterCommand::SettingFiles { cluster } => {
            let cluster = find_cluster(state, &cluster).await?;
            let files = oneclient_core::clusters::list_setting_files(state, cluster.id).await?;
            if files.is_empty() {
                out.line(format!("{} has no settings files", cluster.name));
                return Ok(());
            }
            let rows: Vec<Vec<String>> = files
                .into_iter()
                .map(|file| vec![file.path, bytes(file.size)])
                .collect();
            out.line(table(&["PATH", "SIZE"], &rows));
            Ok(())
        }
        ClusterCommand::Sync {
            cluster,
            from,
            worlds,
        } => {
            let cluster = idle_cluster(state, &cluster).await?;
            if cluster.is_isolated() {
                bail!(
                    "{} keeps its own folder and cannot sync from a shared one",
                    cluster.name
                );
            }
            let from = match from {
                Some(group) => Some(
                    state
                        .clusters
                        .find_shared_group(&group)
                        .await?
                        .with_context(|| format!("no shared folder group named {group}"))?,
                ),
                None => None,
            };
            if cluster.dir_target == oneclient_core::clusters::DirTarget::Shared(from.clone()) {
                bail!("{} already uses that shared folder", cluster.name);
            }
            if busy::modpack_job_in_shared_dir(state, cluster.id).await {
                bail!("{}", Busy::SharedModpackJob);
            }
            let shared = oneclient_common::paths::shared_group_dir(from.as_deref())?;
            let dest = cluster.game_dir()?;
            let _guard = state
                .games
                .begin_sync(cluster.id, &shared, &dest)
                .map_err(|block| anyhow::anyhow!("{block}"))?;
            let events = state.services.events.clone();
            let progress = uuid::Uuid::new_v4();
            let report = state
                .clusters
                .sync_from_shared(
                    cluster.id,
                    from.as_deref(),
                    worlds,
                    move |label, current, total| {
                        events.progress(progress, label, current, total);
                    },
                )
                .await?;
            if report.total == 0 {
                out.line("Nothing to sync in the shared folder");
                return Ok(());
            }
            out.line(format!(
                "{} Copied the shared {} into {}",
                good("✓"),
                if worlds {
                    "settings and worlds"
                } else {
                    "settings"
                },
                bold(&cluster.name)
            ));
            if !report.skipped.is_empty() {
                out.line(dim(format!(
                    "Kept the cluster's own worlds: {}",
                    report.skipped.join(", ")
                )));
            }
            Ok(())
        }
        ClusterCommand::Migrate {
            cluster,
            from,
            configs,
            dry_run,
        } => migrate(state, &cluster, &from, configs, dry_run, out).await,
    }
}

pub(super) fn export_report(report: &ShareExportReport, out: &Output) {
    if !report.embedded.is_empty() {
        out.line(dim(format!(
            "{} files not on Modrinth or CurseForge went inside the file",
            report.embedded.len()
        )));
    }
    if report.settings > 0 {
        out.line(dim(format!(
            "Included {} settings file{}",
            report.settings,
            if report.settings == 1 { "" } else { "s" }
        )));
    }
    if !report.skipped_settings.is_empty() {
        out.error(warn(format!(
            "Left out settings files that are too big: {}",
            report.skipped_settings.join(", ")
        )));
    }
    if !report.missing.is_empty() {
        out.error(warn(format!(
            "Left out files that could not be found: {}",
            report.missing.join(", ")
        )));
    }
}

pub(super) fn claim_export(cluster: &Cluster) -> anyhow::Result<busy::ExportSlot> {
    busy::claim_export(cluster.id).map_err(|busy| anyhow::anyhow!("{}: {busy}", cluster.name))
}

pub(super) async fn idle_cluster(state: &LauncherState, query: &str) -> anyhow::Result<Cluster> {
    let cluster = find_cluster(state, query).await?;
    let busy = if busy::modpack_job_running(cluster.id) {
        Some(Busy::ModpackJob)
    } else if state.games.is_syncing(cluster.id) {
        Some(Busy::Syncing)
    } else if busy::update_check_running(cluster.id) {
        Some(Busy::CheckingUpdates)
    } else {
        busy::modpack_change_block(state, cluster.id)
    };
    if let Some(busy) = busy {
        bail!("{}: {busy}", cluster.name);
    }
    Ok(cluster)
}

fn app_exe() -> anyhow::Result<PathBuf> {
    let name = if cfg!(windows) {
        "oneclient_app.exe"
    } else {
        "oneclient_app"
    };
    let exe = std::env::current_exe()?
        .parent()
        .map(|dir| dir.join(name))
        .filter(|exe| exe.is_file())
        .with_context(|| format!("could not find {name} next to oneclient_cli"))?;
    Ok(dunce::canonicalize(&exe).unwrap_or(exe))
}

fn export_name(cluster: &Cluster) -> String {
    let stem = oneclient_cluster::ClusterManager::sanitize_name(&cluster.name);
    let stem = if stem.is_empty() {
        "instance".to_string()
    } else {
        stem
    };
    format!("{stem}.{}", oneclient_content::modpacks::SHARE_EXTENSION)
}

fn folder_path(cluster: &Cluster, folder: Folder) -> anyhow::Result<PathBuf> {
    let game = cluster.game_dir()?;
    Ok(match folder {
        Folder::Game => game,
        Folder::Cluster => cluster.dir()?,
        Folder::Mods => game.join("mods"),
        Folder::Resourcepacks => game.join("resourcepacks"),
        Folder::Shaderpacks => game.join("shaderpacks"),
        Folder::Saves => game.join("saves"),
        Folder::Screenshots => game.join("screenshots"),
        Folder::Logs => game.join("logs"),
        Folder::CrashReports => game.join("crash-reports"),
        Folder::Config => game.join("config"),
    })
}

fn loader_label(cluster: &Cluster) -> String {
    match (&cluster.mc_loader, &cluster.mc_loader_version) {
        (GameLoader::Vanilla, _) => "Vanilla".to_string(),
        (loader, Some(version)) => format!("{loader} {version}"),
        (loader, None) => loader.to_string(),
    }
}

async fn list(
    state: &Arc<LauncherState>,
    filter: ListFilter,
    loader: Option<Loader>,
    json: bool,
    out: &Output,
) -> anyhow::Result<()> {
    let mut clusters: Vec<Cluster> = state
        .clusters
        .list()
        .await?
        .into_iter()
        .filter(|cluster| match filter {
            ListFilter::All => true,
            ListFilter::Custom => cluster.user_created && cluster.linked_modpack_hash.is_none(),
            ListFilter::Modpacks => cluster.linked_modpack_hash.is_some(),
        })
        .filter(|cluster| loader.is_none_or(|loader| cluster.mc_loader == loader.into()))
        .collect();
    clusters.sort_by(oneclient_cluster::compare_recent_activity);

    if json {
        out.line(serde_json::to_string_pretty(&clusters)?);
        return Ok(());
    }
    if clusters.is_empty() {
        out.line("No clusters match, make one with `oneclient_cli cluster create`");
        return Ok(());
    }

    let rows: Vec<Vec<String>> = clusters
        .iter()
        .map(|cluster| {
            let running = state.games.is_active(cluster.id);
            vec![
                cluster.id.to_string(),
                if running {
                    format!("{} {}", bold(&cluster.name), good("●"))
                } else {
                    bold(&cluster.name)
                },
                cluster.mc_version.clone(),
                loader_label(cluster),
                cluster
                    .last_played
                    .map(when)
                    .unwrap_or_else(|| dim("never")),
                duration(cluster.overall_played),
            ]
        })
        .collect();

    out.line(table(
        &["ID", "NAME", "VERSION", "LOADER", "LAST PLAYED", "PLAYTIME"],
        &rows,
    ));
    Ok(())
}

async fn info(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
    json: bool,
    out: &Output,
) -> anyhow::Result<()> {
    if json {
        out.line(serde_json::to_string_pretty(cluster)?);
        return Ok(());
    }

    let packages = PackageStore::list_linked_artifacts(cluster.id, &state.services.content())
        .await
        .map(|packages| packages.len())
        .unwrap_or(0);
    let status = match state.games.stage(cluster.id) {
        Some(stage) => good(format!("{stage:?}")),
        None => format!("{:?}", cluster.stage),
    };
    let kind = match cluster.kind {
        ClusterKind::OneClient => "OneClient",
        ClusterKind::Vanilla => "Vanilla",
        ClusterKind::Modded => "Modded",
    };

    let field = |name: &str, value: String| format!("  {} {value}", dim(format!("{name:<12}")));
    let mut lines = vec![
        format!(
            "{} {}",
            heading(&cluster.name),
            dim(format!("#{}", cluster.id))
        ),
        field("Minecraft", cluster.mc_version.clone()),
        field("Loader", loader_label(cluster)),
        field("Kind", kind.to_string()),
        field("Status", status),
        field("Packages", packages.to_string()),
        field(
            "Last played",
            cluster
                .last_played
                .map(when)
                .unwrap_or_else(|| "never".into()),
        ),
        field("Playtime", duration(cluster.overall_played)),
        field("Folder", accent(cluster.game_dir()?.display())),
        field(
            "Dedicated",
            if cluster.uses_dedicated_dir() {
                "yes"
            } else {
                "no, shares the game folder"
            }
            .to_string(),
        ),
    ];
    if let Some(created) = cluster.created_at {
        lines.push(field("Created", when(created)));
    }
    if !cluster.tags.is_empty() {
        lines.push(field("Tags", cluster.tags.join(", ")));
    }
    if let Some(cover) = &cluster.cover_path {
        lines.push(field("Cover", dim(cover)));
    }
    if let Some(description) = cluster.description.as_deref().filter(|d| !d.is_empty()) {
        lines.push(String::new());
        lines.push(description.to_string());
    }

    out.line(lines.join("\n"));
    Ok(())
}

async fn create(state: &Arc<LauncherState>, args: CreateArgs, out: &Output) -> anyhow::Result<()> {
    let loader = GameLoader::from(args.loader);
    {
        let mut metadata = state.metadata.lock().await;
        let ctx = state.services.mc();
        let known = oneclient_core::get_version_ids(&mut metadata, &ctx).await?;
        if !known.iter().any(|version| version.id == args.mc_version) {
            bail!(
                "Minecraft {} does not exist, see `oneclient_cli versions`",
                args.mc_version
            );
        }
        if loader.is_modded() {
            let loaders =
                oneclient_core::get_loaders_for_version(&mut metadata, &ctx, &args.mc_version)
                    .await?;
            if !loaders.contains(&loader) {
                bail!(
                    "{loader} is not available for Minecraft {}",
                    args.mc_version
                );
            }
        }
    }

    let kind = match args.kind {
        Some(Kind::Oneclient) => ClusterKind::OneClient,
        Some(Kind::Vanilla) => ClusterKind::Vanilla,
        Some(Kind::Modded) => ClusterKind::Modded,
        None if loader.is_modded() => ClusterKind::Modded,
        None => ClusterKind::Vanilla,
    };
    if !args.bundles.is_empty() && kind != ClusterKind::OneClient {
        bail!("bundles only apply to OneClient clusters, add --kind oneclient");
    }
    let mut selected = args.bundles.clone();
    if kind == ClusterKind::OneClient {
        let content = state.services.content();
        if let Err(err) = state.bundles.sync(&content).await {
            tracing::warn!("bundle catalog sync failed, using the cached one: {err:#}");
        }
        let archives = state
            .bundles
            .archives_for(&content, &args.mc_version, loader)
            .await?;
        if let Some(unknown) = args.bundles.iter().find(|name| {
            !archives.iter().any(|archive| {
                archive.manifest.name.eq_ignore_ascii_case(name)
                    || archive.manifest.category.eq_ignore_ascii_case(name)
            })
        }) {
            bail!(
                "no bundle named \"{unknown}\" for {loader} {}, see `oneclient_cli bundle available`",
                args.mc_version
            );
        }
        selected = archives
            .iter()
            .filter(|archive| {
                archive.manifest.enabled
                    || args.bundles.iter().any(|name| {
                        archive.manifest.name.eq_ignore_ascii_case(name)
                            || archive.manifest.category.eq_ignore_ascii_case(name)
                    })
            })
            .map(|archive| archive.manifest.name.clone())
            .collect();
    }

    let global = state.settings.read().global_game_settings.clone();
    let mut options = CreateClusterOptions::new(args.name, args.mc_version, loader)
        .kind(kind)
        .user_created(true)
        .tags(args.tags);
    options.mc_loader_version = args.loader_version;
    options.description = args.description;

    let cluster = state.clusters.create(&global, options).await?;
    if args.dedicated
        && kind == ClusterKind::OneClient
        && let Err(err) = state
            .clusters
            .set_dedicated_dir(cluster.id, true, false)
            .await
    {
        out.error(warn(format!(
            "Could not give it its own folder, it uses the shared one for now: {err}"
        )));
    }
    clusters_changed(state);
    if let Some(cover) = &args.cover {
        state
            .clusters
            .set_cover_from_file(cluster.id, cover)
            .await?;
    }
    out.line(format!(
        "Created {} {}",
        bold(&cluster.name),
        dim(format!("#{}", cluster.id))
    ));

    if cluster.uses_bundles() {
        let session = oneclient_events::GroupedProgressSession::start(
            &state.services.events,
            format!("Setting up {}", cluster.name),
        );
        oneclient_core::clusters::set_up_bundles(state, &cluster, Some(&selected), &session).await;
        session.finish();
    }

    if args.install {
        let progress = oneclient_events::GroupedProgressSession::start(
            &state.services.events,
            format!("Installing {}", cluster.name),
        );
        let prepared = oneclient_core::clusters::prepare_cluster_locked(
            state,
            cluster.id,
            false,
            true,
            true,
            Some(&progress),
        )
        .await;
        progress.finish();
        prepared?;
        clusters_changed(state);
        out.line(format!("{} {} is ready to play", good("✓"), cluster.name));
    }
    Ok(())
}

async fn edit(state: &Arc<LauncherState>, args: EditArgs, out: &Output) -> anyhow::Result<()> {
    let cluster = find_cluster(state, &args.cluster).await?;

    let cover_path = if args.clear_cover {
        state.clusters.clear_cover(cluster.id).await?;
        Patch::Clear
    } else {
        if let Some(cover) = &args.cover {
            state
                .clusters
                .set_cover_from_file(cluster.id, cover)
                .await?;
        }
        Patch::Unchanged
    };
    let update = ClusterUpdate {
        name: args.name,
        description: match (args.description, args.clear_description) {
            (Some(text), _) => Patch::Set(text),
            (None, true) => Patch::Clear,
            (None, false) => Patch::Unchanged,
        },
        tags: match (args.tags.is_empty(), args.clear_tags) {
            (false, _) => Some(args.tags),
            (true, true) => Some(Vec::new()),
            (true, false) => None,
        },
        cover_path,
        ..ClusterUpdate::default()
    };

    let updated = state.clusters.update(cluster.id, update).await?;
    clusters_changed(state);
    out.line(format!("Updated {}", bold(&updated.name)));
    Ok(())
}

async fn migrate(
    state: &Arc<LauncherState>,
    target: &str,
    source: &str,
    configs: bool,
    dry_run: bool,
    out: &Output,
) -> anyhow::Result<()> {
    let target = idle_cluster(state, target).await?;
    let source = find_cluster(state, source).await?;

    let offer = match oneclient_core::clusters::manual_migration_offer(state, target.id, source.id)
        .await?
    {
        OfferLookup::Offer(offer) => offer,
        OfferLookup::NoSources | OfferLookup::MissingCluster => {
            out.line(format!(
                "{} has nothing that can move to {}",
                source.name, target.name
            ));
            return Ok(());
        }
    };
    let ids: Vec<i64> = offer.sources.iter().map(|c| c.id).collect();
    let plan = oneclient_content::packages::release_migration::plan_release_migrations(
        &ids,
        target.id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .into_iter()
    .find(|(id, _)| *id == source.id)
    .map(|(_, plan)| plan)
    .context("no migration plan came back for the source")??;
    if plan.unreachable {
        bail!("could not reach Modrinth or CurseForge, check your connection");
    }

    let mut rows: Vec<Vec<String>> = plan
        .packages
        .iter()
        .map(|p| vec![bold(&p.display_name), p.version_name.clone(), good("copy")])
        .collect();
    rows.extend(
        plan.dependencies
            .iter()
            .map(|d| vec![d.project.name.clone(), d.version_name(), dim("dependency")]),
    );
    rows.extend(plan.unavailable.iter().map(|s| {
        vec![
            dim(&s.display_name),
            String::new(),
            warn(format!("{:?}", s.reason)),
        ]
    }));
    if rows.is_empty() {
        out.line(format!("{} has no packages to copy", source.name));
        return Ok(());
    }
    out.line(table(&["PACKAGE", "VERSION", "ACTION"], &rows));
    if dry_run {
        return Ok(());
    }

    let packages = plan.packages.clone();
    let outcome = oneclient_core::clusters::migrate_release_packages(
        state,
        Some(&source),
        &target,
        Some(&plan),
        packages,
        configs,
    )
    .await;
    clusters_changed(state);

    match outcome {
        Some(outcome) => {
            super::end_progress(state, outcome.session_id);
            out.line(format!(
                "{} Copied {} of {} packages into {}",
                good("✓"),
                outcome.migrated,
                outcome.total,
                target.name
            ));
        }
        None => out.line("Nothing was copied"),
    }
    Ok(())
}
