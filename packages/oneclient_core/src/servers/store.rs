use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use oneclient_common::GameLoader;
use oneclient_common::paths;
use serde::{Deserialize, Serialize};

use crate::{LauncherError, LauncherResult};

const META_FILE: &str = "oneclient-server.json";
pub const BACKUPS_DIR: &str = "backups";
pub const DEFAULT_PORT: u16 = 25565;
pub const DEFAULT_MEMORY_MB: u32 = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ServerKind {
    FromInstance,
    #[default]
    Scratch,
    Modpack,
    Import,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    #[default]
    Loader,
    Paper,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalServer {
    /// The folder name under the servers dir
    #[serde(skip)]
    pub id: String,
    pub name: String,
    pub kind: ServerKind,
    pub mc_version: String,
    pub loader: GameLoader,
    #[serde(default)]
    pub platform: Platform,
    #[serde(default)]
    pub loader_version: Option<String>,
    pub memory_mb: u32,
    #[serde(default)]
    pub jvm_args: String,
    #[serde(default)]
    pub java_major: Option<u32>,
    #[serde(default)]
    pub java_path: Option<PathBuf>,
    #[serde(default)]
    pub autostart: bool,
    /// Hours between automatic backups, zero turns them off
    #[serde(default)]
    pub backup_hours: u32,
    #[serde(default = "default_kept")]
    pub backups_kept: u32,
    #[serde(default)]
    pub eula: bool,
    #[serde(default)]
    pub source_cluster: Option<i64>,
    #[serde(default)]
    pub source_path: Option<PathBuf>,
    #[serde(default)]
    pub prepared: bool,
    /// What follows the JVM flags on the command line, set once the server is installed
    #[serde(default)]
    pub launch: Vec<String>,
    pub created_at: DateTime<Utc>,
}

fn default_kept() -> u32 {
    10
}

impl LocalServer {
    #[must_use]
    pub fn new(name: String, kind: ServerKind, mc_version: String, loader: GameLoader) -> Self {
        Self {
            id: String::new(),
            name,
            kind,
            mc_version,
            loader,
            platform: Platform::Loader,
            loader_version: None,
            memory_mb: DEFAULT_MEMORY_MB,
            jvm_args: String::new(),
            java_major: None,
            java_path: None,
            autostart: false,
            backup_hours: 6,
            backups_kept: default_kept(),
            eula: false,
            source_cluster: None,
            source_path: None,
            prepared: false,
            launch: Vec::new(),
            created_at: Utc::now(),
        }
    }

    pub fn dir(&self) -> LauncherResult<PathBuf> {
        server_dir(&self.id)
    }

    #[must_use]
    pub fn installed(&self) -> bool {
        !self.launch.is_empty()
    }

    #[must_use]
    pub fn plugins(&self) -> bool {
        self.platform != Platform::Loader
    }

    #[must_use]
    pub fn takes_content(&self) -> bool {
        self.plugins() || self.loader.is_modded()
    }

    #[must_use]
    pub fn content_folder(&self) -> &'static str {
        if self.plugins() { "plugins" } else { "mods" }
    }

    #[must_use]
    pub fn software_name(&self) -> String {
        match self.platform {
            Platform::Loader => self.loader.to_string(),
            Platform::Paper => "Paper".to_string(),
        }
    }
}

pub fn servers_root() -> LauncherResult<PathBuf> {
    Ok(paths::servers_dir()?)
}

pub fn server_dir(id: &str) -> LauncherResult<PathBuf> {
    if id.is_empty() || id.contains(['/', '\\']) || id.starts_with('.') {
        return Err(LauncherError::Server(format!("bad server id {id:?}")));
    }
    Ok(servers_root()?.join(id))
}

