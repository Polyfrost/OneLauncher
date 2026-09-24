use std::path::{Path, PathBuf};

use oneclient_common::domain::{ContentType, GameLoader};
use oneclient_content::packages::PackageStore;
use oneclient_db::dao::cluster::ClusterMigration;
use oneclient_db::dao::{
    applied_migration as migration_dao, bundle as bundle_catalog_dao, cluster as cluster_dao,
    cluster_bundle as bundle_dao,
};
use oneclient_db::models::{ClusterKind, ClusterRow};

use crate::LauncherResult;
use crate::clusters::ClusterStage;
use crate::state::LauncherState;
use crate::versions::{MigrationNode, RemoteMigration, cyclic_migration_ids};

#[tracing::instrument(skip(state))]
pub async fn apply_remote_migrations(state: &LauncherState) -> LauncherResult<usize> {
    let rules = state.versions.migrations().await;
    if rules.is_empty() {
        return Ok(0);
    }

    let cyclic = cyclic_migration_ids(&rules);
    let mut migrated = 0;

    for rule in rules {
        if cyclic.contains(&rule.id) {
            tracing::warn!(
                migration_id = %rule.id,
                "migration rule is part of a cycle; skipping"
            );
            continue;
        }

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
    let names_loader = rule.to.loader.is_some();

    if !names_loader && migration_dao::is_applied(db, &rule.id).await? {
        return Ok(false);
    }

    let Some((from, to)) = rule.endpoints() else {
        if names_loader {
            tracing::warn!(
                migration_id = %rule.id,
                from = %rule.from.loader,
                to = ?rule.to.loader,
                "unknown loader in migration rule; skipping"
            );
            return Ok(false);
        }
        tracing::warn!(
            migration_id = %rule.id,
            loader = %rule.from.loader,
            "unknown loader in migration rule; retiring rule"
        );
        migration_dao::mark_applied(db, &rule.id).await?;
        return Ok(false);
    };

    if from == to {
        tracing::warn!(migration_id = %rule.id, "migration is a no-op; retiring rule");
        migration_dao::mark_applied(db, &rule.id).await?;
        return Ok(false);
    }

    let Some(source) =
        cluster_dao::find_by_version_loader(db, &from.mc_version, from.loader as i64).await?
    else {
        migration_dao::mark_applied(db, &rule.id).await?;
        return Ok(false);
    };

    if cluster_dao::find_by_version_loader(db, &to.mc_version, to.loader as i64)
        .await?
        .is_some()
    {
        tracing::warn!(
            migration_id = %rule.id,
            from = %from.mc_version,
            to = %to.mc_version,
            to_loader = %to.loader,
            "target cluster already exists; skipping migration"
        );
        return Ok(false);
    }

    if crate::game::is_running(state, source.id) {
        tracing::info!(
            migration_id = %rule.id,
            cluster_id = source.id,
            "cluster is running; deferring migration"
        );
        return Ok(false);
    }

    if from.loader != to.loader
        && !(target_loader_supports(state, rule, &to).await
            && target_has_bundles(state, rule, &source, &to).await?)
    {
        return Ok(false);
    }

    migrate_cluster(state, rule, &source, &from, &to).await?;

    migration_dao::mark_applied(db, &rule.id).await?;
    tracing::info!(
        migration_id = %rule.id,
        cluster_id = source.id,
        from = %from.mc_version,
        to = %to.mc_version,
        from_loader = %from.loader,
        to_loader = %to.loader,
        "migrated cluster"
    );

    Ok(true)
}

async fn target_loader_supports(
    state: &LauncherState,
    rule: &RemoteMigration,
    to: &MigrationNode,
) -> bool {
    let mut metadata = state.metadata.lock().await;
    match metadata
        .loader_supports_version(&state.services.mc(), to.loader, &to.mc_version)
        .await
    {
        Ok(true) => true,
        Ok(false) => {
            tracing::warn!(
                migration_id = %rule.id,
                loader = %to.loader,
                mc_version = %to.mc_version,
                "target loader has no build for this version; skipping migration"
            );
            false
        }
        Err(err) => {
            tracing::warn!(
                migration_id = %rule.id,
                loader = %to.loader,
                error = %err,
                "could not read the target loader versions; retrying next launch"
            );
            false
        }
    }
}

async fn target_has_bundles(
    state: &LauncherState,
    rule: &RemoteMigration,
    source: &ClusterRow,
    to: &MigrationNode,
) -> LauncherResult<bool> {
    if rule.allow_without_bundles || source.kind() != ClusterKind::OneClient {
        return Ok(true);
    }

    let bundles = bundle_catalog_dao::list_visible_for_version_loader(
        &state.services.db,
        &to.mc_version,
        to.loader as i64,
    )
    .await?;

    if bundles.is_empty() {
        tracing::warn!(
            migration_id = %rule.id,
            loader = %to.loader,
            mc_version = %to.mc_version,
            "target has no bundles; skipping migration"
        );
        return Ok(false);
    }

    Ok(true)
}

async fn migrate_cluster(
    state: &LauncherState,
    rule: &RemoteMigration,
    source: &ClusterRow,
    from: &MigrationNode,
    to: &MigrationNode,
) -> LauncherResult<()> {
    let clusters_dir = oneclient_common::paths::clusters_dir()?;
    let old_dir = clusters_dir.join(&source.folder_name);

    let new_folder = resolve_new_folder(
        &clusters_dir,
        &source.folder_name,
        retarget_identity(&source.folder_name, from, to),
    )
    .await?;
    let new_name = retarget_identity(&source.name, from, to);

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

    let changes_loader = from.loader != to.loader;
    let migration = if changes_loader {
        ClusterMigration {
            mc_version: &to.mc_version,
            mc_loader: to.loader as i64,
            mc_loader_version: None,
            stage: ClusterStage::NotReady as i64,
            name: new_name.as_deref(),
            folder_name: folder_for_db,
        }
    } else {
        ClusterMigration {
            mc_version: &to.mc_version,
            mc_loader: source.mc_loader,
            mc_loader_version: source.mc_loader_version.as_deref(),
            stage: source.stage,
            name: new_name.as_deref(),
            folder_name: folder_for_db,
        }
    };

    if let Err(err) = cluster_dao::migrate_version(&state.services.db, source.id, migration).await {
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
        return Err(err.into());
    }

    if changes_loader && !keeps_mods(from.loader, to.loader) {
        match disable_mods(state, source.id).await {
            Ok(disabled) => tracing::info!(
                migration_id = %rule.id,
                cluster_id = source.id,
                disabled,
                "disabled mods built for the previous loader"
            ),
            Err(err) => tracing::warn!(
                migration_id = %rule.id,
                cluster_id = source.id,
                error = %err,
                "could not disable mods built for the previous loader"
            ),
        }
    }

    Ok(())
}

fn keeps_mods(from: GameLoader, to: GameLoader) -> bool {
    from == to || (from == GameLoader::Fabric && to == GameLoader::Quilt)
}

async fn disable_mods(state: &LauncherState, cluster_id: i64) -> LauncherResult<usize> {
    let ctx = state.services.content();
    let linked = PackageStore::list_linked_artifacts(cluster_id, &ctx).await?;
    let mut disabled = 0;

    for info in linked
        .iter()
        .filter(|info| info.content_type == ContentType::Mod)
    {
        if info.enabled {
            if let Err(err) =
                PackageStore::set_artifact_enabled_to(cluster_id, &info.hash, false, &ctx).await
            {
                tracing::warn!(
                    cluster_id,
                    hash = %info.hash,
                    error = %err,
                    "could not disable a mod built for the previous loader"
                );
                continue;
            }
            disabled += 1;
        }

        bundle_dao::clear_bundle_tracking(&state.services.db, cluster_id, &info.hash).await?;
    }

    Ok(disabled)
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
    candidate: Option<String>,
) -> LauncherResult<Option<String>> {
    let Some(candidate) = candidate else {
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

fn retarget_identity(value: &str, from: &MigrationNode, to: &MigrationNode) -> Option<String> {
    if from.loader == to.loader {
        return retarget_version_prefix(value, &from.mc_version, &to.mc_version);
    }

    let from_prefix = format!("{} {}", from.mc_version, from.loader);
    let rest = value
        .get(..from_prefix.len())
        .filter(|prefix| prefix.eq_ignore_ascii_case(&from_prefix))
        .map(|_| &value[from_prefix.len()..])?;
    if !(rest.is_empty() || rest.starts_with(' ')) {
        return None;
    }

    let retargeted = format!("{} {}{rest}", to.mc_version, to.loader);
    (retargeted != value).then_some(retargeted)
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

    fn node(mc_version: &str, loader: GameLoader) -> MigrationNode {
        MigrationNode {
            mc_version: mc_version.to_string(),
            loader,
        }
    }

    #[test]
    fn version_rules_keep_the_old_retargeting() {
        let from = node("26.1", GameLoader::Fabric);
        let to = node("26.1.2", GameLoader::Fabric);
        assert_eq!(
            retarget_identity("26.1 fabric (1)", &from, &to).as_deref(),
            Some("26.1.2 fabric (1)")
        );
        assert_eq!(
            retarget_identity("26.1 my pack", &from, &to).as_deref(),
            Some("26.1.2 my pack")
        );
    }

    #[test]
    fn loader_rules_retarget_generated_names() {
        let from = node("1.20.1", GameLoader::Forge);
        let to = node("1.20.1", GameLoader::NeoForge);
        assert_eq!(
            retarget_identity("1.20.1 Forge", &from, &to).as_deref(),
            Some("1.20.1 NeoForge")
        );
        assert_eq!(
            retarget_identity("1.20.1 forge (2)", &from, &to).as_deref(),
            Some("1.20.1 NeoForge (2)")
        );
    }

    #[test]
    fn loader_rules_leave_custom_names_alone() {
        let from = node("1.20.1", GameLoader::Forge);
        let to = node("1.20.1", GameLoader::NeoForge);
        assert_eq!(retarget_identity("1.20.1 my pack", &from, &to), None);
        assert_eq!(retarget_identity("1.20.1 Forgery", &from, &to), None);
        assert_eq!(retarget_identity("my cool pack", &from, &to), None);
    }

    #[test]
    fn loader_rules_with_the_same_display_name_do_not_rename() {
        let from = node("1.8.9", GameLoader::Fabric);
        let to = node("1.8.9", GameLoader::Ornithe);
        assert_eq!(retarget_identity("1.8.9 Fabric", &from, &to), None);
    }

    #[test]
    fn only_fabric_to_quilt_keeps_mods() {
        assert!(keeps_mods(GameLoader::Fabric, GameLoader::Quilt));
        assert!(!keeps_mods(GameLoader::Quilt, GameLoader::Fabric));
        assert!(!keeps_mods(GameLoader::Forge, GameLoader::NeoForge));
        assert!(!keeps_mods(GameLoader::Fabric, GameLoader::Vanilla));
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
