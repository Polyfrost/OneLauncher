//! Local server processes. Output is read on tokio and folded into the radio state on the UI thread

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use freya::prelude::{GlobalContexts, spawn_in_window};
use freya::radio::RadioStation;
use oneclient_core::servers::{self, LocalServer};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use crate::hooks::{invalidate_server, invalidate_server_settings};
use crate::i18n::{t, tf};
use crate::launcher::{self, off_ui};
use crate::notifications::{NotificationAction, NotificationActionKind};
use crate::state::{AppChannel, AppState};

pub(crate) const MAX_LINES: usize = 5000;
const STATS_EVERY: Duration = Duration::from_secs(2);
const STOP_IDLE: Duration = Duration::from_secs(45);
const STOP_CAP: Duration = Duration::from_secs(600);
const STOP_POLL: Duration = Duration::from_millis(250);
const FORCE_AFTER: Duration = Duration::from_secs(20);
const BOOT_WAIT: Duration = Duration::from_secs(600);
const SAVE_WAIT: Duration = Duration::from_secs(120);
const CRASH_TRACE_LINES: usize = 150;
const MAX_AUTO_RESTARTS: u32 = 5;
const LINE_BATCH: Duration = Duration::from_millis(50);

pub(crate) type Station = RadioStation<AppState, AppChannel>;

static PICKED_PACK: Mutex<Option<oneclient_core::clusters::ModpackSource>> = Mutex::new(None);

pub fn pick_pack(source: oneclient_core::clusters::ModpackSource) {
    *PICKED_PACK.lock().unwrap_or_else(|e| e.into_inner()) = Some(source);
    let _ = freya::router::RouterContext::get().push(crate::Route::Servers {});
}

pub fn take_picked_pack() -> Option<oneclient_core::clusters::ModpackSource> {
    PICKED_PACK.lock().unwrap_or_else(|e| e.into_inner()).take()
}

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
    pub shutting_down: bool,
}

impl ServerHub {
    pub fn live_count(&self) -> usize {
        self.live.values().filter(|l| l.status.is_live()).count()
    }

    pub fn get(&self, id: &str) -> ServerLive {
        self.live.get(id).cloned().unwrap_or_default()
    }

    pub fn status(&self, id: &str) -> ServerStatus {
        self.live.get(id).map(|l| l.status).unwrap_or_default()
    }

    pub fn forget(&mut self, id: &str) {
        self.live.remove(id);
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
static BACKUP_HOURS: LazyLock<Mutex<HashMap<String, u32>>> = LazyLock::new(Default::default);
static RESTARTING: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);
static AUTO_RESTARTS: LazyLock<Mutex<HashMap<String, u32>>> = LazyLock::new(Default::default);
static SAVED: LazyLock<Mutex<HashMap<String, Arc<tokio::sync::Notify>>>> =
    LazyLock::new(Default::default);

type Locks = LazyLock<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>;
static FILES: Locks = LazyLock::new(Default::default);
static MODS: Locks = LazyLock::new(Default::default);
static BACKUPS: Locks = LazyLock::new(Default::default);

async fn hold(locks: &'static Locks, id: &str) -> tokio::sync::OwnedMutexGuard<()> {
    let lock = locks
        .lock()
        .unwrap()
        .entry(id.to_string())
        .or_default()
        .clone();
    lock.lock_owned().await
}

fn saved_signal(id: &str) -> Arc<tokio::sync::Notify> {
    SAVED
        .lock()
        .unwrap()
        .entry(id.to_string())
        .or_default()
        .clone()
}

fn is_saved_line(line: &str) -> bool {
    line.contains("Saved the game") || line.contains("Saved the world")
}

static LAST_OUTPUT: LazyLock<Mutex<HashMap<String, Instant>>> = LazyLock::new(Default::default);
static LAST_WRITE: LazyLock<Mutex<HashMap<u32, (u64, Instant)>>> = LazyLock::new(Default::default);

fn last_disk_write(pid: u32) -> Option<Instant> {
    let spid = sysinfo::Pid::from_u32(pid);
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[spid]),
        true,
        sysinfo::ProcessRefreshKind::nothing().with_disk_usage(),
    );
    let written = system.process(spid)?.disk_usage().total_written_bytes;
    let mut last = LAST_WRITE.lock().unwrap();
    let entry = last.entry(pid).or_insert((written, Instant::now()));
    if entry.0 != written {
        *entry = (written, Instant::now());
    }
    Some(entry.1)
}

fn should_kill(id: &str, pid: u32, asked: Instant) -> bool {
    let output = LAST_OUTPUT.lock().unwrap().get(id).copied();
    let since = [output, last_disk_write(pid)]
        .into_iter()
        .flatten()
        .fold(asked, Instant::max);
    since.elapsed() >= STOP_IDLE || asked.elapsed() >= STOP_CAP
}

async fn kill_due(id: &str, pid: u32, asked: Instant) -> bool {
    let id = id.to_string();
    crate::launcher::off_ui_blocking(move || should_kill(&id, pid, asked)).await
}

fn force_kill(pid: u32) {
    let spid = sysinfo::Pid::from_u32(pid);
    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[spid]), true);
    if let Some(process) = system.process(spid) {
        process.kill();
    }
}

fn actions() -> Option<crate::Actions> {
    GlobalContexts::try_get()?.try_get_context::<crate::Actions>()
}

