use std::path::Path;

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};

#[cfg(debug_assertions)]
pub const SCHEME: &str = "oneclient-dev";
#[cfg(not(debug_assertions))]
pub const SCHEME: &str = "oneclient";

const LAUNCH_HOST: &str = "launch";

const FOLDER: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

#[must_use]
pub fn launch_url(folder: &str) -> String {
    format!(
        "{SCHEME}://{LAUNCH_HOST}/{}",
        utf8_percent_encode(folder, FOLDER)
    )
}

#[must_use]
pub fn parse_launch_url(raw: &str) -> Option<String> {
    let (scheme, rest) = raw.trim().split_once("://")?;
    if !scheme.eq_ignore_ascii_case(SCHEME) {
        return None;
    }

    let (host, path) = rest.trim_end_matches('/').split_once('/')?;
    if !host.eq_ignore_ascii_case(LAUNCH_HOST) {
        return None;
    }

    let folder = percent_decode_str(path).decode_utf8().ok()?;
    let folder = folder.trim();
    (!folder.is_empty()).then(|| folder.to_string())
}

pub fn register(exe: &Path) -> anyhow::Result<()> {
    imp::register(exe)
}

#[must_use]
pub fn is_registered() -> bool {
    imp::is_registered()
}

#[cfg(windows)]
mod imp {
    use std::path::Path;

