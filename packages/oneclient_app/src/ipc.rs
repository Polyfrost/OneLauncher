use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

use crate::cli::Cli;

const IO_TIMEOUT: Duration = Duration::from_secs(3);
const CLAIM_ATTEMPTS: usize = 3;
const REPLY_OK: &str = "OK";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpcCommand {
    Launch(String),
    Focus,
}

pub enum Claim {
    Primary(Listener),
    Forwarded,
    Solo(String),
}

impl IpcCommand {
    fn encode(&self) -> String {
        match self {
            Self::Launch(folder) => format!("LAUNCH {folder}"),
            Self::Focus => "FOCUS".to_string(),
        }
    }

    fn decode(line: &str) -> Option<Self> {
        let line = line.trim_end_matches(['\r', '\n']);
        if let Some(folder) = line.strip_prefix("LAUNCH ") {
            let folder = folder.trim();
            return (!folder.is_empty()).then(|| Self::Launch(folder.to_string()));
        }
        (line == "FOCUS").then_some(Self::Focus)
    }
}

#[must_use]
pub fn request_for(cli: &Cli) -> IpcCommand {
    match &cli.launch {
        Some(folder) => IpcCommand::Launch(folder.clone()),
        None => IpcCommand::Focus,
    }
}

pub async fn claim(cli: &Cli) -> Claim {
    let request = request_for(cli);

    for _ in 0..CLAIM_ATTEMPTS {
        if forward(&request).await {
            return Claim::Forwarded;
        }

        match imp::bind().await {
            Ok(listener) => return Claim::Primary(listener),
            Err(BindError::Taken) => continue,
            Err(BindError::Io(err)) => return Claim::Solo(err.to_string()),
        }
    }

    Claim::Solo(format!("the endpoint changed hands {CLAIM_ATTEMPTS} times"))
}

async fn forward(request: &IpcCommand) -> bool {
    imp::send(&request.encode()).await
}

pub async fn serve(listener: Listener, on_command: impl Fn(IpcCommand)) {
    imp::serve(listener, on_command).await;
}

enum BindError {
    Taken,
    Io(std::io::Error),
}

async fn handle<S>(stream: S, on_command: &impl Fn(IpcCommand))
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut stream = BufReader::new(stream);
    let mut line = String::new();

    let read = tokio::time::timeout(IO_TIMEOUT, stream.read_line(&mut line)).await;
    if !matches!(read, Ok(Ok(n)) if n > 0) {
        return;
    }

    let reply = match IpcCommand::decode(&line) {
        Some(command) => {
            on_command(command);
            format!("{REPLY_OK}\n")
        }
        None => "ERR unknown request\n".to_string(),
    };

    let _ = tokio::time::timeout(IO_TIMEOUT, stream.write_all(reply.as_bytes())).await;
    let _ = stream.flush().await;
}

async fn exchange<S>(stream: S, request: &str) -> bool
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut stream = BufReader::new(stream);

    let wrote = tokio::time::timeout(IO_TIMEOUT, async {
        stream.write_all(request.as_bytes()).await?;
        stream.write_all(b"\n").await?;
        stream.flush().await
    })
    .await;
    if !matches!(wrote, Ok(Ok(()))) {
        return false;
    }

    let mut reply = String::new();
    let read = tokio::time::timeout(IO_TIMEOUT, stream.read_line(&mut reply)).await;
    matches!(read, Ok(Ok(n)) if n > 0) && reply.trim_end_matches(['\r', '\n']) == REPLY_OK
}

#[cfg(windows)]
mod imp {
    use super::{BindError, IpcCommand, exchange, handle};
    use std::time::Duration;
    use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions};

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
        match ServerOptions::new().first_pipe_instance(true).create(ENDPOINT) {
            Ok(server) => Ok(Listener { server }),
            Err(err) if err.raw_os_error() == Some(ERROR_ACCESS_DENIED) => Err(BindError::Taken),
            Err(err) => Err(BindError::Io(err)),
        }
    }

    pub async fn send(request: &str) -> bool {
        for _ in 0..BUSY_RETRIES {
            match ClientOptions::new().open(ENDPOINT) {
                Ok(client) => return exchange(client, request).await,
                Err(err) if err.raw_os_error() == Some(ERROR_PIPE_BUSY) => {
                    tokio::time::sleep(BUSY_BACKOFF).await;
                }
                Err(_) => return false,
            }
        }
        false
    }

    pub async fn serve(listener: Listener, on_command: impl Fn(IpcCommand)) {
        let mut server = listener.server;

        loop {
            if server.connect().await.is_err() {
                return;
            }

            let next = match ServerOptions::new().create(ENDPOINT) {
                Ok(next) => next,
                Err(_) => return,
            };
            let connected = std::mem::replace(&mut server, next);

            handle(connected, &on_command).await;
        }
    }
}

#[cfg(unix)]
mod imp {
    use super::{BindError, IpcCommand, exchange, handle};
    use std::path::PathBuf;
    use tokio::net::{UnixListener, UnixStream};

    fn endpoint() -> Option<PathBuf> {
        oneclient_common::paths::launcher_dir()
            .ok()
            .map(|dir| dir.join("ipc.sock"))
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

        match UnixListener::bind(&path) {
            Ok(listener) => Ok(Listener { listener, path }),
            Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => {
                let _ = std::fs::remove_file(&path);
                UnixListener::bind(&path)
                    .map(|listener| Listener { listener, path })
                    .map_err(BindError::Io)
            }
            Err(err) => Err(BindError::Io(err)),
        }
    }

    pub async fn send(request: &str) -> bool {
        let Some(path) = endpoint() else {
            return false;
        };
        let Ok(stream) = UnixStream::connect(&path).await else {
            return false;
        };
        exchange(stream, request).await
    }

    pub async fn serve(listener: Listener, on_command: impl Fn(IpcCommand)) {
        loop {
            match listener.listener.accept().await {
                Ok((stream, _)) => handle(stream, &on_command).await,
                Err(_) => return,
            }
        }
    }
}

pub use imp::Listener;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_launch_request_survives_the_wire() {
        let command = IpcCommand::Launch("fabric-1-20".into());
        assert_eq!(IpcCommand::decode(&command.encode()), Some(command));
    }

    #[test]
    fn focus_survives_the_wire() {
        assert_eq!(IpcCommand::decode(&IpcCommand::Focus.encode()), Some(IpcCommand::Focus));
    }

    #[test]
    fn a_folder_with_spaces_is_not_split() {
        let command = IpcCommand::Launch("My Pack (1.8.9)".into());
        assert_eq!(IpcCommand::decode(&command.encode()), Some(command));
    }

    #[test]
    fn line_endings_are_stripped() {
        assert_eq!(
            IpcCommand::decode("LAUNCH pack\r\n"),
            Some(IpcCommand::Launch("pack".into())),
        );
    }

    #[test]
    fn junk_decodes_to_nothing() {
        assert_eq!(IpcCommand::decode(""), None);
        assert_eq!(IpcCommand::decode("LAUNCH"), None);
        assert_eq!(IpcCommand::decode("LAUNCH   "), None);
        assert_eq!(IpcCommand::decode("QUIT"), None);
    }

    #[test]
    fn a_bare_start_asks_only_for_the_window() {
        assert_eq!(request_for(&Cli::default()), IpcCommand::Focus);
        assert_eq!(
            request_for(&Cli { launch: Some("pack".into()) }),
            IpcCommand::Launch("pack".into()),
        );
    }
}