/// Asks every running server to save and stop before the launcher quits, killing what
/// is still up after the grace period so nothing is left holding a world lock
/// For exits that skip [`stop_all`], like closing the window from the taskbar: runs after the
/// UI is gone, on the still-alive runtime, so it can only ask, wait and then kill
static QUITTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn stop_all_blocking() {
    QUITTING.store(true, std::sync::atomic::Ordering::SeqCst);
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
    let asked = Instant::now();
    let mut left: Vec<(String, u32, Option<Instant>)> =
        running.into_iter().map(|(id, pid)| (id, pid, None)).collect();
    while !left.is_empty() {
        left.retain_mut(|(id, pid, killed)| {
            if !oneclient_core::game::is_process_alive(*pid, None) {
                return false;
            }
            match killed {
                Some(at) if at.elapsed() >= FORCE_AFTER => {
                    force_kill(*pid);
                    return false;
                }
                Some(_) => {}
                None if should_kill(id, *pid, asked) => {
                    oneclient_core::game::kill_process(*pid);
                    *killed = Some(Instant::now());
                }
                None => {}
            }
            true
        });
        std::thread::sleep(STOP_POLL);
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
    QUITTING.store(true, std::sync::atomic::Ordering::SeqCst);
    let running: Vec<String> = PIDS.lock().unwrap().keys().cloned().collect();
    if running.is_empty() {
        return;
    }
    let station = actions().map(|actions| actions.station());
    if let Some(mut station) = station {
        crate::platform::focus_window();
        station
            .write_channel(AppChannel::Servers)
            .servers
            .shutting_down = true;
    }
    for id in &running {
        STOPPING.lock().unwrap().insert(id.clone());
        send_command(id, "stop");
    }
    let asked = Instant::now();
    let mut killed: HashMap<String, Instant> = HashMap::new();
    loop {
        let left: Vec<(String, u32)> = PIDS
            .lock()
            .unwrap()
            .iter()
            .map(|(id, pid)| (id.clone(), *pid))
            .collect();
        if left.is_empty() {
            break;
        }
        for (id, pid) in left {
            match killed.get(&id) {
                Some(at) if at.elapsed() >= FORCE_AFTER => {
                    crate::launcher::off_ui_blocking(move || force_kill(pid)).await;
                }
                Some(_) => {}
                None if kill_due(&id, pid, asked).await => {
                    oneclient_core::game::kill_process(pid);
                    killed.insert(id, Instant::now());
                }
                None => {}
            }
        }
        tokio::time::sleep(STOP_POLL).await;
    }
    if let Some(mut station) = station {
        station
            .write_channel(AppChannel::Servers)
            .servers
            .shutting_down = false;
    }
}

pub fn force_stop_all() {
    let pids: Vec<sysinfo::Pid> = PIDS
        .lock()
        .unwrap()
        .values()
        .map(|pid| sysinfo::Pid::from_u32(*pid))
        .collect();
    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&pids), true);
    for pid in pids {
        if let Some(process) = system.process(pid) {
            process.kill();
        }
    }
}

enum Output {
    Line(String),
    Stats(Option<u64>, Option<f32>),
    Exit(Option<i32>),
}

fn write(mut station: Station, id: &str, f: impl FnOnce(&mut ServerLive)) {
    let mut guard = station.write_channel(AppChannel::Servers);
    let before = guard.servers.live_count();
    f(guard.servers.entry(id));
    let changed = guard.servers.live_count() != before;
    drop(guard);
    if changed {
        station.write_channel(AppChannel::ServerCount);
    }
}

fn write_lines(mut station: Station, id: &str, batch: Vec<String>) {
    let mut guard = station.write_channel(AppChannel::ServerConsole);
    let live = guard.servers.entry(id);
    let before = (live.status, live.booted, live.players.clone(), live.uuids.len());
    push_lines(live, batch);
    let changed = (live.status, live.booted, live.players.clone(), live.uuids.len()) != before;
    drop(guard);
    if changed {
        station.write_channel(AppChannel::Servers);
    }
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
fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\x1b' {
            out.push(ch);
            continue;
        }
        if chars.peek() == Some(&'[') {
            chars.next();
            for code in chars.by_ref() {
                if ('@'..='~').contains(&code) {
                    break;
                }
            }
        }
    }
    out
}

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
    let (name, uuid) = message
        .strip_prefix("UUID of player ")?
        .split_once(" is ")?;
    let uuid = uuid.trim().replace('-', "");
    (uuid.len() == 32 && uuid.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| (name.to_string(), uuid))
}

fn push_lines(live: &mut ServerLive, batch: Vec<String>) {
    let mut lines = std::mem::take(&mut live.lines);
    let target = Arc::make_mut(&mut lines);
    for line in batch {
        note_line(live, &line);
        target.push(Arc::from(line));
    }
    if target.len() > MAX_LINES {
        target.drain(..target.len() - MAX_LINES);
    }
    live.lines = lines;
}

