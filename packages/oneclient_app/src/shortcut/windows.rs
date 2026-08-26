use std::path::Path;

use anyhow::{Result, bail};

use super::ShortcutRequest;
use super::content::url_shortcut;
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
    std::fs::write(path, url_shortcut(&url, exe))?;

    Ok(())
}
