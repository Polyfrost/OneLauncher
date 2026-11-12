#![recursion_limit = "256"]

pub mod ipc;
mod markdown;
mod output;
mod run;
pub mod update;

use std::path::{Path, PathBuf};

use clap::{CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use oneclient_cluster::logs::LogLevel;
use oneclient_common::domain::{ContentType, GameLoader, ProviderId};
use oneclient_content::packages::SearchSort;
use serde::{Deserialize, Serialize};

pub use output::{Output, Reply};
pub use run::about::TermsCommand;
pub use run::account::AccountCommand;
pub use run::bisect::BisectCommand;
pub use run::bundle::BundleCommand;
pub use run::cluster::{ClusterCommand, CreateArgs, EditArgs, Folder, ListFilter};
pub use run::game::{LaunchArgs, StopArgs};
pub use run::logs::{LogsCommand, ShowLogArgs};
pub use run::modpack::ModpackCommand;
pub use run::package::{PackageCommand, SearchArgs};
pub use run::settings::{GameKey, GameSettingsCommand, SettingsCommand};
pub use run::setup::{ImportCommand, SetupArgs, is_set_up, prepare_setup};
pub use run::share::{CloudCommand, ShareCommand};
pub use run::storage::{RecoveryCommand, StorageCommand, recover};
pub use run::system::{JavaCommand, JavaVendorArg, VersionsArgs};
pub use run::world::{ScreenshotCommand, WorldCommand};
pub use run::{Host, run};

#[derive(Debug, Default, Clone, Parser)]
#[command(
    name = "oneclient_cli",
    version,
    about = "Control OneClient from the command line",
    after_help = "Commands run inside OneClient when it is open, and on their own otherwise."
)]
pub struct Cli {
    /// Launch a cluster by its folder name
    #[arg(long, value_name = "FOLDER")]
    pub launch: Option<String>,

    /// Import a shared cluster by its share code
    #[arg(long, value_name = "CODE")]
    pub share: Option<String>,

    /// A oneclient://launch/<folder> or oneclient://share/<code> link
    #[arg(value_name = "URL", hide = true)]
    pub url: Option<String>,

    /// Show launcher logs on stderr, repeat for more detail
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Option<Command>,
}