fn note_line(live: &mut ServerLive, line: &str) {
    if line.contains("Done (") && line.contains(")! For help") {
        live.booted = true;
        if live.status == ServerStatus::Starting {
            live.status = ServerStatus::Running;
        }
    }
    if let Some((name, uuid)) = player_uuid(line) {
        live.uuids.insert(name, uuid);
    }
    match player_event(line) {
        Some((true, name)) if !live.players.contains(&name) => live.players.push(name),
        Some((false, name)) => live.players.retain(|p| *p != name),
        _ => {}
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
                let _files = hold(&FILES, &id).await;
                let state = launcher::state()?;
                let server = servers::load_server(&id).await?;
                let dir = server.dir()?;
                if servers::kill_orphan(&dir).await? {
                    tracing::warn!(
                        "stopped server {id} left running by an earlier launcher session"
                    );
                }
                servers::ensure_lists(&dir, &server.mc_version).await?;
                let port = servers::Properties::read(&dir)
                    .await
                    .get("server-port")
                    .and_then(|port| port.trim().parse().ok())
                    .unwrap_or(servers::DEFAULT_PORT);
                if std::net::TcpListener::bind(("0.0.0.0", port)).is_err() {
                    return Err(oneclient_core::LauncherError::Server(format!(
                        "port {port} is already in use, change it in the server's settings"
                    )));
                }
                let mut command = servers::server_command(&state, &server).await?;
                let mut child = command.spawn()?;
                if QUITTING.load(std::sync::atomic::Ordering::SeqCst) {
                    let _ = child.start_kill();
                    return Err(oneclient_core::LauncherError::Server(
                        "the launcher is closing".into(),
                    ));
                }
                #[cfg(windows)]
                kill_with_launcher::adopt(&child);
                if let Some(pid) = child.id() {
                    // Registered while the file lock is held, so a queued world operation sees it
                    PIDS.lock().unwrap().insert(id.clone(), pid);
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
        write(station, &id, |live| live.pid = pid);
        tokio::spawn(drive(id.clone(), child, tx, cmd_rx));

        BACKUP_HOURS
            .lock()
            .unwrap()
            .insert(id.clone(), server.backup_hours);
        let mut last_backup = Instant::now();
        let mut held = None;
        let mut boot_failed = false;
        loop {
            let output = match held.take() {
                Some(output) => output,
                None => match rx.recv().await {
                    Some(output) => output,
                    None => break,
                },
            };
            match output {
                Output::Line(line) => {
                    tokio::time::sleep(LINE_BATCH).await;
                    let mut batch = vec![line];
                    while batch.len() < MAX_LINES {
                        match rx.try_recv() {
                            Ok(Output::Line(line)) => batch.push(line),
                            Ok(other) => {
                                held = Some(other);
                                break;
                            }
                            Err(_) => break,
                        }
                    }
                    if batch.iter().any(|line| is_saved_line(line)) {
                        saved_signal(&id).notify_waiters();
                    }
                    // A failed boot can leave mod threads holding the JVM open, so it would never exit
                    if !boot_failed && batch.iter().any(|line| is_boot_failure(line)) {
                        boot_failed = true;
                        if let Some(pid) = pid {
                            oneclient_core::game::kill_process(pid);
                        }
                    }
                    write_lines(station, &id, batch);
                }
                Output::Stats(memory, cpu) => {
                    write(station, &id, |live| {
                        live.memory_bytes = memory;
                        live.cpu_percent = cpu;
                    });
                    let hours = BACKUP_HOURS.lock().unwrap().get(&id).copied().unwrap_or(0);
                    let every = Duration::from_secs(u64::from(hours) * 3600);
                    if hours > 0 && last_backup.elapsed() >= every {
                        last_backup = Instant::now();
                        backup(id.clone(), true);
                    }
                }
                Output::Exit(code) => {
                    let asked = STOPPING.lock().unwrap().remove(&id);
                    let (lines, booted) = {
                        let live = station.peek().servers.get(&id);
                        (live.lines.clone(), live.booted)
                    };
                    if booted {
                        AUTO_RESTARTS.lock().unwrap().remove(&id);
                    }
                    STDIN.lock().unwrap().remove(&id);
                    PIDS.lock().unwrap().remove(&id);
                    BACKUP_HOURS.lock().unwrap().remove(&id);
                    if let Some(pid) = pid {
                        LAST_WRITE.lock().unwrap().remove(&pid);
                    }
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
                    if !asked && (boot_failed || code.is_some_and(|c| c != 0)) {
                        report_crash(station, &server, &lines, booted, code.unwrap_or_default())
                            .await;
                    }
                    invalidate_server(&id).await;
                    crate::hooks::invalidate_logs_queries().await;
                }
            }
        }
    });
}

fn is_boot_failure(line: &str) -> bool {
    line.contains("Failed to start the minecraft server")
}

fn is_wrong_side_line(line: &str) -> bool {
    line.contains("for invalid dist DEDICATED_SERVER")
        || line.contains("in environment type SERVER")
}

fn names_jar(line: &str, jar: &str) -> bool {
    line.match_indices(jar).any(|(at, _)| {
        line[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '+')))
    })
}

fn client_mod_culprit(lines: &[Arc<str>], jars: &[String]) -> Option<String> {
    let start = lines.iter().rposition(|line| is_wrong_side_line(line))?;
    lines[start..]
        .iter()
        .take(CRASH_TRACE_LINES)
        .find_map(|line| jars.iter().find(|jar| names_jar(line, jar)).cloned())
}

fn is_missing_client_class(line: &str) -> bool {
    (line.contains("NoClassDefFoundError") || line.contains("ClassNotFoundException"))
        && [
            "net/minecraft/client/",
            "net.minecraft.client.",
            "com/mojang/blaze3d/",
            "com.mojang.blaze3d.",
        ]
        .iter()
        .any(|client| line.contains(client))
}

fn missing_client_class_culprits(lines: &[Arc<str>], jars: &[String]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        if !is_missing_client_class(line) {
            continue;
        }
        let culprit = lines[at..]
            .iter()
            .take(CRASH_TRACE_LINES)
            .enumerate()
            .take_while(|(n, line)| *n == 0 || !(line.starts_with('[') || line.trim().is_empty()))
            .find_map(|(_, line)| jars.iter().find(|jar| names_jar(line, jar)));
        if let Some(jar) = culprit
            && !found.contains(jar)
        {
            found.push(jar.clone());
        }
    }
    found
}

async fn report_crash(
    station: Station,
    server: &LocalServer,
    lines: &[Arc<str>],
    booted: bool,
    code: i32,
) {
    let jars: Vec<String> = match server.dir() {
        Ok(dir) => {
            let folder = server.content_folder();
            off_ui(async move { servers::list_mods(&dir, folder).await })
                .await
                .into_iter()
                .filter(|m| m.enabled)
                .map(|m| m.file_name)
                .collect()
        }
        Err(_) => Vec::new(),
    };
    let mut culprits = missing_client_class_culprits(lines, &jars);
    if let Some(jar) = client_mod_culprit(lines, &jars)
        && !culprits.contains(&jar)
    {
        culprits.push(jar);
    }
    if !booted && server.auto_disable_client_mods && !culprits.is_empty() {
        let attempt = {
            let mut restarts = AUTO_RESTARTS.lock().unwrap();
            let attempt = restarts.entry(server.id.clone()).or_default();
            *attempt += 1;
            *attempt
        };
        if attempt <= MAX_AUTO_RESTARTS {
            let notice = tf(
                "oneclient.servers.client_mod.auto_body",
                &[&server.name, &culprits.join(", ")],
            );
            disable_mods_and_restart(station, server.id.clone(), culprits, Some(notice));
            return;
        }
        AUTO_RESTARTS.lock().unwrap().remove(&server.id);
    }
    match (culprits.into_iter().next(), actions()) {
        (Some(file_name), Some(actions)) => actions
            .notify(t("oneclient.servers.client_mod.title"))
            .body(tf(
                "oneclient.servers.client_mod.body",
                &[&server.name, &file_name],
            ))
            .error()
            .action(NotificationAction {
                label: t("oneclient.servers.client_mod.disable").to_string(),
                kind: NotificationActionKind::DisableServerMod {
                    server_id: server.id.clone(),
                    file_name,
                },
            })
            .send(),
        _ => notify_error(
            t("oneclient.servers.crashed"),
            format!("{} ({code})", server.name),
        ),
    }
}

