#[cfg(all(unix, not(target_os = "macos")))]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod registry;
#[cfg(windows)]
mod windows;

#[cfg(all(unix, not(target_os = "macos")))]
use linux as imp;
#[cfg(target_os = "macos")]
use macos as imp;
#[cfg(windows)]
use windows as imp;

use std::path::Path;

pub fn register(exe: &Path) -> anyhow::Result<()> {
    imp::register(exe)
}

#[must_use]
pub fn is_registered() -> bool {
    imp::is_registered()
}
