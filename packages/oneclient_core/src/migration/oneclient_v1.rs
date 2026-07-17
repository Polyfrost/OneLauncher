use std::path::{Path, PathBuf};

use sqlx::Row;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use crate::LauncherResult;
use crate::packages::domain::GameLoader;
use crate::paths;

use super::fs::{copy_tree, dir_has_content};
use super::{ImportTarget, MigrationDetection, MigrationSource, SourceInstance};

const IMPORT_EXCLUDE_TOP: &[&str] = &["mods", "logs"];

pub fn old_root() -> Option<PathBuf> {
    let base = directories::BaseDirs::new().map(|d| d.data_dir().join("OneClient"));

    let root = match std::env::var("ONECLIENT_V1_DIR") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => base?,
    };

    let exists = root.is_dir();
    tracing::debug!(root = %root.display(), exists, "v1 migration: resolved old root");
    exists.then_some(root)
}

pub fn category_from_bundle_name(name: &str) -> Option<String> {
    let start = name.find('[')?;
    let end = name[start + 1..].find(']')? + start + 1;
    let inner = name[start + 1..end].trim();
    (!inner.is_empty()).then(|| inner.to_string())
}

#[tracing::instrument]
pub async fn detect() -> LauncherResult<Option<MigrationDetection>> {
    let Some(root) = old_root() else {
        return Ok(None);
    };

    let db_path = root.join("user_data.db");
    if !db_path.exists() {
        tracing::debug!(db = %db_path.display(), "v1 migration: no user_data.db, skipping");
        return Ok(None);
    }

    match detect_inner(&root, &db_path).await {
        Ok(detection) => {
            tracing::info!(
                instances = detection.instances.len(),
                "v1 migration: detected old install"
            );
            Ok(Some(detection))
        }
        Err(err) => {
            tracing::warn!(error = %err, "failed to read old launcher database; skipping v1 migration");
            Ok(None)
        }
    }
}

#[tracing::instrument(level = "debug")]
async fn detect_inner(root: &Path, db_path: &Path) -> LauncherResult<MigrationDetection> {
    let opts = SqliteConnectOptions::new()
        .filename(db_path)
        .read_only(true)
        .immutable(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await?;

    let cluster_rows = sqlx::query("SELECT id, folder_name, mc_version, mc_loader FROM clusters")
        .fetch_all(&pool)
        .await?;

    let mut instances = Vec::with_capacity(cluster_rows.len());
    for row in cluster_rows {
        let cluster_id: i64 = row.try_get("id")?;
        let folder_name: String = row.try_get("folder_name")?;
        let mc_version: String = row.try_get("mc_version")?;
        let mc_loader_raw: i64 = row.try_get("mc_loader")?;

        let Some(mc_loader) = GameLoader::from_repr(mc_loader_raw as u8) else {
            tracing::warn!(
                cluster_id,
                mc_loader_raw,
                "old cluster has unknown loader; skipping"
            );
            continue;
        };

        let categories = fetch_categories(&pool, cluster_id).await?;

        let has_game_dir = dir_has_content(&root.join("clusters").join(&folder_name)).await;

        instances.push(SourceInstance {
            instance_id: cluster_id,
            folder_name,
            mc_version,
            target_mc_version: None,
            mc_loader,
            categories,
            has_game_dir,
        });
    }

    pool.close().await;

    Ok(MigrationDetection {
        source: MigrationSource::OneClientV1,
        root: root.to_path_buf(),
        instances,
    })
}

#[tracing::instrument(level = "debug", skip(pool))]
async fn fetch_categories(pool: &sqlx::SqlitePool, cluster_id: i64) -> LauncherResult<Vec<String>> {
    let rows = sqlx::query(
        "SELECT DISTINCT bundle_name FROM cluster_packages \
         WHERE cluster_id = ? AND bundle_name IS NOT NULL AND bundle_name <> ''",
    )
    .bind(cluster_id)
    .fetch_all(pool)
    .await?;

    let mut categories: Vec<String> = Vec::new();
    for row in rows {
        let bundle_name: String = row.try_get("bundle_name")?;
        let Some(category) = category_from_bundle_name(&bundle_name) else {
            continue;
        };

        if !categories.iter().any(|c| c.eq_ignore_ascii_case(&category)) {
            categories.push(category);
        }
    }

    Ok(categories)
}

#[tracing::instrument(skip(target))]
pub async fn import_game_dir(folder_name: &str, target: ImportTarget) -> LauncherResult<()> {
    let Some(root) = old_root() else {
        return Ok(());
    };
    let src = root.join("clusters").join(folder_name);
    if !src.is_dir() {
        tracing::warn!(folder_name, "old cluster folder missing; nothing to import");
        return Ok(());
    }

    let dest = match &target {
        ImportTarget::Shared => paths::shared_minecraft_dir()?,
        ImportTarget::Dedicated { new_cluster_id } => {
            let state = crate::state::LauncherState::get()?;
            let cluster = crate::clusters::ClusterManager::get(&state, *new_cluster_id).await?;
            let dir = cluster.dir()?;
            polyio::create_dir_all(&dir).await?;
            // Mark dedicated so game_dir() resolves to this cluster's own dir.
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

    #[test]
    fn category_extraction() {
        assert_eq!(
            category_from_bundle_name("OneClient 1.21.1 Fabric [HUD]"),
            Some("HUD".to_string())
        );
        assert_eq!(
            category_from_bundle_name("OneClient 1.21.11 Fabric [QoL]"),
            Some("QoL".to_string())
        );
        assert_eq!(category_from_bundle_name("no brackets here"), None);
        assert_eq!(category_from_bundle_name("empty []"), None);
        assert_eq!(
            category_from_bundle_name("trims [  Performance  ]"),
            Some("Performance".to_string())
        );
    }
}
