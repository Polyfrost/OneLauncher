use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use interfrost::api::minecraft::ArgumentType;
use tokio::process::Command;

use crate::ClusterStage;
use oneclient_auth::MinecraftAccount;
use crate::clusters::Cluster;
use oneclient_discord::Presence;
use crate::game::session::SessionRecorder;
use crate::game::tail::spawn_log_tail;
use crate::game::GameError;
use oneclient_mc::{
    self as arguments, download_minecraft, download_version_info, get_loader_version,
    game_files_missing, resolve_minecraft_version,
};
use oneclient_events::{GroupedProgressSession, LaunchStage};
use crate::settings::GameSettingsProfile;
use crate::state::LauncherState;
use crate::LauncherResult;
use oneclient_common::paths;

pub fn is_running(state: &LauncherState, cluster_id: i64) -> bool {
    state.games.is_running(cluster_id)
}

#[derive(Debug, Clone)]
pub struct LaunchedGame {
    pub cluster_id: i64,
    pub pid: Option<u32>,
}

#[tracing::instrument(skip(state, account))]
pub async fn launch_cluster(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    account: &MinecraftAccount,
    search_for_java: bool,
) -> LauncherResult<LaunchedGame> {
    tracing::info!(cluster_id, search_for_java, "launching cluster");

    let parallel = state.settings.read().allow_parallel_running_clusters;
    if !parallel && state.games.is_running(cluster_id) {
        tracing::warn!(cluster_id, "cluster already running; refusing launch");
        return Err(GameError::AlreadyRunning(cluster_id).into());
    }

    let events = state.services.events.clone();
    let stage = |s: LaunchStage| {
        state.games.set_stage(cluster_id, s);
        events.game_stage(cluster_id, s);
    };

    stage(LaunchStage::Checking);

    let existing = state.clusters.get(cluster_id).await?;

    let game_dir = existing.game_dir()?;
    let dedicated = existing.uses_dedicated_dir();

    // Non-dedicated clusters share one directory so a second game there would
    // materialize over the running one's
    // `dir_in_use_by` waves a cluster past
    // its own session which parallel launches make reachable
    if !dedicated && let Some(other) = state.games.dir_in_use(&game_dir) {
        tracing::warn!(cluster_id, other, "shared game dir busy; refusing launch");
        let name = running_cluster_name(state, other).await;
        return Err(GameError::SharedDirectoryBusy(name).into());
    }

    if let Some(other) = state.games.dir_in_use_by(&game_dir, cluster_id) {
        return Err(GameError::DirectoryInUse(other).into());
    }

    let progress = GroupedProgressSession::start(
        &state.services.events,
        format!("Launching {}", existing.name),
    );

    let cluster = if existing.stage == ClusterStage::Ready {
        existing
    } else {
        stage(LaunchStage::Downloading);
        match crate::clusters::prepare_cluster_locked(
            state,
            cluster_id,
            false,
            search_for_java,
            false,
            Some(&progress),
        )
        .await
        {
            Ok(cluster) => cluster,
            Err(err) => {
                progress.finish();
                stage(LaunchStage::Exited);
                return Err(err);
            }
        }
    };
    if let Err(err) = oneclient_content::bundles::install_cluster_bundles(
        cluster_id,
        state.bundles.as_ref(),
        Some(&progress),
        &state.services.content(),
    )
    .await
    {
        tracing::warn!(cluster_id, error = %err, "failed to install bundle content");
    }

    let global = state.settings.read().global_game_settings.clone();
    let profile = state.clusters.resolve_settings(&global, &cluster).await?;

    let mc_version = oneclient_common::version::normalize_mc_version_input(&cluster.mc_version);

    let (version, updated, loader_version, version_info) = {
        let mut metadata = state.metadata.lock().await;

        let (version, _index, updated) =
            resolve_minecraft_version(&mut metadata, &state.services.mc(), &mc_version)
                .await
                .map_err(|_| GameError::InvalidVersion(cluster.mc_version.clone()))?;

        let loader_version = get_loader_version(
            &mut metadata,
            &state.services.mc(),
            &mc_version,
            cluster.mc_loader,
            cluster.mc_loader_version.as_deref(),
        )
        .await?;

        let version_info = download_version_info(
            &state.services.mc(),
            Some(&progress),
            &version,
            loader_version.as_ref(),
            false,
        )
        .await;

        (version, updated, loader_version, version_info)
    };

    let version_info = match version_info {
        Ok(info) => info,
        Err(err) => {
            progress.finish();
            stage(LaunchStage::Exited);
            return Err(err.into());
        }
    };

    let version_name = loader_version.as_ref().map_or_else(
        || version.id.clone(),
        |lv| format!("{}-{}", version.id, lv.id),
    );

    tracing::info!(
        cluster_id,
        mc_version = %version.id,
        loader = %cluster.mc_loader,
        loader_version = loader_version.as_ref().map(|lv| lv.id.as_str()).unwrap_or("none"),
        version_name = %version_name,
        libraries = version_info.libraries.len(),
        main_class = %version_info.main_class,
        "resolved launch metadata"
    );

    let java = if let Some(runtime) =
        state.java.runtime_for_profile(profile.java_path.as_deref()).await?
    {
        runtime
    } else {
        let major = version_info
            .java_version
            .as_ref()
            .map(|v| v.major_version)
            .ok_or(GameError::MissingJavaVersion)?;

        state.java.prepare(major, search_for_java, false, None).await?
    };

    match game_files_missing(&version_info, &java.os_arch, updated) {
        Ok(true) => {
            tracing::info!(cluster_id, "missing game files; repairing");
            let _ = state.clusters.set_stage(cluster_id, ClusterStage::Repairing).await;
            stage(LaunchStage::Downloading);
            if let Err(err) = download_minecraft(
                &state.services.mc(),
                &progress,
                &version_info,
                &java.os_arch,
                updated,
                false,
            )
            .await
            {
                progress.finish();
                stage(LaunchStage::Exited);
                return Err(err.into());
            }
            let _ = state.clusters.set_stage(cluster_id, ClusterStage::Ready).await;
        }
        Ok(false) => {}
        Err(err) => tracing::warn!(cluster_id, error = %err, "repair check failed"),
    }

    progress.finish();

    stage(LaunchStage::Launching);

    let cwd = game_dir;
    polyio::create_dir_all(&cwd).await.ok();

    if let Err(err) = crate::game::write_allowed_symlinks(&cwd).await {
        tracing::warn!(cluster_id, error = %err, "failed to write allowed_symlinks.txt");
    }

    // The one moment nothing holds the content open so this is where a package
    // removed or disabled mid-session actually leaves the folder
    if let Err(err) = crate::game::materialize_content(&state.services, &cluster, &cwd).await {
        tracing::warn!(cluster_id, error = %err, "failed to materialize cluster content");
    }

    if !dedicated {
        // Redirects the shared dir's `logs`/`crash-reports` into this cluster's
        // folder so output is attributable unlinked on exit
        crate::game::link_cluster_logs(&cluster, &cwd).await;
    }

    let client_jar = paths::versions_dir()?
        .join(&version_name)
        .join(format!("{version_name}.jar"));
    let natives = paths::natives_dir()?.join(&version_name);
    let libraries = paths::libraries_dir()?;
    let assets = paths::assets_dir()?;

    let arg_map = version_info.arguments.clone().unwrap_or_default();

    let classpaths = arguments::classpaths(
        &libraries,
        &version_info.libraries,
        &client_jar,
        &java.os_arch,
        updated,
    )?;

    let jvm_args = arguments::java_arguments(
        updated,
        arg_map.get(&ArgumentType::Jvm).map(Vec::as_slice),
        &natives,
        &libraries,
        &classpaths,
        &version_name,
        profile.mem_max.unwrap_or(2048),
        profile.launch_args.clone().unwrap_or_default(),
        &java.os_arch,
        java.major,
    )?;

    let mut mc_args = arguments::minecraft_arguments(
        updated,
        arg_map.get(&ArgumentType::Game).map(Vec::as_slice),
        version_info.minecraft_arguments.as_deref(),
        &account.access_token,
        &account.username,
        account.id,
        &version.id,
        &version_info.asset_index.id,
        &cwd,
        &assets,
        version.type_,
        profile.resolution.unwrap_or_default(),
        &java.os_arch,
    )?;
    arguments::append_profile_game_arguments(&mut mc_args, profile.force_fullscreen, None);

    run_hook(profile.hook_pre.as_deref(), &cwd).await;

    tracing::info!(
        cluster_id,
        java = %java.absolute_path,
        jvm_args = jvm_args.len(),
        mc_args = mc_args.len(),
        cwd = %cwd.display(),
        "spawning minecraft process"
    );
    tracing::debug!(cluster_id, ?jvm_args, main_class = %version_info.main_class, "jvm arguments");

    let mut command = base_command(&profile, &java.absolute_path);
    apply_env(&mut command, &profile);
    command
        .args(jvm_args)
        .arg(&version_info.main_class)
        .args(mc_args)
        .current_dir(&cwd);

    let log_path = oneclient_cluster::logs::cluster_output_log(&cluster)?;
    if let Some(parent) = log_path.parent() {
        polyio::create_dir_all(parent).await.ok();
    }

    // A file not a pipe a pipe would die with the launcher and take the game's
    // next stdout write down with it
    // The cloned handle shares the file offset
    // so stdout and stderr interleave instead of overwriting
    let handles = match tokio::fs::File::create(&log_path).await {
        Ok(out) => match out.try_clone().await {
            Ok(err) => Ok((out.into_std().await, err.into_std().await)),
            Err(err) => Err(err),
        },
        Err(err) => Err(err),
    };

    match handles {
        Ok((out, err)) => {
            command.stdout(Stdio::from(out)).stderr(Stdio::from(err));
        }
        Err(err) => {
            tracing::warn!(cluster_id, error = %err, "failed to open game log; discarding output");
            command.stdout(Stdio::null()).stderr(Stdio::null());
        }
    }
    command.stdin(Stdio::null());
    detach(&mut command);

    let mut child = command
        .spawn()
        .map_err(|err| GameError::Spawn(err.to_string()))?;
    let pid = child.id();

    stage(LaunchStage::Running);
    state.games.set_pid(cluster_id, pid);
    state.games.set_dir(cluster_id, cwd.clone());
    state.discord.set_presence(Presence::Playing {
        cluster: cluster.name.clone(),
        mc_version: cluster.mc_version.clone(),
    });

    let recorder =
        SessionRecorder::start(state, cluster_id, profile.mem_max.unwrap_or(2048), &java).await;

    // Pinned to the session row so that if the launcher exits first the next
    // start can tell whether the game is still playing
    if let (Some(recorder), Some(pid)) = (recorder.as_ref(), pid) {
        recorder
            .record_process(pid, crate::game::process_start_time(pid))
            .await;
    }

    let started_at = recorder
        .as_ref()
        .and_then(SessionRecorder::started_at)
        .unwrap_or_else(Utc::now);
    let crash_watch = crate::game::diagnosis::CrashWatch::new();
    let tail = spawn_log_tail(
        cluster_id,
        log_path,
        events.clone(),
        recorder.clone(),
        crash_watch.clone(),
    );

    let (kill_tx, kill_rx) = tokio::sync::oneshot::channel::<()>();
    state.games.register_kill(cluster_id, kill_tx);

    let state = Arc::clone(state);
    let post_hook = profile.hook_post.clone();
    tokio::spawn(async move {
        let cluster = cluster;
        let status = tokio::select! {
            status = child.wait() => status,
            _ = kill_rx => {
                let _ = child.start_kill();
                child.wait().await
            }
        };

        tail.stop().await;

        let outcome = match status {
            Ok(status) => Exit::Observed {
                code: status.code().map(i64::from),
                success: status.success(),
                display: status.to_string(),
            },
            Err(err) => Exit::Failed(err.to_string()),
        };

        finalize_session(
            &state,
            &cluster,
            &cwd,
            dedicated,
            post_hook.as_deref(),
            recorder,
            SessionEnd {
                started_at,
                ended_at: Utc::now(),
                outcome,
                owns_slot: true,
                diagnosis: crash_watch.take(),
            },
        )
        .await;
    });

    Ok(LaunchedGame { cluster_id, pid })
}

