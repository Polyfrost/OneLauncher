//! Bundle-tracked artifacts are skipped
//! [`crate::bundles::updates`] owns those files and the two flows would fight
//! over the same artifact
//!
//! The `browser_package_updates` cache is only narrowed by a check that reached
//! the provider so an offline run is a no-op rather than "all up to date"

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use chrono::{DateTime, Utc};
use futures_util::{StreamExt};

use oneclient_common::domain::{ContentType, GameLoader, ProviderId};
use oneclient_db::dao::artifact as artifact_dao;
use oneclient_db::dao::browser_package_update as update_dao;
use oneclient_db::dao::cluster_bisect as bisect_dao;
use oneclient_db::dao::cluster_bundle as bundle_dao;
use oneclient_db::dao::cluster_flag::{self as flag_dao, ClusterFlag};
use oneclient_db::models::{
    BrowserPackageUpdateRow, ClusterReleaseRow, ClusterRow, OverrideType,
    SeenStatus,
};
use oneclient_events::{GroupedProgressChild};

use crate::bundles::install::{resolve_bundle_version, taken_bundle_names};
use crate::bundles::{
    BundleFile, BundleFileKind, BundlesManager, check_bundle_updates, cluster_has_bundle_content,
    effective_enabled,
};
use crate::ctx::ContentCtx;
use crate::error::{ContentError, ContentResult};
use crate::packages::dependencies::{
    DependencyResolution, ResolvedDependency, pick_version, resolve_required_or_unpinned, resolve_required_pinned, resolves_dependencies,
    };
use crate::packages::metadata_cache::with_recorded_dependencies;
use crate::packages::store::{PackageStore};
use crate::packages::types::{LinkedArtifactInfo, ProjectDetail, VersionDetail};
use crate::packages::{cached_project_detail, get_version_cached};

/// Matches the bundle installer's fan-out
/// the providers are the bottleneck
const UPDATE_CHECK_CONCURRENCY: usize = 6;

const PLAIN_LOOKUP_BUDGET: Duration = Duration::from_secs(4);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserPackageUpdate {
    pub cluster_id: i64,
    /// The artifact currently linked into the cluster
    pub hash: String,
    pub provider: ProviderId,
    pub project_id: String,
    pub installed_version_id: String,
    pub installed_version_name: String,
    pub latest_version_id: String,
    pub latest_version_name: String,
    /// Local fallback for a UI with no meta cache entry
    pub display_name: String,
    /// The user has already declined this exact newer version
    pub skipped: bool,
}