fn stopped_or_warn(id: &str) -> bool {
    let Some(actions) = actions() else {
        return false;
    };
    if actions.station().peek().servers.status(id) != ServerStatus::Stopped {
        actions
            .notify(t("oneclient.servers.settings.stop_first"))
            .toast_only()
            .send();
        return false;
    }
    true
}

pub fn set_mod_enabled(id: String, file_name: String, enabled: bool) {
    if !stopped_or_warn(&id) {
        return;
    }
    run(
        id.clone(),
        t("oneclient.servers.mods.toggle_failed"),
        async move {
            let _mods = hold(&MODS, &id).await;
            let server = servers::load_server(&id).await?;
            servers::set_mod_enabled(&server.dir()?, server.content_folder(), &file_name, enabled)
                .await?;
            Ok(())
        },
    );
}

pub fn disable_mods_and_restart(
    station: Station,
    id: String,
    file_names: Vec<String>,
    notice: Option<String>,
) {
    spawn_in_window(async move {
        let result = off_ui({
            let id = id.clone();
            async move {
                let _mods = hold(&MODS, &id).await;
                let server = servers::load_server(&id).await?;
                let dir = server.dir()?;
                for file_name in &file_names {
                    servers::set_mod_enabled(&dir, server.content_folder(), file_name, false)
                        .await?;
                }
                Ok::<_, oneclient_core::LauncherError>(())
            }
        })
        .await;
        invalidate_server(&id).await;
        match result {
            Ok(()) => {
                if let (Some(body), Some(actions)) = (notice, actions()) {
                    actions
                        .notify(t("oneclient.servers.client_mod.auto_title"))
                        .body(body)
                        .send();
                }
                start(station, id);
            }
            Err(err) => notify_error(t("oneclient.servers.mods.toggle_failed"), err),
        }
    });
}

async fn drive(
    id: String,
    mut child: tokio::process::Child,
    tx: mpsc::UnboundedSender<Output>,
    mut cmd_rx: mpsc::UnboundedReceiver<String>,
) {
    let mut readers = Vec::new();
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
        let (tx, id) = (tx.clone(), id.clone());
        readers.push(tokio::spawn(async move {
            // Java on Windows writes in the system codepage, so lines are decoded lossily
            // rather than letting one non-UTF-8 byte end the stream
            let mut reader = BufReader::new(reader);
            let mut buf = Vec::new();
            let mut listening = true;
            while matches!(reader.read_until(b'\n', &mut buf).await, Ok(n) if n > 0) {
                LAST_OUTPUT
                    .lock()
                    .unwrap()
                    .insert(id.clone(), Instant::now());
                let line = strip_ansi(String::from_utf8_lossy(&buf).trim_end_matches(['\r', '\n']));
                buf.clear();
                if listening {
                    listening = tx.send(Output::Line(line)).is_ok();
                }
            }
        }));
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
    let _ = tokio::time::timeout(Duration::from_secs(2), async move {
        for reader in readers {
            let _ = reader.await;
        }
    })
    .await;
    let _ = tx.send(Output::Exit(code));
}

pub fn stop(station: Station, id: String) {
    let live = station.peek().servers.get(&id);
    if !live.status.is_live() {
        return;
    }
    STOPPING.lock().unwrap().insert(id.clone());
    write(station, &id, |live| live.status = ServerStatus::Stopping);
    let sent = send_command(&id, "stop");
    let Some(armed) = live.pid else {
        return;
    };
    spawn_in_window(async move {
        let still_stopping = || {
            let now = station.peek().servers.get(&id);
            now.status == ServerStatus::Stopping && now.pid == Some(armed)
        };
        if sent {
            if !live.booted {
                let waiting = Instant::now();
                loop {
                    tokio::time::sleep(STOP_POLL).await;
                    if !still_stopping() {
                        return;
                    }
                    if station.peek().servers.get(&id).booted {
                        send_command(&id, "stop");
                        break;
                    }
                    if waiting.elapsed() >= BOOT_WAIT {
                        break;
                    }
                }
            }
            let asked = Instant::now();
            loop {
                tokio::time::sleep(STOP_POLL).await;
                if !still_stopping() {
                    return;
                }
                if kill_due(&id, armed, asked).await {
                    break;
                }
            }
        }
        oneclient_core::game::kill_process(armed);
        let killed = Instant::now();
        loop {
            tokio::time::sleep(STOP_POLL).await;
            if !still_stopping() {
                return;
            }
            if killed.elapsed() >= FORCE_AFTER {
                crate::launcher::off_ui_blocking(move || force_kill(armed)).await;
                return;
            }
        }
    });
}