impl Cli {
    pub fn parse_lenient(args: impl IntoIterator<Item = String>) -> Self {
        let args = std::iter::once("oneclient".to_string()).chain(args.into_iter().filter(|arg| {
            !arg.starts_with('-') || matches!(arg.split('=').next(), Some("--launch" | "--share"))
        }));
        Self::command()
            .ignore_errors(true)
            .try_get_matches_from(args)
            .ok()
            .and_then(|matches| Self::from_arg_matches(&matches).ok())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn launch_folder(&self) -> Option<String> {
        self.launch.as_deref().and_then(non_empty).or_else(|| {
            self.url
                .as_deref()
                .and_then(oneclient_protocol::parse_launch_url)
        })
    }

    #[must_use]
    pub fn share_code(&self) -> Option<String> {
        self.share
            .as_deref()
            .and_then(oneclient_core::sharing::ShareCode::parse)
            .map(|code| code.as_str().to_string())
            .or_else(|| self.url.as_deref().and_then(parse_share_url))
    }

    #[must_use]
    pub fn window_request(&self) -> ipc::IpcCommand {
        match (self.launch_folder(), self.share_code()) {
            (Some(folder), _) => ipc::IpcCommand::Launch(folder),
            (None, Some(code)) => ipc::IpcCommand::ImportShare(code),
            (None, None) => ipc::IpcCommand::Focus,
        }
    }
}

fn parse_share_url(raw: &str) -> Option<String> {
    let (scheme, _) = raw.trim().split_once("://")?;
    if !scheme.eq_ignore_ascii_case(oneclient_protocol::SCHEME) {
        return None;
    }
    oneclient_core::sharing::ShareCode::parse(raw).map(|code| code.as_str().to_string())
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[must_use]
pub fn parse() -> Cli {
    Cli::parse_lenient(std::env::args().skip(1))
}

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum Command {
    /// Set OneClient up for the first time, or finish setting it up
    Setup(SetupArgs),
    /// Bring over game folders from another launcher
    #[command(subcommand)]
    Import(ImportCommand),
    /// Launch a cluster
    Launch(LaunchArgs),
    /// Stop a running game
    Stop(StopArgs),
    /// List running games
    Ps,
    /// Create, inspect and manage clusters
    #[command(subcommand, visible_alias = "c")]
    Cluster(ClusterCommand),
    /// Install, update and repair modpacks
    #[command(subcommand, visible_alias = "mp")]
    Modpack(ModpackCommand),
    /// Share clusters by code and sync them to the cloud
    #[command(subcommand)]
    Share(ShareCommand),
    /// Manage the bundles OneClient clusters are built from
    #[command(subcommand)]
    Bundle(BundleCommand),
    /// Manage a cluster's worlds and their data packs
    #[command(subcommand)]
    World(WorldCommand),
    /// Manage a cluster's screenshots
    #[command(subcommand, visible_alias = "shot")]
    Screenshot(ScreenshotCommand),
    /// Read and share game logs
    #[command(subcommand, visible_alias = "log")]
    Logs(LogsCommand),
    /// Find, inspect and manage mods, resource packs and shaders
    #[command(subcommand, visible_alias = "pkg")]
    Package(PackageCommand),
    /// Find the mod causing a problem by turning half of them off each launch
    #[command(subcommand, visible_alias = "thanos")]
    Bisect(BisectCommand),
    /// Manage Minecraft accounts
    #[command(subcommand)]
    Account(AccountCommand),
    /// Manage Java runtimes
    #[command(subcommand)]
    Java(JavaCommand),
    /// List Minecraft versions, or the loaders for one version
    Versions(VersionsArgs),
    /// Show playtime and session stats, for one cluster or all of them
    Stats { cluster: Option<String> },
    /// Change launcher settings
    #[command(subcommand)]
    Settings(SettingsCommand),
    /// Change the game settings every cluster starts from
    #[command(subcommand, visible_alias = "gs")]
    GameSettings(GameSettingsCommand),
    /// Show disk usage, free space and move the data folder
    #[command(subcommand)]
    Storage(StorageCommand),
    /// Restore or reset a damaged database
    #[command(subcommand)]
    Recovery(RecoveryCommand),
    /// Read, accept or decline the terms of service
    Terms {
        #[command(subcommand)]
        action: Option<TermsCommand>,
    },
    /// Show what changed in the latest OneClient release
    Changelog {
        /// Every release instead of only the latest
        #[arg(long)]
        all: bool,
    },
    /// Print version and system details for bug reports
    Info,
    /// Check for a OneClient update
    Update,
    /// Bring the OneClient window to the front
    Focus,
    /// Quit OneClient
    Quit,
}

fn absolute(cwd: &Path, path: &mut PathBuf) {
    if path.is_relative() {
        *path = cwd.join(&*path);
    }
}

fn absolute_all(cwd: &Path, paths: &mut [PathBuf]) {
    for path in paths {
        absolute(cwd, path);
    }
}

fn absolute_source(cwd: &Path, source: &mut String) {
    let path = Path::new(source.as_str());
    if path.is_relative() && cwd.join(path).is_file() {
        *source = cwd.join(path).display().to_string();
    }
}

impl Command {
    pub fn resolve_paths(&mut self, cwd: &Path) {
        match self {
            Self::Setup(args) => args.data_dir.iter_mut().for_each(|p| absolute(cwd, p)),
            Self::Import(ImportCommand::Detect { from }) => {
                from.iter_mut().for_each(|p| absolute(cwd, p));
            }
            Self::Import(ImportCommand::Instance { from, scan, .. }) => {
                from.iter_mut().for_each(|p| absolute(cwd, p));
                absolute_all(cwd, scan);
            }
            Self::Import(ImportCommand::File { path, scan, .. }) => {
                absolute(cwd, path);
                absolute_all(cwd, scan);
            }
            Self::Cluster(ClusterCommand::Create(args)) => {
                args.cover.iter_mut().for_each(|p| absolute(cwd, p));
            }
            Self::Cluster(ClusterCommand::Edit(args)) => {
                args.cover.iter_mut().for_each(|p| absolute(cwd, p));
            }
            Self::Cluster(ClusterCommand::Shortcut { dir, .. }) => {
                dir.iter_mut().for_each(|p| absolute(cwd, p));
            }
            Self::Cluster(ClusterCommand::Export { output, .. }) => {
                absolute(cwd, output.get_or_insert_with(|| cwd.to_path_buf()));
            }
            Self::Modpack(
                ModpackCommand::Install { source, scan, .. }
                | ModpackCommand::Add { source, scan, .. },
            ) => {
                absolute_source(cwd, source);
                absolute_all(cwd, scan);
            }
            Self::Modpack(
                ModpackCommand::Update { scan, .. } | ModpackCommand::Repair { scan, .. },
            ) => absolute_all(cwd, scan),
            Self::Share(ShareCommand::Import { scan, .. }) => absolute_all(cwd, scan),
            Self::Package(PackageCommand::Import { files, .. })
            | Self::World(WorldCommand::AddDatapack { files, .. }) => absolute_all(cwd, files),
            Self::Java(JavaCommand::Add { folder }) => absolute(cwd, folder),
            Self::Storage(StorageCommand::Move { dir, .. }) => {
                dir.iter_mut().for_each(|p| absolute(cwd, p));
            }
            _ => {}
        }
    }

    #[must_use]
    pub fn window_request(&self) -> Option<ipc::IpcCommand> {
        match self {
            Self::Focus => Some(ipc::IpcCommand::Focus),
            Self::Quit => Some(ipc::IpcCommand::Quit),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, Deserialize)]
pub enum Loader {
    Vanilla,
    Forge,
    Neoforge,
    Quilt,
    Fabric,
    Ornithe,
}

impl From<Loader> for GameLoader {
    fn from(loader: Loader) -> Self {
        match loader {
            Loader::Vanilla => Self::Vanilla,
            Loader::Forge => Self::Forge,
            Loader::Neoforge => Self::NeoForge,
            Loader::Quilt => Self::Quilt,
            Loader::Fabric => Self::Fabric,
            Loader::Ornithe => Self::Ornithe,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, Deserialize)]
pub enum Provider {
    Modrinth,
    Curseforge,
}

impl From<Provider> for ProviderId {
    fn from(provider: Provider) -> Self {
        match provider {
            Provider::Modrinth => Self::Modrinth,
            Provider::Curseforge => Self::CurseForge,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, Deserialize)]
pub enum Kind {
    Mod,
    Resourcepack,
    Shader,
    Datapack,
    Modpack,
}

impl From<Kind> for ContentType {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Mod => Self::Mod,
            Kind::Resourcepack => Self::ResourcePack,
            Kind::Shader => Self::Shader,
            Kind::Datapack => Self::DataPack,
            Kind::Modpack => Self::Modpack,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, Deserialize)]
pub enum Sort {
    Relevance,
    Downloads,
    Newest,
    Updated,
}

impl From<Sort> for SearchSort {
    fn from(sort: Sort) -> Self {
        match sort {
            Sort::Relevance => Self::Relevance,
            Sort::Downloads => Self::Downloads,
            Sort::Newest => Self::Newest,
            Sort::Updated => Self::Updated,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, Deserialize)]
pub enum Side {
    Client,
    Server,
}

impl From<Side> for oneclient_content::packages::SearchSide {
    fn from(side: Side) -> Self {
        match side {
            Side::Client => Self::Client,
            Side::Server => Self::Server,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, Deserialize)]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

impl From<Level> for LogLevel {
    fn from(level: Level) -> Self {
        match level {
            Level::Debug => Self::Debug,
            Level::Info => Self::Info,
            Level::Warn => Self::Warn,
            Level::Error => Self::Error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        Cli::parse_lenient(args.iter().map(|s| (*s).to_string()))
    }

    #[test]
    fn no_arguments_is_an_ordinary_start() {
        assert_eq!(parse(&[]).launch_folder(), None);
    }

    #[test]
    fn both_spellings_carry_the_folder() {
        assert_eq!(
            parse(&["--launch", "fabric-1-20"])
                .launch_folder()
                .as_deref(),
            Some("fabric-1-20")
        );
        assert_eq!(
            parse(&["--launch=fabric-1-20"]).launch_folder().as_deref(),
            Some("fabric-1-20")
        );
    }

    #[test]
    fn a_blank_folder_is_no_request_at_all() {
        assert_eq!(parse(&["--launch", "   "]).launch_folder(), None);
        assert_eq!(parse(&["--launch="]).launch_folder(), None);
        assert_eq!(parse(&["--launch"]).launch_folder(), None);
    }

    #[test]
    fn spaces_survive_the_round_trip() {
        assert_eq!(
            parse(&["--launch", "My Pack (1.8.9)"])
                .launch_folder()
                .as_deref(),
            Some("My Pack (1.8.9)"),
        );
    }

    #[test]
    fn unknown_flags_are_ignored_not_fatal() {
        assert_eq!(
            parse(&["--bogus", "--launch", "x"])
                .launch_folder()
                .as_deref(),
            Some("x")
        );
    }

    #[test]
    fn a_launch_url_is_accepted_as_a_bare_argument() {
        let url = oneclient_protocol::launch_url("26.1.2 Fabric");
        assert_eq!(
            parse(&[&url]).launch_folder().as_deref(),
            Some("26.1.2 Fabric")
        );
    }

    #[test]
    fn a_share_link_or_flag_carries_the_code() {
        let url = format!("{}://share/abcd-efgh", oneclient_protocol::SCHEME);
        assert_eq!(parse(&[&url]).share_code().as_deref(), Some("ABCDEFGH"));
        assert_eq!(
            parse(&["--share", "ABCD-EFGH"]).share_code().as_deref(),
            Some("ABCDEFGH")
        );
        assert_eq!(
            parse(&["--share=abcdefgh"]).share_code().as_deref(),
            Some("ABCDEFGH")
        );
        assert_eq!(parse(&["--share", "nope"]).share_code(), None);
        assert_eq!(parse(&[&url]).launch_folder(), None);
        assert_eq!(
            parse(&[&url]).window_request(),
            ipc::IpcCommand::ImportShare("ABCDEFGH".into())
        );
    }

    #[test]
    fn a_url_that_is_not_ours_is_not_a_launch() {
        assert_eq!(parse(&["https://polyfrost.org"]).launch_folder(), None);
    }

    #[test]
    fn a_bare_start_asks_only_for_the_window() {
        assert_eq!(parse(&[]).window_request(), ipc::IpcCommand::Focus);
        assert_eq!(
            parse(&["--launch", "pack"]).window_request(),
            ipc::IpcCommand::Launch("pack".into()),
        );
    }
}
