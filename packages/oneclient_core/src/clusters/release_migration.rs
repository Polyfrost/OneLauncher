use std::cmp::Reverse;
use std::collections::HashSet;
use std::path::Path;

use oneclient_common::domain::GameLoader;

use oneclient_cluster::Cluster;
use oneclient_common::version::parse_mc_version;
use oneclient_content::bundles::bundle_override_paths;
use oneclient_content::packages::release_migration::{
    has_migratable_packages, has_packages_to_migrate,
};

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
    Offer(Box<ReleaseMigrationOffer>),
}

fn version_order(mc_version: &str) -> Option<(u32, u32, u32)> {
    let parsed = parse_mc_version(mc_version)?;
    Some((
        parsed.major,
        parsed.minor.unwrap_or(0),
        parsed.patch.unwrap_or(0),
    ))
}

fn source_rank(
    target: (u32, u32, u32),
    source: (u32, u32, u32),
) -> (bool, Reverse<(u32, u32, u32)>) {
    (source > target, Reverse(source))
}

#[must_use]
pub fn can_migrate_manually(source: GameLoader, target: GameLoader) -> bool {
    source == target || (source == GameLoader::Fabric && target == GameLoader::Ornithe)
}

fn is_migration_source(
    candidate: &Cluster,
    target: &Cluster,
    loader_ok: impl Fn(GameLoader, GameLoader) -> bool,
) -> bool {
    candidate.id != target.id && loader_ok(candidate.mc_loader, target.mc_loader)
}

fn is_migration_destination(target: &ReleaseTarget, rules: &[RemoteMigration]) -> bool {
    rules.iter().any(|rule| {
        rule.endpoints()
            .is_some_and(|(_, to)| to.mc_version == target.mc_version && to.loader == target.loader)
    })
}

#[tracing::instrument(skip(state))]
pub async fn record_new_versions(state: &LauncherState) -> LauncherResult<()> {
    let rules = state.versions.migrations().await;
    let mut added: Vec<ReleaseTarget> = Vec::new();
    for target in state.versions.take_added_versions() {
        if is_migration_destination(&target, &rules) {
            continue;
        }
        if !state.versions.shows_initial_migration(&target).await {
            tracing::info!(
                mc_version = %target.mc_version,
                loader = %target.loader,
                "new version opts out of the initial migration offer"
            );
            continue;
        }
        added.push(target);
    }
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
        .filter(|cluster| {
            !cluster.user_created
                && cluster.mc_loader == release.loader
                && cluster.mc_version == release.mc_version
        })
        .min_by_key(|cluster| cluster.created_at)
    else {
        return Ok(OfferLookup::MissingCluster);
    };

    offer_for(target, &clusters, &content).await
}

#[tracing::instrument(skip(state))]
pub async fn manual_migration_offer(
    state: &LauncherState,
    target_cluster_id: i64,
    source_cluster_id: i64,
) -> LauncherResult<OfferLookup> {
    let clusters = state.clusters.list().await?;
    let content = state.services.content();

    let (Some(target), Some(source)) = (
        clusters
            .iter()
            .find(|cluster| cluster.id == target_cluster_id),
        clusters
            .iter()
            .find(|cluster| cluster.id == source_cluster_id),
    ) else {
        return Ok(OfferLookup::MissingCluster);
    };

    if !is_migration_source(source, target, can_migrate_manually)
        || !has_packages_to_migrate(source.id, target.id, &state.bundles, &content).await?
    {
        return Ok(OfferLookup::NoSources);
    }

    Ok(OfferLookup::Offer(Box::new(ReleaseMigrationOffer {
        release: ReleaseTarget {
            mc_version: target.mc_version.clone(),
            loader: target.mc_loader,
        },
        target: target.clone(),
        sources: vec![source.clone()],
    })))
}