pub fn is_restarting(id: &str) -> bool {
    RESTARTING.lock().unwrap().contains(id)
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
        let _turn = hold(&BACKUPS, &id).await;
        let saved = saved_signal(&id);
        let confirmed = saved.notified();
        tokio::pin!(confirmed);
        confirmed.as_mut().enable();
        let running = send_command(&id, "save-off");
        let flushed = !running || {
            send_command(&id, "save-all flush");
            tokio::time::timeout(SAVE_WAIT, confirmed).await.is_ok()
        };
        let result = if flushed {
            off_ui({
                let id = id.clone();
                async move {
                    let server = servers::load_server(&id).await?;
                    servers::create_backup(&server.dir()?, server.backups_kept).await
                }
            })
            .await
        } else {
            Err(oneclient_core::LauncherError::Server(
                "the server did not confirm saving the world in time, no backup was made".into(),
            ))
        };
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
        invalidate_server(&id).await;
    });
}

async fn save_install_log(id: &str, text: String) {
    if text.is_empty() {
        return;
    }
    let id = id.to_string();
    let name = format!(
        "install-{}.log",
        chrono::Local::now().format("%Y-%m-%d_%H-%M-%S")
    );
    let saved = off_ui(async move {
        let dir = servers::server_dir(&id)?.join("logs");
        tokio::fs::create_dir_all(&dir).await?;
        tokio::fs::write(dir.join(name), text).await?;
        Ok::<_, oneclient_core::LauncherError>(())
    })
    .await;
    if let Err(err) = saved {
        tracing::warn!(%err, "could not save the server install log");
    }
    crate::hooks::invalidate_logs_queries().await;
}

/// Installs in the background, the server shows as installing until it finishes
pub fn install(station: Station, mut server: LocalServer, prepare: Prepare, port: Option<u16>) {
    let id = server.id.clone();
    let name = server.name.clone();
    write(station, &id, |live| {
        live.status = ServerStatus::Installing;
        live.lines = Arc::default();
    });
    let (log_tx, mut log_rx) = mpsc::unbounded_channel::<String>();
    spawn_in_window({
        let id = id.clone();
        async move {
            let mut saved = String::new();
            while let Some(line) = log_rx.recv().await {
                let line = strip_ansi(&line);
                saved.push_str(&line);
                saved.push('\n');
                write_lines(station, &id, vec![line]);
            }
            save_install_log(&id, saved).await;
        }
    });
    spawn_in_window(async move {
        let files = hold(&FILES, &id).await;
        let listed = off_ui({
            let server = server.clone();
            async move { servers::save_server(&server).await }
        })
        .await;
        invalidate_server(&id).await;
        let failed_tx = log_tx.clone();
        let mods_id = id.clone();
        let result = off_ui(async move {
            let _files = files;
            let progress = move |line: String| {
                let _ = log_tx.send(line);
            };
            let step = |message: String| progress(servers::step_line("INFO", message));
            listed?;
            let state = launcher::state()?;
            let dir = server.dir()?;
            let mut blocked = Vec::new();
            match prepare {
                Prepare::None => {}
                Prepare::Instance(cluster_id, game_dir) => {
                    let _mods = hold(&MODS, &mods_id).await;
                    step(format!("Copying mods from {}", game_dir.display()));
                    let (copied, skipped) =
                        servers::copy_from_instance(&state, cluster_id, &game_dir, &dir).await?;
                    step(format!(
                        "Copied {copied} mods, left out {skipped} client-only mods"
                    ));
                }
                Prepare::Modpack(pack) => {
                    blocked = servers::unpack_pack(&state, &pack, &dir, &progress).await?;
                }
                Prepare::Import(source) => {
                    if dir.starts_with(&source) {
                        return Err(oneclient_core::LauncherError::Server(
                            "the imported folder contains the OneClient servers folder".into(),
                        ));
                    }
                    step(format!("Copying {}", source.display()));
                    polyio::copy_dir(&source, &dir, &[servers::PID_FILE]).await?;
                }
            }
            if !server.prepared {
                server.prepared = true;
                servers::save_server(&server).await?;
            }
            if let Some(port) = port {
                let mut props = servers::Properties::load(&dir).await?;
                props.set("server-port", &port.to_string());
                props.write(&dir).await?;
            }
            servers::install_server(&state, &mut server, &progress)
                .await
                .map(|()| blocked)
        })
        .await;
        write(station, &id, |live| live.status = ServerStatus::Stopped);
        if let (Ok(files), Some(actions)) = (&result, actions())
            && !files.is_empty()
        {
            actions.open_blocked_downloads(crate::notifications::BlockedDownloads {
                cluster_id: crate::hooks::UNCREATED_CLUSTER,
                server_id: Some(id.clone()),
                cluster_name: name,
                bundle_name: String::new(),
                files: files.clone(),
                added: Default::default(),
                open_when_done: false,
            });
        }
        if let Err(err) = result {
            let _ = failed_tx.send(servers::step_line(
                "ERROR",
                format!("Install failed: {err}"),
            ));
            notify_error(t("oneclient.servers.install_failed"), err);
        }
        invalidate_server(&id).await;
    });
}

pub fn retry_install(station: Station, id: String) {
    if station.peek().servers.status(&id) != ServerStatus::Stopped {
        return;
    }
    write(station, &id, |live| live.status = ServerStatus::Installing);
    spawn_in_window(async move {
        let loaded = off_ui({
            let id = id.clone();
            async move { servers::load_server(&id).await }
        })
        .await;
        match loaded {
            Ok(server) => {
                let prepare = Prepare::redo(&server);
                install(station, server, prepare, None);
            }
            Err(err) => {
                write(station, &id, |live| live.status = ServerStatus::Stopped);
                notify_error(t("oneclient.servers.install_failed"), err);
            }
        }
    });
}

