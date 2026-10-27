use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use anyhow::Result;

use super::ShortcutRequest;
use crate::file_content::desktop_entry;

pub const EXTENSION: &str = "desktop";

const ICON: &str = "oneclient_app";

pub fn write(request: &ShortcutRequest, exe: &Path, path: &Path) -> Result<()> {
    let icon = request
        .icon
        .as_deref()
        .map(|icon| icon.to_string_lossy().into_owned())
        .filter(|icon| !icon.chars().any(char::is_control))
        .or_else(|| std::env::var("FLATPAK_ID").ok())
        .unwrap_or_else(|| ICON.to_string());
    let entry = desktop_entry(&request.cluster_name, exe, &request.folder_name, &icon);
    std::fs::write(path, entry)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    trust(path);

    Ok(())
}

/// GNOME shows a desktop file without this attribute as plain text rather than
/// launching it
fn trust(path: &Path) {
    let _ = std::process::Command::new("gio")
        .arg("set")
        .arg(path)
        .args(["metadata::trusted", "true"])
        .output();
}
