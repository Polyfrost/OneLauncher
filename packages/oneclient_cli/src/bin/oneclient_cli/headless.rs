use std::sync::Arc;

use anyhow::bail;
use oneclient_cli::ipc::{self, Incoming, Listener};
use oneclient_cli::{Command, Host, Output, Reply};
use oneclient_common::paths;
use oneclient_core::LauncherState;

use crate::{events, term};

pub async fn run(listener: Option<Listener>, command: Command) -> i32 {
    if matches!(command, Command::Recovery(_)) {
        ipc::hold();
    }
    if let Some(listener) = listener {
        ipc::listen(listener, false);
    }

    if let Command::Recovery(command) = command {
        if let Err(err) = locate().await {
            eprintln!("{} {err:#}", console::style("error:").red());
            return 1;
        }
        let (out, replies) = Output::channel();
        tokio::spawn(oneclient_cli::recover(command, out));
        return print(replies).await;
    }

    if let Command::Setup(args) = &command
        && let Err(err) = oneclient_cli::prepare_setup(args).await
    {
        eprintln!("{} {err:#}", console::style("error:").red());
        return 1;
    }

    let state = match start().await {
        Ok(state) => state,
        Err(err) => {
            eprintln!("{} {err:#}", console::style("error:").red());
            return 1;
        }
    };

    serve(Arc::clone(&state));

    let (out, replies) = Output::channel();
    tokio::spawn(oneclient_cli::run(
        Host::Headless,
        Arc::clone(&state),
        command,
        out,
    ));

    let code = print(replies).await;
    ipc::release().await;
    ipc::settle().await;
    oneclient_core::shutdown(&state).await;
    code
}

async fn print(mut replies: tokio::sync::mpsc::UnboundedReceiver<Reply>) -> i32 {
    while let Some(reply) = replies.recv().await {
        if let Reply::Exit(code) = reply {
            return code;
        }
        term::print_reply(reply);
    }
    1
}

async fn locate() -> anyhow::Result<()> {
    if !oneclient_cli::is_set_up() {
        bail!("OneClient is not set up yet, run `oneclient_cli setup` or open OneClient");
    }

    let settings = oneclient_core::settings::store::load_settings(None).await;
    if let Some(dir) = settings.data_dir {
        if oneclient_core::relocate::incomplete(&dir) {
            bail!("a move of the game data folder never finished, open OneClient to finish it");
        }
        paths::set_data_dir(dir);
    }
    Ok(())
}

async fn start() -> anyhow::Result<Arc<LauncherState>> {
    locate().await?;

    let (bus, receiver) = oneclient_events::EventBus::channel();
    events::spawn(receiver);

    let state = LauncherState::new(bus).await.map_err(|err| {
        anyhow::anyhow!("{err:#}, if the database is damaged try `oneclient_cli recovery restore`")
    })?;
    oneclient_core::game::recover_sessions(&state).await;
    Ok(state)
}

fn serve(state: Arc<LauncherState>) {
    let Some(mut inbox) = ipc::take_inbox() else {
        return;
    };
    tokio::spawn(async move {
        while let Some(incoming) = inbox.recv().await {
            if let Incoming::Run(command, out) = incoming {
                tokio::spawn(oneclient_cli::run(
                    Host::Headless,
                    Arc::clone(&state),
                    command,
                    out,
                ));
            }
        }
    });
}