pub fn update_software(station: Station, id: String, mc_version: Option<String>) {
    if station.peek().servers.status(&id) != ServerStatus::Stopped {
        return;
    }
    write(station, &id, |live| live.status = ServerStatus::Installing);
    spawn_in_window(async move {
        let prepared = off_ui({
            let id = id.clone();
            async move {
                let _files = hold(&FILES, &id).await;
                let mut server = servers::load_server(&id).await?;
                let dir = server.dir()?;
                let mut moved = false;
                if let Some(version) = mc_version.filter(|v| *v != server.mc_version) {
                    let world = dir.join(&servers::world_folders(&dir).await?[0]);
                    if tokio::fs::try_exists(&world).await.unwrap_or(false) {
                        let _backups = hold(&BACKUPS, &id).await;
                        servers::create_backup(&dir, server.backups_kept).await?;
                    }
                    server.mc_version = version;
                    server.launch.clear();
                    moved = true;
                }
                server.loader_version = None;
                server.java_major = None;
                servers::save_server(&server).await?;
                let disabled = if moved {
                    let _mods = hold(&MODS, &id).await;
                    servers::migrate_mods(&*launcher::state()?, &server).await
                } else {
                    Ok(Vec::new())
                };
                Ok::<_, oneclient_core::LauncherError>((server, disabled))
            }
        })
        .await;
        match prepared {
            Ok((server, disabled)) => {
                match disabled {
                    Ok(disabled) if !disabled.is_empty() => {
                        if let Some(actions) = actions() {
                            actions
                                .notify(t("oneclient.servers.settings.version.mods_disabled.title"))
                                .body(tf(
                                    "oneclient.servers.settings.version.mods_disabled.body",
                                    &[&disabled.join(", ")],
                                ))
                                .send();
                        }
                    }
                    Ok(_) => {}
                    Err(err) => notify_error(t("oneclient.servers.settings.version.mods_failed"), err),
                }
                install(station, server, Prepare::None, None);
            }
            Err(err) => {
                write(station, &id, |live| live.status = ServerStatus::Stopped);
                notify_error(t("oneclient.servers.install_failed"), err);
            }
        }
    });
}

/// Swaps the jar the server starts with for a picked one, then reinstalls around it
pub fn use_custom_jar(station: Station, id: String, jar: std::path::PathBuf) {
    if station.peek().servers.status(&id) != ServerStatus::Stopped {
        return;
    }
    write(station, &id, |live| live.status = ServerStatus::Installing);
    spawn_in_window(async move {
        let prepared = off_ui({
            let id = id.clone();
            async move {
                let _files = hold(&FILES, &id).await;
                ensure_stopped(&id)?;
                let mut server = servers::load_server(&id).await?;
                servers::use_custom_jar(&mut server, &jar).await?;
                servers::save_server(&server).await?;
                Ok::<_, oneclient_core::LauncherError>(server)
            }
        })
        .await;
        match prepared {
            Ok(server) => {
                let prepare = Prepare::redo(&server);
                install(station, server, prepare, None);
            }
            Err(err) => {
                write(station, &id, |live| live.status = ServerStatus::Stopped);
                notify_error(t("oneclient.servers.settings.jar.failed"), err);
                invalidate_server(&id).await;
            }
        }
    });
}

pub fn delete_server(id: String) {
    if !stopped_or_warn(&id) {
        return;
    }
    spawn_in_window(async move {
        let result = off_ui({
            let id = id.clone();
            async move {
                let state = launcher::state()?;
                let name = servers::load_server(&id)
                    .await
                    .map_or_else(|_| id.clone(), |server| server.name);
                let events = &state.services.events;
                let progress_id = uuid::Uuid::new_v4();
                events.progress(
                    progress_id,
                    tf("oneclient.servers.delete.progress", &[&name]),
                    0,
                    0,
                );
                let _files = hold(&FILES, &id).await;
                let deleted = servers::delete_server(&id).await;
                match &deleted {
                    Ok(()) => events.finish_progress(
                        progress_id,
                        t("oneclient.servers.delete.done"),
                        name,
                    ),
                    Err(err) => events.fail_progress(
                        progress_id,
                        t("oneclient.servers.settings.delete.failed"),
                        err.to_string(),
                    ),
                }
                deleted
            }
        })
        .await;
        match result {
            Ok(()) => {
                if let Some(actions) = actions() {
                    actions
                        .station()
                        .write_channel(AppChannel::Servers)
                        .servers
                        .forget(&id);
                }
                LAST_OUTPUT.lock().unwrap().remove(&id);
            }
            Err(err) => tracing::warn!(%err, "could not delete server {id}"),
        }
        invalidate_server(&id).await;
    });
}

impl Prepare {
    fn redo(server: &LocalServer) -> Self {
        if server.prepared {
            return Self::None;
        }
        match (server.kind, server.source_path.clone()) {
            (servers::ServerKind::FromInstance, Some(dir)) => server
                .source_cluster
                .map_or(Self::None, |cluster| Self::Instance(cluster, dir)),
            (servers::ServerKind::Modpack, Some(pack)) => Self::Modpack(pack),
            (servers::ServerKind::Import, Some(folder)) => Self::Import(folder),
            _ => Self::None,
        }
    }
}

pub enum Prepare {
    None,
    Instance(i64, std::path::PathBuf),
    Modpack(std::path::PathBuf),
    Import(std::path::PathBuf),
}

pub fn spawn_autostart(station: Station) {
    spawn_in_window(async move {
        let listed = off_ui(async {
            let listed = servers::list_servers().await.unwrap_or_default();
            for server in &listed {
                if let Ok(dir) = server.dir()
                    && let Ok(true) = servers::kill_orphan(&dir).await
                {
                    tracing::warn!(
                        "stopped server {} left running by an earlier launcher session",
                        server.id
                    );
                }
            }
            listed
        })
        .await;
        for server in listed.into_iter().filter(|s| s.autostart && s.installed()) {
            start(station, server.id);
        }
    });
}

/// Saves a change to the server's own settings
pub fn update(id: String, change: impl FnOnce(&mut LocalServer) + Send + 'static) {
    let saving = crate::hooks::SaveGuard::new();
    spawn_in_window(async move {
        let _saving = saving;
        let result = off_ui({
            let id = id.clone();
            async move {
                let _files = hold(&FILES, &id).await;
                let mut server = servers::load_server(&id).await?;
                change(&mut server);
                servers::save_server(&server).await?;
                if let Some(hours) = BACKUP_HOURS.lock().unwrap().get_mut(&id) {
                    *hours = server.backup_hours;
                }
                Ok::<_, oneclient_core::LauncherError>(())
            }
        })
        .await;
        if let Err(err) = result {
            notify_error(t("oneclient.servers.save_failed"), err);
        }
        invalidate_server_settings(&id).await;
    });
}

