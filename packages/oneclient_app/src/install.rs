use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};

use oneclient_core::LauncherState;

use oneclient_content::packages::PackageStore;
use oneclient_events::Level;

use crate::components::IconType;
use crate::i18n::{t, tf};
use crate::notifications::{
    ClusterUpdateItem, ClusterUpdateSummary, NotificationAction, NotificationActionKind,
    NotificationSpec, OptionalModsGroup, PackageUpdateGroup,
};

/// Falls back to a placeholder a missing row must not lose the whole message
pub async fn cluster_display_name(
    cluster_id: i64,
    services: &oneclient_core::LauncherServices,
) -> String {
    oneclient_core::clusters::ClusterManager::new(services.db.clone())
        .get(cluster_id)
        .await
        .map(|cluster| crate::utils::instance_display_name(&cluster))
        .unwrap_or_else(|_| t("oneclient.install.cluster_fallback").to_string())
}

pub async fn package_update_group(
    cluster_id: i64,
    updates: &[oneclient_core::BrowserPackageUpdate],
    services: &oneclient_core::LauncherServices,
) -> Option<PackageUpdateGroup> {
    if updates.is_empty() {
        return None;
    }

    Some(PackageUpdateGroup {
        cluster_id,
        cluster_name: cluster_display_name(cluster_id, services).await,
        packages: updates.to_vec(),
    })
}

pub fn item_from_bundle_file(file: &oneclient_core::BundleFile) -> ClusterUpdateItem {
    match &file.kind {
        oneclient_core::BundleFileKind::Managed {
            provider,
            project_id,
            ..
        } => ClusterUpdateItem {
            provider: *provider,
            github_hosted: false,
            project_id: Some(project_id.clone()),
            fallback: file.display_name(),
            offer: None,
            status: None,
        },
        oneclient_core::BundleFileKind::External { .. } => ClusterUpdateItem {
            provider: oneclient_content::packages::ProviderId::Local,
            github_hosted: file.is_github_hosted(),
            project_id: Some(file.kind.metadata_id()),
            fallback: file.display_name(),
            offer: None,
            status: None,
        },
    }
}

fn item_from_optional(package: &oneclient_core::BundleOptionalPackage) -> ClusterUpdateItem {
    ClusterUpdateItem {
        offer: Some((package.bundle_name.clone(), package.package_id.clone())),
        ..item_from_bundle_file(&package.file)
    }
}

async fn cluster_update_summary(
    cluster_id: i64,
    result: &oneclient_core::ApplyBundleUpdatesResult,
    services: &oneclient_core::LauncherServices,
) -> Option<ClusterUpdateSummary> {
    let updated: Vec<ClusterUpdateItem> = result
        .updates_applied
        .iter()
        .map(|u| item_from_bundle_file(&u.new_file))
        .collect();
    let added: Vec<ClusterUpdateItem> = result
        .additions_applied
        .iter()
        .map(|a| item_from_bundle_file(&a.new_file))
        .collect();
    let removed: Vec<ClusterUpdateItem> = result
        .removals_applied
        .iter()
        .map(|r| ClusterUpdateItem {
            provider: r
                .provider
                .unwrap_or(oneclient_content::packages::ProviderId::Local),
            github_hosted: false,
            project_id: r.project_id.clone(),
            fallback: r
                .display_name
                .clone()
                .unwrap_or_else(|| r.package_id.clone()),
            offer: None,
            status: None,
        })
        .collect();
    let optional: Vec<ClusterUpdateItem> = result
        .optional_available
        .iter()
        .map(item_from_optional)
        .collect();

    if updated.is_empty() && added.is_empty() && removed.is_empty() && optional.is_empty() {
        return None;
    }

    let cluster_name =
        oneclient_content::packages::PackageStore::get_cluster(cluster_id, &services.content())
            .await
            .map(|c| c.name)
            .unwrap_or_else(|_| t("oneclient.install.cluster_fallback").to_string());

    Some(ClusterUpdateSummary {
        cluster_id,
        cluster_name,
        updated,
        added,
        removed,
        optional,
    })
}

