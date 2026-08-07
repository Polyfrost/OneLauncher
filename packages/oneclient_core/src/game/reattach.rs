//! Games are spawned detached so sessions can outlive the launcher they are
//! settled on the next start (rows with no `ended_at`)

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use oneclient_db::dao::game_session as session_dao;
use oneclient_db::models::UnfinishedSession;

use crate::clusters::Cluster;
use oneclient_discord::Presence;
use crate::game::launch::{Exit, SessionEnd, finalize_session};
use crate::game::log_replay::{self, ServerSpan};
use crate::game::process::{is_process_alive, kill_process};
use crate::game::session::SessionRecorder;
use crate::game::tail::spawn_log_tail;
use oneclient_events::LaunchStage;
use crate::state::LauncherState;

/// Only a fallback for a process we don't own so a slow poll is plenty
const LIVENESS_POLL: Duration = Duration::from_secs(3);

/// Call once at startup before anything can launch a game
#[tracing::instrument(skip(state), level = "debug")]
pub async fn recover_sessions(state: &Arc<LauncherState>) {
	let sessions = match session_dao::unfinished_sessions(&state.services.db).await {
		Ok(sessions) => sessions,
		Err(err) => {
			tracing::warn!(error = %err, "failed to list unfinished game sessions");
			return;
		}
	};

	if sessions.is_empty() {
		return;
	}

	tracing::info!(count = sessions.len(), "recovering unfinished game sessions");

	let mut live = Vec::new();
	let mut dead = Vec::new();
	for session in sessions {
		match classify(state, session).await {
			Some((cluster, session, started_at, Some(pid))) => {
				live.push((cluster, session, started_at, pid));
			}
			Some((cluster, session, started_at, None)) => {
				dead.push((cluster, session, started_at));
			}
			None => {}
		}
	}

	// Live games first reconciling an exited session tears down a shared game
	// dir and that check reads the live registrations
	for (cluster, session, started_at, pid) in live {
		readopt(state, cluster, session, started_at, pid).await;
	}

	for (cluster, session, started_at) in dead {
		reconcile(state, cluster, session, started_at).await;
	}
}

/// `None` means the session was closed out here and needs no further work
async fn classify(
	state: &Arc<LauncherState>,
	session: UnfinishedSession,
) -> Option<(Cluster, UnfinishedSession, DateTime<Utc>, Option<u32>)> {
	let cluster_id = session.cluster_id;

	if state.games.is_active(cluster_id) {
		return None;
	}

	let started_at = match DateTime::parse_from_rfc3339(&session.started_at) {
		Ok(at) => at.with_timezone(&Utc),
		Err(err) => {
			tracing::warn!(cluster_id, session = %session.started_at, error = %err, "unparseable session start; closing it");
			close_untraceable(state, &session).await;
			return None;
		}
	};

	// No cluster means no log and no playtime worth attributing close the row
	// so it stops being reconsidered forever
	let Ok(cluster) = state.clusters.get(cluster_id).await else {
		tracing::warn!(cluster_id, "cluster missing for unfinished session; closing it");
		close_untraceable(state, &session).await;
		return None;
	};

	let alive = session
		.pid
		.and_then(|pid| u32::try_from(pid).ok())
		.filter(|pid| is_process_alive(*pid, session.pid_started_at.map(|t| t as u64)));

	Some((cluster, session, started_at, alive))
}

/// Ends at the start time which books no playtime rather than inventing some
async fn close_untraceable(state: &Arc<LauncherState>, session: &UnfinishedSession) {
	if let Err(err) = session_dao::finish_session_at(
		&state.services.db,
		&session.started_at,
		&session.started_at,
		None,
	)
	.await
	{
		tracing::warn!(error = %err, "failed to close untraceable session");
	}
}

