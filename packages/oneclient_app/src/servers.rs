//! Local server processes. Output is read on tokio and folded into the radio state on the UI thread

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use freya::prelude::spawn_in_window;
use freya::radio::RadioStation;
use oneclient_core::servers::{self, LocalServer};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use crate::hooks::invalidate_server_queries;
use crate::i18n::t;
use crate::launcher::{self, off_ui};
use crate::state::{AppChannel, AppState};

const MAX_LINES: usize = 5000;
const STATS_EVERY: Duration = Duration::from_secs(2);
const STOP_GRACE: Duration = Duration::from_secs(45);
const STOP_POLL: Duration = Duration::from_millis(250);
const BOOT_WAIT: Duration = Duration::from_secs(600);
const STOP_POLLS: u128 = STOP_GRACE.as_millis() / STOP_POLL.as_millis();

type Station = RadioStation<AppState, AppChannel>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ServerStatus {
    #[default]
    Stopped,
    Installing,
    Starting,
    Running,
    Stopping,
}

impl ServerStatus {
    pub fn is_live(self) -> bool {
        matches!(self, Self::Starting | Self::Running | Self::Stopping)
    }
}

#[derive(Clone, Debug, Default)]
pub struct ServerLive {
    pub status: ServerStatus,
    pub lines: Arc<Vec<Arc<str>>>,
    pub started: Option<Instant>,
    pub players: Vec<String>,
    pub uuids: HashMap<String, String>,
    pub memory_bytes: Option<u64>,
    pub cpu_percent: Option<f32>,
    pub pid: Option<u32>,
    pub installing: Vec<String>,
    pub booted: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ServerHub {
    live: HashMap<String, ServerLive>,
}

impl ServerHub {
    pub fn get(&self, id: &str) -> ServerLive {
        self.live.get(id).cloned().unwrap_or_default()
    }

    pub fn status(&self, id: &str) -> ServerStatus {
        self.live.get(id).map(|l| l.status).unwrap_or_default()
    }

    pub fn clear_lines(&mut self, id: &str) {
        if let Some(live) = self.live.get_mut(id) {
            live.lines = Arc::default();
        }
    }

    fn entry(&mut self, id: &str) -> &mut ServerLive {
        self.live.entry(id.to_string()).or_default()
    }
}

static STDIN: LazyLock<Mutex<HashMap<String, mpsc::UnboundedSender<String>>>> =
    LazyLock::new(Default::default);
static STOPPING: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);
static PIDS: LazyLock<Mutex<HashMap<String, u32>>> = LazyLock::new(Default::default);
static RESTARTING: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);

/// Asks every running server to save and stop before the launcher quits, killing what
/// is still up after the grace period so nothing is left holding a world lock
/// For exits that skip [`stop_all`], like closing the window from the taskbar: runs after the
/// UI is gone, on the still-alive runtime, so it can only ask, wait and then kill
pub fn stop_all_blocking() {
    let running: Vec<(String, u32)> = PIDS
        .lock()
        .unwrap()
        .iter()
        .map(|(id, pid)| (id.clone(), *pid))
        .collect();
    if running.is_empty() {
        return;
    }
    for (id, _) in &running {
        send_command(id, "stop");
    }
    for _ in 0..STOP_POLLS {
        if running
            .iter()
            .all(|(_, pid)| !oneclient_core::game::is_process_alive(*pid, None))
        {
            return;
        }
        std::thread::sleep(STOP_POLL);
    }
    for (_, pid) in &running {
        oneclient_core::game::kill_process(*pid);
    }
}

/// Puts each server in a job that Windows kills when the launcher process ends, so a
/// crash or a killed dev run never leaves a server holding the world lock
#[cfg(windows)]
mod kill_with_launcher {
    use std::sync::OnceLock;

    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };
    use windows::core::PCWSTR;

    struct Job(HANDLE);
    // The handle is only passed to thread-safe Win32 calls and never closed
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}

    static JOB: OnceLock<Option<Job>> = OnceLock::new();

    fn job() -> Option<HANDLE> {
        JOB.get_or_init(|| unsafe {
            let job = CreateJobObjectW(None, PCWSTR::null()).ok()?;
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&info).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .ok()?;
            Some(Job(job))
        })
        .as_ref()
        .map(|job| job.0)
    }

    pub fn adopt(child: &tokio::process::Child) {
        let (Some(job), Some(handle)) = (job(), child.raw_handle()) else {
            return;
        };
        if let Err(err) = unsafe { AssignProcessToJobObject(job, HANDLE(handle)) } {
            tracing::warn!("server will not stop with the launcher: {err}");
        }
    }
}

