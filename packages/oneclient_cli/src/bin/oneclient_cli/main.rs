#![recursion_limit = "256"]

mod events;
mod headless;
mod term;

use std::process::ExitCode;

use clap::{CommandFactory, Parser};
use oneclient_cli::ipc::{self, Claim, Request};
use oneclient_cli::{
    Cli, ClusterCommand, Command, LaunchArgs, LogsCommand, RecoveryCommand, ShareCommand,
    StorageCommand, WorldCommand,
};
use tracing_subscriber::EnvFilter;

fn main() -> ExitCode {
    let cli = Cli::parse();
    init_logging(cli.verbose);

    let mut command = match (cli.command.clone(), cli.launch_folder()) {
        (Some(command), _) => command,
        (None, None) if let Some(code) = cli.share_code() => Command::Share(ShareCommand::Import {
            code,
            name: None,
            scan: Vec::new(),
            force: false,
        }),
        (None, Some(folder)) => Command::Launch(LaunchArgs {
            cluster: Some(folder),
            last: false,
            account: None,
            detach: false,
            logs: false,
        }),
        (None, None) => {
            let _ = Cli::command().print_help();
            return ExitCode::SUCCESS;
        }
    };

    if let Ok(cwd) = std::env::current_dir() {
        command.resolve_paths(&cwd);
    }

    if let Some(question) = confirmation(&command)
        && !term::confirm(&question)
    {
        return ExitCode::FAILURE;
    }

    let mut runtime = tokio::runtime::Builder::new_multi_thread();
    runtime.enable_all().max_blocking_threads(64);
    #[cfg(debug_assertions)]
    runtime.thread_stack_size(3 * 1024 * 1024);
    let runtime = runtime.build().expect("failed to start the async runtime");

    let code = runtime.block_on(async move {
        match tokio::spawn(dispatch(command)).await {
            Ok(code) => code,
            Err(err) => std::panic::resume_unwind(err.into_panic()),
        }
    });
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}

fn confirmation(command: &Command) -> Option<String> {
    match command {
        Command::Cluster(ClusterCommand::Delete {
            cluster,
            keep_files,
            yes: false,
        }) => Some(if *keep_files {
            format!("Delete {cluster}? Its game folder stays on disk.")
        } else {
            format!("Delete {cluster} and its game folder?")
        }),
        Command::Cluster(ClusterCommand::Reset {
            cluster,
            yes: false,
        }) => Some(format!(
            "Reset {cluster} to how it was when created? Its changes are moved aside."
        )),
        Command::Cluster(ClusterCommand::DeleteGroup { group, yes: false }) => Some(format!(
            "Delete the {group} shared folder with its worlds and settings? This cannot be undone."
        )),
        Command::World(WorldCommand::Delete {
            cluster,
            world,
            yes: false,
        }) => Some(format!("Move the world {world} of {cluster} to the trash?")),
        Command::Logs(LogsCommand::Delete {
            cluster,
            file,
            yes: false,
        }) => Some(format!("Delete {file} of {cluster} for good?")),
        Command::Storage(StorageCommand::Move {
            dir, yes: false, ..
        }) => Some(match dir {
            Some(dir) => format!("Move the game data folder to {}?", dir.display()),
            None => "Move the game data folder back to the default place?".to_string(),
        }),
        Command::Storage(StorageCommand::DiscardOld { yes: false }) => {
            Some("Delete everything left in the old data folder?".to_string())
        }
        Command::Recovery(RecoveryCommand::Restore { yes: false, .. }) => {
            Some("Replace the database with a snapshot? The current one is kept aside.".to_string())
        }
        Command::Recovery(RecoveryCommand::Reset { yes: false }) => Some(
            "Start over with an empty database? Clusters and accounts need setting up again."
                .to_string(),
        ),
        _ => None,
    }
}

async fn dispatch(command: Command) -> i32 {
    if let Some(window) = command.window_request() {
        let forwarded = ipc::forward(&Request::Window(window), &mut term::print_reply).await;
        return forwarded.unwrap_or_else(|| {
            eprintln!("OneClient is not open");
            1
        });
    }

    match ipc::claim(&Request::Run(command.clone()), term::print_reply).await {
        Claim::Forwarded(code) => code,
        Claim::Primary(listener) => headless::run(Some(listener), command).await,
        Claim::Solo(reason) => {
            tracing::warn!("running without the single-instance endpoint: {reason}");
            headless::run(None, command).await
        }
    }
}

fn init_logging(verbose: u8) {
    let level = match verbose {
        0 => "error",
        1 => "info",
        _ => "debug",
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}
