use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use interfrost::api::minecraft::ArgumentType;
use tokio::process::Command;

use crate::ClusterStage;
use crate::auth::MinecraftAccount;
use crate::clusters::{Cluster, ClusterManager};
use crate::discord::Presence;
use crate::game::session::SessionRecorder;
use crate::game::tail::spawn_log_tail;
use crate::game::{
    GameError, arguments, download_minecraft, download_version_info, get_loader_version,
    libraries_missing, resolve_minecraft_version,
};
use crate::java::JavaManager;
use crate::notification::{GroupedProgressSession, LaunchStage};
use crate::settings::GameSettingsProfile;
use crate::state::LauncherState;
use crate::{LauncherResult, paths};

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

    let notifier = state.services.notifier.clone();
    let stage = |s: LaunchStage| {
        state.games.set_stage(cluster_id, s);
        notifier.game_stage(cluster_id, s);
    };

    stage(LaunchStage::Checking);

    let existing = ClusterManager::get(state, cluster_id).await?;

    let game_dir = existing.game_dir()?;
    if let Some(other) = state.games.dir_in_use_by(&game_dir, cluster_id) {
        return Err(GameError::DirectoryInUse(other).into());
    }

    let dedicated = existing.uses_dedicated_dir();

    let progress = GroupedProgressSession::start(
        &state.services.notifier,
        format!("Launching {}", existing.name),
    );

    let cluster = if existing.stage == ClusterStage::Ready {
        existing
    } else {
        stage(LaunchStage::Downloading);
        match ClusterManager::prepare(
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
    if let Err(err) = crate::bundles::install_cluster_bundles(
        cluster_id,
        state.bundles.as_ref(),
        Some(&progress),
        &state.services,
    )
    .await
    {
        tracing::warn!(cluster_id, error = %err, "failed to install bundle content");
    }

    let profile = ClusterManager::resolve_settings(state, &cluster).await?;

    let mc_version = crate::version::normalize_mc_version_input(&cluster.mc_version);

    let (version, updated, loader_version, version_info) = {
        let mut metadata = state.metadata.lock().await;

        let (version, _index, updated) =
            resolve_minecraft_version(&mut metadata, &state.services, &mc_version)
                .await
                .map_err(|_| GameError::InvalidVersion(cluster.mc_version.clone()))?;

        let loader_version = get_loader_version(
            &mut metadata,
            &state.services,
            &mc_version,
            cluster.mc_loader,
            cluster.mc_loader_version.as_deref(),
        )
        .await?;

        let version_info = download_version_info(
            &state.services,
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
            return Err(err);
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
        JavaManager::java_for_profile(&state.services.db, profile.java_path.as_deref()).await?
    {
        runtime
    } else {
        let major = version_info
            .java_version
            .as_ref()
            .map(|v| v.major_version)
            .ok_or(GameError::MissingJavaVersion)?;

        JavaManager::prepare_java(state, major, search_for_java).await?
    };

    match libraries_missing(&version_info, &java.os_arch, updated) {
        Ok(true) => {
            tracing::info!(cluster_id, "missing game files; repairing");
            let _ = ClusterManager::set_stage(state, cluster_id, ClusterStage::Repairing).await;
            stage(LaunchStage::Downloading);
            if let Err(err) = download_minecraft(
                &state.services,
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
                return Err(err);
            }
            let _ = ClusterManager::set_stage(state, cluster_id, ClusterStage::Ready).await;
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

    if !dedicated {
        if let Err(err) = crate::game::sync_shared_content(&state.services, &cluster, &cwd).await {
            tracing::warn!(cluster_id, error = %err, "failed to sync shared content");
        }
        // Redirect the shared dir's `logs`/`crash-reports` into this cluster's
        // own folder so its output is attributable while it plays; unlinked on
        // exit. Keeps the shared `.minecraft` (and the launcher's own logs dir)
        // free of another cluster's leftovers.
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

    let log_path = crate::logs::cluster_output_log(&cluster)?;
    if let Some(parent) = log_path.parent() {
        polyio::create_dir_all(parent).await.ok();
    }

    // The game's output goes straight to the log file rather than through pipes
    // held by the launcher. A pipe would tie the game's lifetime to ours: once
    // the launcher exits its read end closes, and the game's next write to a
    // broken stdout takes it down with us. Writing to a file keeps the two
    // independent, and the launcher tails that file for the live console.
    // A cloned handle shares the file offset, so stdout and stderr interleave
    // into one stream instead of overwriting each other.
    // `Stdio` needs owned std handles, so the tokio files are unwrapped only
    // once the async work of opening and cloning them is done.
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

    // Pin the process to the session row so that if the launcher exits before
    // the game does, the next start can tell whether it is still playing.
    if let (Some(recorder), Some(pid)) = (recorder.as_ref(), pid) {
        recorder
            .record_process(pid, crate::game::process_start_time(pid))
            .await;
    }

    let started_at = recorder
        .as_ref()
        .and_then(SessionRecorder::started_at)
        .unwrap_or_else(Utc::now);
    let tail = spawn_log_tail(cluster_id, log_path, notifier.clone(), recorder.clone());

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
            },
        )
        .await;
    });

    Ok(LaunchedGame { cluster_id, pid })
}

/// Cut the game loose from the launcher's process group / console, so signals
/// aimed at the launcher (a terminal Ctrl-C, a console window closing) don't
/// reach the game as collateral.
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

/// How a session ended, as far as anyone could tell.
pub(crate) enum Exit {
    /// The launcher was there and saw the process exit.
    Observed {
        code: Option<i64>,
        success: bool,
        display: String,
    },
    /// Waiting on the process itself failed.
    Failed(String),
    /// The game exited while the launcher was closed; the time was recovered
    /// from its log, and the exit code is gone for good.
    Inferred,
}

pub(crate) struct SessionEnd {
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub outcome: Exit,
    /// Whether this session is the one currently holding the cluster's running
    /// slot. A stale session recovered from the database may share its cluster
    /// with a game that is playing right now — booking the old session's
    /// playtime is right, but clearing that slot would report the live game as
    /// exited.
    pub owns_slot: bool,
}

/// Everything that has to happen once a game is gone: clear its running state,
/// bank the playtime, close the session row, run the post hook and unwind the
/// shared-directory plumbing. Shared by the live exit path and by recovery of
/// sessions that outlived the launcher.
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
            .notifier
            .game_stage(cluster_id, LaunchStage::Exited);

        if state.games.running_ids().is_empty() {
            state.discord.set_presence(Presence::Idle);
        }
    }

    if played > Duration::from_secs(1) {
        let _ = ClusterManager::add_playtime(state, cluster_id, played).await;
    }

    if let Some(recorder) = recorder {
        let code = match &end.outcome {
            Exit::Observed { code, .. } => *code,
            Exit::Failed(_) | Exit::Inferred => None,
        };
        recorder.finish_at(&end.ended_at.to_rfc3339(), code).await;
    }

    run_hook(post_hook, cwd).await;

    if !dedicated {
        crate::game::import_manual_content(&state.services, cluster, cwd).await;
        if let Err(err) = crate::game::clear_shared_content(cwd).await {
            tracing::warn!(cluster_id, error = %err, "failed to clear shared content on exit");
        }
        crate::game::unlink_cluster_logs(cwd).await;
    }

    let name = &cluster.name;
    match end.outcome {
        Exit::Observed { success: true, .. } => state
            .services
            .notifier
            .send_info("Game closed", &format!("{name} exited")),
        Exit::Observed { display, .. } => state
            .services
            .notifier
            .send_error("Game crashed", &format!("{name} exited with {display}")),
        Exit::Failed(err) => state
            .services
            .notifier
            .send_error("Game error", &format!("{name}: {err}")),
        // Nothing was watching, so there is no crash to report and no news the
        // user wants a popup about — the session is simply booked and closed.
        Exit::Inferred => {}
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
    if let Some(env) = &profile.launch_env {
        for pair in env.split_whitespace() {
            if let Some((key, value)) = pair.split_once('=') {
                command.env(key, value);
            }
        }
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
