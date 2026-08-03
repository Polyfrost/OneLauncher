//! Shared helpers for content installs and the notifications they raise.
//!
//! Lifted out of the bridge runtime so the actions layer can use them without
//! the command loop existing.

use std::path::PathBuf;
use std::sync::Arc;

use oneclient_core::LauncherState;

use oneclient_content::packages::{
    ContentType, IdentifiedPackage, InstalledCopy, PackageStore, ProjectDetail, VersionDetail,
};
use oneclient_events::{Choice, Level, Prompt};

use crate::components::IconType;
use crate::notifications::{
    ClusterUpdateItem, ClusterUpdateSummary, NotificationAction, NotificationActionKind,
    NotificationSpec, PackageUpdateGroup,
};

/// Names a cluster for a notification or modal header, falling back rather than
/// failing: a missing row is not a reason to lose the whole message.
pub async fn cluster_display_name(
    cluster_id: i64,
    services: &oneclient_core::LauncherServices,
) -> String {
    PackageStore::get_cluster(cluster_id, &services.content())
        .await
        .map(|cluster| cluster.name)
        .unwrap_or_else(|_| "Cluster".to_string())
}

/// Turns the launching cluster's pending browser updates into what the modal
/// renders. `None` when there is nothing to offer.
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
            project_id: Some(project_id.clone()),
            fallback: file.display_name(),
        },
        oneclient_core::BundleFileKind::External(_) => ClusterUpdateItem {
            provider: oneclient_content::packages::ProviderId::Local,
            project_id: None,
            fallback: file.display_name(),
        },
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
            project_id: r.project_id.clone(),
            fallback: r
                .display_name
                .clone()
                .unwrap_or_else(|| r.package_id.clone()),
        })
        .collect();

    if updated.is_empty() && added.is_empty() && removed.is_empty() {
        return None;
    }

    let cluster_name =
        oneclient_content::packages::PackageStore::get_cluster(cluster_id, &services.content())
            .await
            .map(|c| c.name)
            .unwrap_or_else(|_| "Cluster".to_string());

    Some(ClusterUpdateSummary {
        cluster_id,
        cluster_name,
        updated,
        added,
        removed,
    })
}

pub async fn cluster_update_notification(
    cluster_id: i64,
    result: &oneclient_core::ApplyBundleUpdatesResult,
    services: &oneclient_core::LauncherServices,
) -> Option<NotificationSpec> {
    let summary = cluster_update_summary(cluster_id, result, services).await?;
    let total = summary.total();
    let body = format!(
        "{total} package{} changed in {}",
        if total == 1 { "" } else { "s" },
        summary.cluster_name
    );

    Some(NotificationSpec {
        title: "Cluster updated".to_string(),
        body,
        level: Level::Info,
        icon: Some(IconType::DownloadCloud02),
        progress: None,
        actions: vec![NotificationAction {
            label: "View changes".to_string(),
            kind: NotificationActionKind::OpenClusterUpdate(vec![summary]),
        }],
    })
}

/// Builds a single notification summarising a batch bundle sync. Every changed
/// cluster rides along on one "View changes" action so the notification keeps
/// exactly two buttons no matter how many clusters moved; the modal does the
/// per-cluster breakdown. `None` when nothing changed.
pub async fn combined_cluster_update_spec(
    changed: &[(i64, oneclient_core::ApplyBundleUpdatesResult)],
    services: &oneclient_core::LauncherServices,
) -> Option<NotificationSpec> {
    let mut summaries = Vec::new();
    let mut total_changes = 0usize;

    for (cluster_id, result) in changed {
        if let Some(summary) = cluster_update_summary(*cluster_id, result, services).await {
            total_changes += summary.total();
            summaries.push(summary);
        }
    }

    if summaries.is_empty() {
        return None;
    }

    let cluster_count = summaries.len();
    let body = format!(
        "{total_changes} package{} updated across {cluster_count} cluster{}",
        if total_changes == 1 { "" } else { "s" },
        if cluster_count == 1 { "" } else { "s" }
    );

    Some(NotificationSpec {
        title: "Mods updated".to_string(),
        body,
        level: Level::Info,
        icon: Some(IconType::DownloadCloud02),
        progress: None,
        actions: vec![NotificationAction {
            label: "View changes".to_string(),
            kind: NotificationActionKind::OpenClusterUpdate(summaries),
        }],
    })
}

/// An install's outcome together with the progress session it ran under.
///
/// The session is detached rather than finished so the caller can convert that
/// same notification into the "Installed" / "Install failed" result, instead of
/// leaving a finished progress card and raising a second notification beside it.
/// `session_id` is `None` when the install never got as far as downloading.
pub struct PackageInstall {
    pub session_id: Option<uuid::Uuid>,
    pub outcome: InstallOutcome,
    /// Required dependencies pulled in alongside the package, by display name.
    pub dependencies: Vec<String>,
    /// Required dependencies that couldn't be resolved or downloaded. The
    /// package still installs; the caller says so in the notification.
    pub missing_dependencies: Vec<String>,
}

