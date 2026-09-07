use std::path::Path;

use oneclient_common::domain::GameLoader;

const MODS_FOLDER_PROPERTY: &str = "fabric.modsFolder";

// version 0.15.0 is required for fabric.modsFolder to work
const MIN_LOADER_VERSION: Version = Version(0, 15, 0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Version(u32, u32, u32);

fn parse_version(raw: &str) -> Option<Version> {
    let core = raw.trim().split(['+', '-']).next()?;
    let mut parts = core.split('.');

    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let patch = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);

    Some(Version(major, minor, patch))
}

#[must_use]
pub fn uses_cluster_mods_folder(
    loader: GameLoader,
    loader_version: Option<&str>,
    custom_args: &str,
) -> bool {
    if loader != GameLoader::Fabric {
        return false;
    }

    if custom_args.contains(MODS_FOLDER_PROPERTY) {
        tracing::info!("launch args already set {MODS_FOLDER_PROPERTY}; leaving the layout alone");
        return false;
    }

    parse_version(loader_version.unwrap_or_default()).is_some_and(|v| v >= MIN_LOADER_VERSION)
}

#[must_use]
pub fn mods_folder_argument(
    loader: GameLoader,
    loader_version: Option<&str>,
    custom_args: &str,
    mods_dir: &Path,
) -> Option<String> {
    if !uses_cluster_mods_folder(loader, loader_version, custom_args) {
        return None;
    }

    Some(format!("-D{MODS_FOLDER_PROPERTY}={}", mods_dir.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_component_not_by_string() {
        assert!(parse_version("0.9.0") < parse_version("0.15.0"));
        assert!(parse_version("0.16.5") > parse_version("0.15.0"));
        assert_eq!(parse_version("0.15"), Some(Version(0, 15, 0)));
    }

    #[test]
    fn build_metadata_is_ignored() {
        assert_eq!(parse_version("0.16.0+build.1"), Some(Version(0, 16, 0)));
        assert_eq!(parse_version("0.16.0-rc.1"), Some(Version(0, 16, 0)));
    }

    #[test]
    fn only_fabric_and_only_new_enough() {
        assert!(uses_cluster_mods_folder(
            GameLoader::Fabric,
            Some("0.16.5"),
            ""
        ));
        assert!(!uses_cluster_mods_folder(
            GameLoader::Fabric,
            Some("0.11.0"),
            ""
        ));

        assert!(!uses_cluster_mods_folder(GameLoader::Fabric, None, ""));

        for loader in [
            GameLoader::Vanilla,
            GameLoader::Forge,
            GameLoader::NeoForge,
            GameLoader::Quilt,
            GameLoader::LegacyFabric,
        ] {
            assert!(!uses_cluster_mods_folder(loader, Some("0.16.5"), ""));
        }
    }

    #[test]
    fn a_user_set_property_takes_the_whole_layout_with_it() {
        let dir = Path::new("/clusters/one/mods");
        let mine = "-Dfabric.modsFolder=/elsewhere";

        assert!(mods_folder_argument(GameLoader::Fabric, Some("0.16.5"), "-Xmx4G", dir).is_some());

        assert!(!uses_cluster_mods_folder(GameLoader::Fabric, Some("0.16.5"), mine));
        assert!(mods_folder_argument(GameLoader::Fabric, Some("0.16.5"), mine, dir).is_none());
    }
}