async fn running_cluster_name(state: &Arc<LauncherState>, cluster_id: i64) -> String {
    state
        .clusters
        .get(cluster_id)
        .await
        .map_or_else(|_| format!("Cluster {cluster_id}"), |cluster| cluster.name)
}

/// Cuts the game loose from the launcher's process group/console so signals
/// aimed at the launcher (Ctrl-C console close) don't reach the game
fn detach(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command
            .as_std_mut()
            .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
}

pub(crate) enum Exit {
    Observed {
        code: Option<i64>,
        success: bool,
        display: String,
    },
    Failed(String),
    /// Game exited while the launcher was closed time recovered from the log
    /// exit code unrecoverable
    Inferred,
}

pub(crate) struct SessionEnd {
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub outcome: Exit,
    /// False for a stale session recovered from the database whose cluster has a
    /// live game book its playtime but clearing the slot would report the live
    /// game as exited
    pub owns_slot: bool,
    /// `None` for a clean exit an unrecognised crash or a session recovered
    /// after the fact with no log watched
    pub diagnosis: Option<crate::game::diagnosis::CrashDiagnosis>,
}

/// Shared by the live exit path and by recovery of sessions that outlived the
/// launcher
pub(crate) async fn finalize_session(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
    cwd: &Path,
    dedicated: bool,
    post_hook: Option<&str>,
    recorder: Option<SessionRecorder>,
    end: SessionEnd,
) {
    let cluster_id = cluster.id;
    let played = (end.ended_at - end.started_at).to_std().unwrap_or_default();

    if end.owns_slot {
        state.games.remove(cluster_id);
        state
            .services
            .events
            .game_stage(cluster_id, LaunchStage::Exited);

        if state.games.running_ids().is_empty() {
            state.discord.set_presence(Presence::Idle);
        }
    }

    if played > Duration::from_secs(1) {
        let _ = state.clusters.add_playtime(cluster_id, played).await;
    }

    if let Some(recorder) = recorder {
        let code = match &end.outcome {
            Exit::Observed { code, .. } => *code,
            Exit::Failed(_) | Exit::Inferred => None,
        };
        recorder.finish_at(&end.ended_at.to_rfc3339(), code).await;
    }

    run_hook(post_hook, cwd).await;

    if dedicated {
        // The folder stays materialized so it remains a real Minecraft directory
        // for external tools adopting drop-ins now keeps the UI right on close
        crate::game::import_manual_content(&state.services, cluster, cwd).await;
    } else {
        if let Err(err) = crate::game::dematerialize_content(&state.services, cluster, cwd).await {
            tracing::warn!(cluster_id, error = %err, "failed to clear shared content on exit");
        }
        crate::game::unlink_cluster_logs(cwd).await;
    }

    let name = &cluster.name;
    let crashed = !matches!(end.outcome, Exit::Observed { success: true, .. });

    match end.outcome {
        Exit::Observed { success: true, .. } => state
            .services
            .events
            .notify("Game closed").body(format!("{name} exited")).send(),
        Exit::Observed { display, .. } => state
            .services
            .events
            .notify("Game crashed").body(format!("{name} exited with {display}")).error().send(),
        Exit::Failed(err) => state
            .services
            .events
            .notify("Game error").body(format!("{name}: {err}")).error().send(),
        // Nothing was watching so there is no crash to report
        Exit::Inferred => {}
    }

    // Only when the game actually died a `ZipException` it recovered from is not
    // worth interrupting a finished session over
    if crashed && let Some(diagnosis) = end.diagnosis {
        offer_repair(state, cluster_id, &diagnosis).await;
    }
}

