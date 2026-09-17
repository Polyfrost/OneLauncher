use std::cmp::Reverse;
use std::str::FromStr;

use oneclient_common::domain::GameLoader;

use oneclient_cluster::Cluster;
use oneclient_common::version::parse_mc_version;
use oneclient_content::packages::release_migration::has_migratable_packages;

use crate::LauncherResult;
use crate::state::LauncherState;
use crate::versions::{ReleaseTarget, RemoteMigration};

#[derive(Debug, Clone)]
pub struct ReleaseMigrationOffer {
    pub release: ReleaseTarget,
    pub target: Cluster,
    pub sources: Vec<Cluster>,
}

#[derive(Debug, Clone)]
pub enum OfferLookup {
    MissingCluster,
    NoSources,
    Offer(ReleaseMigrationOffer),
}

fn version_order(mc_version: &str) -> Option<(u32, u32, u32)> {
    let parsed = parse_mc_version(mc_version)?;
    Some((parsed.major, parsed.minor.unwrap_or(0), parsed.patch.unwrap_or(0)))
}

fn source_rank(target: (u32, u32, u32), source: (u32, u32, u32)) -> (bool, Reverse<(u32, u32, u32)>) {
    (source > target, Reverse(source))
}

fn is_migration_destination(target: &ReleaseTarget, rules: &[RemoteMigration]) -> bool {
    rules.iter().any(|rule| {
        rule.to.mc_version == target.mc_version
            && GameLoader::from_str(&rule.from.loader).is_ok_and(|loader| loader == target.loader)
    })
}

#[tracing::instrument(skip(state))]
pub async fn record_new_versions(state: &LauncherState) -> LauncherResult<()> {
    let rules = state.versions.migrations().await;
    let added: Vec<ReleaseTarget> = state
        .versions
        .take_added_versions()
        .into_iter()
        .filter(|target| !is_migration_destination(target, &rules))
        .collect();
    if added.is_empty() {
        return Ok(());
    }

    let updated = {
        let mut settings = state.settings.write();
        for target in &added {
            let key = target.key();
            if !settings.pending_release_migrations.contains(&key) {
                settings.pending_release_migrations.push(key);
            }
        }
        settings.clone()
    };

    crate::settings::store::save_settings(&updated).await
}

#[tracing::instrument(skip(state))]
pub async fn release_migration_offer(
    state: &LauncherState,
    release: &ReleaseTarget,
) -> LauncherResult<OfferLookup> {
    let clusters = state.clusters.list().await?;
    let content = state.services.content();

    let Some(target) = clusters
        .iter()
        .filter(|cluster| cluster.mc_loader == release.loader && cluster.mc_version == release.mc_version)
        .min_by_key(|cluster| cluster.created_at)
    else {
        return Ok(OfferLookup::MissingCluster);
    };
    let Some(target_order) = version_order(&target.mc_version) else {
        return Ok(OfferLookup::NoSources);
    };

    let mut sources = Vec::new();
    for cluster in &clusters {
        if cluster.id == target.id || cluster.mc_loader != release.loader {
            continue;
        }
        if !version_order(&cluster.mc_version).is_some_and(|order| order != target_order) {
            continue;
        }
        if has_migratable_packages(cluster.id, &content).await? {
            sources.push(cluster.clone());
        }
    }

    if sources.is_empty() {
        return Ok(OfferLookup::NoSources);
    }

    sources.sort_by_key(|cluster| {
        (
            version_order(&cluster.mc_version).map(|order| source_rank(target_order, order)),
            Reverse(cluster.last_played),
        )
    });

    Ok(OfferLookup::Offer(ReleaseMigrationOffer {
        release: release.clone(),
        target: target.clone(),
        sources,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_minor_orders_above_older_minor() {
        assert!(version_order("26.2") < version_order("26.3"));
        assert!(version_order("26.1.2") < version_order("26.2"));
        assert!(version_order("1.21.4") < version_order("26.1"));
    }

    #[test]
    fn upgrades_prefer_the_newest_older_cluster() {
        let target = version_order("26.3").unwrap();
        let mut sources = vec!["26.1", "26.4", "26.2", "1.21.4"];
        sources.sort_by_key(|v| source_rank(target, version_order(v).unwrap()));
        assert_eq!(sources, vec!["26.2", "26.1", "1.21.4", "26.4"]);
    }

    #[test]
    fn downgrades_prefer_the_newest_cluster() {
        let target = version_order("1.8.9").unwrap();
        let mut sources = vec!["1.21.4", "26.2", "26.1"];
        sources.sort_by_key(|v| source_rank(target, version_order(v).unwrap()));
        assert_eq!(sources, vec!["26.2", "26.1", "1.21.4"]);
    }

    #[test]
    fn a_version_that_existing_clusters_move_to_is_not_a_new_release() {
        let rules = vec![RemoteMigration {
            id: "x".into(),
            from: crate::versions::MigrationSource { mc_version: "26.1".into(), loader: "fabric".into() },
            to: crate::versions::MigrationTarget { mc_version: "26.1.2".into() },
        }];
        let moved = ReleaseTarget { mc_version: "26.1.2".into(), loader: GameLoader::Fabric };
        let released = ReleaseTarget { mc_version: "26.3".into(), loader: GameLoader::Fabric };
        let other_loader = ReleaseTarget { mc_version: "26.1.2".into(), loader: GameLoader::NeoForge };

        assert!(is_migration_destination(&moved, &rules));
        assert!(!is_migration_destination(&released, &rules));
        assert!(!is_migration_destination(&other_loader, &rules));
    }

    #[test]
    fn a_bare_minor_orders_below_its_patches() {
        assert!(version_order("26.3") < version_order("26.3.1"));
    }
}