/// How an install ended.
///
/// Cancelled is its own arm rather than an error because backing out of the
/// "already installed" warning is the warning working: the user was told what
/// the cluster has and said no, and reporting that back to them as a failure
/// would be telling them something they just decided.
pub enum InstallOutcome {
    /// Installed, named by the package's display name.
    Installed(String),
    Cancelled,
    Failed(anyhow::Error),
}

impl InstallOutcome {
    pub fn is_installed(&self) -> bool {
        matches!(self, Self::Installed(_))
    }
}

/// "Added Sodium with 2 dependencies. Could not add: Fabric API."
pub fn install_body(name: &str, dependencies: &[String], missing: &[String]) -> String {
    let mut body = format!("Added {name}");

    if !dependencies.is_empty() {
        body.push_str(&format!(
            " with {} dependenc{}",
            dependencies.len(),
            if dependencies.len() == 1 { "y" } else { "ies" }
        ));
    }
    body.push('.');

    if !missing.is_empty() {
        body.push_str(&format!(" Could not add: {}.", missing.join(", ")));
    }

    body
}

impl PackageInstall {
    fn ended(outcome: InstallOutcome) -> Self {
        Self {
            session_id: None,
            outcome,
            dependencies: Vec::new(),
            missing_dependencies: Vec::new(),
        }
    }
}

/// The one answer the duplicate warnings offer besides backing out.
enum GoAhead {
    Yes,
}

/// Puts the warning up and reports whether the user wants to carry on.
///
/// A prompt nobody could answer counts as no. The bus is only closed when the
/// window is going away, and the front-end holds a single pending prompt at a
/// time — so the other way to land here is a second prompt having displaced this
/// one, and quietly adding a duplicate the user was never shown is the worse of
/// the two outcomes.
async fn ask_to_continue(
    state: &Arc<LauncherState>,
    title: String,
    body: String,
    confirm: &'static str,
) -> bool {
    match state
        .services
        .events
        .ask(
            Prompt::new(title, body)
                .option(Choice::primary("install-anyway", confirm), GoAhead::Yes)
                .dismiss("Cancel"),
        )
        .await
    {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(err) => {
            tracing::warn!(%err, "could not warn about an existing copy; not installing over it");
            false
        }
    }
}

