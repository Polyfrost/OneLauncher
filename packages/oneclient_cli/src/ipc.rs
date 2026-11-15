use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::sync::{Notify, watch};
use tokio_util::task::TaskTracker;

use crate::{Command, Output, Reply};

const IO_TIMEOUT: Duration = Duration::from_secs(3);
const CLAIM_ATTEMPTS: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IpcCommand {
    Launch(String),
    ImportShare(String),
    Focus,
    Close,
    Logs,
    Stop,
    Quit,
    Refresh,
    CheckUpdate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    Window(IpcCommand),
    Start(IpcCommand),
    Run(Command),
}

#[derive(Debug)]
pub enum Incoming {
    Window(IpcCommand),
    Run(Command, Output),
}

pub enum Claim {
    Primary(Listener),
    Forwarded(i32),
    Solo(String),
}

enum Exchange {
    Done(i32),
    Released,
    Gone,
}

pub async fn claim(request: &Request, mut on_reply: impl FnMut(Reply)) -> Claim {
    let mut taken = 0;
    loop {
        match ask(request, &mut on_reply).await {
            Exchange::Done(code) => return Claim::Forwarded(code),
            Exchange::Released => continue,
            Exchange::Gone => {}
        }

        match imp::bind().await {
            Ok(listener) => return Claim::Primary(listener),
            Err(BindError::Taken) => {
                taken += 1;
                if taken == CLAIM_ATTEMPTS {
                    return Claim::Solo(format!(
                        "the endpoint changed hands {CLAIM_ATTEMPTS} times"
                    ));
                }
            }
            Err(BindError::Io(err)) => return Claim::Solo(err.to_string()),
        }
    }
}

pub async fn forward(request: &Request, on_reply: &mut impl FnMut(Reply)) -> Option<i32> {
    match ask(request, on_reply).await {
        Exchange::Done(code) => Some(code),
        Exchange::Released | Exchange::Gone => None,
    }
}

async fn ask(request: &Request, on_reply: &mut impl FnMut(Reply)) -> Exchange {
    match imp::connect().await {
        Some(stream) => exchange(stream, request, on_reply).await,
        None => Exchange::Gone,
    }
}

static WINDING_DOWN: AtomicBool = AtomicBool::new(false);
static HOLDING: AtomicBool = AtomicBool::new(false);
static RELEASE: Notify = Notify::const_new();

#[must_use]
pub fn winding_down() -> bool {
    WINDING_DOWN.load(Ordering::Acquire)
}

pub fn hold() {
    HOLDING.store(true, Ordering::Release);
}

fn listening() -> &'static watch::Sender<bool> {
    static LISTENING: OnceLock<watch::Sender<bool>> = OnceLock::new();
    LISTENING.get_or_init(|| watch::Sender::new(false))
}

pub async fn release() {
    WINDING_DOWN.store(true, Ordering::Release);
    if *listening().borrow() {
        RELEASE.notify_one();
    }
    let _ = listening().subscribe().wait_for(|on| !on).await;
}

fn handlers() -> &'static TaskTracker {
    static HANDLERS: OnceLock<TaskTracker> = OnceLock::new();
    HANDLERS.get_or_init(TaskTracker::new)
}

pub async fn settle() {
    handlers().close();
    handlers().wait().await;
}

static QUEUE: OnceLock<UnboundedSender<Incoming>> = OnceLock::new();
static INBOX: Mutex<Option<UnboundedReceiver<Incoming>>> = Mutex::new(None);

fn queue() -> &'static UnboundedSender<Incoming> {
    QUEUE.get_or_init(|| {
        let (commands, incoming) = unbounded_channel();
        *INBOX.lock().unwrap() = Some(incoming);
        commands
    })
}

pub fn send(command: IpcCommand) {
    let _ = queue().send(Incoming::Window(command));
}

/// The accept loop runs off the UI thread so a stalled client cannot hold up
/// the next shortcut launch
pub fn listen(listener: Listener, windowed: bool) {
    listening().send_replace(true);
    tokio::spawn(async move {
        imp::serve(listener, queue().clone(), windowed).await;
        listening().send_replace(false);
    });
}

#[must_use]
pub fn take_inbox() -> Option<UnboundedReceiver<Incoming>> {
    queue();
    INBOX.lock().unwrap().take()
}

enum BindError {
    Taken,
    Io(std::io::Error),
}