pub async fn cluster_update_notification(
    cluster_id: i64,
    result: &oneclient_core::ApplyBundleUpdatesResult,
    services: &oneclient_core::LauncherServices,
) -> Option<NotificationSpec> {
    let summary = cluster_update_summary(cluster_id, result, services).await?;
    let total = summary.total();
    // A bundle can offer an opt-in mod without changing anything else
    // "0 packages changed" would be both wrong and alarming
    let (title, body) = if total == 0 {
        let offers = summary.optional.len();
        (
            t("oneclient.install.optional_available.title"),
            if offers == 1 {
                tf(
                    "oneclient.install.optional_available.body.one",
                    &[&offers, &summary.cluster_name],
                )
            } else {
                tf(
                    "oneclient.install.optional_available.body.other",
                    &[&offers, &summary.cluster_name],
                )
            },
        )
    } else {
        (
            t("oneclient.install.cluster_updated.title"),
            if total == 1 {
                tf(
                    "oneclient.install.cluster_updated.body.one",
                    &[&total, &summary.cluster_name],
                )
            } else {
                tf(
                    "oneclient.install.cluster_updated.body.other",
                    &[&total, &summary.cluster_name],
                )
            },
        )
    };

    Some(NotificationSpec {
        title: title.to_string(),
        body,
        level: Level::Info,
        icon: Some(IconType::DownloadCloud02),
        progress: None,
        actions: vec![NotificationAction {
            label: t("oneclient.install.view_changes").to_string(),
            kind: NotificationActionKind::OpenClusterUpdate(vec![summary]),
        }],
        toast_only: false,
    })
}

pub async fn pending_optional_group(
    cluster_id: i64,
    pending: &[oneclient_core::PendingOptionalMod],
    services: &oneclient_core::LauncherServices,
) -> Option<OptionalModsGroup> {
    if pending.is_empty() {
        return None;
    }

    let mods: Vec<ClusterUpdateItem> = pending
        .iter()
        .map(|entry| ClusterUpdateItem {
            offer: Some((entry.bundle_name.clone(), entry.package_id.clone())),
            status: Some(entry.status),
            ..item_from_bundle_file(&entry.file)
        })
        .collect();

    Some(OptionalModsGroup {
        cluster_id,
        cluster_name: cluster_display_name(cluster_id, services).await,
        mods,
    })
}

pub struct PackageInstall {
    pub session_id: Option<uuid::Uuid>,
    pub result: anyhow::Result<String>,
    pub dependencies: Vec<String>,
    pub missing_dependencies: Vec<String>,
    /// The game is open but could not take the package, so the toast must not
    /// claim it is there now
    pub live_deferred: bool,
    /// Present only for a brand new package the caller asked to be able to take back
    pub undo: Option<InstallUndo>,
}

static INSTANCE_INSTALLS: std::sync::LazyLock<Mutex<HashMap<i64, Weak<tokio::sync::Mutex<()>>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

