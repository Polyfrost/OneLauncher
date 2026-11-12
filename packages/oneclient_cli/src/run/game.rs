use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use clap::Args;
use oneclient_cluster::PackageUpdateMode;
use oneclient_core::busy::{self, LaunchClaim};
use oneclient_core::{Cluster, LauncherState};
use oneclient_events::{Choice, Prompt};
use serde::{Deserialize, Serialize};

use super::{Host, clusters_changed, find_account, find_cluster, logs};
use crate::Output;
use crate::ipc::{self, IpcCommand};
use crate::output::{bold, dim, good, table};

#[derive(Debug, Clone, Args, Serialize, Deserialize)]
pub struct LaunchArgs {
    /// Cluster id, folder name or name
    #[arg(required_unless_present = "last")]
    pub cluster: Option<String>,
    /// Launch the most recently played cluster
    #[arg(long, conflicts_with = "cluster")]
    pub last: bool,
    /// Account to play with instead of the default one
    #[arg(long)]
    pub account: Option<String>,
    /// Return once the game has started instead of waiting for it to close
    #[arg(short, long)]
    pub detach: bool,
    /// Print the game output while waiting
    #[arg(short, long, conflicts_with = "detach")]
    pub logs: bool,
}

#[derive(Debug, Clone, Args, Serialize, Deserialize)]
pub struct StopArgs {
    /// Cluster id, folder name or name
    #[arg(required_unless_present = "all")]
    pub cluster: Option<String>,
    /// Stop every running game
    #[arg(long, conflicts_with = "cluster")]
    pub all: bool,
}

async fn launch_target(state: &LauncherState, args: &LaunchArgs) -> anyhow::Result<Cluster> {
    match &args.cluster {
        Some(query) => find_cluster(state, query).await,
        None => {
            let mut clusters = state.clusters.list().await?;
            clusters.sort_by(oneclient_cluster::compare_recent_activity);
            clusters
                .into_iter()
                .next()
                .context("there are no clusters to launch")
        }
    }
}

