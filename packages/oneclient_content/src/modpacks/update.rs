use oneclient_db::dao::artifact as artifact_dao;
use oneclient_db::models::{ClusterRow, ProviderReleaseRow};

use crate::ctx::ContentCtx;
use crate::error::ContentResult;
use crate::packages::store::{self, PackageStore};
use crate::packages::types::{ReleaseType, VersionSummary};
use oneclient_common::domain::{GameLoader, ProviderId};

const VERSIONS_LIMIT: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModpackRelease {
    pub provider: ProviderId,
    pub project_id: String,
    pub version_id: String,
    pub name: String,
    pub version: String,
    pub published_at: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ModpackUpdateStatus {
    Unpublished,
    UpToDate,
    Available(VersionSummary),
}

impl ModpackRelease {
    fn from_row(row: ProviderReleaseRow) -> Option<Self> {
        Some(Self {
            provider: ProviderId::from_repr(u8::try_from(row.provider).ok()?)?,
            project_id: row.project_id,
            version_id: row.version_id,
            name: row.display_name,
            version: row.display_version,
            published_at: row.published_at,
        })
    }
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn identify_modpack(
    hash: &str,
    ctx: &ContentCtx,
) -> ContentResult<Option<ModpackRelease>> {
    if let Some(row) = artifact_dao::get_release_by_hash(&ctx.db, hash).await? {
        return Ok(ModpackRelease::from_row(row));
    }

    let Some((provider, version)) = ctx.providers.lookup_version(hash, ctx).await? else {
        return Ok(None);
    };
    store::record_release(provider, &version, hash, ctx).await?;

    Ok(artifact_dao::get_release_by_hash(&ctx.db, hash)
        .await?
        .and_then(ModpackRelease::from_row))
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn cluster_modpack(
    cluster_id: i64,
    ctx: &ContentCtx,
) -> ContentResult<Option<ModpackRelease>> {
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    linked_release(&cluster, ctx).await
}

async fn linked_release(
    cluster: &ClusterRow,
    ctx: &ContentCtx,
) -> ContentResult<Option<ModpackRelease>> {
    match cluster.linked_modpack_hash.as_deref() {
        Some(hash) => identify_modpack(hash, ctx).await,
        None => Ok(None),
    }
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn check_modpack_update(
    cluster_id: i64,
    ctx: &ContentCtx,
) -> ContentResult<ModpackUpdateStatus> {
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    let Some(current) = linked_release(&cluster, ctx).await? else {
        return Ok(ModpackUpdateStatus::Unpublished);
    };

    let provider = ctx.providers.get(current.provider)?;
    let versions = provider
        .list_versions(
            &current.project_id,
            Some(&cluster.mc_version),
            None,
            0,
            VERSIONS_LIMIT,
            ctx,
        )
        .await?
        .items;

    let loader = cluster_loader(cluster.mc_loader);
    let versions = versions
        .into_iter()
        .filter(|version| {
            version.loaders.is_empty()
                || loader.is_none_or(|loader| version.loaders.contains(&loader))
        })
        .collect();

    Ok(match newest(versions, &current) {
        Some(latest) => ModpackUpdateStatus::Available(latest),
        None => ModpackUpdateStatus::UpToDate,
    })
}

fn cluster_loader(repr: i64) -> Option<GameLoader> {
    u8::try_from(repr).ok().and_then(GameLoader::from_repr)
}

fn newest(versions: Vec<VersionSummary>, current: &ModpackRelease) -> Option<VersionSummary> {
    let current_published = versions
        .iter()
        .find(|version| version.version_id == current.version_id)
        .map(|version| version.published)
        .or_else(|| {
            current
                .published_at
                .as_deref()
                .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
                .map(|at| at.with_timezone(&chrono::Utc))
        });

    let newer = versions.into_iter().filter(|version| {
        version.version_id != current.version_id
            && current_published.is_none_or(|published| version.published > published)
    });

    let (releases, others): (Vec<_>, Vec<_>) =
        newer.partition(|version| matches!(version.release_type, ReleaseType::Release));

    let latest =
        |list: Vec<VersionSummary>| list.into_iter().max_by_key(|version| version.published);
    latest(releases).or_else(|| latest(others))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn version(id: &str, day: u32, release_type: ReleaseType) -> VersionSummary {
        VersionSummary {
            version_id: id.into(),
            project_id: "pack".into(),
            name: id.into(),
            version_number: id.into(),
            published: Utc.with_ymd_and_hms(2026, 9, day, 0, 0, 0).unwrap(),
            release_type,
            game_versions: Vec::new(),
            loaders: Vec::new(),
            downloads: 0,
            file_size: 0,
            dependencies: Vec::new(),
        }
    }

    fn current(id: &str) -> ModpackRelease {
        ModpackRelease {
            provider: ProviderId::Modrinth,
            project_id: "pack".into(),
            version_id: id.into(),
            name: "Pack".into(),
            version: id.into(),
            published_at: None,
        }
    }

    #[test]
    fn the_newest_release_wins_over_a_newer_beta() {
        let versions = vec![
            version("v1", 1, ReleaseType::Release),
            version("v2", 5, ReleaseType::Release),
            version("v3-beta", 9, ReleaseType::Beta),
        ];

        let latest = newest(versions, &current("v1")).unwrap();
        assert_eq!(latest.version_id, "v2");
    }

    #[test]
    fn nothing_newer_means_no_update() {
        let versions = vec![
            version("v1", 1, ReleaseType::Release),
            version("v2", 5, ReleaseType::Release),
        ];

        assert!(newest(versions, &current("v2")).is_none());
    }

    #[test]
    fn a_beta_is_offered_when_no_release_is_newer() {
        let versions = vec![
            version("v2", 5, ReleaseType::Release),
            version("v3-beta", 9, ReleaseType::Beta),
        ];

        let latest = newest(versions, &current("v2")).unwrap();
        assert_eq!(latest.version_id, "v3-beta");
    }
}