pub async fn stop_all() {
    let running: Vec<String> = PIDS.lock().unwrap().keys().cloned().collect();
    if running.is_empty() {
        return;
    }
    for id in &running {
        STOPPING.lock().unwrap().insert(id.clone());
        send_command(id, "stop");
    }
    for _ in 0..STOP_POLLS {
        if PIDS.lock().unwrap().is_empty() {
            return;
        }
        tokio::time::sleep(STOP_POLL).await;
    }
    for pid in PIDS.lock().unwrap().values() {
        oneclient_core::game::kill_process(*pid);
    }
}

enum Output {
    Line(String),
    Stats(Option<u64>, Option<f32>),
    Exit(Option<i32>),
}

fn write(mut station: Station, id: &str, f: impl FnOnce(&mut ServerLive)) {
    let mut guard = station.write_channel(AppChannel::Servers);
    f(guard.servers.entry(id));
}

fn notify_error(title: &'static str, err: impl ToString) {
    if let Ok(state) = launcher::state() {
        state
            .services
            .events
            .notify(title)
            .body(err.to_string())
            .error()
            .send();
    }
}

pub fn send_command(id: &str, command: &str) -> bool {
    STDIN
        .lock()
        .unwrap()
        .get(id)
        .is_some_and(|tx| tx.send(command.trim().to_string()).is_ok())
}

/// The name in a "joined the game" or "left the game" line
fn player_event(line: &str) -> Option<(bool, String)> {
    let message = line.split_once("]: ").map_or(line, |(_, m)| m).trim();
    let message = message.strip_prefix("System chat: ").unwrap_or(message);
    let (name, joined) = if let Some(name) = message.strip_suffix(" joined the game") {
        (name, true)
    } else {
        (message.strip_suffix(" left the game")?, false)
    };
    let name = name.split_whitespace().next()?;
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
        .then(|| (joined, name.to_string()))
}

fn player_uuid(line: &str) -> Option<(String, String)> {
    let message = line.split_once("]: ").map_or(line, |(_, m)| m).trim();
    let (name, uuid) = message.strip_prefix("UUID of player ")?.split_once(" is ")?;
    let uuid = uuid.trim().replace('-', "");
    (uuid.len() == 32 && uuid.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| (name.to_string(), uuid))
}

fn push_line(live: &mut ServerLive, line: String) {
    if line.contains("Done (") && line.contains(")! For help") {
        live.booted = true;
        if live.status == ServerStatus::Starting {
            live.status = ServerStatus::Running;
        }
    }
    if let Some((name, uuid)) = player_uuid(&line) {
        live.uuids.insert(name, uuid);
    }
    match player_event(&line) {
        Some((true, name)) if !live.players.contains(&name) => live.players.push(name),
        Some((false, name)) => live.players.retain(|p| *p != name),
        _ => {}
    }
    let lines = Arc::make_mut(&mut live.lines);
    lines.push(Arc::from(line));
    if lines.len() > MAX_LINES {
        lines.drain(..lines.len() - MAX_LINES);
    }
}