impl BrowserPackageUpdate {
    fn from_row(row: BrowserPackageUpdateRow) -> Self {
        Self {
            cluster_id: row.cluster_id,
            hash: row.hash,
            provider: ProviderId::from_repr(row.provider as u8).unwrap_or(ProviderId::Modrinth),
            project_id: row.project_id,
            installed_version_id: row.installed_version_id,
            installed_version_name: row.installed_version_name,
            latest_version_id: row.latest_version_id,
            latest_version_name: row.latest_version_name,
            display_name: row.display_name,
            skipped: row.skipped != 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BrowserUpdateCheck {
    pub cluster_id: i64,
    pub updates: Vec<BrowserPackageUpdate>,
    /// Unreachable this run
    /// their cached rows are preserved not cleared
    pub unchecked: Vec<String>,
    pub bundled: HashMap<String, String>,
    pub unchecked_pins: Vec<String>,
    pub pins_frozen: bool,
}

impl BrowserUpdateCheck {
    #[must_use]
    pub fn pending(&self) -> Vec<BrowserPackageUpdate> {
        self.not_skipped(false)
    }

    #[must_use]
    pub fn bundled_pending(&self) -> Vec<BrowserPackageUpdate> {
        if self.unchecked_pins.is_empty() && !self.pins_frozen {
            self.not_skipped(true)
        } else {
            Vec::new()
        }
    }

    fn not_skipped(&self, bundled: bool) -> Vec<BrowserPackageUpdate> {
        self.updates
            .iter()
            .filter(|u| !u.skipped && self.bundled.contains_key(&u.hash) == bundled)
            .cloned()
            .collect()
    }
}

pub(crate) struct Candidate {
    pub(crate) hash: String,
    pub(crate) provider: ProviderId,
    pub(crate) content_type: ContentType,
    pub(crate) project_id: String,
    pub(crate) version_id: String,
    pub(crate) display_name: String,
    pub(crate) display_version: String,
    pub(crate) published_at: Option<DateTime<Utc>>,
    pub(crate) enabled: bool,
}

/// A missing bundle tracking row is the marker for "the user added this themselves"
pub(crate) fn browser_installed(
    linked: &[LinkedArtifactInfo],
    bundle_hashes: &HashSet<String>,
    bundle_projects: &HashSet<String>,
) -> Vec<Candidate> {
    browser_installed_any(linked, bundle_hashes, bundle_projects)
        .into_iter()
        .filter(|candidate| candidate.enabled)
        .collect()
}

pub(crate) fn browser_installed_any(
    linked: &[LinkedArtifactInfo],
    bundle_hashes: &HashSet<String>,
    bundle_projects: &HashSet<String>,
) -> Vec<Candidate> {
    linked
        .iter()
        .filter(|info| !bundle_hashes.contains(&info.hash))
        .filter(|info| {
            info.project_id
                .as_ref()
                .is_none_or(|id| !bundle_projects.contains(id))
        })
        .filter_map(|info| {
            let provider = info.provider?;
            if provider == ProviderId::Local {
                return None;
            }
            Some(Candidate {
                hash: info.hash.clone(),
                provider,
                content_type: info.content_type,
                project_id: info.project_id.clone()?,
                version_id: info.version_id.clone()?,
                display_name: info
                    .display_name
                    .clone()
                    .unwrap_or_else(|| info.file_name.clone()),
                display_version: info.display_version.clone().unwrap_or_default(),
                published_at: info.published_at.as_deref().and_then(parse_published),
                enabled: info.enabled,
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BundlePin {
    pub(crate) version_id: String,
    pub(crate) sha1: String,
}

pub(crate) type BundlePins = HashMap<(ProviderId, String), BundlePin>;

fn pin_for<'a>(pins: &'a BundlePins, candidate: &Candidate) -> Option<&'a BundlePin> {
    pins.get(&(candidate.provider, candidate.project_id.clone()))
        .or_else(|| pins.values().find(|pin| pin.sha1 == candidate.hash))
}

fn pinned_release<'a>(
    pins: &'a BundlePins,
    releases: &'a [ClusterReleaseRow],
    hash: &str,
) -> Option<(&'a ClusterReleaseRow, ProviderId, &'a BundlePin)> {
    releases
        .iter()
        .filter(|release| release.hash == hash)
        .find_map(|release| {
            let provider = ProviderId::from_repr(release.provider as u8)?;
            let pin = pins.get(&(provider, release.project_id.clone()))?;
            Some((release, provider, pin))
        })
}

fn bundle_pins<'a>(files: impl Iterator<Item = &'a BundleFile>) -> BundlePins {
    let mut pins = HashMap::new();
    for file in files {
        if let BundleFileKind::Managed {
            provider,
            project_id,
            version_id,
            sha1,
        } = &file.kind
        {
            pins.entry((*provider, project_id.clone()))
                .or_insert_with(|| BundlePin {
                    version_id: version_id.clone(),
                    sha1: sha1.clone(),
                });
        }
    }
    pins
}

pub(crate) async fn cluster_bundle_pins(
    cluster: &ClusterRow,
    bundles: &BundlesManager,
    ctx: &ContentCtx,
) -> ContentResult<BundlePins> {
    let loader = GameLoader::from_repr(cluster.mc_loader as u8);
    let (true, Some(loader)) = (cluster.uses_bundles(), loader) else {
        return Ok(HashMap::new());
    };

    let archives = bundles
        .archives_for(ctx, &cluster.mc_version, loader)
        .await?;

    let taken = taken_bundle_names(cluster.id, &archives, ctx).await?;
    let live: HashSet<String> = bundle_dao::list_bundle_tracked(&ctx.db, cluster.id)
        .await?
        .into_iter()
        .filter(|row| row.enabled != 0)
        .filter_map(|row| row.bundle_name)
        .collect();
    let overrides: HashMap<(String, String), OverrideType> =
        bundle_dao::list_overrides(&ctx.db, cluster.id)
            .await?
            .into_iter()
            .filter_map(|row| {
                let override_type = OverrideType::parse(&row.override_type)?;
                Some(((row.bundle_name, row.package_id), override_type))
            })
            .collect();
    let mut ranked: Vec<_> = archives
        .iter()
        .flat_map(|archive| {
            let name = &archive.manifest.name;
            archive.manifest.files.iter().map(move |file| (name, file))
        })
        .collect();
    ranked.sort_by_key(|(name, file)| {
        let user_override = overrides
            .get(&((*name).clone(), file.kind.package_id()))
            .copied();
        (
            !live.contains(*name),
            !taken.contains(*name),
            !effective_enabled(file, user_override),
        )
    });

    Ok(bundle_pins(ranked.into_iter().map(|(_, file)| file)))
}

async fn plain_candidates(cluster_id: i64, ctx: &ContentCtx) -> ContentResult<Vec<Candidate>> {
    let linked = PackageStore::list_linked_artifacts(cluster_id, ctx).await?;

    let tracked = bundle_dao::list_bundle_tracked(&ctx.db, cluster_id).await?;
    let bundle_projects: HashSet<String> = tracked
        .iter()
        .filter_map(|row| row.package_id.clone())
        .collect();
    let bundle_hashes: HashSet<String> = tracked.into_iter().map(|row| row.hash).collect();

    Ok(browser_installed(&linked, &bundle_hashes, &bundle_projects))
}

async fn bind_to_pinned_release(
    candidates: &mut [Candidate],
    cluster_id: i64,
    pins: &BundlePins,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let releases = artifact_dao::list_cluster_releases(&ctx.db, cluster_id).await?;
    for candidate in candidates {
        if let Some((release, provider, _)) = pinned_release(pins, &releases, &candidate.hash) {
            candidate.provider = provider;
            candidate.project_id.clone_from(&release.project_id);
            candidate.version_id.clone_from(&release.version_id);
            candidate.published_at = artifact_dao::get_project_release_by_hash(
                &ctx.db,
                provider as i64,
                &release.project_id,
                &candidate.hash,
            )
            .await?
            .and_then(|row| row.published_at)
            .as_deref()
            .and_then(parse_published);
        }
    }
    Ok(())
}

async fn declined_versions(cluster_id: i64, ctx: &ContentCtx) -> ContentResult<HashSet<String>> {
    Ok(update_dao::list_for_cluster(&ctx.db, cluster_id)
        .await?
        .into_iter()
        .filter(|row| row.skipped != 0)
        .map(|row| format!("{}:{}", row.hash, row.latest_version_id))
        .collect())
}

fn is_on_pin(candidate: &Candidate, pin: &BundlePin) -> bool {
    pin.version_id == candidate.version_id || pin.sha1 == candidate.hash
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn cluster_wants_bundle_catalog(
    cluster_id: i64,
    ctx: &ContentCtx,
) -> ContentResult<bool> {
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    Ok(cluster.uses_bundles() && !plain_candidates(cluster_id, ctx).await?.is_empty())
}

#[tracing::instrument(level = "debug", skip(bundles, ctx))]
pub async fn bundle_changes_left(
    cluster_id: i64,
    bundles: &BundlesManager,
    ctx: &ContentCtx,
) -> ContentResult<Vec<String>> {
    if !flag_dao::is_set(&ctx.db, cluster_id, ClusterFlag::PinsUnsettled).await?
        || bisect_dao::is_active(&ctx.db, cluster_id).await?
        || !cluster_has_bundle_content(cluster_id, ctx).await?
    {
        return Ok(Vec::new());
    }

    let check = check_bundle_updates(cluster_id, bundles, ctx).await?;
    let updates = check.updates_available.iter().map(|u| &u.new_file);
    let additions = check.additions_available.iter().map(|a| &a.new_file);
    Ok(updates
        .chain(additions)
        .map(BundleFile::display_name)
        .chain(check.removals_available.into_iter().map(|r| r.package_id))
        .collect())
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn settle_pins(cluster_id: i64, ctx: &ContentCtx) -> ContentResult<()> {
    flag_dao::clear(&ctx.db, cluster_id, ClusterFlag::PinsUnsettled).await?;
    Ok(())
}

/// An unreachable project lands in [`BrowserUpdateCheck::unchecked`]
#[tracing::instrument(level = "debug", skip(bundles, ctx))]
pub async fn check_browser_package_updates(
    cluster_id: i64,
    plain_budget: Option<Duration>,
    bundles: &BundlesManager,
    ctx: &ContentCtx,
) -> ContentResult<BrowserUpdateCheck> {
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    let pins_frozen = bisect_dao::is_active(&ctx.db, cluster_id).await?;

    let mut candidates = plain_candidates(cluster_id, ctx).await?;
    if candidates.is_empty() {
        return Ok(BrowserUpdateCheck {
            cluster_id,
            pins_frozen,
            ..Default::default()
        });
    }

    let skipped = declined_versions(cluster_id, ctx).await?;

    let pins = cluster_bundle_pins(&cluster, bundles, ctx).await?;
    let pin_of = |candidate: &Candidate| pin_for(&pins, candidate);

    bind_to_pinned_release(&mut candidates, cluster_id, &pins, ctx).await?;
    candidates.sort_by_key(|candidate| pin_of(candidate).is_none());

    let plain_deadline = plain_budget.map(|budget| tokio::time::Instant::now() + budget);
    let results = futures_util::stream::iter(candidates.into_iter().map(|candidate| {
        let cluster = &cluster;
        let pin = pin_of(&candidate);
        let deadline = plain_deadline.filter(|_| pin.is_none());
        async move {
            let lookup = latest_for(&candidate, cluster, pin, ctx);
            let outcome = match deadline {
                Some(deadline) => match tokio::time::timeout_at(deadline, lookup).await {
                    Ok(outcome) => outcome,
                    Err(elapsed) => Err(std::io::Error::from(elapsed).into()),
                },
                None => lookup.await,
            };
            (candidate, outcome)
        }
    }))
    .buffer_unordered(UPDATE_CHECK_CONCURRENCY)
    .collect::<Vec<_>>()
    .await;

    let mut check = BrowserUpdateCheck {
        cluster_id,
        pins_frozen,
        ..Default::default()
    };

    for (candidate, outcome) in results {
        let pin = pin_of(&candidate);
        match outcome {
            Ok(Some((latest_version_id, latest_version_name))) => {
                let key = format!("{}:{latest_version_id}", candidate.hash);
                if let Some(pin) = pin {
                    check
                        .bundled
                        .insert(candidate.hash.clone(), pin.version_id.clone());
                }
                check.updates.push(BrowserPackageUpdate {
                    cluster_id,
                    hash: candidate.hash,
                    provider: candidate.provider,
                    project_id: candidate.project_id,
                    installed_version_id: candidate.version_id,
                    installed_version_name: candidate.display_version,
                    latest_version_id,
                    latest_version_name,
                    display_name: candidate.display_name,
                    skipped: pin.is_none() && skipped.contains(&key),
                });
            }
            Ok(None) => {}
            Err(err) => {
                // Offline is the common case and says nothing about the launcher
                if err.is_transient() {
                    tracing::debug!(
                        project_id = %candidate.project_id,
                        error = %err,
                        "update check skipped, provider unreachable"
                    );
                } else {
                    tracing::warn!(
                        project_id = %candidate.project_id,
                        error = %err,
                        "update check failed for package"
                    );
                }
                if pin.is_some() {
                    check.unchecked_pins.push(candidate.display_name);
                }
                check.unchecked.push(candidate.hash);
            }
        }
    }

    Ok(check)
}

fn parse_published(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|at| at.with_timezone(&Utc))
}

fn is_upgrade(installed_published: Option<DateTime<Utc>>, latest_published: DateTime<Utc>) -> bool {
    installed_published.is_none_or(|installed| latest_published > installed)
}

async fn latest_for(
    candidate: &Candidate,
    cluster: &ClusterRow,
    pin: Option<&BundlePin>,
    ctx: &ContentCtx,
) -> ContentResult<Option<(String, String)>> {
    if pin.is_some_and(|pin| is_on_pin(candidate, pin)) {
        return Ok(None);
    }

    let (version_id, name, version_number, published) = match pin {
        Some(pin) => {
            let version = resolve_bundle_version(
                candidate.provider,
                &candidate.project_id,
                &pin.version_id,
                &pin.sha1,
                ctx,
            )
            .await?;
            (
                version.version_id,
                version.name,
                version.version_number,
                version.published,
            )
        }
        None => {
            let provider = ctx.providers.get(candidate.provider)?;
            let Some(latest) = pick_version(provider, &candidate.project_id, cluster, ctx).await?
            else {
                return Ok(None);
            };
            (
                latest.version_id,
                latest.name,
                latest.version_number,
                latest.published,
            )
        }
    };

    if version_id == candidate.version_id {
        return Ok(None);
    }

    // A pin is a floor, so a copy newer than it stays
    if !is_upgrade(candidate.published_at, published) {
        tracing::debug!(
            project_id = %candidate.project_id,
            installed = %candidate.version_id,
            offered = %version_id,
            "update not offered, the installed version is not older than the pick"
        );
        return Ok(None);
    }

    let name = if version_number.is_empty() {
        name
    } else {
        version_number
    };

    Ok(Some((version_id, name)))
}

pub async fn refresh_browser_package_updates(
    cluster_id: i64,
    bundles: &BundlesManager,
    ctx: &ContentCtx,
) -> ContentResult<BrowserUpdateCheck> {
    refresh_with_budget(cluster_id, Some(PLAIN_LOOKUP_BUDGET), bundles, ctx).await
}

pub async fn refresh_all_browser_package_updates(
    cluster_id: i64,
    bundles: &BundlesManager,
    ctx: &ContentCtx,
) -> ContentResult<BrowserUpdateCheck> {
    refresh_with_budget(cluster_id, None, bundles, ctx).await
}

/// Unreported rows are dropped except unreachable ones which keep their last
/// known state
#[tracing::instrument(level = "debug", skip(bundles, ctx))]
async fn refresh_with_budget(
    cluster_id: i64,
    plain_budget: Option<Duration>,
    bundles: &BundlesManager,
    ctx: &ContentCtx,
) -> ContentResult<BrowserUpdateCheck> {
    let check = check_browser_package_updates(cluster_id, plain_budget, bundles, ctx).await?;

    let rows = update_dao::list_for_cluster(&ctx.db, cluster_id).await?;

    let mut keep: Vec<String> = check.unchecked.clone();
    let mut stored = Vec::with_capacity(check.updates.len());

    for update in &check.updates {
        let row = update_dao::upsert(
            &ctx.db,
            cluster_id,
            &update.hash,
            update.provider as i64,
            &update.project_id,
            &update.installed_version_id,
            &update.installed_version_name,
            &update.latest_version_id,
            &update.latest_version_name,
            &update.display_name,
            update.skipped,
            check.bundled.get(&update.hash).map(String::as_str),
        )
        .await?;
        keep.push(row.hash.clone());
        stored.push(BrowserPackageUpdate::from_row(row));
    }

    let disabled: HashSet<String> = PackageStore::list_linked_artifacts(cluster_id, ctx)
        .await?
        .into_iter()
        .filter(|info| !info.enabled)
        .map(|info| info.hash)
        .collect();
    keep.extend(
        rows.into_iter()
            .filter(|row| row.skipped != 0 && disabled.contains(&row.hash))
            .map(|row| row.hash),
    );

    update_dao::retain_hashes(&ctx.db, cluster_id, &keep).await?;

    Ok(BrowserUpdateCheck {
        updates: stored,
        ..check
    })
}

/// Never touches the network
#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn cached_browser_package_updates(
    cluster_id: i64,
    ctx: &ContentCtx,
) -> ContentResult<Vec<BrowserPackageUpdate>> {
    Ok(update_dao::list_for_cluster(&ctx.db, cluster_id)
        .await?
        .into_iter()
        .map(BrowserPackageUpdate::from_row)
        .collect())
}

/// The row stays and the package still reads as out of date
/// only the modal stops asking until a newer version than the declined one
/// appears
#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn skip_browser_package_update(
    cluster_id: i64,
    hash: &str,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    update_dao::set_skipped(&ctx.db, cluster_id, hash, true).await?;
    Ok(())
}

pub struct StagedPackageUpdate {
    update: BrowserPackageUpdate,
    project: ProjectDetail,
    version: VersionDetail,
    dependencies: Vec<ResolvedDependency>,
}

async fn ensure_not_bundle_owned(
    update: &BrowserPackageUpdate,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    // Never touch anything a bundle owns however this was called
    if bundle_dao::get_bundle_tracked(&ctx.db, update.cluster_id, &update.hash)
        .await?
        .is_some()
    {
        return Err(ContentError::InvalidData {
            reason: format!(
                "{} is provided by a bundle and is not updated by the browser flow",
                update.display_name
            ),
        });
    }
    Ok(())
}

async fn resolve_dependencies_on_pins(
    provider: ProviderId,
    cluster_id: i64,
    version: &VersionDetail,
    bundles: &BundlesManager,
    ctx: &ContentCtx,
) -> ContentResult<DependencyResolution> {
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    let pins = cluster_bundle_pins(&cluster, bundles, ctx).await?;
    resolve_required_pinned(provider, version, cluster_id, &pins, ctx).await
}

#[tracing::instrument(level = "debug", skip(update, bundles, child, ctx), fields(cluster_id = update.cluster_id, project_id = %update.project_id))]
pub async fn stage_browser_package_update(
    update: &BrowserPackageUpdate,
    bundles: &BundlesManager,
    child: Option<&GroupedProgressChild>,
    ctx: &ContentCtx,
) -> ContentResult<StagedPackageUpdate> {
    ensure_not_bundle_owned(update, ctx).await?;

    let content_type = artifact_dao::get_artifact_by_hash(&ctx.db, &update.hash)
        .await?
        .and_then(|artifact| ContentType::from_repr(artifact.content_type as u8))
        .unwrap_or(ContentType::Mod);
    let project =
        cached_project_detail(ctx, update.provider, &update.project_id, content_type).await;
    let version = get_version_cached(
        ctx,
        update.provider,
        &update.project_id,
        &update.latest_version_id,
    )
    .await?;
    let version = with_recorded_dependencies(ctx, update.provider, version).await?;

    let resolution = if resolves_dependencies(project.content_type) {
        let resolution = resolve_dependencies_on_pins(
            update.provider,
            update.cluster_id,
            &version,
            bundles,
            ctx,
        )
        .await?;
        if !resolution.unresolved.is_empty() {
            return Err(ContentError::InvalidData {
                reason: format!(
                    "{} needs {}, which could not be resolved",
                    update.display_name,
                    resolution.unresolved.join(", ")
                ),
            });
        }
        if !resolution.switched_off.is_empty() {
            return Err(ContentError::InvalidData {
                reason: format!(
                    "{} needs {}, which is switched off",
                    update.display_name,
                    resolution.switched_off.join(", ")
                ),
            });
        }
        resolution
    } else {
        DependencyResolution::default()
    };
    for dependency in &resolution.install {
        PackageStore::download_and_cache(
            update.provider,
            &dependency.project,
            &dependency.version,
            false,
            None,
            ctx,
        )
        .await?;
    }
    PackageStore::download_and_cache(update.provider, &project, &version, false, child, ctx)
        .await?;

    Ok(StagedPackageUpdate {
        update: update.clone(),
        project,
        version,
        dependencies: resolution.install,
    })
}

#[tracing::instrument(level = "debug", skip(staged, ctx), fields(cluster_id = staged.update.cluster_id, project_id = %staged.update.project_id))]
pub async fn apply_staged_package_update(
    staged: &StagedPackageUpdate,
    ctx: &ContentCtx,
) -> ContentResult<String> {
    flag_dao::set(
        &ctx.db,
        staged.update.cluster_id,
        ClusterFlag::PinsUnsettled,
        None,
    )
    .await?;

    for dependency in &staged.dependencies {
        install_dependency(
            staged.update.provider,
            staged.update.cluster_id,
            dependency,
            ctx,
        )
        .await?;
    }

    replace_package(&staged.update, &staged.project, &staged.version, None, ctx).await
}

/// Download first unlink second so a failed download leaves the cluster as it was
#[tracing::instrument(level = "debug", skip(update, bundles, child, ctx), fields(cluster_id = update.cluster_id, project_id = %update.project_id))]
pub async fn apply_browser_package_update(
    update: &BrowserPackageUpdate,
    bundles: &BundlesManager,
    child: Option<&GroupedProgressChild>,
    ctx: &ContentCtx,
) -> ContentResult<String> {
    ensure_not_bundle_owned(update, ctx).await?;

    let provider = ctx.providers.get(update.provider)?;
    let project = provider.get_project(&update.project_id, ctx).await?;
    let version = provider
        .get_version(&update.project_id, &update.latest_version_id, ctx)
        .await?;
    let dependencies = new_dependencies(
        update.provider,
        update.cluster_id,
        &project,
        &version,
        bundles,
        ctx,
    )
    .await?;

    for dependency in &dependencies {
        if let Err(err) =
            install_dependency(update.provider, update.cluster_id, dependency, ctx).await
        {
            tracing::warn!(
                dependency = %dependency.project.name,
                %err,
                "failed to install a dependency the newer version needs"
            );
        }
    }

    replace_package(update, &project, &version, child, ctx).await
}

async fn replace_package(
    update: &BrowserPackageUpdate,
    project: &ProjectDetail,
    version: &VersionDetail,
    child: Option<&GroupedProgressChild>,
    ctx: &ContentCtx,
) -> ContentResult<String> {
    let enabled_before =
        artifact_dao::get_cluster_artifact(&ctx.db, update.cluster_id, &update.hash)
            .await?
            .map(|link| link.enabled != 0);

    // Compatibility is not re-checked
    // a provider disagreeing at install time would strand the user on a build
    // they cannot move off
    let (installed, _) = PackageStore::install_to_cluster(
        update.provider,
        project,
        version,
        update.cluster_id,
        true,
        false,
        child,
        ctx,
    )
    .await?;

    // Provider handed back the already-linked file
    // leave the link untouched
    if installed.hash == update.hash {
        update_dao::delete(&ctx.db, update.cluster_id, &update.hash).await?;
        return Ok(installed.hash);
    }

    // A package switched off stays off
    // the fresh link starts enabled so this is a toggle rather than a set
    if enabled_before == Some(false) {
        PackageStore::set_artifact_enabled(update.cluster_id, &installed.hash, ctx).await?;
    }

    artifact_dao::set_seen_status(
        &ctx.db,
        update.cluster_id,
        &installed.hash,
        SeenStatus::Updated,
    )
    .await?;

    update_dao::delete(&ctx.db, update.cluster_id, &update.hash).await?;

    Ok(installed.hash)
}

#[tracing::instrument(level = "debug", skip(project, version, bundles, ctx))]
async fn new_dependencies(
    provider: ProviderId,
    cluster_id: i64,
    project: &ProjectDetail,
    version: &VersionDetail,
    bundles: &BundlesManager,
    ctx: &ContentCtx,
) -> ContentResult<Vec<ResolvedDependency>> {
    if !resolves_dependencies(project.content_type) {
        return Ok(Vec::new());
    }

    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    let mut pins = cluster_bundle_pins(&cluster, bundles, ctx).await?;
    let resolution =
        resolve_required_or_unpinned(provider, version, cluster_id, &mut pins, ctx).await?;

    for missing in &resolution.unresolved {
        tracing::warn!(dependency = %missing, "no compatible version for a new dependency");
    }

    Ok(resolution.install)
}

#[tracing::instrument(level = "debug", skip(dependency, ctx))]
async fn install_dependency(
    provider: ProviderId,
    cluster_id: i64,
    dependency: &ResolvedDependency,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let (artifact, _) = PackageStore::install_to_cluster(
        provider,
        &dependency.project,
        &dependency.version,
        cluster_id,
        true,
        false,
        None,
        ctx,
    )
    .await?;

    tracing::info!(
        dependency = %dependency.project.name,
        "installed a dependency the newer version needs"
    );
    if let Err(err) =
        artifact_dao::set_seen_status(&ctx.db, cluster_id, &artifact.hash, SeenStatus::New).await
    {
        tracing::debug!(%err, "could not mark a new dependency as new");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(raw: &str) -> DateTime<Utc> {
        parse_published(raw).expect("test dates are rfc 3339")
    }

    fn linked(
        hash: &str,
        provider: Option<ProviderId>,
        project_id: Option<&str>,
        version_id: Option<&str>,
    ) -> LinkedArtifactInfo {
        LinkedArtifactInfo {
            hash: hash.into(),
            cluster_file_name: format!("{hash}.jar"),
            enabled: true,
            content_type: ContentType::Mod,
            file_name: format!("{hash}.jar"),
            size_bytes: None,
            project_id: project_id.map(Into::into),
            version_id: version_id.map(Into::into),
            display_name: None,
            display_version: None,
            provider,
            published_at: None,
            added_at: None,
            seen_status: SeenStatus::Seen,
        }
    }

    #[test]
    fn bundle_provided_packages_are_never_candidates() {
        let linked = vec![linked(
            "a",
            Some(ProviderId::Modrinth),
            Some("sodium"),
            Some("v1"),
        )];
        let bundles: HashSet<String> = ["a".to_string()].into_iter().collect();

        assert!(
            browser_installed(&linked, &bundles, &HashSet::new()).is_empty(),
            "a bundle-tracked artifact must stay out of the browser update flow"
        );
    }

    #[test]
    fn browser_installed_remote_packages_are_candidates() {
        let linked = vec![linked(
            "a",
            Some(ProviderId::Modrinth),
            Some("sodium"),
            Some("v1"),
        )];

        let found = browser_installed(&linked, &HashSet::new(), &HashSet::new());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].project_id, "sodium");
        assert_eq!(found[0].version_id, "v1");
    }

    #[test]
    fn local_imports_are_not_candidates() {
        let linked = vec![
            linked("a", Some(ProviderId::Local), Some("x"), Some("v1")),
            linked("b", None, None, None),
        ];

        assert!(
            browser_installed(&linked, &HashSet::new(), &HashSet::new()).is_empty(),
            "a file with no provider release has no version to compare"
        );
    }

    #[test]
    fn a_remote_package_with_no_recorded_version_is_not_a_candidate() {
        let linked = vec![linked(
            "a",
            Some(ProviderId::Modrinth),
            Some("sodium"),
            None,
        )];

        assert!(
            browser_installed(&linked, &HashSet::new(), &HashSet::new()).is_empty(),
            "without an installed version id there is nothing to compare against"
        );
    }

    #[test]
    fn pending_hides_versions_the_user_declined() {
        let make = |hash: &str, skipped: bool| BrowserPackageUpdate {
            cluster_id: 1,
            hash: hash.into(),
            provider: ProviderId::Modrinth,
            project_id: "sodium".into(),
            installed_version_id: "v1".into(),
            installed_version_name: "1.0".into(),
            latest_version_id: "v2".into(),
            latest_version_name: "2.0".into(),
            display_name: "Sodium".into(),
            skipped,
        };

        let check = BrowserUpdateCheck {
            cluster_id: 1,
            updates: vec![make("a", true), make("b", false)],
            unchecked: Vec::new(),
            bundled: HashMap::new(),
            unchecked_pins: Vec::new(),
            pins_frozen: false,
        };

        let pending = check.pending();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].hash, "b");
    }