async fn write_reply<S: AsyncWrite + Unpin>(stream: &mut S, reply: &Reply) -> bool {
    let Ok(mut line) = serde_json::to_string(reply) else {
        return false;
    };
    line.push('\n');
    let wrote = tokio::time::timeout(IO_TIMEOUT, async {
        stream.write_all(line.as_bytes()).await?;
        stream.flush().await
    })
    .await;
    matches!(wrote, Ok(Ok(())))
}

async fn handle<S>(stream: S, commands: &UnboundedSender<Incoming>, windowed: bool)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut stream = BufReader::new(stream);
    let mut line = String::new();

    let read = tokio::time::timeout(IO_TIMEOUT, stream.read_line(&mut line)).await;
    if !matches!(read, Ok(Ok(n)) if n > 0) {
        return;
    }

    let refuse = |message: &str| [Reply::Err(message.to_string()), Reply::Exit(1)];
    let replies = match serde_json::from_str::<Request>(&line) {
        Err(_) => refuse("unknown request").to_vec(),
        Ok(Request::Start(_) | Request::Run(_)) if HOLDING.load(Ordering::Acquire) => {
            refuse("OneClient's database is being recovered, try again once that finishes").to_vec()
        }
        Ok(Request::Start(_)) if !windowed => {
            release().await;
            vec![Reply::Released]
        }
        Ok(Request::Window(_)) if !windowed => {
            refuse("OneClient is busy running a command line task, try again once it finishes")
                .to_vec()
        }
        Ok(Request::Window(command) | Request::Start(command)) => {
            match commands.send(Incoming::Window(command)) {
                Ok(()) => vec![Reply::Exit(0)],
                Err(_) => refuse("the launcher is shutting down").to_vec(),
            }
        }
        Ok(Request::Run(command)) => {
            let (output, mut replies) = Output::channel();
            if commands.send(Incoming::Run(command, output)).is_err() {
                refuse("the launcher is shutting down").to_vec()
            } else {
                while let Some(reply) = replies.recv().await {
                    let done = matches!(reply, Reply::Exit(_));
                    if !write_reply(&mut stream, &reply).await || done {
                        return;
                    }
                }
                vec![Reply::Exit(1)]
            }
        }
    };

    for reply in &replies {
        if !write_reply(&mut stream, reply).await {
            return;
        }
    }
}

async fn exchange<S>(stream: S, request: &Request, on_reply: &mut impl FnMut(Reply)) -> Exchange
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut stream = BufReader::new(stream);
    let Ok(mut line) = serde_json::to_string(request) else {
        return Exchange::Gone;
    };
    line.push('\n');

    let wrote = tokio::time::timeout(IO_TIMEOUT, async {
        stream.write_all(line.as_bytes()).await?;
        stream.flush().await
    })
    .await;
    if !matches!(wrote, Ok(Ok(()))) {
        return Exchange::Gone;
    }

    let patient = matches!(request, Request::Run(_));
    loop {
        let mut reply = String::new();
        let read = stream.read_line(&mut reply);
        let read = if patient {
            Ok(read.await)
        } else {
            tokio::time::timeout(IO_TIMEOUT, read).await
        };
        if !matches!(read, Ok(Ok(n)) if n > 0) {
            if patient {
                on_reply(Reply::Err(
                    "OneClient closed before the command finished, check what it changed before running it again"
                        .to_string(),
                ));
                return Exchange::Done(1);
            }
            return Exchange::Gone;
        }

        match serde_json::from_str::<Reply>(&reply) {
            Ok(Reply::Exit(code)) => return Exchange::Done(code),
            Ok(Reply::Released) => return Exchange::Released,
            Ok(reply) => on_reply(reply),
            Err(_) => {
                on_reply(Reply::Err(
                    "the running OneClient is a different version, restart it".to_string(),
                ));
                return Exchange::Done(1);
            }
        }
    }
}

#[cfg(windows)]
mod imp {
    use super::{BindError, Incoming, handle};
    use std::time::Duration;
    use tokio::net::windows::named_pipe::{
        ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
    };
    use tokio::sync::mpsc::UnboundedSender;

    #[cfg(not(debug_assertions))]
    const ENDPOINT: &str = r"\\.\pipe\org.polyfrost.OneClient.ipc";
    #[cfg(debug_assertions)]
    const ENDPOINT: &str = r"\\.\pipe\org.polyfrost.OneClient-dev.ipc";

    const ERROR_ACCESS_DENIED: i32 = 5;
    const ERROR_PIPE_BUSY: i32 = 231;

    const BUSY_RETRIES: usize = 5;
    const BUSY_BACKOFF: Duration = Duration::from_millis(60);

