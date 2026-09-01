use std::path::{Path, PathBuf};
use std::str::FromStr;

use oneclient_db::dao::{applied_migration as migration_dao, cluster as cluster_dao};
use oneclient_db::models::ClusterRow;

use crate::LauncherResult;
use oneclient_cluster::ClusterIdentity;
use oneclient_common::domain::GameLoader;
use crate::state::LauncherState;
use crate::versions::RemoteMigration;

#[tracing::instrument(skip(state))]
pub async fn apply_remote_migrations(state: &LauncherState) -> LauncherResult<usize> {
    let rules = state.versions.migrations().await;
    if rules.is_empty() {
        return Ok(0);
    }

    let mut migrated = 0;

    for rule in rules {
        match apply_rule(state, &rule).await {
            Ok(true) => migrated += 1,
            Ok(false) => {}
            Err(err) => {
                tracing::warn!(
                    migration_id = %rule.id,
                    error = %err,
                    "cluster migration failed; leaving install untouched"
                );
            }
        }
    }

    if migrated > 0 {
        tracing::info!(migrated, "applied cluster migrations");
    }

    Ok(migrated)
}

async fn apply_rule(state: &LauncherState, rule: &RemoteMigration) -> LauncherResult<bool> {
    let db = &state.services.db;

    if migration_dao::is_applied(db, &rule.id).await? {
        return Ok(false);
    }

    let Ok(loader) = GameLoader::from_str(&rule.from.loader) else {
        tracing::warn!(
            migration_id = %rule.id,
            loader = %rule.from.loader,
            "unknown loader in migration rule; retiring rule"
        );
        migration_dao::mark_applied(db, &rule.id).await?;
        return Ok(false);
    };

    if rule.from.mc_version == rule.to.mc_version {
        tracing::warn!(migration_id = %rule.id, "migration is a no-op; retiring rule");
        migration_dao::mark_applied(db, &rule.id).await?;
        return Ok(false);
    }

    let sources: Vec<ClusterRow> = cluster_dao::list_all(db)
        .await?
        .into_iter()
        .filter(|row| row.mc_version == rule.from.mc_version && row.mc_loader == loader as i64)
        .collect();

    if sources.is_empty() {
        migration_dao::mark_applied(db, &rule.id).await?;
        return Ok(false);
    }

    if let Some(running) = sources
        .iter()
        .find(|source| crate::game::is_running(state, source.id))
    {
        tracing::info!(
            migration_id = %rule.id,
            cluster_id = running.id,
            "cluster is running; deferring migration"
        );
        return Ok(false);
    }

    let mut migrated = 0;
    let mut failed = 0;

    for source in &sources {
        match migrate_cluster(state, rule, source).await {
            Ok(()) => {
                migrated += 1;
                tracing::info!(
                    migration_id = %rule.id,
                    cluster_id = source.id,
                    from = %rule.from.mc_version,
                    to = %rule.to.mc_version,
                    "migrated cluster"
                );
            }
            Err(err) => {
                failed += 1;
                tracing::warn!(
                    migration_id = %rule.id,
                    cluster_id = source.id,
                    error = %err,
                    "failed to migrate cluster; leaving it on the old version"
                );
            }
        }
    }

    if failed == 0 {
        migration_dao::mark_applied(db, &rule.id).await?;
    } else {
        tracing::warn!(
            migration_id = %rule.id,
            migrated,
            failed,
            "migration incomplete; keeping the rule pending for the next start"
        );
    }

    Ok(migrated > 0)
}

