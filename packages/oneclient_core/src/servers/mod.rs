mod backups;
mod install;
mod mods;
mod players;
mod properties;
mod store;

use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use backups::{
    BackupInfo, create_backup, list_backups, reset_world, restore_backup, world_name,
};
pub use install::{
    PackInfo, copy_from_instance, install_server, java_for, read_mrpack, supports_loader,
    unpack_mrpack,
};
pub use mods::{
    ModSource, ServerMod, identify_mods, install_mod, list_mods, record_sources, set_mod_enabled,
};
pub use players::{ListedPlayer, OPS, WHITELIST, add_to_list, read_list, remove_from_list};
pub use properties::Properties;
pub use store::{
    DEFAULT_MEMORY_MB, DEFAULT_PORT, LocalServer, ServerKind, accept_eula, allocate_id, clear_pid,
    delete_server, dir_size, kill_orphan, list_servers, load_server, save_server, server_dir,
    servers_root, write_pid,
};

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

/// The address other machines on the network reach this one by, no packet is sent
#[must_use]
pub fn lan_ip() -> Option<std::net::IpAddr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:80").ok()?;
    Some(socket.local_addr().ok()?.ip()).filter(|ip| !ip.is_loopback() && !ip.is_unspecified())
}