/// Asks before a package the cluster already has is added again.
///
/// This sits on the install path rather than on the buttons that start one. The
/// browser's listing cards, the package page's sidebar and its version list all
/// come through [`install_package`], and a check attached to a button is a check
/// the next button can forget to make.
///
/// Not knowing is not a reason to refuse: if the cluster's contents cannot be
/// read the install goes ahead unwarned.
async fn confirm_duplicate_package(
    state: &Arc<LauncherState>,
    project: &ProjectDetail,
    version: &VersionDetail,
    cluster_id: i64,
) -> bool {
    let copies = match oneclient_content::packages::installed_copies(
        project.provider,
        &project.id,
        cluster_id,
        &state.services.content(),
    )
    .await
    {
        Ok(copies) if copies.is_empty() => return true,
        Ok(copies) => copies,
        Err(err) => {
            tracing::warn!(%err, "could not check the cluster for an existing copy");
            return true;
        }
    };

    let cluster = cluster_display_name(cluster_id, &state.services).await;
    let installed = copies
        .iter()
        .map(|copy| copy.label.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    // The same version again and a second version beside the first are
    // different mistakes, and the warning is only worth reading if it says which
    // one is about to happen.
    let same_version = copies
        .iter()
        .any(|copy| copy.version_id.as_deref() == Some(version.version_id.as_str()));

    let (body, confirm) = if same_version {
        (
            format!(
                "{cluster} already has {installed}. Installing it again downloads the same files over the copy that is there."
            ),
            "Reinstall",
        )
    } else {
        (
            format!(
                "{cluster} already has {installed}. Adding {} leaves both in the cluster, and only the newest one is loaded.",
                version.name
            ),
            "Install anyway",
        )
    };

    ask_to_continue(
        state,
        format!("{} is already installed", project.name),
        body,
        confirm,
    )
    .await
}

/// A file the user has agreed to import, and what the providers said about it.
pub struct PendingImport {
    pub content_type: ContentType,
    pub path: PathBuf,
    /// Set when a provider recognised the file. The import records it against
    /// the artifact so what lands in the cluster is a browsed package rather
    /// than a nameless local jar.
    pub identified: Option<IdentifiedPackage>,
}

/// What a batch of dropped or picked files should turn into once the user has
/// been told which of them the cluster already has. `None` means they cancelled.
///
/// Takes the whole batch because the front-end renders one prompt at a time: a
/// five-file drop asked about five times over would leave four of the questions
/// unanswerable. The batch also makes the provider lookup one round trip per
/// provider instead of one per file.
pub async fn confirm_duplicate_files(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    files: Vec<(ContentType, PathBuf)>,
) -> Option<Vec<PendingImport>> {
    let content = state.services.content();

    // The one place a local file gets a name. Everything below — which question
    // the user is asked, and what the import writes down afterwards — follows
    // from whether a provider claimed the bytes.
    let paths: Vec<PathBuf> = files.iter().map(|(_, path)| path.clone()).collect();
    let identified = oneclient_content::packages::identify_for_install(
        &paths,
        oneclient_content::packages::InstallIntent::user_initiated(),
        &content,
    )
    .await
    .unwrap_or_else(|err| {
        // Offline, or a provider having a bad day. The files still import; they
        // just import as local files, which is what they were before.
        tracing::warn!(%err, "could not identify the dropped files");
        vec![None; files.len()]
    });

    let mut fresh: Vec<PendingImport> = Vec::new();
    let mut duplicates: Vec<(PendingImport, InstalledCopy)> = Vec::new();

    for ((content_type, path), identified) in files.into_iter().zip(identified) {
        // A recognised file is asked about as the package it is, not as the file
        // it arrived as: the cluster's copy of Sodium is a duplicate of this jar
        // whatever either of them is called on disk, and only the project id can
        // see that.
        let existing = match &identified {
            Some(found) => oneclient_content::packages::installed_copies(
                found.provider,
                &found.version.project_id,
                cluster_id,
                &content,
            )
            .await
            .map(|copies| copies.into_iter().next()),
            None => {
                oneclient_content::packages::installed_local_copy(
                    &path,
                    content_type,
                    cluster_id,
                    &content,
                )
                .await
            }
        };

        let pending = PendingImport {
            content_type,
            path,
            identified,
        };

        match existing {
            Ok(Some(copy)) => duplicates.push((pending, copy)),
            Ok(None) => fresh.push(pending),
            // Same as the browser path: a file we could not place is imported
            // rather than held back, and the import reports its own failure.
            Err(err) => {
                tracing::warn!(%err, path = %pending.path.display(), "could not check the cluster for this file");
                fresh.push(pending);
            }
        }
    }

    if duplicates.is_empty() {
        return Some(fresh);
    }

    let cluster = cluster_display_name(cluster_id, &state.services).await;
    let names = duplicates
        .iter()
        .map(|(_, copy)| copy.label.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    let body = if fresh.is_empty() {
        format!("{cluster} already has {names}. Adding them again replaces what is there.")
    } else {
        format!(
            "{cluster} already has {names}. The other {} will be added either way.",
            match fresh.len() {
                1 => "file".to_string(),
                n => format!("{n} files"),
            }
        )
    };

    let title = match duplicates.len() {
        1 => "1 file is already in this cluster".to_string(),
        n => format!("{n} files are already in this cluster"),
    };

    if ask_to_continue(state, title, body, "Add anyway").await {
        fresh.extend(duplicates.into_iter().map(|(pending, _)| pending));
        return Some(fresh);
    }

    // Backing out means "not the ones I already have", not "none of them" —
    // the files the cluster has never seen were never in question.
    (!fresh.is_empty()).then_some(fresh)
}

pub async fn install_package(
    state: &Arc<LauncherState>,
    provider: oneclient_content::packages::ProviderId,
    project_id: &str,
    version_id: &str,
    cluster_id: i64,
) -> PackageInstall {
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
        Err(err) => return PackageInstall::ended(InstallOutcome::Failed(err)),
    };

    // Asked before anything is resolved or downloaded, so declining costs the
    // user nothing and no progress card appears behind the question.
    if !confirm_duplicate_package(state, &project, &version, cluster_id).await {
        return PackageInstall::ended(InstallOutcome::Cancelled);
    }

    // Worked out before the session starts so its children can be announced up
    // front; a failure here leaves the package itself installable.
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

    let session = oneclient_events::GroupedProgressSession::start(
        &state.services.events,
        format!("Installing {}", project.name),
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
    let mut missing_dependencies = resolution.unresolved;

    // Dependencies go in first so the package is never sitting in a cluster
    // without them, however the run ends.
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

        let result = PackageStore::install_to_cluster(
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
            Ok(_) => installed_dependencies.push(dependency.project.name.clone()),
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

    let result = PackageStore::install_to_cluster(
        provider,
        &project,
        &version,
        cluster_id,
        false,
        false,
        Some(&child),
        &state.services.content(),
    )
    .await;

    child.finish();

    PackageInstall {
        session_id: Some(session.detach()),
        outcome: match result {
            Ok(_) => InstallOutcome::Installed(project.name),
            Err(err) => InstallOutcome::Failed(err.into()),
        },
        dependencies: installed_dependencies,
        missing_dependencies,
    }
}
