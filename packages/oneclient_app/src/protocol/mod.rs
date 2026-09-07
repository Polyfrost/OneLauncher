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