pub async fn list_servers() -> LauncherResult<Vec<LocalServer>> {
    let root = servers_root()?;
    let mut servers = Vec::new();
    let Ok(mut entries) = tokio::fs::read_dir(&root).await else {
        return Ok(servers);
    };
    while let Some(entry) = entries.next_entry().await? {
        let id = entry.file_name().to_string_lossy().to_string();
        match read_meta(&entry.path(), &id).await {
            Ok(Some(server)) => servers.push(server),
            Ok(None) => {}
            Err(err) => tracing::warn!("skipping server {id}: {err}"),
        }
    }
    servers.sort_by_key(|s| std::cmp::Reverse(s.created_at));
    Ok(servers)
}

pub async fn load_server(id: &str) -> LauncherResult<LocalServer> {
    read_meta(&server_dir(id)?, id)
        .await?
        .ok_or_else(|| LauncherError::Server(format!("server {id} not found")))
}

async fn read_meta(dir: &Path, id: &str) -> LauncherResult<Option<LocalServer>> {
    let Ok(text) = tokio::fs::read_to_string(dir.join(META_FILE)).await else {
        return Ok(None);
    };
    let mut server: LocalServer = serde_json::from_str(&text)?;
    server.id = id.to_string();
    Ok(Some(server))
}

pub async fn save_server(server: &LocalServer) -> LauncherResult<()> {
    let dir = server.dir()?;
    tokio::fs::create_dir_all(&dir).await?;
    let tmp = dir.join(format!("{META_FILE}.tmp"));
    tokio::fs::write(&tmp, serde_json::to_vec_pretty(server)?).await?;
    tokio::fs::rename(&tmp, dir.join(META_FILE)).await?;
    Ok(())
}

/// Reserves a fresh folder named after the server
pub async fn allocate_id(name: &str) -> LauncherResult<String> {
    let root = servers_root()?;
    tokio::fs::create_dir_all(&root).await?;
    let base = slug(name);
    let mut id = base.clone();
    let mut n = 2;
    while tokio::fs::try_exists(root.join(&id)).await? {
        id = format!("{base}-{n}");
        n += 1;
    }
    tokio::fs::create_dir_all(root.join(&id)).await?;
    Ok(id)
}

fn raw_slug(name: &str) -> String {
    let mut out = String::new();
    for ch in name.trim().to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() {
        "server".to_string()
    } else {
        out
    }
}

fn slug(name: &str) -> String {
    let out = raw_slug(name);
    let device = |prefix: &str| {
        out.strip_prefix(prefix)
            .is_some_and(|n| n.len() == 1 && n.as_bytes()[0].is_ascii_digit() && n != "0")
    };
    if matches!(out.as_str(), "con" | "prn" | "aux" | "nul") || device("com") || device("lpt") {
        format!("{out}-server")
    } else {
        out
    }
}

pub async fn free_port() -> u16 {
    let mut taken = std::collections::HashSet::new();
    for server in list_servers().await.unwrap_or_default() {
        if let Ok(dir) = server.dir() {
            taken.insert(
                super::Properties::read(&dir)
                    .await
                    .get("server-port")
                    .and_then(|port| port.trim().parse().ok())
                    .unwrap_or(DEFAULT_PORT),
            );
        }
    }
    next_free_port(DEFAULT_PORT, &taken, |port| {
        std::net::TcpListener::bind(("0.0.0.0", port)).is_ok()
    })
}

fn next_free_port(
    start: u16,
    taken: &std::collections::HashSet<u16>,
    bindable: impl Fn(u16) -> bool,
) -> u16 {
    (start..=u16::MAX)
        .find(|port| !taken.contains(port) && bindable(*port))
        .unwrap_or(start)
}

pub async fn delete_server(id: &str) -> LauncherResult<()> {
    let dir = server_dir(id)?;
    if !tokio::fs::try_exists(&dir).await? {
        return Ok(());
    }
    let trashed = tokio::task::spawn_blocking({
        let dir = dir.clone();
        move || trash::delete(&dir)
    })
    .await;
    match trashed {
        Ok(Ok(())) => Ok(()),
        Ok(Err(err)) => {
            tracing::warn!(%err, "the recycle bin refused server {id}, deleting it for good");
            Ok(tokio::fs::remove_dir_all(&dir).await?)
        }
        Err(err) => Err(LauncherError::Server(err.to_string())),
    }
}