pub fn set_property(id: String, key: &'static str, value: String) {
    let saving = crate::hooks::SaveGuard::new();
    spawn_in_window(async move {
        let _saving = saving;
        let result = off_ui({
            let id = id.clone();
            async move {
                let _files = hold(&FILES, &id).await;
                let dir = servers::server_dir(&id)?;
                let mut props = servers::Properties::load(&dir).await?;
                props.set(key, &value);
                props.write(&dir).await?;
                Ok::<_, oneclient_core::LauncherError>(())
            }
        })
        .await;
        if let Err(err) = result {
            notify_error(t("oneclient.servers.save_failed"), err);
        }
        invalidate_server_settings(&id).await;
    });
}

pub fn load_properties(id: String, mut editing: freya::prelude::State<Option<String>>) {
    use freya::prelude::*;
    spawn_in_window(async move {
        let loaded = off_ui(async move {
            let dir = servers::server_dir(&id)?;
            let props = servers::Properties::load(&dir).await?;
            Ok::<_, oneclient_core::LauncherError>(props.to_string())
        })
        .await;
        match loaded {
            Ok(text) => editing.set(Some(text)),
            Err(err) => notify_error(t("oneclient.servers.settings.properties.load_failed"), err),
        }
    });
}

pub fn write_properties(id: String, text: String) {
    let saving = crate::hooks::SaveGuard::new();
    spawn_in_window(async move {
        let _saving = saving;
        let result = off_ui({
            let id = id.clone();
            async move {
                let _files = hold(&FILES, &id).await;
                let dir = servers::server_dir(&id)?;
                let mut props = servers::Properties::load(&dir).await?;
                props.replace_text(&text);
                props.write(&dir).await?;
                Ok::<_, oneclient_core::LauncherError>(())
            }
        })
        .await;
        if let Err(err) = result {
            notify_error(t("oneclient.servers.save_failed"), err);
        }
        invalidate_server_settings(&id).await;
    });
}

