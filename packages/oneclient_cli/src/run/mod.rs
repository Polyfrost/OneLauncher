pub(crate) mod about;
pub(crate) mod account;
pub(crate) mod bisect;
pub(crate) mod bundle;
pub(crate) mod cluster;
pub(crate) mod game;
pub(crate) mod logs;
pub(crate) mod modpack;
pub(crate) mod package;
pub(crate) mod settings;
pub(crate) mod setup;
pub(crate) mod share;
mod stats;
pub(crate) mod storage;
pub(crate) mod system;
pub(crate) mod world;

use std::sync::Arc;

use anyhow::{Context, bail};
use oneclient_core::{Cluster, LauncherState};

use crate::{Command, Output};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    Gui,
    Headless,
}

pub async fn run(host: Host, state: Arc<LauncherState>, command: Command, out: Output) {
    let code = match dispatch(host, &state, command, &out).await {
        Ok(()) => {
            if host == Host::Gui {
                crate::ipc::send(crate::ipc::IpcCommand::Refresh);
            }
            0
        }
        Err(err) => {
            out.error(format!("{} {err:#}", crate::output::bad("error:")));
            1
        }
    };
    out.exit(code);
}

async fn dispatch(
    host: Host,
    state: &Arc<LauncherState>,
    command: Command,
    out: &Output,
) -> anyhow::Result<()> {
    match command {
        Command::Setup(args) => setup::run(host, state, args, out).await,
        Command::Import(command) => setup::import_command(state, command, out).await,
        Command::Launch(args) => game::launch(host, state, args, out).await,
        Command::Stop(args) => game::stop(state, args, out).await,
        Command::Ps => game::ps(state, out).await,
        Command::Cluster(command) => cluster::run(host, state, command, out).await,
        Command::Stats { cluster } => stats::run(state, cluster, out).await,
        Command::Bundle(command) => bundle::run(state, command, out).await,
        Command::Logs(command) => logs::run(state, command, out).await,
        Command::World(command) => world::run(state, command, out).await,
        Command::Screenshot(command) => world::screenshots(state, command, out).await,
        Command::Modpack(command) => modpack::run(state, command, out).await,
        Command::Share(command) => share::run(state, command, out).await,
        Command::Package(command) => package::run(state, command, out).await,
        Command::Bisect(command) => bisect::run(state, command, out).await,
        Command::Account(command) => account::run(state, command, out).await,
        Command::Java(command) => system::java(state, command, out).await,
        Command::Versions(args) => system::versions(state, args, out).await,
        Command::Settings(command) => settings::run(state, command, out).await,
        Command::GameSettings(command) => settings::global(state, command, out).await,
        Command::Storage(command) => storage::run(host, state, command, out).await,
        Command::Recovery(_) => {
            bail!("close OneClient and other oneclient_cli commands first, the database is open")
        }
        Command::Terms { action } => {
            about::terms(state, action.unwrap_or(crate::TermsCommand::Show), out).await
        }
        Command::Changelog { all } => about::changelog(state, all, out).await,
        Command::Info => about::info(out),
        Command::Update => about::update(host, out).await,
        Command::Focus | Command::Quit => bail!("OneClient is not open"),
    }
}

pub(crate) async fn find_cluster(state: &LauncherState, query: &str) -> anyhow::Result<Cluster> {
    let clusters = state.clusters.list().await?;
    let query = query.trim();
    let lower = query.to_lowercase();

    if let Ok(id) = query.parse::<i64>()
        && let Some(cluster) = clusters.iter().find(|c| c.id == id)
    {
        return Ok(cluster.clone());
    }

    if let Some(cluster) = clusters.iter().find(|c| c.folder_name == query) {
        return Ok(cluster.clone());
    }

    let exact: Vec<&Cluster> = clusters
        .iter()
        .filter(|c| c.name.to_lowercase() == lower)
        .collect();
    let matches: Vec<&Cluster> = if exact.is_empty() {
        clusters
            .iter()
            .filter(|c| c.name.to_lowercase().contains(&lower))
            .collect()
    } else {
        exact
    };
    match matches.as_slice() {
        [only] => Ok((*only).clone()),
        [] => bail!("no cluster matches \"{query}\", see `oneclient_cli cluster list`"),
        many => bail!(
            "\"{query}\" matches {} clusters, pick one by its id or folder name: {}",
            many.len(),
            many.iter()
                .map(|c| format!("{} (id {}, folder \"{}\")", c.name, c.id, c.folder_name))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

pub(crate) async fn find_account(
    state: &LauncherState,
    query: &str,
) -> anyhow::Result<oneclient_auth::MinecraftAccount> {
    let accounts = state.auth.list_accounts().await;
    accounts
        .into_iter()
        .find(|a| a.username.eq_ignore_ascii_case(query) || a.id.to_string() == query)
        .with_context(|| format!("no account named \"{query}\", see `oneclient_cli account list`"))
}

pub(crate) fn end_progress(state: &LauncherState, session_id: uuid::Uuid) {
    state
        .services
        .events
        .emit(oneclient_events::Event::Progress(
            oneclient_events::ProgressEvent::Grouped(oneclient_events::GroupedProgressEvent::End {
                session_id,
            }),
        ));
}

pub(crate) fn clusters_changed(state: &LauncherState) {
    state
        .services
        .events
        .signal(oneclient_events::Signal::ClustersChanged);
}