pub fn start(station: Station, id: String) {
    if station.peek().servers.status(&id) != ServerStatus::Stopped {
        return;
    }
    write(station, &id, |live| {
        *live = ServerLive {
            status: ServerStatus::Starting,
            started: Some(Instant::now()),
            ..Default::default()
        };
    });

    spawn_in_window(async move {
        let spawned = off_ui({
            let id = id.clone();
            async move {
                let state = launcher::state()?;
                let server = servers::load_server(&id).await?;
                let dir = server.dir()?;
                if servers::kill_orphan(&dir).await {
                    tracing::warn!("stopped server {id} left running by an earlier launcher session");
                }
                let mut command = servers::server_command(&state, &server).await?;
                let child = command.spawn()?;
                #[cfg(windows)]
                kill_with_launcher::adopt(&child);
                if let Some(pid) = child.id() {
                    servers::write_pid(&dir, pid).await;
                }
                Ok::<_, oneclient_core::LauncherError>((child, server))
            }
        })
        .await;

        let (mut child, server) = match spawned {
            Ok(spawned) => spawned,
            Err(err) => {
                STOPPING.lock().unwrap().remove(&id);
                write(station, &id, |live| live.status = ServerStatus::Stopped);
                notify_error(t("oneclient.servers.start_failed"), err);
                return;
            }
        };

        if STOPPING.lock().unwrap().contains(&id) {
            let _ = child.start_kill();
        }
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        STDIN.lock().unwrap().insert(id.clone(), cmd_tx);
        let pid = child.id();
        if let Some(pid) = pid {
            PIDS.lock().unwrap().insert(id.clone(), pid);
        }
        write(station, &id, |live| live.pid = pid);
        tokio::spawn(drive(child, tx, cmd_rx));

        let mut last_backup = Instant::now();
        while let Some(output) = rx.recv().await {
            match output {
                Output::Line(line) => write(station, &id, |live| push_line(live, line)),
                Output::Stats(memory, cpu) => {
                    write(station, &id, |live| {
                        live.memory_bytes = memory;
                        live.cpu_percent = cpu;
                    });
                    let every = Duration::from_secs(u64::from(server.backup_hours) * 3600);
                    if server.backup_hours > 0 && last_backup.elapsed() >= every {
                        last_backup = Instant::now();
                        backup(id.clone(), true);
                    }
                }
                Output::Exit(code) => {
                    let asked = STOPPING.lock().unwrap().remove(&id);
                    STDIN.lock().unwrap().remove(&id);
                    PIDS.lock().unwrap().remove(&id);
                    if let Ok(dir) = server.dir() {
                        servers::clear_pid(&dir).await;
                    }
                    write(station, &id, |live| {
                        live.status = ServerStatus::Stopped;
                        live.started = None;
                        live.players.clear();
                        live.uuids.clear();
                        live.pid = None;
                        live.memory_bytes = None;
                        live.cpu_percent = None;
                    });
                    if !asked && code.is_some_and(|c| c != 0) {
                        notify_error(
                            t("oneclient.servers.crashed"),
                            format!("{} ({})", server.name, code.unwrap_or_default()),
                        );
                    }
                    invalidate_server_queries().await;
                }
            }
        }
    });
}

async fn drive(
    mut child: tokio::process::Child,
    tx: mpsc::UnboundedSender<Output>,
    mut cmd_rx: mpsc::UnboundedReceiver<String>,
) {
    for reader in [
        child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Send + Unpin>),
        child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Send + Unpin>),
    ]
    .into_iter()
    .flatten()
    {
        let tx = tx.clone();
        tokio::spawn(async move {
            // Java on Windows writes in the system codepage, so lines are decoded lossily
            // rather than letting one non-UTF-8 byte end the stream
            let mut reader = BufReader::new(reader);
            let mut buf = Vec::new();
            while matches!(reader.read_until(b'\n', &mut buf).await, Ok(n) if n > 0) {
                let line = String::from_utf8_lossy(&buf)
                    .trim_end_matches(['\r', '\n'])
                    .to_string();
                buf.clear();
                if tx.send(Output::Line(line)).is_err() {
                    break;
                }
            }
        });
    }

    if let Some(mut stdin) = child.stdin.take() {
        tokio::spawn(async move {
            while let Some(command) = cmd_rx.recv().await {
                if stdin
                    .write_all(format!("{command}\n").as_bytes())
                    .await
                    .is_err()
                {
                    break;
                }
                let _ = stdin.flush().await;
            }
        });
    }

    let stats = child.id().map(|pid| {
        let tx = tx.clone();
        tokio::spawn(async move {
            let pid = sysinfo::Pid::from_u32(pid);
            let mut system = sysinfo::System::new();
            let cpus = std::thread::available_parallelism().map_or(1, |n| n.get()) as f32;
            loop {
                system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
                let process = system.process(pid);
                let sample = Output::Stats(
                    process.map(|p| p.memory()),
                    process.map(|p| p.cpu_usage() / cpus),
                );
                if tx.send(sample).is_err() {
                    break;
                }
                tokio::time::sleep(STATS_EVERY).await;
            }
        })
    });

    let code = child.wait().await.ok().and_then(|s| s.code());
    if let Some(stats) = stats {
        stats.abort();
    }
    let _ = tx.send(Output::Exit(code));
}

pub fn stop(station: Station, id: String) {
    let live = station.peek().servers.get(&id);
    if !live.status.is_live() {
        return;
    }
    STOPPING.lock().unwrap().insert(id.clone());
    write(station, &id, |live| live.status = ServerStatus::Stopping);
    if !send_command(&id, "stop")
        && let Some(pid) = live.pid
    {
        oneclient_core::game::kill_process(pid);
        return;
    }
    let Some(armed) = live.pid else {
        return;
    };
    spawn_in_window(async move {
        if !live.booted {
            let waiting = Instant::now();
            loop {
                tokio::time::sleep(STOP_POLL).await;
                let now = station.peek().servers.get(&id);
                if now.status != ServerStatus::Stopping || now.pid != Some(armed) {
                    return;
                }
                if now.booted {
                    send_command(&id, "stop");
                    break;
                }
                if waiting.elapsed() >= BOOT_WAIT {
                    break;
                }
            }
        }
        tokio::time::sleep(STOP_GRACE).await;
        let live = station.peek().servers.get(&id);
        if live.status == ServerStatus::Stopping && live.pid == Some(armed) {
            oneclient_core::game::kill_process(armed);
        }
    });
}

