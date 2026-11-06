//! Bundle-tracked artifacts are skipped
//! [`crate::bundles::updates`] owns those files and the two flows would fight
//! over the same artifact
//!
//! The `browser_package_updates` cache is only narrowed by a check that reached
//! the provider so an offline run is a no-op rather than "all up to date"

use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;

use chrono::{DateTime, Utc};
use futures_util::{StreamExt, TryStreamExt};

use oneclient_common::domain::{ContentType, GameLoader, ProviderId};
use oneclient_db::dao::artifact as artifact_dao;
use oneclient_db::dao::browser_package_update as update_dao;
use oneclient_db::dao::cluster_bisect as bisect_dao;
use oneclient_db::dao::cluster_bundle as bundle_dao;
use oneclient_db::dao::cluster_flag::{self as flag_dao, ClusterFlag};
use oneclient_db::models::{
    BrowserPackageUpdateRow, ClusterDependencyEdgeRow, ClusterReleaseRow, ClusterRow, OverrideType,
    SeenStatus,
};
use oneclient_events::{GroupedProgressChild, VersionRange};

use crate::bundles::install::{resolve_bundle_version, taken_bundle_names};
use crate::bundles::{
    BundleFile, BundleFileKind, BundlesManager, check_bundle_updates, cluster_has_bundle_content,
    effective_enabled,
};
use crate::ctx::ContentCtx;
use crate::error::{ContentError, ContentResult};
use crate::packages::dependencies::{
    DependencyResolution, ResolvedDependency, compare_semver, maven_fits, number_among,
    pick_version, resolve_required_or_unpinned, resolve_required_pinned, resolves_dependencies,
    rules_out, semver_rules_out,
};
use crate::packages::dependents::ensure_cluster_dependencies;
use crate::packages::local_manifest::{JarDependencies, read_jar_dependencies};
use crate::packages::metadata_cache::with_recorded_dependencies;
use crate::packages::store::{PackageStore, artifact_absolute_path};
use crate::packages::types::{DependencyKind, LinkedArtifactInfo, ProjectDetail, VersionDetail};
use crate::packages::{cached_project_detail, get_version_cached};

/// Matches the bundle installer's fan-out
/// the providers are the bottleneck
const UPDATE_CHECK_CONCURRENCY: usize = 6;

const HELD_PIN_LOOKUP_BUDGET: Duration = Duration::from_secs(2);

const PLAIN_LOOKUP_BUDGET: Duration = Duration::from_secs(4);

const DEPENDENCY_BACKFILL_BUDGET: Duration = Duration::from_secs(2);

const TARGET_JAR_BUDGET: Duration = Duration::from_secs(3);

const UNBOUND_HOLD: &str = "";

const GAME_IDS: [&str; 2] = ["minecraft", "java"];

const FABRIC_MIXINEXTRAS: &str = "0.15.0";

fn loader_ids(loader: GameLoader) -> (&'static [&'static str], &'static [&'static str]) {
    match loader {
        GameLoader::Fabric | GameLoader::Ornithe => (&["fabricloader", "fabric-loader"], &[]),
        GameLoader::Quilt => (&["quilt_loader"], &["fabricloader", "fabric-loader"]),
        GameLoader::Forge => (&["forge"], &["fml", "javafml", "mcp", "mod_minecraftforge"]),
        GameLoader::NeoForge => (&["neoforge"], &["forge", "fml", "javafml", "mcp"]),
        GameLoader::Vanilla => (&[], &[]),
    }
}

fn has_mixinextras(loader: GameLoader, loader_version: Option<&str>) -> bool {
    !matches!(loader, GameLoader::Fabric | GameLoader::Ornithe)
        || loader_version.is_none_or(|version| {
            compare_semver(version, FABRIC_MIXINEXTRAS).is_none_or(|ordering| ordering.is_ge())
        })
}

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

async fn current_pin(
    cluster_id: i64,
    hash: &str,
    provider: ProviderId,
    project_id: &str,
    bundles: &BundlesManager,
    ctx: &ContentCtx,
) -> ContentResult<Option<BundlePin>> {
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    let pins = cluster_bundle_pins(&cluster, bundles, ctx).await?;
    let releases = artifact_dao::list_cluster_releases(&ctx.db, cluster_id).await?;

    Ok(pinned_release(&pins, &releases, hash)
        .map(|(_, _, pin)| pin)
        .or_else(|| pins.get(&(provider, project_id.to_string())))
        .cloned())
}

async fn awaits_first_catalog(cluster: &ClusterRow, ctx: &ContentCtx) -> ContentResult<bool> {
    Ok(cluster.uses_bundles()
        && oneclient_db::dao::bundle::list_all(&ctx.db)
            .await?
            .is_empty())
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
        }
    }
    Ok(())
}

async fn declined_versions(
    cluster_id: i64,
    ctx: &ContentCtx,
) -> ContentResult<(HashSet<String>, HashSet<String>)> {
    let rows = update_dao::list_for_cluster(&ctx.db, cluster_id).await?;
    let keys = |row: &BrowserPackageUpdateRow| {
        let pin = row
            .pin_version_id
            .as_ref()
            .map(|pin| format!("{}:{pin}", row.hash));
        [Some(format!("{}:{}", row.hash, row.latest_version_id)), pin]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
    };
    let skipped = || rows.iter().filter(|row| row.skipped != 0);

    Ok((
        skipped().flat_map(keys).collect(),
        skipped()
            .filter(|row| row.pin_version_id.is_some() || row.latest_version_id == UNBOUND_HOLD)
            .flat_map(keys)
            .collect(),
    ))
}

fn is_held(candidate: &Candidate, pin: &BundlePin, holds: &HashSet<String>) -> bool {
    holds.contains(&format!("{}:{UNBOUND_HOLD}", candidate.hash))
        || holds.contains(&format!("{}:{}", candidate.hash, pin.version_id))
}

fn is_on_pin(candidate: &Candidate, pin: &BundlePin) -> bool {
    pin.version_id == candidate.version_id || pin.sha1 == candidate.hash
}

fn held_copies(
    candidates: &[Candidate],
    pins: &BundlePins,
    holds: &HashSet<String>,
) -> HashSet<String> {
    candidates
        .iter()
        .filter(|candidate| {
            pin_for(pins, candidate)
                .is_some_and(|pin| !is_on_pin(candidate, pin) && is_held(candidate, pin, holds))
        })
        .map(|candidate| candidate.hash.clone())
        .collect()
}

fn required_versions(
    edges: &[ClusterDependencyEdgeRow],
    mut staying: HashSet<String>,
    moving: &HashMap<String, HashSet<(i64, String)>>,
) -> HashSet<(i64, String)> {
    loop {
        let required: HashSet<(i64, String)> = edges
            .iter()
            .filter(|edge| {
                staying.contains(&edge.hash)
                    && !edge.dependency_version_id.is_empty()
                    && DependencyKind::parse(&edge.kind) == Some(DependencyKind::Required)
            })
            .map(|edge| (edge.provider, edge.dependency_version_id.clone()))
            .collect();

        let held: Vec<String> = moving
            .iter()
            .filter(|(hash, versions)| !staying.contains(*hash) && !versions.is_disjoint(&required))
            .map(|(hash, _)| hash.clone())
            .collect();
        if held.is_empty() {
            return required;
        }
        staying.extend(held);
    }
}

#[derive(Debug, Clone, Default)]
struct VersionText {
    number: String,
    declared: Option<String>,
    aliases: BTreeMap<String, String>,
    semver: bool,
    unread: bool,
}

impl VersionText {
    fn from_jar(number: String, jar: &JarDependencies) -> Self {
        Self {
            number,
            declared: jar.version.clone(),
            aliases: jar.alias_versions.clone(),
            semver: jar.semver,
            unread: false,
        }
    }

    fn ruled_out(&self, id: &str, ranges: &[VersionRange], mc_version: &str) -> bool {
        match self.aliases.get(id).or(self.declared.as_ref()) {
            Some(declared) if self.semver => semver_rules_out(ranges, declared),
            Some(declared) => maven_fits(ranges, declared) == Some(false),
            None => rules_out(ranges, &self.number, mc_version),
        }
    }