    #[test]
    fn a_pinned_update_leaves_the_mode_gated_list() {
        let make = |hash: &str, skipped: bool| BrowserPackageUpdate {
            cluster_id: 1,
            hash: hash.into(),
            provider: ProviderId::Modrinth,
            project_id: hash.into(),
            installed_version_id: "v1".into(),
            installed_version_name: "1.0".into(),
            latest_version_id: "v2".into(),
            latest_version_name: "2.0".into(),
            display_name: hash.into(),
            skipped,
        };

        let check = BrowserUpdateCheck {
            cluster_id: 1,
            updates: vec![
                make("pinned", false),
                make("declined", true),
                make("own", false),
            ],
            unchecked: Vec::new(),
            bundled: ["pinned", "declined"]
                .into_iter()
                .map(|hash| (hash.to_string(), "v2".to_string()))
                .collect(),
            unchecked_pins: Vec::new(),
            pins_frozen: false,
        };

        let hashes = |updates: Vec<BrowserPackageUpdate>| -> Vec<String> {
            updates.into_iter().map(|update| update.hash).collect()
        };
        assert_eq!(hashes(check.bundled_pending()), ["pinned"]);
        assert_eq!(hashes(check.pending()), ["own"]);

        let waiting = BrowserUpdateCheck {
            unchecked_pins: vec!["other".to_string()],
            ..check.clone()
        };
        assert!(
            waiting.bundled_pending().is_empty(),
            "a pin that could not be looked up holds the others back"
        );
        assert_eq!(hashes(waiting.pending()), ["own"]);

        let frozen = BrowserUpdateCheck {
            pins_frozen: true,
            ..check
        };
        assert!(frozen.bundled_pending().is_empty());
        assert_eq!(hashes(frozen.pending()), ["own"]);
    }

