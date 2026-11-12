use std::sync::Arc;

use oneclient_content::packages::PackageStore;

use crate::LauncherState;

pub struct PackageInstall {
    pub session_id: Option<uuid::Uuid>,
    pub result: anyhow::Result<String>,
    pub dependencies: Vec<String>,
    pub missing_dependencies: Vec<String>,
    /// The game is open but could not take the package, so the toast must not
    /// claim it is there now
    pub live_deferred: bool,
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

    if live_deferred {
        body.push_str(" Minecraft is running, so it will be there at the next launch.");
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

pub async fn install_package(
    state: &Arc<LauncherState>,
    provider: oneclient_content::packages::ProviderId,
    project_id: &str,
    version_id: &str,
    cluster_id: i64,
    world: Option<String>,
    allow_flagged: bool,
) -> Result<PackageInstall, FlaggedInstall> {
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
        return Err(flagged_install(project.name, entry, cluster_id, &content).await);
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
        let name = format!("{} (required by {})", dependency.project.name, project.name);
        return Err(flagged_install(name, entry, cluster_id, &content).await);
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
            Ok((artifact, _)) => {
                mark_new(cluster_id, &artifact.hash, state).await;
                installed_dependencies.push(dependency.project.name.clone());
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

    let live_deferred = matches!(
        result,
        Ok((_, oneclient_content::packages::LiveSync::Deferred))
    ) && state.games.is_active(cluster_id);

    if let Ok((artifact, _)) = &result {
        mark_new(cluster_id, &artifact.hash, state).await;
    }

    Ok(PackageInstall {
        session_id: Some(session.detach()),
        result: result.map(|_| project.name).map_err(anyhow::Error::from),
        dependencies: installed_dependencies,
        missing_dependencies,
        live_deferred,
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
        return PackageInstall::failed(anyhow::anyhow!("This version has no file to download"));
    };
    if !file.file_name.to_lowercase().ends_with(".zip") {
        return PackageInstall::failed(anyhow::anyhow!(
            "This version is a mod, not a data pack. Pick a .zip version instead"
        ));
    }

    let session = oneclient_events::GroupedProgressSession::start(
        &state.services.events,
        format!("Installing {}", project.name),
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
        let cluster = state.clusters.get(cluster_id).await?;
        crate::add_world_datapacks(&cluster, &world, &[path]).await?;
        anyhow::Ok(())
    }
    .await;

    child.finish();

    PackageInstall {
        session_id: Some(session.detach()),
        result: result.map(|()| format!("{} to {world}", project.name)),
        dependencies: Vec::new(),
        missing_dependencies: Vec::new(),
        live_deferred: false,
    }
}

async fn mark_new(cluster_id: i64, hash: &str, state: &LauncherState) {
    if let Err(err) =
        PackageStore::mark_artifact_new(cluster_id, hash, &state.services.content()).await
    {
        tracing::warn!(cluster_id, hash, %err, "failed to badge package as new");
    }
}

pub type OptionalMod = (i64, (String, String));

fn group_by_cluster(mods: &[OptionalMod]) -> Vec<(i64, Vec<String>)> {
    let mut grouped: Vec<(i64, Vec<String>)> = Vec::new();

    for (cluster_id, (_bundle_name, package_id)) in mods {
        match grouped.iter_mut().find(|(id, _)| id == cluster_id) {
            Some((_, package_ids)) => package_ids.push(package_id.clone()),
            None => grouped.push((*cluster_id, vec![package_id.clone()])),
        }
    }

    grouped
}

pub async fn skip_optional_mods(state: &LauncherState, mods: &[OptionalMod]) {
    let content = state.services.content();

    for (cluster_id, package_ids) in group_by_cluster(mods) {
        if let Err(err) = crate::skip_optional_mods(cluster_id, &package_ids, &content).await {
            tracing::warn!(
                cluster_id,
                error = %err,
                "failed to record skipped optional mods, they will come back as new"
            );
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OptionalModsOutcome {
    pub installed: usize,
    pub failed: usize,
}

pub async fn enable_optional_mods(
    state: &Arc<LauncherState>,
    mods: &[OptionalMod],
) -> OptionalModsOutcome {
    let content = state.services.content();
    let session = oneclient_events::GroupedProgressSession::start(
        &state.services.events,
        "Adding optional mods",
    );
    let opt_in = session.child(
        "Enabling mods",
        mods.len() as u64,
        oneclient_events::TaskCategory::Packages,
    );
    opt_in.set_phase(oneclient_events::TaskPhase::Installing);

    let mut clusters: Vec<i64> = Vec::new();
    let mut enabled: Vec<OptionalMod> = Vec::new();
    let mut failed = 0usize;
    for (cluster_id, (bundle_name, package_id)) in mods {
        match crate::set_bundle_package_enabled(
            *cluster_id,
            bundle_name,
            package_id,
            true,
            false,
            &content,
        )
        .await
        {
            Ok(()) => {
                if !clusters.contains(cluster_id) {
                    clusters.push(*cluster_id);
                }
                enabled.push((*cluster_id, (bundle_name.clone(), package_id.clone())));
            }
            Err(err) => {
                failed += 1;
                tracing::warn!(
                    cluster_id,
                    %bundle_name,
                    %package_id,
                    error = %err,
                    "failed to opt in to an optional mod"
                );
            }
        }
    }

    opt_in.finish();

    let mut installed = 0usize;
    let mut applied: Vec<i64> = Vec::new();
    for cluster_id in &clusters {
        match crate::apply_bundle_updates_with(
            *cluster_id,
            state.bundles.as_ref(),
            &content,
            Some(&session),
            None,
        )
        .await
        {
            Ok(result) => {
                installed += result.additions_applied.len();
                failed += result.additions_failed.len() + result.downloads_failed.len();
                applied.push(*cluster_id);
            }
            Err(err) => {
                tracing::warn!(cluster_id, error = %err, "failed to install optional mods");
            }
        }
    }

    // An offer only counts as answered once its mod really was enabled
    // and installed otherwise the click is lost and never comes back
    enabled.retain(|(cluster_id, _)| applied.contains(cluster_id));
    for (cluster_id, package_ids) in group_by_cluster(&enabled) {
        if let Err(err) = crate::resolve_optional_mods(cluster_id, &package_ids, &content).await {
            tracing::warn!(cluster_id, error = %err, "failed to clear answered offers");
        }
    }

    session.finish();
    OptionalModsOutcome { installed, failed }
}

pub async fn apply_package_updates(
    state: &LauncherState,
    updates: &[crate::BrowserPackageUpdate],
    title: &str,
) -> (usize, uuid::Uuid) {
    let content = state.services.content();
    let session = oneclient_events::GroupedProgressSession::start(&state.services.events, title);
    session.expect(
        oneclient_events::TaskCategory::Packages,
        updates.len() as u64,
        updates.len() as u64,
    );

    let mut applied = 0usize;
    for update in updates {
        let child = session.child(
            update.display_name.clone(),
            1,
            oneclient_events::TaskCategory::Packages,
        );
        match crate::apply_browser_package_update(update, Some(&child), &content).await {
            Ok(_) => applied += 1,
            // Not a reason to hold the game back the cache row survives
            // so the next launch offers it again
            Err(err) => tracing::warn!(
                package = %update.display_name,
                error = %err,
                "automatic package update failed"
            ),
        }
        child.finish();
    }

    (applied, session.detach())
}