    fn is_among(&self, id: &str, ranges: &[VersionRange], mc_version: &str) -> bool {
        match self.aliases.get(id).or(self.declared.as_ref()) {
            Some(declared) if self.semver => !semver_rules_out(ranges, declared),
            Some(declared) => maven_fits(ranges, declared) == Some(true),
            None => !self.semver && number_among(ranges, &self.number, mc_version),
        }
    }
}

fn ruled_out_by_jars(
    jars: &HashMap<String, JarDependencies>,
    staying: &HashSet<String>,
    targets: &HashMap<String, VersionText>,
    mc_version: &str,
) -> (Vec<String>, Vec<String>) {
    let cluster = || staying.iter().filter_map(|hash| jars.get(hash));
    let (unread, ruled_out): (Vec<_>, Vec<_>) = targets
        .iter()
        .filter(|(hash, target)| {
            let Some(mover) = jars.get(*hash) else {
                return false;
            };
            let broken = cluster()
                .flat_map(JarDependencies::all_breaks)
                .any(|(id, banned)| {
                    let clear = |version: &VersionText| !version.is_among(id, banned, mc_version);
                    mover.provides.contains(id)
                        && target.is_among(id, banned, mc_version)
                        && !a_copy_passes(cluster().map(|jar| (None, jar)), id, clear)
                });
            broken
                || cluster()
                    .flat_map(JarDependencies::all_ranges)
                    .any(|(id, ranges)| {
                        mover.provides.contains(id)
                            && target.ruled_out(id, ranges, mc_version)
                            && !a_copy_fits(
                                cluster().map(|jar| (None, jar)),
                                id,
                                ranges,
                                mc_version,
                            )
                    })
        })
        .partition(|(_, target)| target.unread);
    let hashes = |found: Vec<(&String, &VersionText)>| -> Vec<String> {
        found.into_iter().map(|(hash, _)| hash.clone()).collect()
    };
    (hashes(ruled_out), hashes(unread))
}

fn is_limited(jars: &HashMap<String, JarDependencies>, hash: &str) -> bool {
    jars.get(hash).is_some_and(|mover| {
        jars.values()
            .flat_map(|jar| jar.all_ranges().chain(jar.all_breaks()))
            .any(|(id, _)| mover.provides.contains(id))
    })
}

fn release_name<'a>(releases: &'a [ClusterReleaseRow], hash: &'a str) -> &'a str {
    releases
        .iter()
        .find(|release| release.hash == hash)
        .map_or(hash, |release| release.display_name.as_str())
}

fn a_copy_fits<'a>(
    jars: impl Iterator<Item = (Option<VersionText>, &'a JarDependencies)>,
    id: &str,
    ranges: &[VersionRange],
    mc_version: &str,
) -> bool {
    a_copy_passes(jars, id, |version| {
        !version.ruled_out(id, ranges, mc_version)
    })
}

fn a_copy_passes<'a>(
    mut jars: impl Iterator<Item = (Option<VersionText>, &'a JarDependencies)>,
    id: &str,
    fits: impl Fn(&VersionText) -> bool,
) -> bool {
    jars.any(|(version, jar)| {
        let own = jar.provides.iter().any(|provided| provided == id);
        let version = version.unwrap_or_else(|| VersionText::from_jar(String::new(), jar));
        (own && fits(&version))
            || jar.nested.iter().any(|nested| {
                nested.provides.iter().any(|provided| provided == id)
                    && fits(&VersionText::from_jar(String::new(), nested))
            })
    })
}

fn disputed_holds(
    held: &HashSet<String>,
    staying: &HashSet<String>,
    jars: &HashMap<String, JarDependencies>,
    edges: &[ClusterDependencyEdgeRow],
    releases: &[ClusterReleaseRow],
    mc_version: &str,
) -> Vec<String> {
    let needs_another_release = |hash: &str| {
        let own: Vec<&ClusterReleaseRow> = releases
            .iter()
            .filter(|release| release.hash == hash)
            .collect();
        edges.iter().any(|edge| {
            edge.hash != hash
                && staying.contains(&edge.hash)
                && !edge.dependency_version_id.is_empty()
                && DependencyKind::parse(&edge.kind) == Some(DependencyKind::Required)
                && own.iter().any(|release| {
                    release.provider == edge.provider
                        && release.project_id == edge.dependency_project_id
                })
                && !own.iter().any(|release| {
                    release.provider == edge.provider
                        && release.version_id == edge.dependency_version_id
                })
        })
    };
    let outside_a_range = |hash: &str| {
        let Some(mover) = jars.get(hash) else {
            return false;
        };
        let cluster = || staying.iter().filter_map(|other| jars.get(other));
        cluster()
            .filter(|jar| !std::ptr::eq(*jar, mover))
            .flat_map(JarDependencies::all_ranges)
            .any(|(id, ranges)| {
                mover.provides.contains(id)
                    && !a_copy_fits(cluster().map(|jar| (None, jar)), id, ranges, mc_version)
            })
    };

    let mut disputed: Vec<String> = held
        .iter()
        .filter(|hash| needs_another_release(hash) || outside_a_range(hash))
        .map(|hash| {
            format!(
                "{} is needed on two different versions",
                release_name(releases, hash)
            )
        })
        .collect();
    disputed.sort();
    disputed
}

#[derive(Debug, Default)]
struct PinRequirements {
    held: HashSet<String>,
    unknown: Vec<String>,
    disputed: Vec<String>,
}

async fn backfill_dependencies(
    cluster_id: i64,
    budget: Option<Duration>,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let backfill = tokio::spawn({
        let ctx = ctx.clone();
        async move { ensure_cluster_dependencies(cluster_id, &ctx).await }
    });
    let finished = match budget {
        Some(budget) => tokio::time::timeout(budget, backfill).await.ok(),
        None => Some(backfill.await),
    };
    if let Some(Ok(backfilled)) = finished {
        backfilled?;
    }
    Ok(())
}

async fn target_jar(
    candidate: &Candidate,
    pin: &BundlePin,
    loader: Option<GameLoader>,
    deadline: Option<tokio::time::Instant>,
    ctx: &ContentCtx,
) -> Result<Option<JarDependencies>, tokio::time::error::Elapsed> {
    let read = tokio::spawn({
        let provider = candidate.provider;
        let project_id = candidate.project_id.clone();
        let pin = pin.clone();
        let ctx = ctx.clone();
        async move {
            let version =
                resolve_bundle_version(provider, &project_id, &pin.version_id, &pin.sha1, &ctx)
                    .await
                    .ok()?;
            let project =
                cached_project_detail(&ctx, provider, &project_id, ContentType::Mod).await;
            let artifact =
                PackageStore::download_and_cache(provider, &project, &version, false, None, &ctx)
                    .await
                    .inspect_err(|err| tracing::debug!(%err, "could not read a pin's jar"))
                    .ok()?;
            Some(read_jar_dependencies(&artifact_absolute_path(&artifact.path).ok()?, loader).await)
        }
    });

    let finished = match deadline {
        Some(deadline) => tokio::time::timeout_at(deadline, read).await?,
        None => read.await,
    };
    Ok(finished.ok().flatten())
}

async fn enabled_jars(
    linked: &[LinkedArtifactInfo],
    loader: Option<GameLoader>,
    ctx: &ContentCtx,
) -> ContentResult<HashMap<String, JarDependencies>> {
    let hashes: Vec<String> = linked
        .iter()
        .filter(|info| info.enabled && info.content_type == ContentType::Mod)
        .map(|info| info.hash.clone())
        .collect();

    futures_util::stream::iter(hashes)
        .map(|hash| async move {
            let dependencies = match artifact_dao::get_artifact_by_hash(&ctx.db, &hash).await? {
                Some(artifact) => {
                    read_jar_dependencies(&artifact_absolute_path(&artifact.path)?, loader).await
                }
                None => JarDependencies::default(),
            };
            ContentResult::Ok((hash, dependencies))
        })
        .buffer_unordered(UPDATE_CHECK_CONCURRENCY)
        .try_filter(|(_, jar)| std::future::ready(!jar.server_only))
        .try_collect()
        .await
}

