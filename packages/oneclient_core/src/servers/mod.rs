mod backups;
mod install;
mod mods;
mod players;
mod properties;
mod store;

use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use backups::{
    BackupInfo, create_backup, list_backups, reset_world, restore_backup, world_folders,
    world_name,
};
pub use install::{
    ImportGuess, PackInfo, Progress, copy_from_instance, detect_import, download_pack,
    install_server, java_for, pack_icon, platform_available, read_mrpack, step_line,
    supports_loader, unpack_mrpack,
};
pub use mods::{
    ModSource, ModUpdate, ServerMod, check_mod_updates, identify_mods, install_mod, list_mods,
    migrate_mods, plugin_builds, record_sources, set_mod_enabled,
};
pub use players::{
    ListedPlayer, add_to_list, ensure_lists, list_file, read_list, remove_from_list,
};
pub use properties::Properties;
pub use store::{
    DEFAULT_MEMORY_MB, DEFAULT_PORT, LocalServer, PID_FILE, Platform, ServerKind, accept_eula,
    allocate_id, clear_pid, delete_server, free_port, kill_orphan, list_servers, load_server,
    save_server, server_dir, servers_root, write_pid,
};
pub use crate::storage::dir_size;

use crate::state::LauncherState;
use crate::{LauncherError, LauncherResult};

pub const ICON_FILE: &str = "server-icon.png";

/// The full `java ...` command, stdio piped so the console can read and type into it
pub async fn server_command(
    state: &Arc<LauncherState>,
    server: &LocalServer,
) -> LauncherResult<tokio::process::Command> {
    if !server.installed() {
        return Err(LauncherError::Server("the server is not installed".into()));
    }
    let dir = server.dir()?;
    let java = java_for(state, server).await?;
    let mut command = tokio::process::Command::new(java);
    command
        .arg(format!("-Xmx{}M", server.memory_mb))
        .args(server.jvm_args.split_whitespace())
        .args(&server.launch)
        .current_dir(&dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(false);
    oneclient_common::process::no_window(command.as_std_mut());
    Ok(command)
}

/// Scales any picked image down to the 64x64 PNG the Multiplayer list shows
pub async fn set_icon(dir: &Path, source: PathBuf) -> LauncherResult<()> {
    set_icon_bytes(dir, tokio::fs::read(source).await?).await
}

pub async fn set_icon_bytes(dir: &Path, bytes: Vec<u8>) -> LauncherResult<()> {
    let target = dir.join(ICON_FILE);
    tokio::task::spawn_blocking(move || {
        image::load_from_memory(&bytes)
            .and_then(|img| {
                img.resize_to_fill(64, 64, image::imageops::FilterType::Lanczos3)
                    .save_with_format(&target, image::ImageFormat::Png)
            })
            .map_err(|e| LauncherError::Server(e.to_string()))
    })
    .await
    .map_err(|e| LauncherError::Server(e.to_string()))?
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reachability {
    pub address: String,
    pub online: bool,
}

pub async fn check_reachable(state: &LauncherState, port: u16) -> LauncherResult<Reachability> {
    #[derive(serde::Deserialize)]
    struct PublicIp {
        ip: String,
    }
    #[derive(serde::Deserialize)]
    struct PortCheck {
        #[serde(default)]
        check: Vec<PortStatus>,
    }
    #[derive(serde::Deserialize)]
    struct PortStatus {
        port: u16,
        status: bool,
    }
    let ip: PublicIp = install::get_json(state, "https://api.ipify.org?format=json").await?;
    let mut request = reqwest::Request::new(
        reqwest::Method::POST,
        "https://portchecker.io/api/v1/query".parse()?,
    );
    request.headers_mut().insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("application/json"),
    );
    *request.body_mut() =
        Some(serde_json::to_vec(&serde_json::json!({ "host": ip.ip, "ports": [port] }))?.into());
    let result: PortCheck = state.services.requester.send_as(request).await?;
    Ok(Reachability {
        address: format!("{}:{port}", ip.ip),
        online: result.check.iter().any(|c| c.port == port && c.status),
    })
}

fn version_at_least(mc_version: &str, major: u32, minor: u32) -> bool {
    oneclient_common::parse_mc_version(mc_version)
        .is_none_or(|v| (v.major, v.minor.unwrap_or(0)) >= (major, minor))
}

#[must_use]
pub fn json_lists(mc_version: &str) -> bool {
    version_at_least(mc_version, 7, 6)
}

#[must_use]
pub fn named_modes(mc_version: &str) -> bool {
    version_at_least(mc_version, 14, 0)
}

#[must_use]
pub fn has_simulation_distance(mc_version: &str) -> bool {
    version_at_least(mc_version, 18, 0)
}

/// The address other machines on the network reach this one by, no packet is sent
#[must_use]
pub fn lan_ip() -> Option<std::net::IpAddr> {
    static CACHE: std::sync::Mutex<Option<(std::time::Instant, Option<std::net::IpAddr>)>> =
        std::sync::Mutex::new(None);
    let mut cache = CACHE.lock().unwrap();
    if let Some((at, ip)) = *cache
        && at.elapsed() < std::time::Duration::from_secs(30)
    {
        return ip;
    }
    let ip = probe_lan_ip();
    *cache = Some((std::time::Instant::now(), ip));
    ip
}

fn probe_lan_ip() -> Option<std::net::IpAddr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:80").ok()?;
    Some(socket.local_addr().ok()?.ip()).filter(|ip| !ip.is_loopback() && !ip.is_unspecified())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_versions_use_legacy_property_and_list_formats() {
        assert!(!json_lists("1.7.2"));
        assert!(json_lists("1.7.10"));
        assert!(!named_modes("1.8.9"));
        assert!(!named_modes("1.13.2"));
        assert!(named_modes("1.14"));
        assert!(named_modes("26.1"));
        assert!(!has_simulation_distance("1.17.1"));
        assert!(has_simulation_distance("1.18.2"));
    }
}
