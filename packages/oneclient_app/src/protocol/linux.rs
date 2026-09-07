use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::SCHEME;
use crate::file_content::url_handler_entry;

const ICON: &str = "oneclient_app";

fn handler_name() -> String {
    format!("{SCHEME}-url-handler.desktop")
}

fn applications_dir() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|dirs| dirs.data_dir().join("applications"))
}

pub fn register(exe: &Path) -> Result<()> {
    let dir = applications_dir().context("no user data directory")?;
    let path = dir.join(handler_name());
    let entry = url_handler_entry(exe, SCHEME, ICON);

    if std::fs::read_to_string(&path).is_ok_and(|existing| existing == entry) {
        return Ok(());
    }

    std::fs::create_dir_all(&dir)?;
    std::fs::write(&path, entry)?;
    announce(&dir);

    tracing::info!(scheme = SCHEME, "registered the url scheme");
    Ok(())
}

fn announce(dir: &Path) {
    let _ = std::process::Command::new("update-desktop-database")
        .arg(dir)
        .output();
    let _ = std::process::Command::new("xdg-mime")
        .args([
            "default",
            &handler_name(),
            &format!("x-scheme-handler/{SCHEME}"),
        ])
        .output();
}

pub fn is_registered() -> bool {
    applications_dir().is_some_and(|dir| dir.join(handler_name()).is_file())
}