/// Asks rather than acting verification can re-download much of the game too
/// much to start unprompted
/// The offer returns on the next crash
#[tracing::instrument(skip(state, diagnosis), level = "debug")]
pub async fn offer_repair(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    diagnosis: &crate::game::diagnosis::CrashDiagnosis,
) {
    enum Answer {
        Verify,
    }

    let answer = state
        .services
        .events
        .ask(
            oneclient_events::Prompt::new("A damaged file crashed the game", diagnosis.body())
                .option(
                    oneclient_events::Choice::primary("verify", "Verify and repair"),
                    Answer::Verify,
                )
                .dismiss("Not now"),
        )
        .await;

    match answer {
        Ok(Some(_)) => {}
        // Dismissed or nobody there to ask neither is consent to re-download
        Ok(None) => {
            tracing::info!(cluster_id, "user declined the post-crash repair");
            return;
        }
        Err(err) => {
            tracing::warn!(cluster_id, "could not offer a post-crash repair: {err}");
            return;
        }
    }

    match crate::verify::verify_cluster_files(state, cluster_id).await {
        Ok(report) => {
            state
                .services
                .events
                .notify("Repair complete")
                .body(report.summary())
                .send();
        }
        Err(err) => {
            tracing::error!(cluster_id, "post-crash repair failed: {err:#}");
            state
                .services
                .events
                .notify("Repair failed")
                .body(err.to_string())
                .error()
                .send();
        }
    }
}