pub fn restart(station: Station, id: String) {
    if !station.peek().servers.status(&id).is_live() {
        start(station, id);
        return;
    }
    if !RESTARTING.lock().unwrap().insert(id.clone()) {
        return;
    }
    stop(station, id.clone());
    spawn_in_window(async move {
        while station.peek().servers.status(&id) == ServerStatus::Stopping {
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        RESTARTING.lock().unwrap().remove(&id);
        if station.peek().servers.status(&id) == ServerStatus::Stopped {
            start(station, id);
        }
    });
}

/// While running, saving is paused so the copy is not torn mid-write
pub fn backup(id: String, quiet: bool) {
    spawn_in_window(async move {
        let running = send_command(&id, "save-off");
        if running {
            send_command(&id, "save-all flush");
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
        let result = off_ui({
            let id = id.clone();
            async move {
                let server = servers::load_server(&id).await?;
                servers::create_backup(&server.dir()?, server.backups_kept).await
            }
        })
        .await;
        if running {
            send_command(&id, "save-on");
        }
        match result {
            Ok(_) if !quiet => {
                if let Ok(state) = launcher::state() {
                    state
                        .services
                        .events
                        .notify(t("oneclient.servers.backup.done"))
                        .send();
                }
            }
            Ok(_) => {}
            Err(err) => notify_error(t("oneclient.servers.backup.failed"), err),
        }
        invalidate_server_queries().await;
    });
}

/// Installs in the background, the server shows as installing until it finishes
pub fn install(station: Station, mut server: LocalServer, prepare: Prepare, port: u16) {
    let id = server.id.clone();
    write(station, &id, |live| live.status = ServerStatus::Installing);
    spawn_in_window(async move {
        let listed = off_ui({
            let server = server.clone();
            async move { servers::save_server(&server).await }
        })
        .await;
        invalidate_server_queries().await;
        let result = off_ui(async move {
            listed?;
            let state = launcher::state()?;
            let dir = server.dir()?;
            match prepare {
                Prepare::None => {}
                Prepare::Instance(cluster_id, game_dir) => {
                    servers::copy_from_instance(&state, cluster_id, &game_dir, &dir).await?;
                }
                Prepare::Modpack(pack) => servers::unpack_mrpack(&state, &pack, &dir).await?,
                Prepare::Import(source) => polyio::copy_dir(&source, &dir, &[]).await?,
            }
            let mut props = servers::Properties::load(&dir).await?;
            props.set("server-port", &port.to_string());
            props.write(&dir).await?;
            servers::install_server(&state, &mut server).await
        })
        .await;
        write(station, &id, |live| live.status = ServerStatus::Stopped);
        if let Err(err) = result {
            notify_error(t("oneclient.servers.install_failed"), err);
        }
        invalidate_server_queries().await;
    });
}

pub enum Prepare {
    None,
    Instance(i64, std::path::PathBuf),
    Modpack(std::path::PathBuf),
    Import(std::path::PathBuf),
}

pub fn spawn_autostart(station: Station) {
    spawn_in_window(async move {
        let listed = off_ui(servers::list_servers()).await.unwrap_or_default();
        for server in listed.into_iter().filter(|s| s.autostart && s.installed()) {
            start(station, server.id);
        }
    });
}

/// Saves a change to the server's own settings
pub fn update(id: String, change: impl FnOnce(&mut LocalServer) + Send + 'static) {
    spawn_in_window(async move {
        let result = off_ui(async move {
            let mut server = servers::load_server(&id).await?;
            change(&mut server);
            servers::save_server(&server).await
        })
        .await;
        if let Err(err) = result {
            notify_error(t("oneclient.servers.save_failed"), err);
        }
        invalidate_server_queries().await;
    });
}

pub fn set_property(id: String, key: &'static str, value: String) {
    spawn_in_window(async move {
        let result = off_ui(async move {
            let dir = servers::server_dir(&id)?;
            let mut props = servers::Properties::load(&dir).await?;
            props.set(key, &value);
            props.write(&dir).await?;
            Ok::<_, oneclient_core::LauncherError>(())
        })
        .await;
        if let Err(err) = result {
            notify_error(t("oneclient.servers.save_failed"), err);
        }
        invalidate_server_queries().await;
    });
}

/// Runs a file operation in the background and refreshes the server views after
pub fn run<F>(failure: &'static str, work: F)
where
    F: Future<Output = oneclient_core::LauncherResult<()>> + Send + 'static,
{
    spawn_in_window(async move {
        if let Err(err) = off_ui(work).await {
            notify_error(failure, err);
        }
        invalidate_server_queries().await;
    });
}

pub fn identify_mods(id: String) {
    spawn_in_window(async move {
        let result = off_ui(async move {
            let state = launcher::state()?;
            servers::identify_mods(&state, &servers::server_dir(&id)?).await
        })
        .await;
        match result {
            Ok(true) => invalidate_server_queries().await,
            Ok(false) => {}
            Err(err) => tracing::warn!(%err, "could not identify the server's mods"),
        }
    });
}

pub fn install_mod(
    station: Station,
    id: String,
    provider: oneclient_common::domain::ProviderId,
    project_id: String,
    version_id: String,
) {
    write(station, &id, |live| live.installing.push(project_id.clone()));
    spawn_in_window(async move {
        let result = off_ui({
            let (id, project_id) = (id.clone(), project_id.clone());
            async move {
                let state = launcher::state()?;
                let server = servers::load_server(&id).await?;
                servers::install_mod(&state, &server, provider, &project_id, &version_id).await
            }
        })
        .await;
        write(station, &id, |live| live.installing.retain(|p| *p != project_id));
        if let Err(err) = result {
            notify_error(t("oneclient.servers.mods.install_failed"), err);
        }
        invalidate_server_queries().await;
    });
}

/// Uses the console while the server runs so it applies at once, edits the file otherwise
pub fn add_player(id: String, name: String, op: bool) {
    let file = if op { servers::OPS } else { servers::WHITELIST };
    let command = if op { "op" } else { "whitelist add" };
    if send_command(&id, &format!("{command} {name}")) {
        spawn_in_window(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            invalidate_server_queries().await;
        });
        return;
    }
    run(t("oneclient.servers.players.add_failed"), async move {
        let state = launcher::state()?;
        let (uuid, name) = oneclient_mc::resolve_username(&state.services.requester, &name)
            .await
            .map_err(|e| oneclient_core::LauncherError::Server(e.to_string()))?;
        servers::add_to_list(
            &servers::server_dir(&id)?,
            file,
            servers::ListedPlayer::new(&uuid, name, op),
        )
        .await
    });
}

pub fn remove_player(id: String, name: String, op: bool) {
    let file = if op { servers::OPS } else { servers::WHITELIST };
    let command = if op { "deop" } else { "whitelist remove" };
    if send_command(&id, &format!("{command} {name}")) {
        spawn_in_window(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            invalidate_server_queries().await;
        });
        return;
    }
    run(t("oneclient.servers.players.remove_failed"), async move {
        servers::remove_from_list(&servers::server_dir(&id)?, file, &name).await
    });
}

#[cfg(test)]
mod tests {
    use super::{player_event, player_uuid};

    #[test]
    fn reads_the_uuid_the_authenticator_logs() {
        assert_eq!(
            player_uuid(
                "[14:10:54] [User Authenticator #1/INFO]: UUID of player frostbyte is 069a79f4-44e9-4726-a5be-fca90e38aaf5"
            ),
            Some((
                "frostbyte".into(),
                "069a79f444e94726a5befca90e38aaf5".into()
            ))
        );
        assert_eq!(
            player_uuid("[14:10:54] [Server thread/INFO]: <Steve> UUID of player x is y"),
            None
        );
        assert_eq!(
            player_uuid(
                "[14:10:54] [Server thread/INFO]: <Steve> ]: UUID of player Notch is 069a79f444e94726a5befca90e38aaf5"
            ),
            None
        );
    }

    #[test]
    fn reads_join_and_leave_lines() {
        assert_eq!(
            player_event("[14:10:55] [Server thread/INFO]: frostbyte joined the game"),
            Some((true, "frostbyte".into()))
        );
        assert_eq!(
            player_event(
                "[14:10:55] [Server thread/INFO] [minecraft/MinecraftServer]: Steve_Builds left the game"
            ),
            Some((false, "Steve_Builds".into()))
        );
        assert_eq!(
            player_event("[13:05:42] [Server thread/INFO]: System chat: rozwader joined the game"),
            Some((true, "rozwader".into()))
        );
        assert_eq!(
            player_event("[14:10:55] [Server thread/INFO]: <Steve> I joined the game"),
            None
        );
        assert_eq!(
            player_event("[14:10:55] [Server thread/INFO]: <Steve> x ]: Notch joined the game"),
            None
        );
    }
}
