use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use super::ShortcutRequest;
use crate::file_content::url_shortcut;
use crate::protocol;

pub const EXTENSION: &str = "url";

pub fn write(request: &ShortcutRequest, exe: &Path, path: &Path) -> Result<()> {
    if !protocol::is_registered() {
        bail!(
            "the {} :// handler is not registered on this machine",
            protocol::SCHEME
        );
    }

    let url = protocol::launch_url(&request.folder_name);
    let icon = request
        .icon
        .as_deref()
        .and_then(cluster_icon)
        .unwrap_or_else(|| exe.to_path_buf());
    std::fs::write(path, url_shortcut(&url, &icon))?;

    Ok(())
}

fn cluster_icon(source: &Path) -> Option<PathBuf> {
    let ico = super::square_icon(source, image::ImageFormat::Ico)?;
    let dest = source.with_extension("ico");
    std::fs::write(&dest, ico).ok()?;
    Some(dest)
}