    pub struct Listener {
        server: NamedPipeServer,
    }

    pub async fn bind() -> Result<Listener, BindError> {
        match ServerOptions::new()
            .first_pipe_instance(true)
            .create(ENDPOINT)
        {
            Ok(server) => Ok(Listener { server }),
            Err(err) if err.raw_os_error() == Some(ERROR_ACCESS_DENIED) => Err(BindError::Taken),
            Err(err) => Err(BindError::Io(err)),
        }
    }

    pub async fn connect() -> Option<NamedPipeClient> {
        for _ in 0..BUSY_RETRIES {
            match ClientOptions::new().open(ENDPOINT) {
                Ok(client) => return Some(client),
                Err(err) if err.raw_os_error() == Some(ERROR_PIPE_BUSY) => {
                    tokio::time::sleep(BUSY_BACKOFF).await;
                }
                Err(_) => return None,
            }
        }
        None
    }

    pub async fn serve(listener: Listener, commands: UnboundedSender<Incoming>, windowed: bool) {
        let mut server = listener.server;

        loop {
            tokio::select! {
                connected = server.connect() => {
                    if connected.is_err() {
                        return;
                    }
                }
                () = super::RELEASE.notified() => return,
            }

            let next = match ServerOptions::new().create(ENDPOINT) {
                Ok(next) => next,
                Err(_) => return,
            };
            let connected = std::mem::replace(&mut server, next);

            let commands = commands.clone();
            super::handlers().spawn(async move { handle(connected, &commands, windowed).await });
        }
    }
}

#[cfg(unix)]
mod imp {
    use super::{BindError, Incoming, handle};
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use tokio::net::{UnixListener, UnixStream};
    use tokio::sync::mpsc::UnboundedSender;

    fn endpoint() -> Option<PathBuf> {
        oneclient_common::paths::ipc_socket().ok()
    }

    pub struct Listener {
        listener: UnixListener,
        path: PathBuf,
    }

    impl Drop for Listener {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    pub async fn bind() -> Result<Listener, BindError> {
        let Some(path) = endpoint() else {
            return Err(BindError::Io(std::io::Error::other("no data directory")));
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(BindError::Io)?;
        }

        match bound(&path) {
            Ok(listener) => Ok(listener),
            Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => {
                if std::os::unix::net::UnixStream::connect(&path).is_ok() {
                    return Err(BindError::Taken);
                }
                let _ = std::fs::remove_file(&path);
                bound(&path).map_err(BindError::Io)
            }
            Err(err) => Err(BindError::Io(err)),
        }
    }

    /// The launcher directory is not private, so the endpoint itself has to
    /// keep other local users from sending a launch of their choosing
    fn bound(path: &Path) -> std::io::Result<Listener> {
        let listener = UnixListener::bind(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;

        Ok(Listener {
            listener,
            path: path.to_path_buf(),
        })
    }

    pub async fn connect() -> Option<UnixStream> {
        UnixStream::connect(endpoint()?).await.ok()
    }

    pub async fn serve(listener: Listener, commands: UnboundedSender<Incoming>, windowed: bool) {
        loop {
            let accepted = tokio::select! {
                accepted = listener.listener.accept() => accepted,
                () = super::RELEASE.notified() => return,
            };
            match accepted {
                Ok((stream, _)) => {
                    let commands = commands.clone();
                    super::handlers()
                        .spawn(async move { handle(stream, &commands, windowed).await });
                }
                Err(_) => return,
            }
        }
    }
}

pub use imp::Listener;

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(request: Request) -> Request {
        serde_json::from_str(&serde_json::to_string(&request).unwrap()).unwrap()
    }

    #[test]
    fn focus_survives_the_wire() {
        assert!(matches!(
            round_trip(Request::Window(IpcCommand::Focus)),
            Request::Window(IpcCommand::Focus)
        ));
    }

    #[test]
    fn a_folder_with_spaces_is_not_split() {
        let folder = "My Pack (1.8.9)".to_string();
        assert!(matches!(
            round_trip(Request::Window(IpcCommand::Launch(folder.clone()))),
            Request::Window(IpcCommand::Launch(got)) if got == folder
        ));
    }

    #[test]
    fn junk_decodes_to_nothing() {
        assert!(serde_json::from_str::<Request>("").is_err());
        assert!(serde_json::from_str::<Request>("LAUNCH pack").is_err());
        assert!(serde_json::from_str::<Request>("RESTART").is_err());
    }
}
