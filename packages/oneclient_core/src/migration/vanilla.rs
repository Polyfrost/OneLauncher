use std::path::PathBuf;

use crate::LauncherResult;
use crate::packages::domain::GameLoader;
use crate::paths;

use super::fs::{copy_tree, dir_has_content};
use super::{ImportTarget, MigrationDetection, MigrationSource, SourceInstance};

const ROOT_INSTANCE_NAME: &str = ".minecraft";

const IMPORT_EXCLUDE_TOP: &[&str] = &[
    "mods",
    "resourcepacks",
    "shaderpacks",
    "datapacks",
    "versions",
    "libraries",
    "assets",
    "bin",
    "jre",
    "logs",
    "crash-reports",
    "launcher_accounts.json",
    "launcher_accounts_microsoft_store.json",
    "launcher_msa_credentials.bin",
    "launcher_profiles.json",
    "launcher_settings.json",
    "usercache.json",
    "webcache",
    "webcache2",
    ".mojang",
    "cheatbreaker_accounts.json",
    "feather",
    "feather-mods",
    "feather-versions",
    "optionsLC.txt",
    "servers.dat_old",
    "cache",
    "downloads",
    "jfr",
    "skyblock-repo-cache",
    "meowdding-repo-cache",
];

pub fn old_root() -> Option<PathBuf> {
    let base = directories::BaseDirs::new().map(|d| {
        // Windows: `%APPDATA%\.minecraft` (data_dir is Roaming).
        #[cfg(target_os = "windows")]
        {
            d.data_dir().join(".minecraft")
        }
        // macOS: `~/Library/Application Support/minecraft` — no leading dot.
        #[cfg(target_os = "macos")]
        {
            d.data_dir().join("minecraft")
        }
        // Linux and friends: `~/.minecraft`. Not data_dir(), which is
        // `~/.local/share`.
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            d.home_dir().join(".minecraft")
        }
    });

    let root = match std::env::var("VANILLA_MC_DIR") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => base?,
    };

    let exists = root.is_dir();
    tracing::debug!(root = %root.display(), exists, "vanilla migration: resolved old root");
    exists.then_some(root)
}

#[tracing::instrument]
pub async fn detect() -> LauncherResult<Option<MigrationDetection>> {
    let Some(root) = old_root() else {
        return Ok(None);
    };

    if !dir_has_content(&root).await {
        tracing::debug!(root = %root.display(), "vanilla migration: empty install, skipping");
        return Ok(None);
    }

    tracing::info!(root = %root.display(), "vanilla migration: detected install");

    Ok(Some(MigrationDetection {
        source: MigrationSource::Vanilla,
        instances: vec![SourceInstance {
            instance_id: 0,
            folder_name: ROOT_INSTANCE_NAME.to_string(),
            mc_version: String::new(),
            target_mc_version: None,
            mc_loader: GameLoader::Vanilla,
            categories: Vec::new(),
            has_game_dir: true,
        }],
        root,
    }))
}

#[tracing::instrument(skip(target))]
pub async fn import_game_dir(target: ImportTarget) -> LauncherResult<()> {
    let Some(src) = old_root() else {
        return Ok(());
    };

    let dest = match &target {
        ImportTarget::Shared => paths::shared_minecraft_dir()?,
        ImportTarget::Dedicated { new_cluster_id } => {
            let state = crate::state::LauncherState::get()?;
            let cluster = crate::clusters::ClusterManager::get(&state, *new_cluster_id).await?;
            let dir = cluster.dir()?;
            polyio::create_dir_all(&dir).await?;
            polyio::write(cluster.dedicated_marker()?, Vec::new()).await?;
            dir
        }
    };

    polyio::create_dir_all(&dest).await?;
    copy_tree(&src, &dest, IMPORT_EXCLUDE_TOP).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn excluded(name: &str) -> bool {
        IMPORT_EXCLUDE_TOP
            .iter()
            .any(|e| e.eq_ignore_ascii_case(name))
    }

    #[test]
    fn excludes_package_and_launcher_managed_dirs() {
        for name in [
            "mods",
            "resourcepacks",
            "shaderpacks",
            "datapacks",
            "versions",
            "libraries",
            "assets",
            "logs",
        ] {
            assert!(excluded(name), "{name} should be excluded");
        }
    }

    #[test]
    fn excludes_mojang_credentials() {
        assert!(excluded("launcher_accounts.json"));
        assert!(excluded("launcher_msa_credentials.bin"));
    }

    #[test]
    fn excludes_third_party_launcher_data() {
        for name in ["cheatbreaker_accounts.json", "feather-mods", "jre"] {
            assert!(excluded(name), "{name} should be excluded");
        }
    }

    #[test]
    fn keeps_user_data() {
        for name in [
            "saves",
            "config",
            "options.txt",
            "screenshots",
            "servers.dat",
        ] {
            assert!(!excluded(name), "{name} must not be excluded");
        }
    }

    #[tokio::test]
    async fn import_copies_user_data_and_skips_the_rest() {
        let src = polyio::tempdir().await.expect("src dir");
        let dst = polyio::tempdir().await.expect("dst dir");
        let (src, dst) = (src.dir_path(), dst.dir_path());

        for file in [
            "options.txt",
            "servers.dat",
            "launcher_accounts.json",
            "launcher_msa_credentials.bin",
        ] {
            polyio::write(src.join(file), b"x".to_vec()).await.unwrap();
        }
        for (dir, file) in [
            ("saves", "world/level.dat"),
            ("config", "sodium.json"),
            ("screenshots", "shot.png"),
            ("mods", "sodium.jar"),
            ("resourcepacks", "faithful.zip"),
            ("shaderpacks", "bsl.zip"),
            ("versions", "1.21.1/1.21.1.jar"),
            ("libraries", "com/foo/foo.jar"),
            ("assets", "objects/ab/abcdef"),
            ("logs", "latest.log"),
        ] {
            let path = src.join(dir).join(file);
            polyio::create_dir_all(path.parent().unwrap())
                .await
                .unwrap();
            polyio::write(path, b"x".to_vec()).await.unwrap();
        }

        copy_tree(src, dst, IMPORT_EXCLUDE_TOP).await.unwrap();

        for kept in [
            "options.txt",
            "servers.dat",
            "saves/world/level.dat",
            "config/sodium.json",
            "screenshots/shot.png",
        ] {
            assert!(dst.join(kept).exists(), "{kept} should have been copied");
        }
        for skipped in [
            "mods",
            "resourcepacks",
            "shaderpacks",
            "versions",
            "libraries",
            "assets",
            "logs",
            "launcher_accounts.json",
            "launcher_msa_credentials.bin",
        ] {
            assert!(
                !dst.join(skipped).exists(),
                "{skipped} should not have been copied"
            );
        }
    }
}