async fn migrate_cluster(
    state: &LauncherState,
    rule: &RemoteMigration,
    source: &ClusterRow,
) -> LauncherResult<()> {
    let from = &rule.from.mc_version;
    let to = &rule.to.mc_version;

    let clusters_dir = oneclient_common::paths::clusters_dir()?;
    let old_dir = clusters_dir.join(&source.folder_name);

    let new_folder = resolve_new_folder(&clusters_dir, &source.folder_name, from, to).await?;
    let new_name = retarget_version_prefix(&source.name, from, to);

    let mut renamed: Option<(PathBuf, PathBuf)> = None;
    if let Some(folder) = &new_folder {
        let new_dir = clusters_dir.join(folder);
        if polyio::try_exists(&old_dir).await? {
            polyio::rename(&old_dir, &new_dir).await?;
            renamed = Some((old_dir.clone(), new_dir));
        } else {
            tracing::warn!(
                folder = %source.folder_name,
                "cluster directory is missing on disk; migrating the row only"
            );
        }
    }

    let folder_for_db = folder_for_db(
        &source.folder_name,
        new_folder.as_deref(),
        renamed.is_some(),
    );

    match cluster_dao::migrate_version(
        &state.services.db,
        source.id,
        to,
        new_name.as_deref(),
        folder_for_db,
    )
    .await
    {
        Ok(_) => {
            ClusterIdentity::amend(folder_for_db, |identity| {
                identity.mc_version = to.clone();
                if let Some(name) = &new_name {
                    identity.name = name.clone();
                }
            })
            .await;
            Ok(())
        }
        Err(err) => {
            if let Some((old_dir, new_dir)) = renamed
                && let Err(rollback) = polyio::rename(&new_dir, &old_dir).await
            {
                tracing::error!(
                    migration_id = %rule.id,
                    cluster_id = source.id,
                    from = ?new_dir,
                    to = ?old_dir,
                    error = %rollback,
                    "failed to roll back cluster directory rename after database error"
                );
            }
            Err(err.into())
        }
    }
}

fn folder_for_db<'a>(current: &'a str, new_folder: Option<&'a str>, renamed: bool) -> &'a str {
    match (renamed, new_folder) {
        (true, Some(folder)) => folder,
        _ => current,
    }
}

async fn resolve_new_folder(
    clusters_dir: &Path,
    folder_name: &str,
    from: &str,
    to: &str,
) -> LauncherResult<Option<String>> {
    let Some(candidate) = retarget_version_prefix(folder_name, from, to) else {
        tracing::debug!(
            folder = %folder_name,
            "folder name is not in generated form; leaving it in place"
        );
        return Ok(None);
    };

    if polyio::try_exists(&clusters_dir.join(&candidate)).await? {
        tracing::warn!(
            folder = %folder_name,
            candidate = %candidate,
            "target folder already exists; leaving directory in place"
        );
        return Ok(None);
    }

    Ok(Some(candidate))
}

fn retarget_version_prefix(value: &str, from: &str, to: &str) -> Option<String> {
    let rest = value.strip_prefix(from)?;
    if !rest.starts_with(' ') {
        return None;
    }
    Some(format!("{to}{rest}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retargets_generated_folder() {
        assert_eq!(
            retarget_version_prefix("26.1 fabric", "26.1", "26.1.2").as_deref(),
            Some("26.1.2 fabric")
        );
    }

    #[test]
    fn retargets_deduplicated_folder() {
        assert_eq!(
            retarget_version_prefix("26.1 fabric (1)", "26.1", "26.1.2").as_deref(),
            Some("26.1.2 fabric (1)")
        );
    }

    #[test]
    fn does_not_match_adjacent_minor() {
        assert_eq!(
            retarget_version_prefix("26.10 fabric", "26.1", "26.1.2"),
            None
        );
    }

    #[test]
    fn leaves_custom_names_alone() {
        assert_eq!(
            retarget_version_prefix("my cool pack", "26.1", "26.1.2"),
            None
        );
        assert_eq!(
            retarget_version_prefix("26.1fabric", "26.1", "26.1.2"),
            None
        );
    }

    #[test]
    fn exact_version_with_no_suffix_is_not_retargeted() {
        assert_eq!(retarget_version_prefix("26.1", "26.1", "26.1.2"), None);
    }

    #[test]
    fn row_claims_new_folder_only_when_the_directory_moved() {
        assert_eq!(
            folder_for_db("26.1 fabric", Some("26.1.2 fabric"), true),
            "26.1.2 fabric"
        );
    }

    #[test]
    fn row_keeps_its_folder_when_the_rename_was_skipped() {
        assert_eq!(
            folder_for_db("26.1 fabric", Some("26.1.2 fabric"), false),
            "26.1 fabric"
        );
        assert_eq!(folder_for_db("my cool pack", None, false), "my cool pack");
    }
}