    #[test]
    fn a_declined_bundle_still_pins_what_it_ships() {
        let managed = |project: &str, version: &str| BundleFile {
            enabled: false,
            hidden: false,
            path: format!("mods/{project}.jar"),
            size: 1,
            file_type: crate::bundles::BundleFileType::Normal,
            kind: BundleFileKind::Managed {
                provider: ProviderId::Modrinth,
                project_id: project.into(),
                version_id: version.into(),
                sha1: "a".repeat(40),
            },
        };

        let pins = bundle_pins(
            [
                managed("evergreenhud", "v3.9.13"),
                managed("evergreenhud", "v0"),
            ]
            .iter(),
        );

        assert_eq!(pins.len(), 1);
        assert_eq!(
            pins.get(&(ProviderId::Modrinth, "evergreenhud".to_string())),
            Some(&BundlePin {
                version_id: "v3.9.13".to_string(),
                sha1: "a".repeat(40),
            }),
            "the first bundle listing a project sets its pin"
        );
    }

    #[test]
    fn a_newer_pick_is_an_upgrade() {
        assert!(is_upgrade(
            Some(at("2026-01-01T00:00:00Z")),
            at("2026-02-01T00:00:00Z")
        ));
    }

    #[test]
    fn a_pick_older_than_the_installed_build_is_not_an_upgrade() {
        assert!(
            !is_upgrade(Some(at("2026-02-01T00:00:00Z")), at("2026-01-01T00:00:00Z")),
            "a hand-installed build newer than the provider pick must not be offered a downgrade"
        );
    }

    #[test]
    fn a_pick_published_at_the_same_moment_is_not_an_upgrade() {
        assert!(!is_upgrade(
            Some(at("2026-02-01T00:00:00Z")),
            at("2026-02-01T00:00:00Z")
        ));
    }

    #[test]
    fn an_unknown_installed_date_still_offers_the_update() {
        assert!(is_upgrade(None, at("2026-02-01T00:00:00Z")));
    }

    #[test]
    fn candidates_carry_the_installed_publish_date() {
        let mut info = linked("a", Some(ProviderId::Modrinth), Some("sodium"), Some("v1"));
        info.published_at = Some("2026-02-01T00:00:00Z".into());

        let found = browser_installed(&[info], &HashSet::new(), &HashSet::new());
        assert_eq!(found[0].published_at, Some(at("2026-02-01T00:00:00Z")));
    }
    #[test]
    fn an_unparseable_publish_date_leaves_the_candidate_undated() {
        let mut info = linked("a", Some(ProviderId::Modrinth), Some("sodium"), Some("v1"));
        info.published_at = Some("not a date".into());

        let found = browser_installed(&[info], &HashSet::new(), &HashSet::new());
        assert_eq!(found[0].published_at, None);
    }
}
