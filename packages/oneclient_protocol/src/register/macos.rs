use std::path::Path;

use anyhow::Result;

pub fn register(_exe: &Path) -> Result<()> {
    Ok(())
}

pub fn is_registered() -> bool {
    false
}
