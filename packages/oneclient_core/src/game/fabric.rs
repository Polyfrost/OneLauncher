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
    if !matches!(loader, GameLoader::Fabric | GameLoader::Ornithe) {
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

pub async fn drop_stale_remapped_jars<'a>(
    game_dir: &Path,
    mc_version: &str,
    libraries: impl IntoIterator<Item = &'a str>,
) {
    let mappings = libraries
        .into_iter()
        .filter(|name| name.contains("intermediary"))
        .collect::<Vec<_>>()
        .join("\n");
    if mappings.is_empty() {
        return;
    }

    let dir = game_dir.join(".fabric").join("remappedJars");
    let safe_version: String = mc_version
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || "-. ".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let marker = dir.join(format!("oneclient-{safe_version}.mappings"));
    if polyio::read_to_string(&marker).await.ok().as_deref() == Some(mappings.as_str()) {
        return;
    }

    let prefix = format!("minecraft-{safe_version}-");
    if let Ok(mut entries) = polyio::read_dir(&dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            if !entry.file_name().to_string_lossy().starts_with(&prefix) {
                continue;
            }
            if let Err(err) = polyio::remove_dir_all(entry.path()).await {
                tracing::warn!(error = %err, "failed to drop a stale remapped game jar");
                return;
            }
        }
    }

    if polyio::create_dir_all(&dir).await.is_ok()
        && let Err(err) = polyio::write(&marker, mappings).await
    {
        tracing::warn!(error = %err, "failed to record the remapped jar mappings");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_changed_intermediary_drops_the_remapped_game_jar() {
        let root = polyio::testing::ScratchDir::new("remapped_jars");
        let jars = root.path().join(".fabric/remappedJars");
        let cached = jars.join("minecraft-1.8.9-0.20.1/client-intermediary.jar");
        let other = jars.join("minecraft-1.21.11-0.20.1/client-intermediary.jar");
        for jar in [&cached, &other] {
            polyio::create_dir_all(jar.parent().unwrap()).await.unwrap();
            polyio::write(jar, b"jar".as_slice()).await.unwrap();
        }

        let legacy = ["net.legacyfabric:intermediary:1.8.9"];
        drop_stale_remapped_jars(root.path(), "1.8.9", legacy).await;
        assert!(!cached.exists(), "no record yet, the cache is untrusted");

        polyio::create_dir_all(cached.parent().unwrap())
            .await
            .unwrap();
        polyio::write(&cached, b"jar".as_slice()).await.unwrap();
        drop_stale_remapped_jars(root.path(), "1.8.9", legacy).await;
        assert!(cached.exists(), "same intermediary keeps the cache");

        let gen2 = ["net.ornithemc:calamus-intermediary-gen2:1.8.9"];
        drop_stale_remapped_jars(root.path(), "1.8.9", gen2).await;
        assert!(!cached.exists());
        assert!(other.exists(), "other versions are left alone");
    }

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
        assert!(uses_cluster_mods_folder(
            GameLoader::Ornithe,
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
        ] {
            assert!(!uses_cluster_mods_folder(loader, Some("0.16.5"), ""));
        }
    }

    #[test]
    fn a_user_set_property_takes_the_whole_layout_with_it() {
        let dir = Path::new("/clusters/one/mods");
        let mine = "-Dfabric.modsFolder=/elsewhere";

        assert!(mods_folder_argument(GameLoader::Fabric, Some("0.16.5"), "-Xmx4G", dir).is_some());

        assert!(!uses_cluster_mods_folder(
            GameLoader::Fabric,
            Some("0.16.5"),
            mine
        ));
        assert!(mods_folder_argument(GameLoader::Fabric, Some("0.16.5"), mine, dir).is_none());
    }
}