#[tracing::instrument(skip(state, cluster, session), fields(cluster_id = cluster.id, pid), level = "debug")]
async fn readopt(
	state: &Arc<LauncherState>,
	cluster: Cluster,
	session: UnfinishedSession,
	started_at: DateTime<Utc>,
	pid: u32,
) {
	let cluster_id = cluster.id;
	let Ok(cwd) = cluster.game_dir() else {
		return;
	};

	tracing::info!(cluster_id, pid, "re-adopting running game");

	let open_server = open_server_of(state, &session.started_at).await;
	let recorder = SessionRecorder::resume(
		state.services.db.clone(),
		session.started_at.clone(),
		open_server,
	);

	state.games.set_stage(cluster_id, LaunchStage::Running);
	state.games.set_pid(cluster_id, Some(pid));
	state.games.set_dir(cluster_id, cwd.clone());
	state.services.events.game_stage(cluster_id, LaunchStage::Running);
	state.discord.set_presence(Presence::Playing {
		cluster: cluster.name.clone(),
		mc_version: cluster.mc_version.clone(),
	});

	let Ok(log_path) = oneclient_cluster::logs::cluster_output_log(&cluster) else {
		return;
	};
	// A re-adopted game still writes its log so crashes stay readable even
	// though the exit code went with the launcher that spawned it
	let crash_watch = crate::game::diagnosis::CrashWatch::new();
	let tail = spawn_log_tail(
		cluster_id,
		log_path,
		state.services.events.clone(),
		Some(recorder.clone()),
		crash_watch.clone(),
	);

	let (kill_tx, kill_rx) = tokio::sync::oneshot::channel::<()>();
	state.games.register_kill(cluster_id, kill_tx);

	// Resolved up front so the watcher task doesn't reach back into the state
	let global = state.settings.read().global_game_settings.clone();
	let post_hook = state.clusters.resolve_settings(&global, &cluster)
		.await
		.ok()
		.and_then(|profile| profile.hook_post);
	let dedicated = cluster.uses_dedicated_dir();
	let pid_started_at = session.pid_started_at.map(|t| t as u64);
	let state = Arc::clone(state);

	tokio::spawn(async move {
		tokio::select! {
			() = wait_for_exit(pid, pid_started_at) => {}
			_ = kill_rx => {
				kill_process(pid);
				wait_for_exit(pid, pid_started_at).await;
			}
		}

		tail.stop().await;

		finalize_session(
			&state,
			&cluster,
			&cwd,
			dedicated,
			post_hook.as_deref(),
			Some(recorder),
			SessionEnd {
				started_at,
				ended_at: Utc::now(),
				outcome: Exit::Inferred,
				// With parallel clusters a newer game may already own this
				// cluster's slot only tear down if it is still ours
				owns_slot: state.games.pid(cluster_id) == Some(pid),
				diagnosis: crash_watch.take(),
			},
		)
		.await;
	});
}

async fn wait_for_exit(pid: u32, pid_started_at: Option<u64>) {
	while is_process_alive(pid, pid_started_at) {
		tokio::time::sleep(LIVENESS_POLL).await;
	}
}

#[tracing::instrument(skip(state, cluster, session), fields(cluster_id = cluster.id), level = "debug")]
async fn reconcile(
	state: &Arc<LauncherState>,
	cluster: Cluster,
	session: UnfinishedSession,
	started_at: DateTime<Utc>,
) {
	let cluster_id = cluster.id;
	let log = read_session_log(&cluster).await;

	let replay = log
		.as_ref()
		.map(|log| log_replay::replay(&log.content, started_at))
		.unwrap_or_default();

	// Prefer the log's own timestamps over mtime mtime only helps when nothing
	// in the log carries a time
	let ended_at = replay
		.stopped_at
		.or(replay.last_activity)
		.or_else(|| log.as_ref().and_then(|log| log.modified))
		.unwrap_or(started_at)
		.clamp(started_at, Utc::now());

	tracing::info!(
		cluster_id,
		session = %session.started_at,
		%ended_at,
		clean_exit = replay.stopped_at.is_some(),
		servers = replay.servers.len(),
		"recovered session that outlived the launcher"
	);

	let open_server = apply_spans(state, &session.started_at, &replay.servers).await;
	let recorder = SessionRecorder::resume(
		state.services.db.clone(),
		session.started_at.clone(),
		open_server,
	);

	let Ok(cwd) = cluster.game_dir() else {
		recorder.finish_at(&ended_at.to_rfc3339(), None).await;
		return;
	};

	// A cluster re-adopted moments ago may share this directory clearing it
	// mid-session would pull that game apart underneath it
	let shared_dir_busy = state.games.dir_in_use_by(&cwd, cluster_id).is_some();
	if shared_dir_busy {
		tracing::debug!(cluster_id, "shared game dir still in use; skipping exit cleanup");
	}

	finalize_session(
		state,
		&cluster,
		&cwd,
		cluster.uses_dedicated_dir() || shared_dir_busy,
		// Post hook skipped on purpose the exit already happened possibly days
		// ago so firing it during startup would surprise the user
		None,
		Some(recorder),
		SessionEnd {
			started_at,
			ended_at,
			outcome: Exit::Inferred,
			// The cluster may have a newer session playing right now so only
			// claim the slot if nothing else holds it
			owns_slot: !state.games.is_active(cluster_id),
			// Nothing watched this log while it ran so no crash was recognised
			diagnosis: None,
		},
	)
	.await;
}