async fn pin_requirements(
    cluster_id: i64,
    movers: &HashSet<String>,
    user_held: &HashSet<String>,
    targets: &HashMap<String, VersionText>,
    jars: &HashMap<String, JarDependencies>,
    mc_version: &str,
    ctx: &ContentCtx,
) -> ContentResult<PinRequirements> {
    let releases = artifact_dao::list_cluster_releases(&ctx.db, cluster_id).await?;
    let edges = artifact_dao::list_cluster_dependency_edges(&ctx.db, cluster_id).await?;
    let linked = PackageStore::list_linked_artifacts(cluster_id, ctx).await?;

    let mut moving: HashMap<String, HashSet<(i64, String)>> = HashMap::new();
    for release in releases.iter().filter(|r| movers.contains(&r.hash)) {
        moving
            .entry(release.hash.clone())
            .or_default()
            .insert((release.provider, release.version_id.clone()));
    }
    let mut staying: HashSet<String> = linked
        .iter()
        .filter(|info| info.enabled && !movers.contains(&info.hash))
        .map(|info| info.hash.clone())
        .collect();

    let mut held: HashSet<String> = HashSet::new();
    let unread = loop {
        let required = required_versions(&edges, staying.clone(), &moving);
        let before = held.len();
        held.extend(
            moving
                .iter()
                .filter(|(_, versions)| !versions.is_disjoint(&required))
                .map(|(hash, _)| hash.clone()),
        );
        let (ruled_out, waiting) = ruled_out_by_jars(jars, &staying, targets, mc_version);
        held.extend(ruled_out);
        if held.len() == before {
            break waiting;
        }
        staying.extend(held.iter().cloned());
    };
    let contested: HashSet<String> = held.union(user_held).cloned().collect();
    let disputed = disputed_holds(&contested, &staying, jars, &edges, &releases, mc_version);
    if held.len() == movers.len() {
        return Ok(PinRequirements {
            held,
            unknown: Vec::new(),
            disputed,
        });
    }

    let mut unknown: Vec<String> = releases
        .iter()
        .filter(|release| {
            release.enabled != 0
                && !release.dependencies_settled
                && release.provider != ProviderId::Local as i64
                && (!movers.contains(&release.hash) || held.contains(&release.hash))
        })
        .map(|release| release.display_name.clone())
        .collect();
    unknown.extend(
        unread
            .iter()
            .filter(|hash| !held.contains(*hash))
            .map(|hash| release_name(&releases, hash).to_string()),
    );
    unknown.sort();
    unknown.dedup();

    Ok(PinRequirements {
        held,
        unknown,
        disputed,
    })
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
pub async fn pins_must_settle(
    cluster_id: i64,
    bundles: &BundlesManager,
    ctx: &ContentCtx,
) -> ContentResult<bool> {
    if !flag_dao::is_set(&ctx.db, cluster_id, ClusterFlag::PinsUnsettled).await?
        || bisect_dao::is_active(&ctx.db, cluster_id).await?
    {
        return Ok(false);
    }

    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    let pins = cluster_bundle_pins(&cluster, bundles, ctx).await?;
    let (_, holds) = declined_versions(cluster_id, ctx).await?;
    let mut candidates = plain_candidates(cluster_id, ctx).await?;
    bind_to_pinned_release(&mut candidates, cluster_id, &pins, ctx).await?;
    let behind = candidates.iter().any(|candidate| {
        pin_for(&pins, candidate)
            .is_some_and(|pin| !is_on_pin(candidate, pin) && !is_held(candidate, pin, &holds))
    });
    if behind {
        return Ok(true);
    }

    let held = held_copies(&candidates, &pins, &holds);
    if held.is_empty() {
        return Ok(false);
    }
    let linked = PackageStore::list_linked_artifacts(cluster_id, ctx).await?;
    let requirements = pin_requirements(
        cluster_id,
        &HashSet::new(),
        &held,
        &HashMap::new(),
        &enabled_jars(&linked, GameLoader::from_repr(cluster.mc_loader as u8), ctx).await?,
        &cluster.mc_version,
        ctx,
    )
    .await?;
    Ok(!requirements.disputed.is_empty())
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

    let (skipped, holds) = declined_versions(cluster_id, ctx).await?;

    let pins = cluster_bundle_pins(&cluster, bundles, ctx).await?;
    let pin_of = |candidate: &Candidate| pin_for(&pins, candidate);

    bind_to_pinned_release(&mut candidates, cluster_id, &pins, ctx).await?;
    candidates.sort_by_key(|candidate| pin_of(candidate).is_none());

    let movers: HashSet<String> = candidates
        .iter()
        .filter(|candidate| {
            pin_of(candidate)
                .is_some_and(|pin| !is_on_pin(candidate, pin) && !is_held(candidate, pin, &holds))
        })
        .map(|candidate| candidate.hash.clone())
        .collect();
    let user_held = held_copies(&candidates, &pins, &holds);
    let nothing_to_check = movers.is_empty() && user_held.is_empty();
    let requirements = if nothing_to_check {
        PinRequirements::default()
    } else {
        let budget = plain_budget.map(|_| DEPENDENCY_BACKFILL_BUDGET);
        backfill_dependencies(cluster_id, budget, ctx).await?;
        pin_requirements(
            cluster_id,
            &movers,
            &user_held,
            &HashMap::new(),
            &HashMap::new(),
            &cluster.mc_version,
            ctx,
        )
        .await?
    };

    let now = tokio::time::Instant::now();
    let held_deadline = now + HELD_PIN_LOOKUP_BUDGET;
    let plain_deadline = plain_budget.map(|budget| now + budget);
    let results = futures_util::stream::iter(candidates.into_iter().map(|candidate| {
        let cluster = &cluster;
        let pin = pin_of(&candidate);
        let held = pin.is_some_and(|pin| is_held(&candidate, pin, &holds));
        let required = requirements.held.contains(&candidate.hash);
        let deadline = if held {
            Some(held_deadline)
        } else if pin.is_none() {
            plain_deadline
        } else {
            None
        };
        async move {
            let outcome = if required {
                Ok(None)
            } else if let Some(deadline) = deadline {
                let lookup = latest_for(&candidate, cluster, pin, ctx);
                match tokio::time::timeout_at(deadline, lookup).await {
                    Ok(outcome) => outcome,
                    Err(elapsed) => Err(std::io::Error::from(elapsed).into()),
                }
            } else {
                latest_for(&candidate, cluster, pin, ctx).await
            };
            (candidate, outcome)
        }
    }))
    .buffer_unordered(UPDATE_CHECK_CONCURRENCY)
    .collect::<Vec<_>>()
    .await;

    let requirements = if nothing_to_check {
        requirements
    } else {
        let linked = PackageStore::list_linked_artifacts(cluster_id, ctx).await?;
        let loader = GameLoader::from_repr(cluster.mc_loader as u8);
        let jars = &enabled_jars(&linked, loader, ctx).await?;
        let deadline = plain_budget.map(|_| tokio::time::Instant::now() + TARGET_JAR_BUDGET);
        let mut lookups = Vec::new();
        for (candidate, outcome) in &results {
            let Ok(Some((_, number))) = outcome else {
                continue;
            };
            if !movers.contains(&candidate.hash) {
                continue;
            }
            let pin = pin_of(candidate);
            lookups.push(async move {
                let number = number.clone();
                let target = match pin {
                    Some(pin) if is_limited(jars, &candidate.hash) => {
                        match target_jar(candidate, pin, loader, deadline, ctx).await {
                            Ok(Some(jar)) => VersionText::from_jar(number, &jar),
                            Ok(None) => VersionText {
                                number,
                                ..VersionText::default()
                            },
                            Err(_) => VersionText {
                                number,
                                unread: true,
                                ..VersionText::default()
                            },
                        }
                    }
                    _ => VersionText {
                        number,
                        ..VersionText::default()
                    },
                };
                (candidate.hash.clone(), target)
            });
        }
        let targets: HashMap<String, VersionText> = futures_util::stream::iter(lookups)
            .buffer_unordered(UPDATE_CHECK_CONCURRENCY)
            .collect()
            .await;
        pin_requirements(
            cluster_id,
            &movers,
            &user_held,
            &targets,
            jars,
            &cluster.mc_version,
            ctx,
        )
        .await?
    };

    let mut check = BrowserUpdateCheck {
        cluster_id,
        pins_frozen,
        unchecked_pins: [requirements.unknown, requirements.disputed].concat(),
        ..Default::default()
    };

    for (candidate, outcome) in results {
        let pin = pin_of(&candidate);
        let held = pin.is_some_and(|pin| is_held(&candidate, pin, &holds));
        let outcome = if requirements.held.contains(&candidate.hash) {
            tracing::warn!(
                project_id = %candidate.project_id,
                version_id = %candidate.version_id,
                "pin not followed, an installed mod needs the version it is on"
            );
            Ok(None)
        } else {
            outcome
        };
        match outcome {
            Ok(Some((latest_version_id, latest_version_name))) => {
                let key = format!("{}:{latest_version_id}", candidate.hash);
                let declined = if pin.is_some() { &holds } else { &skipped };
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
                    skipped: held || declined.contains(&key),
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
                if pin.is_some() && !held {
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

    if pin.is_none() && !is_upgrade(candidate.published_at, published) {
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
    let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
    let unbound: HashSet<String> = if awaits_first_catalog(&cluster, ctx).await? {
        rows.iter()
            .filter(|row| row.skipped != 0 && row.latest_version_id == UNBOUND_HOLD)
            .map(|row| row.hash.clone())
            .collect()
    } else {
        HashSet::new()
    };

    let mut keep: Vec<String> = check.unchecked.clone();
    keep.extend(unbound.iter().cloned());
    let mut stored = Vec::with_capacity(check.updates.len());

    for update in check
        .updates
        .iter()
        .filter(|update| !unbound.contains(&update.hash))
    {
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
        .filter(|row| row.skipped == 0 || !row.latest_version_name.is_empty())
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

#[tracing::instrument(level = "debug", skip(bundles, ctx))]
pub async fn hold_against_bundle_pin(
    cluster_id: i64,
    hash: &str,
    provider: ProviderId,
    project_id: &str,
    version_id: &str,
    bundles: &BundlesManager,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let pin = match current_pin(cluster_id, hash, provider, project_id, bundles, ctx).await {
        Ok(Some(pin)) => Some(pin.version_id),
        Ok(None) => {
            let cluster = PackageStore::get_cluster(cluster_id, ctx).await?;
            if !awaits_first_catalog(&cluster, ctx).await? {
                return Ok(());
            }
            None
        }
        Err(err) => {
            tracing::debug!(%err, "holding a version with no readable pin");
            None
        }
    };

    update_dao::upsert(
        &ctx.db,
        cluster_id,
        hash,
        provider as i64,
        project_id,
        version_id,
        "",
        pin.as_deref().unwrap_or(UNBOUND_HOLD),
        "",
        "",
        true,
        pin.as_deref(),
    )
    .await?;
    Ok(())
}

#[tracing::instrument(level = "debug", skip(bundles, ctx))]
pub async fn hold_import_against_bundle_pin(
    cluster_id: i64,
    hash: &str,
    bundles: &BundlesManager,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let Some(release) = artifact_dao::get_release_by_hash(&ctx.db, hash).await? else {
        return Ok(());
    };
    let Some(provider) = ProviderId::from_repr(release.provider as u8) else {
        return Ok(());
    };

    hold_against_bundle_pin(
        cluster_id,
        hash,
        provider,
        &release.project_id,
        &release.version_id,
        bundles,
        ctx,
    )
    .await
}

pub struct StagedPackageUpdate {
    update: BrowserPackageUpdate,
    project: ProjectDetail,
    version: VersionDetail,
    dependencies: Vec<ResolvedDependency>,
    exact: Vec<(String, String)>,
}

struct FinalJar {
    name: String,
    version: VersionText,
    jar: JarDependencies,
    landing: bool,
    requested: bool,
    on_pin: bool,
}

fn jar_range_conflicts(mods: &[FinalJar], mc_version: &str) -> Vec<String> {
    conflicts_moved_by(mods, mc_version, |jar| jar.landing)
}

struct Demand<'a> {
    dependent: &'a FinalJar,
    ranges: &'a [VersionRange],
    ban: bool,
}

impl Demand<'_> {
    fn allows(&self, id: &str, version: &VersionText, mc_version: &str) -> bool {
        if self.ban {
            !version.is_among(id, self.ranges, mc_version)
        } else {
            !version.ruled_out(id, self.ranges, mc_version)
        }
    }

    fn conflict(&self, id: &str) -> String {
        let name = &self.dependent.name;
        if self.ban {
            format!("{name} cannot run beside {id}")
        } else {
            format!("{name} needs another version of {id}")
        }
    }
}

fn conflicts_moved_by(
    mods: &[FinalJar],
    mc_version: &str,
    limited: impl Fn(&FinalJar) -> bool,
) -> Vec<String> {
    let mut demands: BTreeMap<&str, Vec<Demand>> = BTreeMap::new();
    for dependent in mods {
        let ranges = dependent.jar.all_ranges().map(|entry| (entry, false));
        let bans = dependent.jar.all_breaks().map(|entry| (entry, true));
        for ((id, ranges), ban) in ranges.chain(bans) {
            demands.entry(id).or_default().push(Demand {
                dependent,
                ranges,
                ban,
            });
        }
    }

    let mut conflicts = Vec::new();
    for (id, demands) in demands {
        let copies: Vec<&FinalJar> = mods
            .iter()
            .filter(|jar| jar.jar.all_ids().any(|provided| provided == id))
            .collect();
        let held_back = copies.iter().any(|jar| limited(jar));
        let landed = copies.iter().any(|jar| jar.landing);
        let moved = held_back
            || demands
                .iter()
                .any(|demand| demand.dependent.landing || (landed && demand.dependent.on_pin));
        if !moved || copies.is_empty() {
            continue;
        }

        let forced = !held_back && landed;
        let counted: Vec<&Demand> = demands
            .iter()
            .filter(|demand| !forced || demand.dependent.landing || demand.dependent.on_pin)
            .collect();
        let passes = |fits: &dyn Fn(&VersionText) -> bool| {
            let jars = copies
                .iter()
                .map(|jar| (Some(jar.version.clone()), &jar.jar));
            a_copy_passes(jars, id, fits)
        };

        let unmet = counted
            .iter()
            .filter(|demand| !passes(&|version| demand.allows(id, version, mc_version)));
        let before = conflicts.len();
        conflicts.extend(unmet.map(|demand| demand.conflict(id)));
        let together = |version: &VersionText| {
            counted
                .iter()
                .all(|demand| demand.allows(id, version, mc_version))
        };
        if conflicts.len() == before && !passes(&together) {
            conflicts.push(format!("no version of {id} suits every mod that needs it"));
        }
    }
    conflicts
}

fn jar_missing_mods(mods: &[FinalJar], lost: &HashSet<String>) -> Vec<String> {
    if mods.iter().any(|found| found.jar.provides.is_empty()) {
        return Vec::new();
    }

    let provided: HashSet<&str> = mods
        .iter()
        .flat_map(|found| found.jar.all_ids())
        .map(String::as_str)
        .collect();

    mods.iter()
        .filter(|found| !found.jar.multi_loader)
        .flat_map(|found| {
            std::iter::once(&found.jar)
                .chain(&found.jar.nested)
                .filter(|jar| !jar.multi_loader)
                .flat_map(|jar| &jar.requires)
                .filter(|id| found.landing || lost.contains(*id))
                .filter(|id| !provided.contains(id.as_str()))
                .map(|id| format!("{} needs {id}, which is not installed", found.name))
        })
        .collect()
}

fn game_jar(cluster: &ClusterRow, java_major: Option<u32>) -> FinalJar {
    let loader = GameLoader::from_repr(cluster.mc_loader as u8).unwrap_or(GameLoader::Vanilla);
    let (versioned, others) = loader_ids(loader);

    let mut versions = BTreeMap::new();
    let released = compare_semver(&cluster.mc_version, &cluster.mc_version).is_some();
    if released && !cluster.mc_version.contains('-') {
        versions.insert("minecraft".to_string(), cluster.mc_version.clone());
    }
    if let Some(java_major) = java_major {
        versions.insert("java".to_string(), java_major.to_string());
    }
    if let Some(version) = &cluster.mc_loader_version {
        let version = version
            .strip_prefix(&format!("{}-", cluster.mc_version))
            .unwrap_or(version);
        for id in versioned {
            versions.insert((*id).to_string(), version.to_string());
        }
    }

    let mixinextras =
        has_mixinextras(loader, cluster.mc_loader_version.as_deref()).then_some("mixinextras");
    let jar = JarDependencies {
        provides: GAME_IDS
            .iter()
            .chain(versioned)
            .chain(others)
            .copied()
            .chain(mixinextras)
            .map(ToString::to_string)
            .collect(),
        semver: !matches!(loader, GameLoader::Forge | GameLoader::NeoForge),
        alias_versions: versions,
        ..JarDependencies::default()
    };

    FinalJar {
        name: "Minecraft".to_string(),
        version: VersionText::from_jar(String::new(), &jar),
        jar,
        landing: false,
        requested: false,
        on_pin: false,
    }
}

async fn staged_jar(
    version: &VersionDetail,
    loader: Option<GameLoader>,
    ctx: &ContentCtx,
) -> ContentResult<FinalJar> {
    let artifact = match version.primary_file() {
        Some(file) => {
            artifact_dao::get_artifact_by_hash(&ctx.db, &polyio::normalize_hash(&file.sha1)).await?
        }
        None => None,
    };
    let jar = match artifact {
        Some(artifact) => {
            read_jar_dependencies(&artifact_absolute_path(&artifact.path)?, loader).await
        }
        None => JarDependencies::default(),
    };

    Ok(FinalJar {
        name: version.name.clone(),
        version: VersionText::from_jar(version.version_number.clone(), &jar),
        jar,
        landing: true,
        requested: false,
        on_pin: false,
    })
}

fn runs_a_pin(pins: &BundlePins, info: &LinkedArtifactInfo) -> bool {
    let by_project = info
        .provider
        .zip(info.project_id.clone())
        .and_then(|key| pins.get(&key));
    by_project.is_some_and(|pin| info.version_id.as_ref() == Some(&pin.version_id))
        || pins.values().any(|pin| pin.sha1 == info.hash)
}

async fn final_jars(
    staged: &[StagedPackageUpdate],
    leaving: &HashSet<&str>,
    pins: &BundlePins,
    cluster: &ClusterRow,
    java_major: Option<u32>,
    ctx: &ContentCtx,
) -> ContentResult<(Vec<FinalJar>, HashSet<String>)> {
    let loader = GameLoader::from_repr(cluster.mc_loader as u8);
    let linked = PackageStore::list_linked_artifacts(cluster.id, ctx).await?;
    let mut jars = enabled_jars(&linked, loader, ctx).await?;

    let mut mods = vec![game_jar(cluster, java_major)];
    for info in linked
        .iter()
        .filter(|info| !leaving.contains(info.hash.as_str()))
    {
        let Some(jar) = jars.remove(&info.hash) else {
            continue;
        };
        mods.push(FinalJar {
            name: info
                .display_name
                .clone()
                .unwrap_or_else(|| info.cluster_file_name.clone()),
            version: VersionText::from_jar(info.display_version.clone().unwrap_or_default(), &jar),
            jar,
            landing: false,
            requested: false,
            on_pin: runs_a_pin(pins, info),
        });
    }

    for item in staged {
        if jars.contains_key(&item.update.hash) {
            let mut landing = staged_jar(&item.version, loader, ctx).await?;
            landing.name.clone_from(&item.update.display_name);
            landing.requested = true;
            mods.push(landing);
        }
        for dependency in &item.dependencies {
            mods.push(staged_jar(&dependency.version, loader, ctx).await?);
        }
    }
    mods.retain(|found| !found.jar.server_only);

    let lost = jars
        .values()
        .flat_map(JarDependencies::all_ids)
        .cloned()
        .collect();
    Ok((mods, lost))
}

pub(crate) async fn unfit_pins(
    provider: ProviderId,
    project: &ProjectDetail,
    root: &VersionDetail,
    install: &[ResolvedDependency],
    pins: &BundlePins,
    cluster: &ClusterRow,
    ctx: &ContentCtx,
) -> ContentResult<Vec<(ProviderId, String)>> {
    let pin_of =
        |dependency: &ResolvedDependency| (provider, dependency.version.project_id.clone());
    let pinned = |dependency: &ResolvedDependency| pins.contains_key(&pin_of(dependency));
    if project.content_type != ContentType::Mod || !install.iter().any(pinned) {
        return Ok(Vec::new());
    }

    let loader = GameLoader::from_repr(cluster.mc_loader as u8);
    let landing = install
        .iter()
        .map(|dependency| (&dependency.project, &dependency.version));
    let mut mods = Vec::new();
    for (project, version) in std::iter::once((project, root)).chain(landing) {
        let cached =
            PackageStore::download_and_cache(provider, project, version, false, None, ctx).await;
        if let Err(err) = cached {
            tracing::debug!(%err, "no jar to check a pin against");
        }
        let jar = staged_jar(version, loader, ctx).await?;
        mods.push(FinalJar {
            landing: false,
            ..jar
        });
    }

    Ok(install
        .iter()
        .zip(&mods[1..])
        .filter(|(dependency, jar)| {
            pinned(dependency)
                && !conflicts_moved_by(&mods, &cluster.mc_version, |found| {
                    std::ptr::eq(found, *jar)
                })
                .is_empty()
        })
        .map(|(dependency, _)| pin_of(dependency))
        .collect())
}

pub async fn ensure_staged_versions_agree(
    staged: &[StagedPackageUpdate],
    java_major: Option<u32>,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let Some(first) = staged.first() else {
        return Ok(());
    };
    let leaving: HashSet<&str> = staged
        .iter()
        .map(|item| item.update.hash.as_str())
        .collect();
    let installed: HashSet<(i64, String, String)> =
        artifact_dao::list_cluster_releases(&ctx.db, first.update.cluster_id)
            .await?
            .into_iter()
            .filter(|release| release.enabled != 0 && !leaving.contains(release.hash.as_str()))
            .map(|release| (release.provider, release.project_id, release.version_id))
            .collect();

    let mut landing: HashMap<(ProviderId, &str), HashSet<&str>> = HashMap::new();
    for item in staged {
        let provider = item.update.provider;
        landing
            .entry((provider, &item.update.project_id))
            .or_default()
            .insert(&item.version.version_id);
        for dependency in &item.dependencies {
            landing
                .entry((provider, &dependency.version.project_id))
                .or_default()
                .insert(&dependency.version.version_id);
        }
    }

    let mut conflicts: Vec<String> = landing
        .iter()
        .filter(|(_, versions)| versions.len() > 1)
        .map(|((_, project_id), _)| format!("{project_id} would land on two versions"))
        .collect();
    for item in staged {
        for (project_id, version_id) in &item.exact {
            let provider = item.update.provider;
            let met = match landing.get(&(provider, project_id.as_str())) {
                Some(versions) => versions.contains(version_id.as_str()),
                None => {
                    installed.contains(&(provider as i64, project_id.clone(), version_id.clone()))
                }
            };
            if !met {
                conflicts.push(format!(
                    "{} needs {project_id} {version_id}",
                    item.update.display_name
                ));
            }
        }
    }

    let cluster = PackageStore::get_cluster(first.update.cluster_id, ctx).await?;
    let pins = BundlePins::new();
    let (mods, lost) = final_jars(staged, &leaving, &pins, &cluster, java_major, ctx).await?;
    conflicts.extend(jar_range_conflicts(&mods, &cluster.mc_version));
    conflicts.extend(jar_missing_mods(&mods, &lost));
    conflicts.sort();
    conflicts.dedup();

    if conflicts.is_empty() {
        return Ok(());
    }
    Err(ContentError::InvalidData {
        reason: conflicts.join(", "),
    })
}

#[allow(clippy::too_many_arguments)]
async fn ensure_dependencies_fit(
    update: &BrowserPackageUpdate,
    project: &ProjectDetail,
    version: &VersionDetail,
    dependencies: &[ResolvedDependency],
    bundles: &BundlesManager,
    java_major: Option<u32>,
    child: Option<&GroupedProgressChild>,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    if dependencies.is_empty() && project.content_type != ContentType::Mod {
        return Ok(());
    }

    PackageStore::download_and_cache(update.provider, project, version, false, child, ctx).await?;
    for dependency in dependencies {
        let cached = PackageStore::download_and_cache(
            update.provider,
            &dependency.project,
            &dependency.version,
            false,
            None,
            ctx,
        )
        .await;
        if let Err(err) = cached {
            tracing::debug!(%err, "no jar to check a dependency against");
        }
    }

    let staged = [StagedPackageUpdate {
        update: update.clone(),
        project: project.clone(),
        version: version.clone(),
        dependencies: dependencies.to_vec(),
        exact: Vec::new(),
    }];
    let leaving = HashSet::from([update.hash.as_str()]);
    let cluster = PackageStore::get_cluster(update.cluster_id, ctx).await?;
    let pins = cluster_bundle_pins(&cluster, bundles, ctx).await?;
    let (mods, _) = final_jars(&staged, &leaving, &pins, &cluster, java_major, ctx).await?;

    let mut conflicts = conflicts_moved_by(&mods, &cluster.mc_version, |jar| {
        jar.landing && !jar.requested
    });

    let on_pin: HashSet<String> = PackageStore::list_linked_artifacts(update.cluster_id, ctx)
        .await?
        .into_iter()
        .filter(|info| info.enabled && info.hash != update.hash && runs_a_pin(&pins, info))
        .map(|info| info.hash)
        .collect();
    let edges = artifact_dao::list_cluster_dependency_edges(&ctx.db, update.cluster_id).await?;
    let installed = (update.provider as i64, update.installed_version_id.clone());
    if version.version_id != update.installed_version_id
        && required_versions(&edges, on_pin, &HashMap::new()).contains(&installed)
    {
        conflicts.push(format!(
            "a pinned mod needs {} {}",
            update.display_name, update.installed_version_name
        ));
    }

    if conflicts.is_empty() {
        return Ok(());
    }
    Err(ContentError::InvalidData {
        reason: conflicts.join(", "),
    })
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
        exact: resolution.exact,
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
    java_major: Option<u32>,
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
    ensure_dependencies_fit(
        update,
        &project,
        &version,
        &dependencies,
        bundles,
        java_major,
        child,
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
    fn a_jar_only_rules_out_a_mover_on_an_id_the_mover_owns() {
        let jar = |provides: &[&str], needs: &[(&str, &str)]| JarDependencies {
            provides: provides.iter().map(ToString::to_string).collect(),
            ranges: needs
                .iter()
                .map(|(id, min)| {
                    (
                        (*id).to_string(),
                        vec![oneclient_events::VersionRange::at_least(min)],
                    )
                })
                .collect(),
            ..JarDependencies::default()
        };
        let jars = HashMap::from([
            ("library".to_string(), jar(&["library"], &[])),
            ("nested".to_string(), jar(&["nested"], &[])),
            (
                "app".to_string(),
                jar(&["app"], &[("library", "2.0"), ("other", "9.0")]),
            ),
        ]);
        let target = VersionText {
            number: "1.0".to_string(),
            ..VersionText::default()
        };
        let targets = HashMap::from([
            ("library".to_string(), target.clone()),
            ("nested".to_string(), target),
        ]);

        let staying = HashSet::from(["app".to_string()]);
        assert_eq!(
            ruled_out_by_jars(&jars, &staying, &targets, "26.3").0,
            ["library"]
        );
        assert!(
            ruled_out_by_jars(&jars, &HashSet::new(), &targets, "26.3")
                .0
                .is_empty()
        );

        let unread = HashMap::from([(
            "library".to_string(),
            VersionText {
                number: "1.0".to_string(),
                unread: true,
                ..VersionText::default()
            },
        )]);
        let (ruled_out, waiting) = ruled_out_by_jars(&jars, &staying, &unread, "26.3");
        assert!(ruled_out.is_empty(), "the provider's number proves nothing");
        assert_eq!(waiting, ["library"]);
    }

    #[test]
    fn a_declared_version_outranks_the_providers_number() {
        let at_least = [oneclient_events::VersionRange::at_least("0.6")];
        let target = VersionText {
            number: "1.20.1-0.5.0".to_string(),
            ..VersionText::default()
        };
        assert!(!target.ruled_out("library", &at_least, "1.20.4"));

        let declared = VersionText {
            declared: Some("0.5.0".to_string()),
            ..target
        };
        assert!(declared.ruled_out("library", &at_least, "1.20.4"));
    }

    #[test]
    fn a_provided_id_compares_on_its_own_version() {
        let at_least = [oneclient_events::VersionRange::at_least("2.0")];
        let jar = JarDependencies {
            provides: vec!["app".to_string(), "library".to_string()],
            version: Some("5.0".to_string()),
            semver: true,
            alias_versions: [("library".to_string(), "1.0".to_string())].into(),
            ..JarDependencies::default()
        };
        let version = VersionText::from_jar(String::new(), &jar);

        assert!(version.ruled_out("library", &at_least, "26.3"));
        assert!(!version.ruled_out("app", &at_least, "26.3"));
        assert!(!a_copy_fits(
            [(None, &jar)].into_iter(),
            "library",
            &at_least,
            "26.3"
        ));
    }

    #[test]
    fn a_nested_mod_is_held_to_what_it_asks_for() {
        let host = |range: &str| {
            let mut host = final_jar("Host", Some("1.0"), &["host"], &[], true);
            host.jar.nested = vec![JarDependencies {
                provides: vec!["inner".to_string()],
                requires: vec!["library".to_string()],
                ranges: [("library".to_string(), vec![VersionRange::at_least(range)])].into(),
                ..JarDependencies::default()
            }];
            host
        };

        assert_eq!(
            jar_missing_mods(&[host("1.0")], &HashSet::new()),
            ["Host needs library, which is not installed"]
        );

        let library = final_jar("Library", Some("1.5"), &["library"], &[], false);
        assert!(jar_missing_mods(&[host("1.0"), library], &HashSet::new()).is_empty());
        let library = final_jar("Library", Some("1.5"), &["library"], &[], false);
        assert_eq!(
            jar_range_conflicts(&[host("2.0"), library], "26.3"),
            ["Host needs another version of library"]
        );
    }

    #[test]
    fn a_mod_that_stays_keeps_the_last_provider_of_what_it_needs() {
        let stays = || {
            let mut app = final_jar("App", None, &["app"], &[], false);
            app.jar.requires = vec!["library".to_string()];
            app
        };
        let lost = HashSet::from(["library".to_string()]);

        assert_eq!(
            jar_missing_mods(&[stays()], &lost),
            ["App needs library, which is not installed"]
        );
        assert!(
            jar_missing_mods(&[stays()], &HashSet::new()).is_empty(),
            "it was lacking before"
        );

        let replaced = final_jar("Fork", None, &["library"], &[], true);
        assert!(jar_missing_mods(&[stays(), replaced], &lost).is_empty());
    }

    fn final_jar(
        name: &str,
        version: Option<&str>,
        provides: &[&str],
        needs: &[(&str, &str)],
        landing: bool,
    ) -> FinalJar {
        FinalJar {
            name: name.to_string(),
            version: VersionText {
                declared: version.map(ToString::to_string),
                ..VersionText::default()
            },
            jar: JarDependencies {
                provides: provides.iter().map(ToString::to_string).collect(),
                ranges: needs
                    .iter()
                    .map(|(id, min)| ((*id).to_string(), vec![VersionRange::at_least(min)]))
                    .collect(),
                ..JarDependencies::default()
            },
            landing,
            requested: false,
            on_pin: false,
        }
    }

    fn cluster_row(
        mc_version: &str,
        loader: GameLoader,
        loader_version: Option<&str>,
    ) -> ClusterRow {
        ClusterRow {
            id: 1,
            name: "c".into(),
            folder_name: "c".into(),
            setting_profile_name: None,
            mc_version: mc_version.into(),
            mc_loader: loader as i64,
            stage: 0,
            mc_loader_version: loader_version.map(ToString::to_string),
            created_at: None,
            last_played: None,
            overall_played: None,
            linked_modpack_hash: None,
            kind: 0,
            user_created: 0,
            description: None,
            tags: "[]".into(),
            cover_path: None,
        }
    }

    fn game(mc_version: &str, loader: GameLoader) -> FinalJar {
        let version = format!("{mc_version}-47.2.0");
        game_jar(&cluster_row(mc_version, loader, Some(&version)), Some(21))
    }

    #[test]
    fn a_landing_jar_is_held_to_the_game_and_its_loader() {
        let needs =
            |id: &str, min: &str, landing| final_jar("App", None, &["app"], &[(id, min)], landing);
        let check = |app, loader| jar_range_conflicts(&[game("1.21.1", loader), app], "1.21.1");

        for loader in [GameLoader::Fabric, GameLoader::Forge] {
            assert_eq!(
                check(needs("minecraft", "99.0", true), loader),
                ["App needs another version of minecraft"]
            );
            assert!(check(needs("minecraft", "1.21", true), loader).is_empty());
            assert_eq!(
                check(needs("java", "22", true), loader),
                ["App needs another version of java"]
            );
            assert!(check(needs("java", "21", true), loader).is_empty());
            assert!(
                check(needs("minecraft", "99.0", false), loader).is_empty(),
                "already running"
            );
        }

        assert_eq!(
            check(needs("forge", "48", true), GameLoader::Forge),
            ["App needs another version of forge"]
        );
        assert!(check(needs("forge", "47", true), GameLoader::Forge).is_empty());
        assert_eq!(
            check(needs("fabricloader", "48", true), GameLoader::Fabric).len(),
            1
        );

        let snapshot = jar_range_conflicts(
            &[
                game("26.1-pre-1", GameLoader::Fabric),
                needs("minecraft", "99.0", true),
            ],
            "26.1-pre-1",
        );
        assert!(
            snapshot.is_empty(),
            "loaders rewrite these before comparing"
        );

        let unknown = game_jar(&cluster_row("1.21.1", GameLoader::Fabric, None), None);
        let mods = [
            unknown,
            needs("java", "99", true),
            needs("fabricloader", "99", true),
        ];
        assert!(
            jar_range_conflicts(&mods, "1.21.1").is_empty(),
            "nothing to compare against"
        );
    }

    #[test]
    fn mixinextras_comes_with_fabric_loader_0_15() {
        let app = || FinalJar {
            jar: JarDependencies {
                requires: vec!["mixinextras".to_string()],
                ..final_jar("App", None, &["app"], &[], true).jar
            },
            ..final_jar("App", None, &["app"], &[], true)
        };
        let missing = |loader, version: Option<&str>| {
            let game = game_jar(&cluster_row("1.20.1", loader, version), None);
            !jar_missing_mods(&[game, app()], &HashSet::new()).is_empty()
        };

        assert!(missing(GameLoader::Fabric, Some("0.14.25")));
        assert!(!missing(GameLoader::Fabric, Some("0.15.0")));
        assert!(!missing(GameLoader::Fabric, None));
        assert!(!missing(GameLoader::NeoForge, None));
    }

    #[test]
    fn a_game_version_fabric_rewrites_rules_nothing_out() {
        let app = final_jar("App", None, &["app"], &[("minecraft", "1.21")], true);
        let game = game_jar(&cluster_row("25w14a", GameLoader::Fabric, None), None);
        assert!(jar_range_conflicts(&[game, app], "25w14a").is_empty());
    }

    #[test]
    fn a_ban_stops_what_it_names_on_either_side_of_a_move() {
        let semver = |mut found: FinalJar| {
            found.version.semver = true;
            found
        };
        let library = |version, landing| {
            semver(final_jar(
                "Library",
                Some(version),
                &["library"],
                &[],
                landing,
            ))
        };
        let banning = |landing| {
            let mut found = final_jar("App", Some("1.0"), &["app"], &[], landing);
            found.jar.breaks =
                [("library".to_string(), vec![VersionRange::at_least("2.0")])].into();
            found
        };
        let check = |app, library| jar_range_conflicts(&[app, library], "1.21.1");

        assert_eq!(
            check(banning(true), library("2.0", false)),
            ["App cannot run beside library"]
        );
        assert_eq!(check(banning(false), library("2.1", true)).len(), 1);
        assert!(check(banning(true), library("1.0", false)).is_empty());
        assert!(check(banning(false), library("2.0", false)).is_empty());

        let mut other_copy = final_jar("Host", None, &["host"], &[], false);
        let mut nested = library("1.0", false).jar;
        nested.semver = true;
        other_copy.jar.nested.push(nested);
        let found = jar_range_conflicts(
            &[banning(true), library("2.0", false), other_copy],
            "1.21.1",
        );
        assert!(found.is_empty(), "the loader can take the copy it allows");
    }

    #[test]
    fn a_forge_ban_reads_its_range_the_way_maven_does() {
        let ban = [VersionRange::at_least("2.0")];
        let version = |declared: Option<&str>, number: &str| VersionText {
            declared: declared.map(ToString::to_string),
            number: number.to_string(),
            ..VersionText::default()
        };

        assert!(version(Some("2.1"), "").is_among("library", &ban, "1.21.1"));
        assert!(!version(Some("2.0-m1"), "").is_among("library", &ban, "1.21.1"));
        assert!(
            version(None, "2.1-neoforge").is_among("library", &ban, "1.21.1"),
            "a build placeholder leaves the provider's number"
        );
        assert!(!version(None, "beta").is_among("library", &ban, "1.21.1"));
    }

    #[test]
    fn a_mod_on_its_pin_holds_a_requested_update_to_what_it_needs() {
        let mut library = final_jar("Library", Some("2.0"), &["library"], &[], true);
        library.requested = true;
        let mut dependent = final_jar("App", Some("1.0"), &["app"], &[], false);
        dependent.jar.ranges = [(
            "library".to_string(),
            vec![VersionRange {
                max: Some("2.0".to_string()),
                ..VersionRange::default()
            }],
        )]
        .into();
        let check = |mods: &[FinalJar]| {
            conflicts_moved_by(mods, "1.21.1", |jar| jar.landing && !jar.requested)
        };

        let mut mods = [library, dependent];
        assert!(check(&mods).is_empty(), "the update was asked for");
        mods[1].on_pin = true;
        assert_eq!(check(&mods), ["App needs another version of library"]);
    }

    #[test]
    fn a_mod_that_stays_holds_a_mover_it_bans() {
        let mut banning = JarDependencies::default();
        banning.breaks = [("library".to_string(), vec![VersionRange::at_least("2.0")])].into();
        let mover = JarDependencies {
            provides: vec!["library".to_string()],
            ..JarDependencies::default()
        };
        let jars = HashMap::from([("stays".to_string(), banning), ("moves".to_string(), mover)]);
        let target = |version: &str| {
            let text = VersionText {
                declared: Some(version.to_string()),
                semver: true,
                ..VersionText::default()
            };
            HashMap::from([("moves".to_string(), text)])
        };
        let staying = HashSet::from(["stays".to_string()]);

        let (held, _) = ruled_out_by_jars(&jars, &staying, &target("2.0"), "1.21.1");
        assert_eq!(held, ["moves"]);
        let (held, _) = ruled_out_by_jars(&jars, &staying, &target("1.0"), "1.21.1");
        assert!(held.is_empty());
    }

    #[test]
    fn one_version_has_to_answer_every_range_on_an_id() {
        let nesting = |name: &str, version: &str| {
            let mut host = final_jar(name, Some("1.0"), &[], &[], false);
            host.jar.provides = vec![name.to_string()];
            host.jar.nested = vec![JarDependencies {
                provides: vec!["library".to_string()],
                version: Some(version.to_string()),
                ..JarDependencies::default()
            }];
            host
        };
        let staying = |max: &str| {
            let mut found = final_jar("Stays", None, &["stays"], &[], false);
            let below = VersionRange {
                max: Some(max.to_string()),
                ..VersionRange::default()
            };
            found.jar.ranges = [("library".to_string(), vec![below])].into();
            found
        };
        let update = || final_jar("Update", None, &["update"], &[("library", "2.0")], true);

        let mods = [
            nesting("old", "1.0"),
            nesting("new", "2.0"),
            staying("2.0"),
            update(),
        ];
        assert_eq!(
            jar_range_conflicts(&mods, "26.3"),
            ["no version of library suits every mod that needs it"]
        );

        let mods = [
            nesting("old", "1.0"),
            nesting("new", "2.0"),
            staying("3.0"),
            update(),
        ];
        assert!(jar_range_conflicts(&mods, "26.3").is_empty());

        let mods = [nesting("old", "1.0"), nesting("new", "2.0"), staying("2.0")];
        assert!(
            jar_range_conflicts(&mods, "26.3").is_empty(),
            "nothing on the id moved"
        );
    }

    #[test]
    fn what_stays_does_not_hold_back_the_update_itself() {
        let mut update = final_jar("Update", Some("2.0"), &["update"], &[], true);
        update.requested = true;
        let stays = final_jar("Stays", None, &["stays"], &[("update", "3.0")], false);
        let brought = final_jar("Brought", None, &["brought"], &[("update", "1.0")], true);

        let mods = [update, stays, brought];
        let found = conflicts_moved_by(&mods, "26.3", |jar| jar.landing && !jar.requested);
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn a_staged_jars_range_is_checked_against_what_stays() {
        let library = |landing| final_jar("Library", Some("1.0"), &["library"], &[], landing);
        let app = |landing| final_jar("App", None, &["app"], &[("library", "2.0")], landing);

        assert_eq!(
            jar_range_conflicts(&[library(false), app(true)], "26.3"),
            ["App needs another version of library"]
        );
        assert_eq!(
            jar_range_conflicts(&[library(true), app(false)], "26.3"),
            ["App needs another version of library"]
        );
        assert!(jar_range_conflicts(&[library(false), app(false)], "26.3").is_empty());
        assert!(jar_range_conflicts(&[app(true)], "26.3").is_empty());

        let unread = final_jar("Library", None, &["library"], &[], false);
        assert!(jar_range_conflicts(&[unread, app(true)], "26.3").is_empty());

        let nesting = |version: &str| {
            let mut host = final_jar("Host", Some("1.0"), &["host"], &[], false);
            host.jar.nested = vec![JarDependencies {
                provides: vec!["library".to_string()],
                version: Some(version.to_string()),
                ..JarDependencies::default()
            }];
            host
        };
        assert!(
            jar_range_conflicts(&[library(false), app(true), nesting("2.1")], "26.3").is_empty()
        );
        assert_eq!(
            jar_range_conflicts(&[library(false), app(true), nesting("1.5")], "26.3").len(),
            1,
            "every copy is too old"
        );
        assert_eq!(
            jar_range_conflicts(&[app(true), nesting("1.5")], "26.3").len(),
            1,
            "a nested copy is held to the range with no other copy around"
        );
    }

    #[test]
    fn a_landing_jar_needs_something_to_answer_each_id() {
        let none = HashSet::new();
        let app = |requires: &[&str]| {
            let mut app = final_jar("App", None, &["app"], &[], true);
            app.jar.requires = requires.iter().map(ToString::to_string).collect();
            app
        };
        let host = |landing| {
            let mut host = final_jar("Host", None, &["host"], &[], landing);
            host.jar.nested = vec![JarDependencies {
                provides: vec!["library".to_string()],
                ..JarDependencies::default()
            }];
            host
        };

        assert_eq!(
            jar_missing_mods(&[app(&["library"])], &none),
            ["App needs library, which is not installed"]
        );
        assert!(
            jar_missing_mods(
                &[
                    app(&["minecraft", "fabricloader", "java"]),
                    game("1.21.1", GameLoader::Fabric)
                ],
                &none,
            )
            .is_empty()
        );
        assert_eq!(
            jar_missing_mods(
                &[app(&["forge"]), game("1.21.1", GameLoader::Fabric)],
                &none,
            ),
            ["App needs forge, which is not installed"]
        );
        assert!(
            jar_missing_mods(
                &[app(&["forge"]), game("1.21.1", GameLoader::NeoForge)],
                &none,
            )
            .is_empty(),
            "NeoForge still answers to it"
        );
        assert!(jar_missing_mods(&[app(&["library"]), host(false)], &none).is_empty());

        let mut stays = app(&["library"]);
        stays.landing = false;
        assert!(
            jar_missing_mods(&[stays], &none).is_empty(),
            "already running"
        );

        let mut multi = app(&["library"]);
        multi.jar.multi_loader = true;
        assert!(jar_missing_mods(&[multi], &none).is_empty());

        let unreadable = final_jar("Unknown", None, &[], &[], false);
        assert!(jar_missing_mods(&[app(&["library"]), unreadable], &none).is_empty());
    }

    #[test]
    fn a_hold_is_disputed_only_when_another_mod_needs_it_moved() {
        let held = HashSet::from(["library".to_string()]);
        let staying = HashSet::from(["library".to_string(), "app".to_string()]);
        let release = ClusterReleaseRow {
            hash: "library".to_string(),
            enabled: 1,
            provider: ProviderId::Modrinth as i64,
            project_id: "library-project".to_string(),
            version_id: "library-v1".to_string(),
            display_name: "Library".to_string(),
            dependencies_settled: true,
        };
        let edge = |hash: &str, version: &str| ClusterDependencyEdgeRow {
            hash: hash.into(),
            provider: ProviderId::Modrinth as i64,
            dependency_project_id: "library-project".into(),
            dependency_version_id: version.into(),
            kind: DependencyKind::Required.as_str().into(),
        };
        let disputed = |edges: &[ClusterDependencyEdgeRow], jars: &HashMap<_, _>| {
            disputed_holds(
                &held,
                &staying,
                jars,
                edges,
                std::slice::from_ref(&release),
                "26.3",
            )
        };

        let none = HashMap::new();
        assert!(disputed(&[edge("app", "library-v1")], &none).is_empty());
        assert_eq!(
            disputed(&[edge("app", "library-v2")], &none),
            ["Library is needed on two different versions"]
        );

        let jars = |needs: &str| {
            HashMap::from([
                (
                    "library".to_string(),
                    JarDependencies {
                        version: Some("1.0".to_string()),
                        provides: vec!["library".to_string()],
                        ..JarDependencies::default()
                    },
                ),
                (
                    "app".to_string(),
                    JarDependencies {
                        ranges: [("library".to_string(), vec![VersionRange::at_least(needs)])]
                            .into(),
                        ..JarDependencies::default()
                    },
                ),
            ])
        };
        assert!(disputed(&[], &jars("1.0")).is_empty());
        assert_eq!(disputed(&[], &jars("2.0")).len(), 1);

        let mut nested = jars("2.0");
        nested.get_mut("app").unwrap().nested = vec![JarDependencies {
            provides: vec!["library".to_string()],
            version: Some("2.5".to_string()),
            ..JarDependencies::default()
        }];
        assert!(disputed(&[], &nested).is_empty(), "a nested copy fits");
    }

    #[test]
    fn a_held_mover_holds_what_it_needs_in_turn() {
        let edge = |hash: &str, version: &str| ClusterDependencyEdgeRow {
            hash: hash.into(),
            provider: ProviderId::Modrinth as i64,
            dependency_project_id: String::new(),
            dependency_version_id: version.into(),
            kind: DependencyKind::Required.as_str().into(),
        };
        let moving = |version: &str| {
            HashSet::from([
                (ProviderId::CurseForge as i64, format!("mirror-{version}")),
                (ProviderId::Modrinth as i64, version.to_string()),
            ])
        };
        let moving = HashMap::from([
            ("library".to_string(), moving("library-v1")),
            ("base".to_string(), moving("base-v1")),
        ]);

        let required = required_versions(
            &[edge("app", "library-v1"), edge("library", "base-v1")],
            HashSet::from(["app".to_string()]),
            &moving,
        );

        assert!(required.contains(&(ProviderId::Modrinth as i64, "base-v1".to_string())));
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