/// Runs a file operation in the background and refreshes the server views after
pub fn run<F>(id: String, failure: &'static str, work: F)
where
    F: Future<Output = oneclient_core::LauncherResult<()>> + Send + 'static,
{
    spawn_in_window(async move {
        if let Err(err) = off_ui(work).await {
            notify_error(failure, err);
        }
        invalidate_server(&id).await;
    });
}

/// Start registers its process under the file lock, so checking after taking it closes the race
fn ensure_stopped(id: &str) -> oneclient_core::LauncherResult<()> {
    if PIDS.lock().unwrap().contains_key(id) {
        return Err(oneclient_core::LauncherError::Server(
            "the server is running, stop it first".into(),
        ));
    }
    Ok(())
}

pub fn reset_world(id: String, keep: u32) {
    run(
        id.clone(),
        t("oneclient.servers.settings.reset.failed"),
        async move {
            let _files = hold(&FILES, &id).await;
            let _backups = hold(&BACKUPS, &id).await;
            ensure_stopped(&id)?;
            servers::reset_world(&servers::server_dir(&id)?, keep).await
        },
    );
}

pub fn restore_backup(id: String, backup: std::path::PathBuf) {
    run(
        id.clone(),
        t("oneclient.servers.backups.restore_failed"),
        async move {
            let _files = hold(&FILES, &id).await;
            let _backups = hold(&BACKUPS, &id).await;
            ensure_stopped(&id)?;
            let server = servers::load_server(&id).await?;
            servers::restore_backup(&server.dir()?, &backup, server.backups_kept).await
        },
    );
}

pub fn identify_mods(id: String) {
    spawn_in_window(async move {
        let result = off_ui({
            let id = id.clone();
            async move {
                let _mods = hold(&MODS, &id).await;
                let state = launcher::state()?;
                let server = servers::load_server(&id).await?;
                servers::identify_mods(&state, &server.dir()?, server.content_folder()).await
            }
        })
        .await;
        match result {
            Ok(true) => invalidate_server(&id).await,
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
    if !stopped_or_warn(&id) {
        return;
    }
    write(station, &id, |live| {
        live.installing.push(project_id.clone())
    });
    spawn_in_window(async move {
        let result = off_ui({
            let (id, project_id) = (id.clone(), project_id.clone());
            async move {
                let _mods = hold(&MODS, &id).await;
                let state = launcher::state()?;
                let server = servers::load_server(&id).await?;
                servers::install_mod(&state, &server, provider, &project_id, &version_id).await
            }
        })
        .await;
        write(station, &id, |live| {
            live.installing.retain(|p| *p != project_id)
        });
        if let Err(err) = result {
            notify_error(t("oneclient.servers.mods.install_failed"), err);
        }
        invalidate_server(&id).await;
    });
}

fn refresh_after_command(id: String) {
    spawn_in_window(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        invalidate_server_settings(&id).await;
    });
}

/// Uses the console while the server runs so it applies at once, edits the file otherwise
pub fn add_player(id: String, name: String, op: bool) {
    let command = if op { "op" } else { "whitelist add" };
    if send_command(&id, &format!("{command} {name}")) {
        refresh_after_command(id);
        return;
    }
    run(
        id.clone(),
        t("oneclient.servers.players.add_failed"),
        async move {
            let state = launcher::state()?;
            let mc_version = servers::load_server(&id).await?.mc_version;
            let dir = servers::server_dir(&id)?;
            let online = servers::Properties::read(&dir)
                .await
                .get_or("online-mode", "true")
                .trim()
                != "false";
            let (uuid, name) = if online {
                oneclient_mc::resolve_username(&state.services.requester, &name)
                    .await
                    .map_err(|e| oneclient_core::LauncherError::Server(e.to_string()))?
            } else {
                (
                    oneclient_auth::offline_uuid(&name).simple().to_string(),
                    name,
                )
            };
            let _files = hold(&FILES, &id).await;
            servers::add_to_list(
                &servers::server_dir(&id)?,
                servers::list_file(&mc_version, op),
                servers::ListedPlayer::new(&uuid, name, op),
            )
            .await
        },
    );
}

pub fn remove_player(id: String, name: String, op: bool) {
    let command = if op { "deop" } else { "whitelist remove" };
    if send_command(&id, &format!("{command} {name}")) {
        refresh_after_command(id);
        return;
    }
    run(
        id.clone(),
        t("oneclient.servers.players.remove_failed"),
        async move {
            let mc_version = servers::load_server(&id).await?.mc_version;
            let _files = hold(&FILES, &id).await;
            servers::remove_from_list(
                &servers::server_dir(&id)?,
                servers::list_file(&mc_version, op),
                &name,
            )
            .await
        },
    );
}

#[cfg(test)]
mod tests {
    use super::{
        client_mod_culprit, is_boot_failure, missing_client_class_culprits, player_event,
        player_uuid,
    };

    #[test]
    fn finds_every_mod_that_needs_a_client_class() {
        let lines: Vec<std::sync::Arc<str>> = [
            "[15:57:09] [modloading-worker-0/ERROR]: Failed to create mod instance. ModID: citresewn, class shcm.shsupercm.forge.citresewn.CITResewn",
            "java.lang.NoClassDefFoundError: net/minecraft/client/renderer/texture/atlas/SpriteSource",
            "\tat shcm.shsupercm.forge.citresewn.CITResewn.<clinit>(CITResewn.java:21) ~[citresewn-1.20.1-5.jar%23487!/:1.20.1-5]",
            "\tat com.example.lib.Init.run(Init.java:3) ~[examplelib-1.0.jar%23189!/:?]",
            "Caused by: java.lang.ClassNotFoundException: net.minecraft.client.renderer.texture.atlas.SpriteSource",
            "[15:57:09] [modloading-worker-0/INFO]: Hybrid Aquatic loaded.",
            "[15:57:09] [modloading-worker-0/ERROR]: Failed to create mod instance. ModID: stop_rendering, class com.corosus.stop_rendering.loader.forge.StopRenderingForge",
            "java.lang.NoClassDefFoundError: net/minecraft/client/renderer/entity/layers/RenderLayer",
            "\tat net.minecraftforge.eventbus.EventBus.registerObject(EventBus.java:80) ~[eventbus-6.2.33.jar%2352!/:?]",
            "\tat com.corosus.stop_rendering.loader.forge.StopRenderingForge.<init>(StopRenderingForge.java:32) ~[stop_rendering-forge-1.0.3.jar%23699!/:1.0.3]",
            "[15:57:10] [modloading-worker-0/INFO]: OldCannons loading... hybrid_aquatic-forge-1.20.1-1.7.4.jar",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        let jars = [
            "citresewn-1.20.1-5.jar".to_string(),
            "examplelib-1.0.jar".to_string(),
            "stop_rendering-forge-1.0.3.jar".to_string(),
            "hybrid_aquatic-forge-1.20.1-1.7.4.jar".to_string(),
        ];
        assert_eq!(
            missing_client_class_culprits(&lines, &jars),
            ["citresewn-1.20.1-5.jar", "stop_rendering-forge-1.0.3.jar"]
        );
        assert!(missing_client_class_culprits(&lines[5..7], &jars).is_empty());
    }

    #[test]
    fn spots_the_line_vanilla_logs_when_boot_fails() {
        assert!(is_boot_failure(
            "[10:12:51] [main/ERROR]: Failed to start the minecraft server"
        ));
        assert!(!is_boot_failure(
            "[10:12:51] [Server thread/INFO]: Starting minecraft server version 1.20.1"
        ));
    }

    #[test]
    fn finds_the_client_only_mod_in_a_wrong_side_crash() {
        let lines: Vec<std::sync::Arc<str>> = [
            "[main/INFO]: Loading 3 mods",
            "java.lang.RuntimeException: Attempted to load class net/minecraft/client/gui/screens/Screen for invalid dist DEDICATED_SERVER",
            "\tat net.minecraftforge.fml.loading.RuntimeDistCleaner.processClassWithFlags(RuntimeDistCleaner.java:57) ~[fmlloader-1.20.1-47.2.0.jar%23101!/:1.0]",
            "\tat com.example.minimap.Hud.<init>(Hud.java:12) ~[xminimap-1.0.jar%23190!/:?]",
            "\tat com.example.lib.Init.run(Init.java:3) ~[minimap-1.0.jar%23189!/:?]",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        let jars = [
            "minimap-1.0.jar".to_string(),
            "xminimap-1.0.jar".to_string(),
        ];
        assert_eq!(
            client_mod_culprit(&lines, &jars).as_deref(),
            Some("xminimap-1.0.jar")
        );
        assert_eq!(client_mod_culprit(&lines[..1], &jars), None);
    }

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
        let paper = super::strip_ansi(
            "[11:47:54 INFO]: System chat: \u{1b}[38;2;255;255;85mProbeBot joined the game\u{1b}[0m",
        );
        assert_eq!(
            paper,
            "[11:47:54 INFO]: System chat: ProbeBot joined the game"
        );
        assert_eq!(player_event(&paper), Some((true, "ProbeBot".into())));
        assert_eq!(super::strip_ansi("plain \u{1b}"), "plain ");
        assert_eq!(
            player_event("[11:39:52 INFO]: System chat: rozwader joined the game"),
            Some((true, "rozwader".into()))
        );
        assert_eq!(
            player_uuid(
                "[11:39:52 INFO]: UUID of player rozwader is 20961f1a-99fb-4eca-bebd-78b528defb28"
            ),
            Some(("rozwader".into(), "20961f1a99fb4ecabebd78b528defb28".into()))
        );
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