    use anyhow::{Result, bail};
    use windows::Win32::Foundation::{ERROR_SUCCESS, WIN32_ERROR};
    use windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ,
        RegCloseKey, RegCreateKeyExW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    };
    use windows::core::{HSTRING, PCWSTR};

    use super::SCHEME;

    fn command_key() -> String {
        format!(r"Software\Classes\{SCHEME}\shell\open\command")
    }

    pub fn register(exe: &Path) -> Result<()> {
        let exe = exe.display().to_string();
        let command = format!("\"{exe}\" \"%1\"");

        if read_string(&command_key(), None).as_deref() == Some(command.as_str()) {
            return Ok(());
        }

        let root = format!(r"Software\Classes\{SCHEME}");
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

    fn check(status: WIN32_ERROR, what: &str) -> Result<()> {
        if status == ERROR_SUCCESS {
            return Ok(());
        }
        bail!("{what} failed with Windows error {}", status.0)
    }

    fn write_string(subkey: &str, name: Option<&str>, value: &str) -> Result<()> {
        let mut key = HKEY::default();
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                &HSTRING::from(subkey),
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                None,
                &raw mut key,
                None,
            )
        };
        check(status, "creating the registry key")?;

        let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes =
            unsafe { std::slice::from_raw_parts(wide.as_ptr().cast::<u8>(), wide.len() * 2) };

        let name = name.map(HSTRING::from);
        let status = unsafe {
            RegSetValueExW(
                key,
                name.as_ref()
                    .map_or_else(PCWSTR::null, |name| PCWSTR(name.as_ptr())),
                None,
                REG_SZ,
                Some(bytes),
            )
        };
        unsafe { let _ = RegCloseKey(key); };

        check(status, "writing the registry value")
    }

    fn read_string(subkey: &str, name: Option<&str>) -> Option<String> {
        let mut key = HKEY::default();
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                &HSTRING::from(subkey),
                None,
                KEY_READ,
                &raw mut key,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }

        let name = name.map(HSTRING::from);
        let name = name
            .as_ref()
            .map_or_else(PCWSTR::null, |name| PCWSTR(name.as_ptr()));

        let mut size = 0u32;
        let status =
            unsafe { RegQueryValueExW(key, name, None, None, None, Some(&raw mut size)) };
        if status != ERROR_SUCCESS || size == 0 {
            unsafe { let _ = RegCloseKey(key); };
            return None;
        }

        let mut buffer = vec![0u8; size as usize];
        let status = unsafe {
            RegQueryValueExW(
                key,
                name,
                None,
                None,
                Some(buffer.as_mut_ptr()),
                Some(&raw mut size),
            )
        };
        unsafe { let _ = RegCloseKey(key); };
        if status != ERROR_SUCCESS {
            return None;
        }

        buffer.truncate(size as usize);
        let wide: Vec<u16> = buffer
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        let value = String::from_utf16_lossy(&wide);

        Some(value.trim_end_matches('\0').to_string())
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod imp {
    use std::path::{Path, PathBuf};

    use anyhow::{Context, Result};

    use super::SCHEME;

    fn handler_name() -> String {
        format!("{SCHEME}-url-handler.desktop")
    }

    fn applications_dir() -> Option<PathBuf> {
        directories::BaseDirs::new().map(|dirs| dirs.data_dir().join("applications"))
    }

    pub fn register(exe: &Path) -> Result<()> {
        let dir = applications_dir().context("no user data directory")?;
        let path = dir.join(handler_name());

        let entry = format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=OneClient\n\
             Exec={} %u\n\
             Icon=oneclient_app\n\
             Terminal=false\n\
             NoDisplay=true\n\
             MimeType=x-scheme-handler/{SCHEME};\n",
            exe.display(),
        );

        if std::fs::read_to_string(&path).is_ok_and(|existing| existing == entry) {
            return Ok(());
        }

        std::fs::create_dir_all(&dir)?;
        std::fs::write(&path, entry)?;

        let _ = std::process::Command::new("update-desktop-database")
            .arg(&dir)
            .output();
        let _ = std::process::Command::new("xdg-mime")
            .args([
                "default",
                &handler_name(),
                &format!("x-scheme-handler/{SCHEME}"),
            ])
            .output();

        tracing::info!(scheme = SCHEME, "registered the url scheme");
        Ok(())
    }

    pub fn is_registered() -> bool {
        applications_dir().is_some_and(|dir| dir.join(handler_name()).is_file())
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use std::path::Path;

    use anyhow::Result;

    pub fn register(_exe: &Path) -> Result<()> {
        Ok(())
    }

    pub fn is_registered() -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_survives_the_round_trip() {
        let folder = "26.1.2 Fabric";
        assert_eq!(parse_launch_url(&launch_url(folder)).as_deref(), Some(folder));
    }

    #[test]
    fn a_space_is_encoded_rather_than_left_to_split_the_url() {
        assert!(launch_url("My Pack").ends_with("/My%20Pack"));
    }

    #[test]
    fn punctuation_in_a_folder_name_round_trips() {
        for folder in ["My Pack (1.8.9)", "a/b", "100%", "zażółć gęślą jaźń"] {
            assert_eq!(
                parse_launch_url(&launch_url(folder)).as_deref(),
                Some(folder),
                "failed for {folder}",
            );
        }
    }

    #[test]
    fn a_trailing_slash_is_tolerated() {
        assert_eq!(
            parse_launch_url(&format!("{SCHEME}://launch/pack/")).as_deref(),
            Some("pack"),
        );
    }

    #[test]
    fn the_scheme_and_host_are_matched_case_insensitively() {
        assert_eq!(
            parse_launch_url(&format!("{}://LAUNCH/pack", SCHEME.to_uppercase())).as_deref(),
            Some("pack"),
        );
    }

    #[test]
    fn a_foreign_url_is_not_ours() {
        assert_eq!(parse_launch_url("https://polyfrost.org/pack"), None);
        assert_eq!(parse_launch_url("file:///etc/passwd"), None);
        assert_eq!(parse_launch_url(""), None);
    }

    #[test]
    fn an_unknown_host_is_refused() {
        assert_eq!(parse_launch_url(&format!("{SCHEME}://install/pack")), None);
    }

    #[test]
    fn an_empty_folder_is_not_a_request() {
        assert_eq!(parse_launch_url(&format!("{SCHEME}://launch/")), None);
        assert_eq!(parse_launch_url(&format!("{SCHEME}://launch/%20")), None);
        assert_eq!(parse_launch_url(&format!("{SCHEME}://launch")), None);
    }
}
