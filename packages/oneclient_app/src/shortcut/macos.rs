use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::Result;

use super::ShortcutRequest;
use super::content::{info_plist, shell_script};

pub const EXTENSION: &str = "app";

const EXECUTABLE: &str = "launch";

pub fn write(request: &ShortcutRequest, exe: &Path, path: &Path) -> Result<()> {
    let result = build(request, exe, path);
    if result.is_err() {
        let _ = std::fs::remove_dir_all(path);
    }
    result
}

fn build(request: &ShortcutRequest, exe: &Path, path: &Path) -> Result<()> {
    let contents = path.join("Contents");
    let macos = contents.join("MacOS");
    let resources = contents.join("Resources");
    std::fs::create_dir_all(&macos)?;
    std::fs::create_dir_all(&resources)?;

    let script = macos.join(EXECUTABLE);
    std::fs::write(&script, shell_script(exe, &request.folder_name))?;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))?;

    let icon = copy_icon(exe, &resources);
    std::fs::write(
        contents.join("Info.plist"),
        info_plist(
            &request.cluster_name,
            EXECUTABLE,
            &request.folder_name,
            icon.as_deref(),
        ),
    )?;

    Ok(())
}

fn copy_icon(exe: &Path, resources: &Path) -> Option<String> {
    let source = bundle_root(exe)?
        .join("Contents")
        .join("Resources")
        .join("icon.icns");

    std::fs::copy(&source, resources.join("icon.icns")).ok()?;
    Some("icon".to_string())
}

fn bundle_root(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;

    (macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && app.extension()? == "app")
        .then(|| app.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_installed_binary_finds_its_bundle() {
        let exe = PathBuf::from("/Applications/OneClient.app/Contents/MacOS/oneclient_app");
        assert_eq!(
            bundle_root(&exe),
            Some(PathBuf::from("/Applications/OneClient.app")),
        );
    }

    #[test]
    fn a_bare_binary_has_no_bundle() {
        assert_eq!(bundle_root(&PathBuf::from("/usr/local/bin/oneclient_app")), None);
        assert_eq!(
            bundle_root(&PathBuf::from("/tmp/target/debug/oneclient_app")),
            None,
        );
    }
}