pub const PID_FILE: &str = ".oneclient-server.pid";

/// Remembers the running process so a launcher that crashed can find it again
pub async fn write_pid(dir: &Path, pid: u32) {
    let started = crate::game::process_start_time(pid).unwrap_or_default();
    let _ = tokio::fs::write(dir.join(PID_FILE), format!("{pid} {started}")).await;
}

pub async fn clear_pid(dir: &Path) {
    let _ = tokio::fs::remove_file(dir.join(PID_FILE)).await;
}

/// Kills a server left running by an earlier launcher session. Nothing can reach its
/// console any more and it holds the world lock, so a new start would fail otherwise.
/// Returns whether one was found
pub async fn kill_orphan(dir: &Path) -> LauncherResult<bool> {
    let Ok(text) = tokio::fs::read_to_string(dir.join(PID_FILE)).await else {
        return Ok(false);
    };
    let mut parts = text
        .split_whitespace()
        .filter_map(|p| p.parse::<u64>().ok());
    let (Some(pid), Some(started)) = (parts.next(), parts.next()) else {
        clear_pid(dir).await;
        return Ok(false);
    };
    let pid = pid as u32;
    // The start time tells our server apart from an unrelated process that reused the pid
    let ours = |pid: u32| {
        if started > 0 {
            crate::game::is_process_alive(pid, Some(started))
        } else {
            java_running_in(pid, dir)
        }
    };
    if !ours(pid) {
        clear_pid(dir).await;
        return Ok(false);
    }
    crate::game::kill_process(pid);
    for _ in 0..50 {
        if !ours(pid) {
            clear_pid(dir).await;
            return Ok(true);
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    Err(LauncherError::Server(format!(
        "a server left running by an earlier launcher session (process {pid}) could not be stopped"
    )))
}

fn java_running_in(pid: u32, dir: &Path) -> bool {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let pid = sysinfo::Pid::from_u32(pid);
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().with_cwd(UpdateKind::Always),
    );
    system.process(pid).is_some_and(|process| {
        process
            .name()
            .to_string_lossy()
            .to_lowercase()
            .starts_with("java")
            && process.cwd().is_some_and(|cwd| same_dir(cwd, dir))
    })
}

fn same_dir(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| {
        let text = p.to_string_lossy();
        let text = text.trim_end_matches(['/', '\\']);
        if cfg!(windows) {
            text.replace('/', "\\").to_lowercase()
        } else {
            text.to_string()
        }
    };
    norm(a) == norm(b)
}

pub async fn accept_eula(dir: &Path) -> LauncherResult<()> {
    tokio::fs::write(
        dir.join("eula.txt"),
        "# Accepted in OneClient, see https://aka.ms/MinecraftEULA\neula=true\n",
    )
    .await?;
    Ok(())
}

pub async fn dir_size(dir: PathBuf) -> u64 {
    tokio::task::spawn_blocking(move || dir_size_blocking(&dir))
        .await
        .unwrap_or(0)
}

fn dir_size_blocking(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => dir_size_blocking(&entry.path()),
            Ok(_) => entry.metadata().map(|m| m.len()).unwrap_or(0),
            Err(_) => 0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::{next_free_port, slug};

    #[test]
    fn next_free_port_skips_ports_of_other_servers_and_bound_ones() {
        let taken = [25565, 25566].into_iter().collect();
        assert_eq!(next_free_port(25565, &taken, |p| p != 25567), 25568);
        assert_eq!(next_free_port(25565, &Default::default(), |_| true), 25565);
    }

    #[test]
    fn slug_makes_a_safe_folder_name() {
        assert_eq!(slug("ever SMP"), "ever-smp");
        assert_eq!(slug("  --My  Server!! "), "my-server");
        assert_eq!(slug("§§"), "server");
        assert_eq!(slug("CON"), "con-server");
        assert_eq!(slug("lpt3"), "lpt3-server");
        assert_eq!(slug("com10"), "com10");
        assert_eq!(slug("console"), "console");
    }
}