async fn instance_install_guard(cluster_id: i64) -> tokio::sync::OwnedMutexGuard<()> {
    let instance = {
        let mut locks = INSTANCE_INSTALLS
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        locks.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = locks.get(&cluster_id).and_then(Weak::upgrade) {
            lock
        } else {
            let lock = Arc::new(tokio::sync::Mutex::new(()));
            locks.insert(cluster_id, Arc::downgrade(&lock));
            lock
        }
    };
    instance.lock_owned().await
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UndoPackage {
    pub provider: oneclient_content::packages::ProviderId,
    pub project_id: String,
    pub hash: String,
    pub name: String,
    content_type: oneclient_content::packages::ContentType,
    added_at: String,
}

/// The exact links one install added.
#[derive(Clone, Debug)]
pub struct InstallUndo {
    token: uuid::Uuid,
    pub cluster_id: i64,
    pub packages: Vec<UndoPackage>,
    claimed: Arc<AtomicBool>,
}

impl PartialEq for InstallUndo {
    fn eq(&self, other: &Self) -> bool {
        self.token == other.token
    }
}

impl Eq for InstallUndo {}

impl InstallUndo {
    pub fn claim(&self) -> bool {
        !self.claimed.swap(true, Ordering::AcqRel)
    }

    pub fn release(&self) {
        self.claimed.store(false, Ordering::Release);
    }

    pub fn target_name(&self) -> &str {
        self.packages
            .first()
            .map_or("", |package| package.name.as_str())
    }
}

/// What a cluster held before an install ran
struct ClusterSnapshot {
    hashes: HashSet<String>,
    /// A project already present means the install replaced a version, which
    /// Undo could not put back
    projects: HashSet<(i64, String)>,
}

impl ClusterSnapshot {
    async fn take(cluster_id: i64, state: &LauncherState) -> Option<Self> {
        let rows = oneclient_db::dao::artifact::list_cluster_artifacts_detailed(
            &state.services.db,
            cluster_id,
        )
        .await
        .inspect_err(|err| tracing::warn!(cluster_id, %err, "cannot offer Undo without a baseline"))
        .ok()?;

        let identities = oneclient_db::dao::artifact::list_cluster_release_identities(
            &state.services.db, cluster_id,
        )
        .await
        .inspect_err(|err| tracing::warn!(cluster_id, %err, "cannot offer Undo without all existing release identities"))
        .ok()?;

        Some(Self::from_links(
            rows.into_iter().map(|row| row.hash),
            identities,
        ))
    }

    fn from_links(
        hashes: impl IntoIterator<Item = String>,
        identities: Vec<oneclient_db::models::ClusterReleaseIdentityRow>,
    ) -> Self {
        Self {
            hashes: hashes.into_iter().collect(),
            projects: identities
                .into_iter()
                .map(|row| (row.provider, row.project_id))
                .collect(),
        }
    }

    fn is_new(&self, package: &UndoPackage) -> bool {
        !self.hashes.contains(&package.hash)
            && !self
                .projects
                .contains(&(package.provider as i64, package.project_id.clone()))
    }
}

/// Undo is offered only when the install added something that was not in the
/// instance before
///
/// A reinstall, or a new version that superseded an older one, changes content
/// the user already had, so it gets no Undo rather than one that cannot restore it
fn plan_undo(
    snapshot: Option<&ClusterSnapshot>,
    cluster_id: i64,
    target: UndoPackage,
    dependencies: Vec<UndoPackage>,
) -> Option<InstallUndo> {
    let snapshot = snapshot?;
    if !snapshot.is_new(&target) {
        return None;
    }

    let mut packages = vec![target];
    packages.extend(
        dependencies
            .into_iter()
            .filter(|package| snapshot.is_new(package)),
    );

    Some(InstallUndo {
        token: uuid::Uuid::new_v4(),
        cluster_id,
        packages,
        claimed: Arc::new(AtomicBool::new(false)),
    })
}

#[derive(Debug, Default)]
pub struct UndoReport {
    pub target: String,
    pub target_removed: bool,
    /// Names, in the order they were taken out
    pub removed: Vec<String>,
    /// Dependencies something else now needs or that changed since the install
    pub kept: Vec<String>,
    pub error: Option<String>,
}

/// Keep a target and its dependencies together if the target is no longer safe to remove.
pub async fn undo_install(state: &LauncherState, undo: &InstallUndo) -> UndoReport {
    let content = state.services.content();
    let mut report = UndoReport {
        target: undo.target_name().to_string(),
        ..UndoReport::default()
    };
    if let Err(err) = remove_install_group(undo, &content, &mut report).await {
        tracing::warn!(cluster_id = undo.cluster_id, %err, "failed to undo a package install");
        report.error = Some(err.to_string());
    }
    report
}

async fn remove_install_group(
    undo: &InstallUndo,
    content: &oneclient_content::ContentCtx,
    report: &mut UndoReport,
) -> anyhow::Result<()> {
    use oneclient_content::{bundles, packages};
    use oneclient_db::dao::artifact as artifacts;

    let Some(target) = undo.packages.first() else {
        return Ok(());
    };
    let cluster_id = undo.cluster_id;
    if !bundles::can_undo_artifact(cluster_id, &target.hash, &target.added_at, content).await? {
        return Ok(());
    }

    // Don't undo a dependency between resolution and linking in this instance.
    let _instance = instance_install_guard(cluster_id).await;
    // Provider requests can be slow. Backfill before taking the global guard, then
    // verify that every currently linked remote package can be checked.
    packages::ensure_cluster_dependencies(cluster_id, content).await?;
    let _exclusive = packages::exclusive_package_mutation_guard().await;

    let mut candidates = HashSet::new();
    for package in &undo.packages {
        if bundles::can_undo_artifact(cluster_id, &package.hash, &package.added_at, content).await?
        {
            candidates.insert(package.hash.as_str());
        }
    }
    if !candidates.contains(target.hash.as_str()) {
        return Ok(());
    }

    let missing = artifacts::list_releases_missing_dependencies(&content.db, cluster_id).await?;
    if missing
        .iter()
        .any(|release| release.provider != packages::ProviderId::Local as i64)
    {
        anyhow::bail!(t("oneclient.actions.undo.dependencies_unverified"));
    }

    let mut blocked =
        packages::dependents::removal_blockers(cluster_id, &candidates, content).await?;
    let candidate_mods: HashSet<_> = undo
        .packages
        .iter()
        .filter(|package| {
            package.content_type == packages::ContentType::Mod
                && candidates.contains(package.hash.as_str())
                && !blocked.contains_key(&package.hash)
        })
        .map(|package| package.hash.as_str())
        .collect();
    if !candidate_mods.is_empty()
        && packages::store::undo::local_mods_may_need(cluster_id, &candidate_mods, content).await?
    {
        if candidate_mods.contains(target.hash.as_str()) {
            anyhow::bail!(t("oneclient.actions.undo.local_dependencies_unverified"));
        }
        candidates.retain(|hash| !candidate_mods.contains(hash));
        blocked = packages::dependents::removal_blockers(cluster_id, &candidates, content).await?;
    }

    if let Some(dependents) = blocked.get(&target.hash) {
        let names = dependents
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        anyhow::bail!(tf(
            "oneclient.actions.undo.required",
            &[&target.name, &names]
        ));
    }

    let (remove, keep): (Vec<_>, Vec<_>) = undo.packages.iter().partition(|package| {
        candidates.contains(package.hash.as_str()) && !blocked.contains_key(&package.hash)
    });
    let links: Vec<_> = remove
        .iter()
        .map(|p| (p.hash.as_str(), p.added_at.as_str()))
        .collect();
    if bundles::detach_undo_artifacts(cluster_id, &links, content).await? {
        report.target_removed = true;
        report.removed = remove
            .into_iter()
            .map(|package| package.name.clone())
            .collect();
        report.kept = keep
            .into_iter()
            .map(|package| package.name.clone())
            .collect();
    }
    Ok(())
}

pub struct FlaggedInstall {
    pub name: String,
    pub mc_version: String,
    pub explanation: Option<String>,
    pub alternatives: Vec<oneclient_content::packages::ResolvedAlternative>,
}

pub fn install_body(
    name: &str,
    dependencies: &[String],
    missing: &[String],
    live_deferred: bool,
) -> String {
    let mut body = match dependencies.len() {
        0 => tf("oneclient.install.added", &[&name]),
        1 => tf(
            "oneclient.install.added_with_dependencies.one",
            &[&name, &1],
        ),
        count => tf(
            "oneclient.install.added_with_dependencies.other",
            &[&name, &count],
        ),
    };

    if !missing.is_empty() {
        body.push(' ');
        body.push_str(&tf(
            "oneclient.install.could_not_add",
            &[&missing.join(", ")],
        ));
    }

    if live_deferred {
        body.push(' ');
        body.push_str(t("oneclient.install.live_deferred"));
    }

    body
}

impl PackageInstall {
    fn failed(err: anyhow::Error) -> Self {
        Self {
            session_id: None,
            result: Err(err),
            dependencies: Vec::new(),
            missing_dependencies: Vec::new(),
            live_deferred: false,
            undo: None,
        }
    }
}

async fn flagged_install(
    name: String,
    entry: &oneclient_content::packages::BadMod,
    cluster_id: i64,
    content: &oneclient_content::ContentCtx,
) -> FlaggedInstall {
    let resolve = async {
        match PackageStore::get_cluster(cluster_id, content).await {
            Ok(cluster) => {
                let alternatives =
                    oneclient_content::packages::resolve_alternatives(entry, &cluster, content)
                        .await;
                (cluster.mc_version, alternatives)
            }
            Err(err) => {
                tracing::warn!(%err, cluster_id, "cannot resolve alternatives without the cluster");
                (String::new(), Vec::new())
            }
        }
    };
    let ((mc_version, alternatives), explanation) = tokio::join!(
        resolve,
        oneclient_content::packages::fetch_explanation(entry, content),
    );
    FlaggedInstall {
        name,
        mc_version,
        explanation,
        alternatives,
    }
}

pub enum InstallBlocked {
    Flagged(FlaggedInstall),
    Incompatible { name: String },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IncompatibleMode {
    Refuse,
    Ask,
    Allow,
}

#[allow(clippy::too_many_arguments)]
pub async fn install_package(
    state: &Arc<LauncherState>,
    provider: oneclient_content::packages::ProviderId,
    project_id: &str,
    version_id: &str,
    cluster_id: i64,
    world: Option<String>,
    allow_flagged: bool,
    incompatible: IncompatibleMode,
) -> Result<PackageInstall, InstallBlocked> {
    install_package_with(
        state,
        provider,
        project_id,
        version_id,
        cluster_id,
        world,
        allow_flagged,
        incompatible,
        false,
    )
    .await
}

/// `offer_undo` is for installs the user chose by hand in the browser. Worlds
/// are never undoable because a data pack is copied into the world's own
/// folder, which has no record to take back
#[allow(clippy::too_many_arguments)]
pub async fn install_package_with(
    state: &Arc<LauncherState>,
    provider: oneclient_content::packages::ProviderId,
    project_id: &str,
    version_id: &str,
    cluster_id: i64,
    world: Option<String>,
    allow_flagged: bool,
    incompatible: IncompatibleMode,
    offer_undo: bool,
) -> Result<PackageInstall, InstallBlocked> {
    let _instance = instance_install_guard(cluster_id).await;
    let lookup = async {
        let provider_impl = state.services.packages.get(provider)?;
        let project = provider_impl
            .get_project(project_id, &state.services.content())
            .await?;
        let version = provider_impl
            .get_version(project_id, version_id, &state.services.content())
            .await?;
        anyhow::Ok((project, version))
    }
    .await;

    let (project, version) = match lookup {
        Ok(found) => found,
        Err(err) => return Ok(PackageInstall::failed(err)),
    };

    if let Some(world) = world {
        let project = oneclient_content::packages::types::ProjectDetail {
            content_type: oneclient_content::packages::ContentType::DataPack,
            ..project
        };
        if incompatible != IncompatibleMode::Allow {
            match PackageStore::check_compatible(
                &project,
                &version,
                cluster_id,
                &state.services.content(),
            )
            .await
            {
                Ok(true) => {}
                Ok(false) if incompatible == IncompatibleMode::Ask => {
                    return Err(InstallBlocked::Incompatible { name: project.name });
                }
                Ok(false) => {
                    return Ok(PackageInstall::failed(anyhow::anyhow!(tf(
                        "oneclient.components.incompatible_install.body",
                        &[&project.name]
                    ))));
                }
                Err(err) => return Ok(PackageInstall::failed(err.into())),
            }
        }
        return Ok(install_datapack(state, provider, &project, &version, cluster_id, world).await);
    }

    let content = state.services.content();
    let bad_mods = oneclient_content::packages::load_bad_mods(&content).await;
    let screened = !allow_flagged
        && !state
            .clusters
            .get(cluster_id)
            .await
            .is_ok_and(|cluster| cluster.user_created);

    if screened && let Some(entry) = bad_mods.check(&project, &version) {
        tracing::warn!(project = %project.name, version = %version.version_number, "refusing to install flagged mod");
        return Err(InstallBlocked::Flagged(
            flagged_install(project.name, entry, cluster_id, &content).await,
        ));
    }

    if incompatible == IncompatibleMode::Ask {
        match PackageStore::check_compatible(&project, &version, cluster_id, &content).await {
            Ok(true) => {}
            Ok(false) => return Err(InstallBlocked::Incompatible { name: project.name }),
            Err(err) => return Ok(PackageInstall::failed(err.into())),
        }
    }

    // Resolved before the session starts so its children can be announced up front
    let mut resolution = oneclient_content::packages::DependencyResolution::default();
    if oneclient_content::packages::resolves_dependencies(project.content_type) {
        match oneclient_content::packages::resolve_required(
            provider,
            &version,
            cluster_id,
            &state.services.content(),
        )
        .await
        {
            Ok(resolved) => resolution = resolved,
            Err(err) => {
                tracing::warn!(%err, "dependency resolution failed, installing package alone");
            }
        }
    }

    if screened
        && let Some((dependency, entry)) = resolution.install.iter().find_map(|dependency| {
            bad_mods
                .check(&dependency.project, &dependency.version)
                .map(|entry| (dependency, entry))
        })
    {
        tracing::warn!(dependency = %dependency.project.name, project = %project.name, "refusing to install flagged dependency");
        let name = tf(
            "oneclient.install.required_by",
            &[&dependency.project.name, &project.name],
        );
        return Err(InstallBlocked::Flagged(
            flagged_install(name, entry, cluster_id, &content).await,
        ));
    }

    let baseline = if offer_undo
        && project.content_type != oneclient_content::packages::ContentType::Modpack
    {
        ClusterSnapshot::take(cluster_id, state).await
    } else {
        None
    };

    let session = oneclient_events::GroupedProgressSession::start(
        &state.services.events,
        tf("oneclient.install.installing", &[&project.name]),
    );

    let size = version.primary_file().map(|f| f.size).unwrap_or(0);
    let dependency_bytes: u64 = resolution
        .install
        .iter()
        .map(|dep| dep.version.primary_file().map(|f| f.size).unwrap_or(0))
        .sum();
    session.expect(
        oneclient_events::TaskCategory::Packages,
        1 + resolution.install.len() as u64,
        size + dependency_bytes,
    );

    let mut installed_dependencies = Vec::new();
    let mut undo_dependencies = Vec::new();
    let mut missing_dependencies = resolution.unresolved;

    // Dependencies first so the package is never in a cluster without them however the run ends
    for dependency in &resolution.install {
        let child = session.child(
            dependency.project.name.clone(),
            dependency
                .version
                .primary_file()
                .map(|f| f.size)
                .unwrap_or(0),
            oneclient_events::TaskCategory::Packages,
        );

        let result = PackageStore::install_to_cluster_recording_link(
            provider,
            &dependency.project,
            &dependency.version,
            cluster_id,
            false,
            false,
            Some(&child),
            &state.services.content(),
        )
        .await;

        child.finish();

        match result {
            Ok((artifact, _, added_at)) => {
                mark_new(cluster_id, &artifact.hash, state).await;
                installed_dependencies.push(dependency.project.name.clone());
                if baseline.is_some()
                    && let Some(added_at) = added_at
                {
                    undo_dependencies.push(UndoPackage {
                        provider,
                        project_id: dependency.project.id.clone(),
                        hash: artifact.hash,
                        name: dependency.project.name.clone(),
                        content_type: dependency.project.content_type,
                        added_at,
                    });
                }
            }
            Err(err) => {
                tracing::warn!(
                    dependency = %dependency.project.name,
                    %err,
                    "failed to install dependency"
                );
                missing_dependencies.push(dependency.project.name.clone());
            }
        }
    }

    let child = session.child(
        project.name.clone(),
        size,
        oneclient_events::TaskCategory::Packages,
    );

    let result = PackageStore::install_to_cluster_recording_link(
        provider,
        &project,
        &version,
        cluster_id,
        incompatible == IncompatibleMode::Allow,
        false,
        Some(&child),
        &state.services.content(),
    )
    .await;

    child.finish();

    let live_deferred = matches!(
        result,
        Ok((_, oneclient_content::packages::LiveSync::Deferred, _))
    ) && state.games.is_active(cluster_id);

    let mut undo = None;
    if let Ok((artifact, _, added_at)) = &result {
        mark_new(cluster_id, &artifact.hash, state).await;
        if let Some(added_at) = added_at {
            let target = UndoPackage {
                provider,
                project_id: project.id.clone(),
                hash: artifact.hash.clone(),
                name: project.name.clone(),
                content_type: project.content_type,
                added_at: added_at.clone(),
            };
            undo = plan_undo(baseline.as_ref(), cluster_id, target, undo_dependencies);
        }
    }

    Ok(PackageInstall {
        session_id: Some(session.detach()),
        result: result.map(|_| project.name).map_err(anyhow::Error::from),
        dependencies: installed_dependencies,
        missing_dependencies,
        live_deferred,
        undo,
    })
}

async fn install_datapack(
    state: &Arc<LauncherState>,
    provider: oneclient_content::packages::ProviderId,
    project: &oneclient_content::packages::types::ProjectDetail,
    version: &oneclient_content::packages::types::VersionDetail,
    cluster_id: i64,
    world: String,
) -> PackageInstall {
    let Some(file) = version.primary_file() else {
        return PackageInstall::failed(anyhow::anyhow!(t("oneclient.install.datapack.no_file")));
    };
    if !file.file_name.to_lowercase().ends_with(".zip") {
        return PackageInstall::failed(anyhow::anyhow!(t("oneclient.install.datapack.not_zip")));
    }

    let session = oneclient_events::GroupedProgressSession::start(
        &state.services.events,
        tf("oneclient.install.installing", &[&project.name]),
    );
    session.expect(oneclient_events::TaskCategory::Packages, 1, file.size);
    let child = session.child(
        project.name.clone(),
        file.size,
        oneclient_events::TaskCategory::Packages,
    );

    let result = async {
        let artifact = PackageStore::download_and_cache(
            provider,
            project,
            version,
            false,
            Some(&child),
            &state.services.content(),
        )
        .await?;
        let path = oneclient_content::packages::store::artifact_absolute_path(&artifact.path)?;
        let _mutation = oneclient_content::packages::package_mutation_guard().await;
        let cluster = state.clusters.get(cluster_id).await?;
        oneclient_core::add_world_datapacks(&cluster, &world, &[path]).await?;
        anyhow::Ok(())
    }
    .await;

    child.finish();

    PackageInstall {
        session_id: Some(session.detach()),
        result: result.map(|()| {
            tf(
                "oneclient.install.datapack.target",
                &[&project.name, &world],
            )
        }),
        dependencies: Vec::new(),
        missing_dependencies: Vec::new(),
        live_deferred: false,
        undo: None,
    }
}

async fn mark_new(cluster_id: i64, hash: &str, state: &LauncherState) {
    if let Err(err) =
        PackageStore::mark_artifact_new(cluster_id, hash, &state.services.content()).await
    {
        tracing::warn!(cluster_id, hash, %err, "failed to badge package as new");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oneclient_content::packages::ProviderId;

    fn package(project: &str, hash: &str) -> UndoPackage {
        UndoPackage {
            provider: ProviderId::Modrinth,
            project_id: project.into(),
            hash: hash.into(),
            name: project.into(),
            content_type: oneclient_content::packages::ContentType::Mod,
            added_at: "original-link".to_string(),
        }
    }

    fn snapshot(installed: &[(&str, &str)]) -> ClusterSnapshot {
        ClusterSnapshot {
            hashes: installed.iter().map(|(_, hash)| hash.to_string()).collect(),
            projects: installed
                .iter()
                .map(|(project, _)| (ProviderId::Modrinth as i64, project.to_string()))
                .collect(),
        }
    }

    #[test]
    fn a_new_package_and_its_new_dependencies_can_be_undone() {
        let before = snapshot(&[("fabric-api", "f1")]);

        let undo = plan_undo(
            Some(&before),
            7,
            package("sodium", "s1"),
            vec![package("indium", "i1")],
        )
        .expect("a package the instance never had is undoable");

        assert_eq!(undo.cluster_id, 7);
        let hashes: Vec<_> = undo.packages.iter().map(|p| p.hash.as_str()).collect();
        assert_eq!(hashes, ["s1", "i1"]);
    }

    #[test]
    fn a_dependency_the_instance_already_had_is_never_undone() {
        let before = snapshot(&[("fabric-api", "f1")]);

        let undo = plan_undo(
            Some(&before),
            1,
            package("sodium", "s1"),
            vec![package("fabric-api", "f1"), package("indium", "i1")],
        )
        .unwrap();

        let hashes: Vec<_> = undo.packages.iter().map(|p| p.hash.as_str()).collect();
        assert_eq!(hashes, ["s1", "i1"]);
    }

    #[test]
    fn an_update_or_reinstall_gets_no_undo() {
        let before = snapshot(&[("sodium", "s1")]);

        // Same file again
        assert!(plan_undo(Some(&before), 1, package("sodium", "s1"), vec![]).is_none());
        // A newer version replaced the old one, which Undo could not restore
        assert!(plan_undo(Some(&before), 1, package("sodium", "s2"), vec![]).is_none());
    }

    #[test]
    fn replacing_a_release_through_another_provider_alias_gets_no_undo() {
        use oneclient_db::models::ClusterReleaseIdentityRow;

        let identities = [
            (ProviderId::Modrinth, "modrinth-project"),
            (ProviderId::CurseForge, "curseforge-project"),
        ]
        .map(|(provider, project_id)| ClusterReleaseIdentityRow {
            hash: "original-file".into(),
            provider: provider as i64,
            project_id: project_id.into(),
            version_id: "original-release".into(),
        });
        let before = ClusterSnapshot::from_links(["original-file".into()], identities.into());
        let update = UndoPackage {
            provider: ProviderId::CurseForge,
            ..package("curseforge-project", "new-file")
        };

        assert!(plan_undo(Some(&before), 1, update, vec![]).is_none());
    }

    #[test]
    fn no_baseline_means_no_undo() {
        assert!(plan_undo(None, 1, package("sodium", "s1"), vec![]).is_none());
    }

    #[tokio::test]
    async fn undo_waits_for_in_flight_package_mutations() {
        let install = oneclient_content::packages::package_mutation_guard().await;
        let mut undo =
            std::pin::pin!(oneclient_content::packages::exclusive_package_mutation_guard());
        assert!(futures_util::poll!(&mut undo).is_pending());

        drop(install);
        let _undo = undo.await;
    }

    #[test]
    fn an_undo_failure_releases_the_action_for_retry() {
        let undo = plan_undo(Some(&snapshot(&[])), 1, package("sodium", "s1"), vec![]).unwrap();
        assert!(undo.claim());
        undo.release();

        let retry = undo.clone();
        assert!(retry.claim());
        assert!(!undo.claim());
    }

    #[test]
    fn an_undo_can_only_be_claimed_once() {
        let undo = plan_undo(Some(&snapshot(&[])), 1, package("sodium", "s1"), vec![]).unwrap();

        assert!(undo.claim());
        assert!(!undo.claim());
        assert!(!undo.clone().claim());
    }
}
