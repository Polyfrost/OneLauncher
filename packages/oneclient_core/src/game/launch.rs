
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use interfrost::api::minecraft::ArgumentType;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex as AsyncMutex;

use crate::auth::MinecraftAccount;
use crate::ClusterStage;
use crate::clusters::ClusterManager;
use crate::discord::Presence;
use crate::game::{
    GameError, arguments, download_minecraft, download_version_info, get_loader_version,
    libraries_missing, resolve_minecraft_version,
};
use crate::game::session::SessionRecorder;
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

        let version_info =
            download_version_info(&state.services, Some(&progress), &version, loader_version.as_ref(), false)
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

    let version_name = loader_version
        .as_ref()
        .map_or_else(|| version.id.clone(), |lv| format!("{}-{}", version.id, lv.id));

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
            let _ =
                ClusterManager::set_stage(state, cluster_id, ClusterStage::Repairing).await;
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
    tokio::fs::create_dir_all(&cwd).await.ok();

    if let Err(err) = crate::game::write_allowed_symlinks(&cwd).await {
        tracing::warn!(cluster_id, error = %err, "failed to write allowed_symlinks.txt");
    }

    if !dedicated {
        if let Err(err) =
            crate::game::sync_shared_content(&state.services, &cluster, &cwd).await
        {
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
    arguments::append_profile_game_arguments(
        &mut mc_args,
        profile.force_fullscreen,
        None,
    );

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

    command.stdout(Stdio::piped()).stderr(Stdio::piped());

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

    let log_path = crate::logs::cluster_output_log(&cluster)?;
    if let Some(parent) = log_path.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    let log_file = tokio::fs::File::create(&log_path)
        .await
        .ok()
        .map(|file| Arc::new(tokio::sync::Mutex::new(file)));

    let recorder = SessionRecorder::start(
        state,
        cluster_id,
        profile.mem_max.unwrap_or(2048),
        &java,
    )
    .await;

    if let Some(stdout) = child.stdout.take() {
        spawn_log_reader(cluster_id, stdout, notifier.clone(), log_file.clone(), recorder.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_log_reader(cluster_id, stderr, notifier.clone(), log_file.clone(), recorder.clone());
    }

    let (kill_tx, kill_rx) = tokio::sync::oneshot::channel::<()>();
    state.games.register_kill(cluster_id, kill_tx);

    let state = Arc::clone(state);
    let post_hook = profile.hook_post.clone();
    let cluster_name = cluster.name.clone();
    tokio::spawn(async move {
        let cluster = cluster;
        let started = Instant::now();
        let status = tokio::select! {
            status = child.wait() => status,
            _ = kill_rx => {
                let _ = child.start_kill();
                child.wait().await
            }
        };
        let played = started.elapsed();

        state.games.remove(cluster_id);
        state.services.notifier.game_stage(cluster_id, LaunchStage::Exited);

        if state.games.running_ids().is_empty() {
            state.discord.set_presence(Presence::Idle);
        }

        if played > Duration::from_secs(1) {
            let _ = ClusterManager::add_playtime(&state, cluster_id, played).await;
        }

        if let Some(recorder) = recorder {
            let exit_code = status.as_ref().ok().and_then(|s| s.code()).map(i64::from);
            recorder.finish(exit_code).await;
        }

        run_hook(post_hook.as_deref(), &cwd).await;

        if !dedicated {
            crate::game::import_manual_content(&state.services, &cluster, &cwd).await;
            if let Err(err) = crate::game::clear_shared_content(&cwd).await {
                tracing::warn!(cluster_id, error = %err, "failed to clear shared content on exit");
            }
            crate::game::unlink_cluster_logs(&cwd).await;
        }

        match status {
            Ok(status) if status.success() => state
                .services
                .notifier
                .send_info("Game closed", &format!("{cluster_name} exited")),
            Ok(status) => state.services.notifier.send_error(
                "Game crashed",
                &format!("{cluster_name} exited with {status}"),
            ),
            Err(err) => state
                .services
                .notifier
                .send_error("Game error", &format!("{cluster_name}: {err}")),
        }
    });

    Ok(LaunchedGame {
        cluster_id,
        pid,
    })
}

fn spawn_log_reader<R>(
    cluster_id: i64,
    reader: R,
    notifier: crate::notification::NotificationService,
    file: Option<Arc<AsyncMutex<tokio::fs::File>>>,
    recorder: Option<SessionRecorder>,
) where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(file) = &file {
                let mut handle = file.lock().await;
                let _ = handle.write_all(line.as_bytes()).await;
                let _ = handle.write_all(b"\n").await;
            }
            if let Some(recorder) = &recorder {
                recorder.observe(&line).await;
            }
            notifier.game_log(cluster_id, line);
        }
    });
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