#[must_use]
pub fn rank_migration_sources(target: &Cluster, clusters: &[Cluster]) -> Vec<Cluster> {
    let target_order = version_order(&target.mc_version);
    let mut sources: Vec<Cluster> = clusters
        .iter()
        .filter(|cluster| is_migration_source(cluster, target, can_migrate_manually))
        .cloned()
        .collect();
    sources.sort_by_key(|cluster| {
        (
            cluster.mc_loader != target.mc_loader,
            target_order
                .zip(version_order(&cluster.mc_version))
                .map(|(target, source)| source_rank(target, source)),
            Reverse(cluster.last_played),
        )
    });
    sources
}

async fn offer_for(
    target: &Cluster,
    clusters: &[Cluster],
    content: &oneclient_content::ContentCtx,
) -> LauncherResult<OfferLookup> {
    let Some(target_order) = version_order(&target.mc_version) else {
        return Ok(OfferLookup::NoSources);
    };

    let mut sources = Vec::new();
    for cluster in clusters {
        if cluster.user_created || !is_migration_source(cluster, target, |from, to| from == to) {
            continue;
        }
        if !version_order(&cluster.mc_version).is_some_and(|order| order != target_order) {
            continue;
        }
        if has_migratable_packages(cluster.id, content).await? {
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

    Ok(OfferLookup::Offer(Box::new(ReleaseMigrationOffer {
        release: ReleaseTarget {
            mc_version: target.mc_version.clone(),
            loader: target.mc_loader,
        },
        target: target.clone(),
        sources,
    })))
}

pub async fn copy_configs(
    state: &LauncherState,
    source: &Cluster,
    target: &Cluster,
) -> LauncherResult<usize> {
    if !target.uses_dedicated_dir() {
        return Ok(0);
    }
    let from = source.game_dir()?.join("config");
    let to = target.game_dir()?.join("config");
    if from == to || !polyio::try_exists(&from).await? {
        return Ok(0);
    }
    let bundled = bundle_override_paths(
        &state.bundles,
        &state.services.content(),
        &target.mc_version,
        target.mc_loader,
    )
    .await?;
    copy_missing(&from, &to, "config", &bundled).await
}

async fn copy_missing(
    from: &Path,
    to: &Path,
    rel: &str,
    skip: &HashSet<String>,
) -> LauncherResult<usize> {
    let mut copied = 0;
    let mut stack = vec![(from.to_path_buf(), to.to_path_buf(), rel.to_string())];
    while let Some((src, dst, rel)) = stack.pop() {
        polyio::create_dir_all(&dst).await?;
        let mut entries = polyio::read_dir(&src).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name();
            let child = dst.join(&name);
            let child_rel = format!("{rel}/{}", name.to_string_lossy());
            let file_type = entry.file_type().await?;
            if file_type.is_dir() {
                stack.push((entry.path(), child, child_rel));
            } else if file_type.is_file()
                && !skip.contains(&child_rel)
                && !polyio::try_exists(&child).await?
            {
                polyio::copy(entry.path(), &child).await?;
                copied += 1;
            }
        }
    }
    Ok(copied)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn copy_missing_keeps_existing_and_bundled_files() {
        let src = polyio::tempdir().await.expect("src dir");
        let dst = polyio::tempdir().await.expect("dst dir");
        let (src, dst) = (src.dir_path(), dst.dir_path());

        polyio::create_dir_all(src.join("sub")).await.unwrap();
        polyio::write(src.join("a.json"), b"old").await.unwrap();
        polyio::write(src.join("sub/b.json"), b"b").await.unwrap();
        polyio::write(src.join("sub/bundled.json"), b"bundled")
            .await
            .unwrap();
        polyio::write(dst.join("a.json"), b"keep").await.unwrap();

        let skip = HashSet::from(["config/sub/bundled.json".to_string()]);
        assert_eq!(copy_missing(src, dst, "config", &skip).await.unwrap(), 1);
        assert!(
            !polyio::try_exists(dst.join("sub/bundled.json"))
                .await
                .unwrap()
        );
        assert_eq!(
            polyio::read_to_string(dst.join("a.json")).await.unwrap(),
            "keep"
        );
        assert_eq!(
            polyio::read_to_string(dst.join("sub/b.json"))
                .await
                .unwrap(),
            "b"
        );
    }

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
            from: crate::versions::MigrationSource {
                mc_version: "26.1".into(),
                loader: "fabric".into(),
            },
            to: crate::versions::MigrationTarget {
                mc_version: "26.1.2".into(),
                loader: None,
            },
            allow_without_bundles: false,
        }];
        let moved = ReleaseTarget {
            mc_version: "26.1.2".into(),
            loader: GameLoader::Fabric,
        };
        let released = ReleaseTarget {
            mc_version: "26.3".into(),
            loader: GameLoader::Fabric,
        };
        let other_loader = ReleaseTarget {
            mc_version: "26.1.2".into(),
            loader: GameLoader::NeoForge,
        };

        assert!(is_migration_destination(&moved, &rules));
        assert!(!is_migration_destination(&released, &rules));
        assert!(!is_migration_destination(&other_loader, &rules));
    }

    #[test]
    fn a_loader_switch_destination_uses_the_target_loader() {
        let rules = vec![RemoteMigration {
            id: "x".into(),
            from: crate::versions::MigrationSource {
                mc_version: "1.20.1".into(),
                loader: "forge".into(),
            },
            to: crate::versions::MigrationTarget {
                mc_version: "1.20.1".into(),
                loader: Some("neoforge".into()),
            },
            allow_without_bundles: false,
        }];
        let destination = ReleaseTarget {
            mc_version: "1.20.1".into(),
            loader: GameLoader::NeoForge,
        };
        let source = ReleaseTarget {
            mc_version: "1.20.1".into(),
            loader: GameLoader::Forge,
        };

        assert!(is_migration_destination(&destination, &rules));
        assert!(!is_migration_destination(&source, &rules));
    }

    #[test]
    fn a_same_version_source_comes_before_older_ones() {
        let target = version_order("26.2").unwrap();
        let mut sources = vec!["26.1.2", "26.2", "1.21.11"];
        sources.sort_by_key(|v| source_rank(target, version_order(v).unwrap()));
        assert_eq!(sources, vec!["26.2", "26.1.2", "1.21.11"]);
    }

    #[test]
    fn fabric_sources_can_fill_an_ornithe_cluster_manually() {
        assert!(can_migrate_manually(
            GameLoader::Fabric,
            GameLoader::Ornithe
        ));
        assert!(can_migrate_manually(GameLoader::Fabric, GameLoader::Fabric));
        assert!(!can_migrate_manually(
            GameLoader::Ornithe,
            GameLoader::Fabric
        ));
        assert!(!can_migrate_manually(
            GameLoader::Forge,
            GameLoader::Ornithe
        ));
    }

    #[test]
    fn a_bare_minor_orders_below_its_patches() {
        assert!(version_order("26.3") < version_order("26.3.1"));
    }

    fn user_cluster(id: i64, mc_version: &str) -> Cluster {
        Cluster {
            id,
            name: format!("{mc_version} Fabric"),
            folder_name: format!("cluster-{id}"),
            setting_profile_name: None,
            mc_version: mc_version.to_string(),
            mc_loader: GameLoader::Fabric,
            mc_loader_version: None,
            stage: oneclient_cluster::ClusterStage::default(),
            created_at: None,
            last_played: None,
            overall_played: std::time::Duration::ZERO,
            linked_modpack_hash: None,
            kind: oneclient_cluster::ClusterKind::OneClient,
            user_created: true,
            description: None,
            tags: Vec::new(),
            cover_path: None,
        }
    }

    #[test]
    fn manual_migration_lists_user_created_sources() {
        let target = user_cluster(1, "26.2");
        let clusters = [target.clone(), user_cluster(2, "26.3")];
        let sources = rank_migration_sources(&target, &clusters);
        assert_eq!(sources.iter().map(|c| c.id).collect::<Vec<_>>(), [2]);
    }
}
