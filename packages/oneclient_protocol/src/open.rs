use anyhow::{Result, bail};

const ALLOWED_URL_SCHEMES: [&str; 3] = ["http", "https", "mailto"];

/// Blocks until the opener has been handed the url, so a short lived process
/// can call it right before exiting
pub fn url(url: &str) -> Result<()> {
    if !has_allowed_scheme(url) {
        bail!("refused to open url with a disallowed scheme: {url}");
    }

    target(url)
}

/// For paths the launcher itself produced, which carry no url scheme
pub fn path(path: impl AsRef<std::path::Path>) -> Result<()> {
    target(&path.as_ref().to_string_lossy())
}

fn has_allowed_scheme(url: &str) -> bool {
    let Some((scheme, _)) = url.split_once(':') else {
        return false;
    };

    ALLOWED_URL_SCHEMES
        .iter()
        .any(|allowed| scheme.eq_ignore_ascii_case(allowed))
}

#[cfg(target_os = "linux")]
fn portal_open(target: &str) -> zbus::Result<()> {
    use std::collections::HashMap;
    use zbus::zvariant::{Fd, Value};

    const DESTINATION: &str = "org.freedesktop.portal.Desktop";
    const PATH: &str = "/org/freedesktop/portal/desktop";
    const INTERFACE: &str = "org.freedesktop.portal.OpenURI";

    let connection = zbus::blocking::Connection::session()?;
    let options = HashMap::<&str, Value>::new();
    if has_allowed_scheme(target) {
        connection.call_method(
            Some(DESTINATION),
            PATH,
            Some(INTERFACE),
            "OpenURI",
            &("", target, options),
        )?;
    } else {
        let file = std::fs::File::open(target)?;
        connection.call_method(
            Some(DESTINATION),
            PATH,
            Some(INTERFACE),
            "OpenFile",
            &("", Fd::from(&file), options),
        )?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn file_manager_show(target: &str) -> zbus::Result<()> {
    let uri = url::Url::from_directory_path(target)
        .map_err(|()| zbus::Error::Failure(format!("not an absolute path: {target}")))?;

    zbus::blocking::Connection::session()?.call_method(
        Some("org.freedesktop.FileManager1"),
        "/org/freedesktop/FileManager1",
        Some("org.freedesktop.FileManager1"),
        "ShowFolders",
        &(vec![uri.as_str()], ""),
    )?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn target(target: &str) -> Result<()> {
    use std::process::Stdio;

    if std::path::Path::new(target).is_dir() {
        let Err(err) = file_manager_show(target) else {
            return Ok(());
        };
        tracing::debug!(%err, "file manager failed to show {target}, falling back");
    }

    let Err(err) = portal_open(target) else {
        return Ok(());
    };
    tracing::debug!(%err, "portal failed to open {target}, falling back");

    let opened = open::commands(target).into_iter().any(|mut command| {
        oneclient_common::process::strip_appimage_env(&mut command);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    });

    if !opened {
        bail!("failed to open {target}");
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn target(target: &str) -> Result<()> {
    open::that_detached(target).map_err(|err| anyhow::anyhow!("failed to open {target}: {err}"))
}