pub(super) async fn launch(
    host: Host,
    state: &Arc<LauncherState>,
    args: LaunchArgs,
    out: &Output,
) -> anyhow::Result<()> {
    let cluster = launch_target(state, &args).await?;

    if host == Host::Gui {
        if args.account.is_some() {
            out.error(
                "--account is ignored while OneClient is open, it plays as the default account",
            );
        }
        ipc::send(IpcCommand::Launch(cluster.folder_name.clone()));
        out.line(format!("Launching {} in OneClient", bold(&cluster.name)));
        return Ok(());
    }

    let claim = LaunchClaim::take(cluster.id)
        .with_context(|| format!("{} is already launching", cluster.name))?;
    if state.games.is_active(cluster.id) {
        bail!("{} is already running", cluster.name);
    }
    if let Some(busy) = busy::launch_block(state, cluster.id).await {
        bail!("{}: {busy}", cluster.name);
    }

    let account = match &args.account {
        Some(query) => {
            let account = find_account(state, query).await?;
            state.auth.account_for_launch(account.id).await?
        }
        None => state
            .auth
            .default_account_for_launch()
            .await?
            .context("add a Minecraft account first with `oneclient_cli account login`")?,
    };

    super::share::before_launch(state, &cluster, out).await;
    prepare(state, &cluster, out).await?;

    out.line(format!(
        "Launching {} as {}",
        bold(&cluster.name),
        bold(&account.username)
    ));
    let game = oneclient_core::launch_cluster_or_repair(state, cluster.id, &account, true).await?;
    drop(claim);
    out.line(format!(
        "{} {} is running{}",
        good("✓"),
        cluster.name,
        game.pid
            .map(|pid| format!(" (pid {pid})"))
            .unwrap_or_default()
    ));

    if args.detach {
        return Ok(());
    }

    out.line(dim(
        "Waiting for the game to close, Ctrl-C leaves it running",
    ));
    let follow = args
        .logs
        .then(|| tokio::spawn(logs::follow(cluster.clone(), out.clone())));

    while state.games.is_active(cluster.id) && !out.is_closed() && !ipc::winding_down() {
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    if let Some(follow) = follow {
        follow.abort();
    }
    if state.games.is_active(cluster.id) {
        out.line(format!("Stopped waiting, {} keeps running", cluster.name));
        return Ok(());
    }

    tokio::time::sleep(Duration::from_secs(2)).await;
    out.line(format!("{} closed", cluster.name));
    super::share::cloud_sync_synced(state, &cluster, out).await;
    Ok(())
}

pub(super) async fn stop(
    state: &Arc<LauncherState>,
    args: StopArgs,
    out: &Output,
) -> anyhow::Result<()> {
    let ids = match &args.cluster {
        Some(query) => vec![find_cluster(state, query).await?.id],
        None => state.games.active_ids(),
    };
    if ids.is_empty() {
        out.line("No games are running");
    }

    for id in ids {
        let name = state.clusters.get(id).await.map(|c| c.name)?;
        if state.games.kill(id) {
            out.line(format!("Stopped {}", bold(name)));
        } else {
            out.error(format!("{name} is not running"));
        }
    }
    Ok(())
}

pub(super) async fn ps(state: &Arc<LauncherState>, out: &Output) -> anyhow::Result<()> {
    let ids = state.games.active_ids();
    if ids.is_empty() {
        out.line("No games are running");
        return Ok(());
    }

    let mut rows = Vec::new();
    for id in ids {
        let cluster = state.clusters.get(id).await?;
        rows.push(vec![
            id.to_string(),
            bold(&cluster.name),
            state
                .games
                .stage(id)
                .map(|stage| format!("{stage:?}"))
                .unwrap_or_default(),
            state
                .games
                .pid(id)
                .map(|pid| pid.to_string())
                .unwrap_or_default(),
        ]);
    }
    out.line(table(&["ID", "CLUSTER", "STAGE", "PID"], &rows));
    Ok(())
}

const BUNDLE_SYNC_BUDGET: Duration = Duration::from_secs(10);
const BUNDLE_APPLY_BUDGET: Duration = Duration::from_secs(60);
const UPDATE_CHECK_BUDGET: Duration = Duration::from_secs(10);

async fn prepare(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
    out: &Output,
) -> anyhow::Result<()> {
    let content = state.services.content();
    tokio::spawn({
        let content = content.clone();
        async move {
            if let Err(err) = oneclient_content::packages::refresh_bad_mods(&content).await {
                tracing::warn!(%err, "bad mods list refresh failed, keeping the last one");
            }
            if let Err(err) = oneclient_content::packages::refresh_bad_modpacks(&content).await {
                tracing::warn!(%err, "bad modpacks list refresh failed, keeping the last one");
            }
        }
    });

    loader_update(state, cluster).await;

    if cluster.uses_bundles() {
        choose_bundles(state, cluster).await;
        match tokio::time::timeout(BUNDLE_SYNC_BUDGET, state.bundles.sync(&content)).await {
            Ok(Err(err)) => {
                tracing::warn!("bundle catalog sync failed, using the cached one: {err:#}")
            }
            Err(_) => tracing::debug!("bundle catalog sync exceeded its launch budget"),
            Ok(Ok(_)) => {}
        }
        bundle_updates(state, cluster).await?;
    }

    package_updates(state, cluster, UPDATE_CHECK_BUDGET, out).await;

    if cluster.uses_bundles() {
        optional_mods(state, cluster).await;
    }
    Ok(())
}

async fn loader_update(state: &Arc<LauncherState>, cluster: &Cluster) -> bool {
    let Some((current, latest)) =
        oneclient_core::clusters::loader_update_offer(state, cluster.id).await
    else {
        return false;
    };
    let prompt = oneclient_core::clusters::loader_update_prompt(&current, &latest);
    let Ok(Some(chosen)) = state.services.events.ask(prompt).await else {
        return true;
    };
    if let Err(err) =
        oneclient_core::clusters::answer_loader_update(state, cluster.id, latest, chosen.value)
            .await
    {
        tracing::warn!(cluster_id = cluster.id, error = %err, "could not apply the loader update choice");
    }
    true
}

/// A failed update leaves the cluster half updated, e.g. a mod that needs a
/// newer library next to the old library, so one retry then refuse to launch
/// rather than hand the loader a set that cannot load
async fn bundle_updates(state: &Arc<LauncherState>, cluster: &Cluster) -> anyhow::Result<()> {
    let content = state.services.content();
    let mut failures = Vec::new();
    for attempt in 1..=2 {
        failures = match oneclient_core::apply_bundle_updates(
            cluster.id,
            state.bundles.as_ref(),
            &content,
            Some(Instant::now() + BUNDLE_APPLY_BUDGET),
        )
        .await
        {
            Ok(result) if !result.stopped_early => result.failures().cloned().collect(),
            Ok(result) if attempt == 1 => {
                tracing::warn!(
                    failed = result.downloads_failed.len(),
                    "bundle updates could not be downloaded in full; launching on the current content"
                );
                Vec::new()
            }
            Ok(result) => result.downloads_failed,
            Err(err) if attempt == 1 => {
                tracing::warn!("bundle update failed, launching anyway: {err:#}");
                return Ok(());
            }
            Err(err) => vec![format!("{err:#}")],
        };
        if failures.is_empty() {
            return Ok(());
        }
        tracing::warn!(
            attempt,
            failed = failures.len(),
            "bundle updates failed before launch"
        );
    }

    bail!(
        "could not update {} of {}'s mods, so the game was not started. Try launching again.\n{}",
        failures.len(),
        cluster.name,
        failures.join("\n")
    )
}

pub(super) async fn choose_bundles(state: &Arc<LauncherState>, cluster: &Cluster) {
    let content = state.services.content();
    let pending = oneclient_content::bundles::pending_bundle_choices(
        cluster.id,
        state.bundles.as_ref(),
        &content,
    )
    .await
    .unwrap_or_default();

    for archive in pending {
        let name = match archive.manifest.category.trim() {
            "" => archive.manifest.name.clone(),
            category => category.to_string(),
        };
        let prompt = Prompt::new(
            format!("Add {name}?"),
            format!("{} can include the {name} bundle.", cluster.name),
        )
        .option(Choice::primary("bundle.take", "Add it"), true)
        .option(Choice::new("bundle.decline", "No thanks"), false)
        .dismiss("Ask me later");
        let Ok(Some(chosen)) = state.services.events.ask(prompt).await else {
            continue;
        };
        if let Err(err) = oneclient_content::bundles::set_bundle_choices(
            cluster.id,
            &[(archive.manifest.name.clone(), chosen.value)],
            &content,
        )
        .await
        {
            tracing::warn!(cluster_id = cluster.id, error = %err, "could not record the bundle choice");
        }
    }
}

async fn package_updates(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
    budget: Duration,
    out: &Output,
) -> bool {
    let content = state.services.content();
    let check = match tokio::time::timeout(
        budget,
        oneclient_core::refresh_browser_package_updates(cluster.id, &content),
    )
    .await
    {
        Ok(Ok(check)) => check,
        Ok(Err(err)) => {
            tracing::warn!("package update check failed, launching anyway: {err:#}");
            return false;
        }
        Err(_) => return false,
    };
    let pending = check.pending();
    if pending.is_empty() {
        return false;
    }

    let global = state.settings.read().global_game_settings.clone();
    let mode = state
        .clusters
        .resolve_settings(&global, cluster)
        .await
        .ok()
        .and_then(|profile| profile.browser_update_mode)
        .unwrap_or_default();
    let apply = match mode {
        PackageUpdateMode::Skip => false,
        PackageUpdateMode::Automatic => true,
        PackageUpdateMode::Prompt => {
            let listed = pending
                .iter()
                .map(|u| {
                    format!(
                        "{} {} -> {}",
                        u.display_name, u.installed_version_name, u.latest_version_name
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            let prompt = Prompt::new(format!("{} package updates", pending.len()), listed)
                .option(Choice::primary("updates.apply", "Update them"), ())
                .dismiss("Not now");
            matches!(state.services.events.ask(prompt).await, Ok(Some(_)))
        }
    };
    if !apply {
        return true;
    }

    let (applied, session_id) =
        oneclient_core::install::apply_package_updates(state, &pending, "Updating packages").await;
    super::end_progress(state, session_id);
    if applied > 0 {
        out.line(format!(
            "Updated {applied} package{}",
            if applied == 1 { "" } else { "s" }
        ));
    }
    true
}

pub(super) async fn check_updates(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
    out: &Output,
) -> anyhow::Result<()> {
    if let Some(busy) = busy::launch_block(state, cluster.id).await {
        bail!("{}: {busy}", cluster.name);
    }
    if busy::exporting(cluster.id) {
        bail!("{}: {}", cluster.name, busy::Busy::Exporting);
    }
    let _claim = busy::UpdateCheckClaim::take(cluster.id)
        .with_context(|| format!("{} is already being checked for updates", cluster.name))?;

    out.line(format!("Checking {} for updates", bold(&cluster.name)));
    let loader = loader_update(state, cluster).await;
    if cluster.uses_bundles() {
        choose_bundles(state, cluster).await;
        state.bundles.sync(&state.services.content()).await?;
        bundle_updates(state, cluster).await?;
    }
    let packages = package_updates(state, cluster, Duration::MAX, out).await;
    clusters_changed(state);
    if !loader && !packages {
        out.line("No loader or package updates found");
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum OptionalAnswer {
    Enable,
    Skip,
}

async fn optional_mods(state: &Arc<LauncherState>, cluster: &Cluster) {
    let Ok(pending) = oneclient_core::pending_optional_mods(
        cluster.id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    else {
        return;
    };
    if pending.is_empty() {
        return;
    }

    let mods: Vec<oneclient_core::install::OptionalMod> = pending
        .iter()
        .map(|p| (cluster.id, (p.bundle_name.clone(), p.package_id.clone())))
        .collect();
    let listed = pending
        .iter()
        .map(|p| p.file.display_name())
        .collect::<Vec<_>>()
        .join("\n");
    let prompt = Prompt::new(format!("{} optional mods", pending.len()), listed)
        .option(
            Choice::primary("optional.enable", "Add them all"),
            OptionalAnswer::Enable,
        )
        .option(
            Choice::new("optional.skip", "Don't offer them again"),
            OptionalAnswer::Skip,
        )
        .dismiss("Decide later");

    if let Ok(Some(chosen)) = state.services.events.ask(prompt).await {
        match chosen.value {
            OptionalAnswer::Enable => {
                oneclient_core::install::enable_optional_mods(state, &mods).await;
            }
            OptionalAnswer::Skip => oneclient_core::install::skip_optional_mods(state, &mods).await,
        }
    }
}