/// Returns the `joined_at` of a span still left open for the caller to close
async fn apply_spans(
	state: &Arc<LauncherState>,
	session_id: &str,
	spans: &[ServerSpan],
) -> Option<String> {
	let db = &state.services.db;
	let existing = session_dao::list_session_servers(db, session_id)
		.await
		.unwrap_or_default();

	// Fewer replayed spans than recorded means the log was rotated or truncated
	// so the live rows are the better record keep them
	if spans.len() < existing.len() {
		tracing::debug!(
			session = %session_id,
			replayed = spans.len(),
			existing = existing.len(),
			"log replay is thinner than recorded spans; keeping recorded ones"
		);
		return existing
			.into_iter()
			.find(|row| row.disconnected_at.is_none())
			.map(|row| row.joined_at);
	}

	if let Err(err) = session_dao::delete_session_servers(db, session_id).await {
		tracing::warn!(session = %session_id, error = %err, "failed to clear server spans for replay");
		return None;
	}

	let mut open = None;
	for span in spans {
		let joined_at = span.joined_at.to_rfc3339();
		if let Err(err) = session_dao::insert_server_join_at(
			db,
			session_id,
			&span.host,
			span.port.map(i64::from),
			&joined_at,
		)
		.await
		{
			tracing::warn!(session = %session_id, error = %err, "failed to replay server join");
			continue;
		}

		match span.disconnected_at {
			Some(at) => {
				if let Err(err) =
					session_dao::finish_server_at(db, &joined_at, &at.to_rfc3339()).await
				{
					tracing::warn!(session = %session_id, error = %err, "failed to replay server leave");
				}
			}
			None => open = Some(joined_at),
		}
	}

	open
}

async fn open_server_of(state: &Arc<LauncherState>, session_id: &str) -> Option<String> {
	session_dao::list_session_servers(&state.services.db, session_id)
		.await
		.unwrap_or_default()
		.into_iter()
		.find(|row| row.disconnected_at.is_none())
		.map(|row| row.joined_at)
}

struct SessionLog {
	content: String,
	modified: Option<DateTime<Utc>>,
}

/// `cluster-output.log` is preferred it is truncated at every launch so it
/// belongs to one session whereas `latest.log` may already have rotated
async fn read_session_log(cluster: &Cluster) -> Option<SessionLog> {
	let mut candidates: Vec<PathBuf> = Vec::new();
	if let Ok(path) = oneclient_cluster::logs::cluster_output_log(cluster) {
		candidates.push(path);
	}
	if let Ok(dir) = cluster.dir() {
		candidates.push(dir.join("logs").join("latest.log"));
	}

	for path in candidates {
		match read_log(&path).await {
			Some(log) if !log.content.trim().is_empty() => return Some(log),
			_ => continue,
		}
	}

	None
}

async fn read_log(path: &Path) -> Option<SessionLog> {
	let content = polyio::read_to_string(path).await.ok()?;
	let modified = polyio::stat(path)
		.await
		.ok()
		.and_then(|meta| meta.modified().ok())
		.map(DateTime::<Utc>::from);

	Some(SessionLog { content, modified })
}
