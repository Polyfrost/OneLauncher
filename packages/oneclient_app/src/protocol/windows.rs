use std::path::Path;

use anyhow::Result;

use super::SCHEME;
use super::registry::{read_string, write_string};

fn root_key() -> String {
    format!(r"Software\Classes\{SCHEME}")
}

fn command_key() -> String {
    format!(r"{}\shell\open\command", root_key())
}

pub fn register(exe: &Path) -> Result<()> {
    let exe = exe.display().to_string();
    let command = format!("\"{exe}\" \"%1\"");

    if read_string(&command_key(), None).as_deref() == Some(command.as_str()) {
        return Ok(());
    }

    let root = root_key();
    write_string(&root, None, &format!("URL:{SCHEME} Protocol"))?;
    write_string(&root, Some("URL Protocol"), "")?;
    write_string(&format!(r"{root}\DefaultIcon"), None, &format!("{exe},0"))?;
    write_string(&command_key(), None, &command)?;

    tracing::info!(scheme = SCHEME, "registered the url scheme");
    Ok(())
}

pub fn is_registered() -> bool {
    read_string(&command_key(), None).is_some_and(|value| !value.is_empty())
}