fn base_command(profile: &GameSettingsProfile, java_path: &str) -> Command {
    if let Some(wrapper) = profile
        .hook_wrapper
        .as_deref()
        .map(str::trim)
        .filter(|hook| !hook.is_empty())
    {
        let mut split = wrapper.split_whitespace();
        let mut command = Command::new(split.next().unwrap_or("sh"));
        command.args(split);
        command.arg(java_path);
        command
    } else {
        Command::new(java_path)
    }
}

fn apply_env(command: &mut Command, profile: &GameSettingsProfile) {
    command.env_remove("_JAVA_OPTIONS");

    #[cfg(target_os = "linux")]
    apply_discrete_gpu(command, profile);

    if let Some(env) = &profile.launch_env {
        for pair in env.split_whitespace() {
            if let Some((key, value)) = pair.split_once('=') {
                command.env(key, value);
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn apply_discrete_gpu(command: &mut Command, profile: &GameSettingsProfile) {
    let requested = profile
        .os_extra
        .as_ref()
        .and_then(|extra| extra.use_discrete_gpu)
        .unwrap_or(false);

    if !requested {
        return;
    }

    let gpus = crate::game::gpu::detect();
    let env = crate::game::gpu::offload_env(&gpus);

    if env.is_empty() {
        tracing::info!(
            gpus = gpus.len(),
            "discrete GPU was requested but nothing here is a valid offload target; \
             leaving the renderer alone"
        );
        return;
    }

    for (key, value) in env {
        tracing::debug!(key, value, "offloading the game to the discrete GPU");
        command.env(key, value);
    }
}

#[tracing::instrument(skip(cwd), fields(hook), level = "debug")]
async fn run_hook(hook: Option<&str>, cwd: &Path) {
    let Some(hook) = hook.map(str::trim).filter(|h| !h.is_empty()) else {
        return;
    };

    #[cfg(windows)]
    let mut command = {
        let mut c = Command::new("cmd");
        c.args(["/C", hook]);
        c
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut c = Command::new("sh");
        c.args(["-c", hook]);
        c
    };

    command.current_dir(cwd);
    if let Err(err) = command.status().await {
        tracing::warn!("hook '{hook}' failed: {err}");
    }
}
