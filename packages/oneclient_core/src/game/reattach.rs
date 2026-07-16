//! Picking up game sessions that outlived the launcher.
//!
//! Games are spawned detached, so closing the launcher no longer takes the game
//! down with it. That leaves two loose ends to tidy on the next start, both of
//! them sessions whose row still has no `ended_at`:
//!
//! * the game is **still running** — re-adopt it, so the UI shows it as playing
//!   and its exit is recorded properly when it finally happens;
//! * the game **already exited** unobserved — its log is the only witness, so
//!   replay it to recover when it ended and which servers it visited.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use oneclient_db::dao::game_session as session_dao;
use oneclient_db::models::UnfinishedSession;

use crate::clusters::{Cluster, ClusterManager};
use crate::discord::Presence;
use crate::game::launch::{Exit, SessionEnd, finalize_session};
use crate::game::log_replay::{self, ServerSpan};
use crate::game::process::{is_process_alive, kill_process};
use crate::game::session::SessionRecorder;
use crate::game::tail::spawn_log_tail;
use crate::notification::LaunchStage;
use crate::state::LauncherState;

/// How often to check whether a re-adopted game is still alive. It is only a
/// fallback for a process we don't own, so a slow poll is plenty.
const LIVENESS_POLL: Duration = Duration::from_secs(3);

/// Settle every session left open by a previous launcher run. Safe to call once
/// at startup, before anything can launch a game.
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

	// Re-adopt the live games first. Reconciling an exited session tears down
	// its shared game directory, which a still-running cluster may be sharing —
	// registering the live ones up front is what makes that check possible.
	for (cluster, session, started_at, pid) in live {
		readopt(state, cluster, session, started_at, pid).await;
	}

	for (cluster, session, started_at) in dead {
		reconcile(state, cluster, session, started_at).await;
	}
}

/// Resolve a session to its cluster and decide whether its process is alive.
/// `None` means the session was closed out here and needs no further work.
async fn classify(
	state: &Arc<LauncherState>,
	session: UnfinishedSession,
) -> Option<(Cluster, UnfinishedSession, DateTime<Utc>, Option<u32>)> {
	let cluster_id = session.cluster_id;

	// A session this launcher already owns needs no recovery.
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

	// The cluster is gone, so there is no log to consult and no playtime worth
	// attributing. Just close the row so it stops being reconsidered forever.
	let Ok(cluster) = ClusterManager::get(state, cluster_id).await else {
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

/// Close a session we can say nothing about. Ending it at its start time books
/// no playtime, which beats inventing some.
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

/// The game is still playing. Wire it back up as though we had launched it.
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
	state.services.notifier.game_stage(cluster_id, LaunchStage::Running);
	state.discord.set_presence(Presence::Playing {
		cluster: cluster.name.clone(),
		mc_version: cluster.mc_version.clone(),
	});

	let Ok(log_path) = crate::logs::cluster_output_log(&cluster) else {
		return;
	};
	let tail = spawn_log_tail(
		cluster_id,
		log_path,
		state.services.notifier.clone(),
		Some(recorder.clone()),
	);

	let (kill_tx, kill_rx) = tokio::sync::oneshot::channel::<()>();
	state.games.register_kill(cluster_id, kill_tx);

	// The post hook belongs to whichever settings apply now; resolve it up front
	// so the watcher task doesn't have to reach back into the state.
	let post_hook = ClusterManager::resolve_settings(state, &cluster)
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
				// We watched this one die, so the time is real even though the
				// exit code went with the launcher that spawned it.
				ended_at: Utc::now(),
				outcome: Exit::Inferred,
				// Only tear down the cluster's running state if it is still
				// ours. With parallel clusters enabled the user can start a
				// fresh game on this cluster while the re-adopted one plays on,
				// and that newer game now owns the slot.
				owns_slot: state.games.pid(cluster_id) == Some(pid),
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

/// The game exited while nothing was watching. Rebuild what we can from the log.
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

	// Prefer the log's own testimony over the file's mtime: a clean shutdown
	// says exactly when it happened, and the last timestamped line is the last
	// proof of life. mtime only helps when nothing in the log carries a time.
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

	// Unwinding the shared directory is only safe if nobody is playing in it.
	// A cluster re-adopted moments ago may share this exact directory, and
	// clearing its content mid-session would pull the game apart underneath it.
	let shared_dir_busy = state.games.dir_in_use_by(&cwd, cluster_id).is_some();
	if shared_dir_busy {
		tracing::debug!(cluster_id, "shared game dir still in use; skipping exit cleanup");
	}

	finalize_session(
		state,
		&cluster,
		&cwd,
		cluster.uses_dedicated_dir() || shared_dir_busy,
		// The post hook is deliberately skipped. It is an *on exit* hook, and
		// the exit already happened — possibly days ago. Firing it now, during
		// startup, would surprise anyone whose hook does something real.
		None,
		Some(recorder),
		SessionEnd {
			started_at,
			ended_at,
			outcome: Exit::Inferred,
			// This session is over, but the cluster may have a newer one playing
			// right now — only claim the slot if nothing else holds it.
			owns_slot: !state.games.is_active(cluster_id),
		},
	)
	.await;
}

/// Rewrite a session's server spans from the log replay, which — unlike the
/// live rows — covers the whole session including the part we missed. Returns
/// the `joined_at` of a span still left open, for the caller to close.
async fn apply_spans(
	state: &Arc<LauncherState>,
	session_id: &str,
	spans: &[ServerSpan],
) -> Option<String> {
	let db = &state.services.db;
	let existing = session_dao::list_session_servers(db, session_id)
		.await
		.unwrap_or_default();

	// A log that yields fewer spans than we already recorded has been rotated or
	// truncated. Live rows are then the better record; keep them and just report
	// whichever is still open so it gets closed at the session's end.
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

/// Read whichever log covers the session. `cluster-output.log` is preferred: the
/// launcher truncates it at every launch, so it belongs to exactly one session,
/// whereas `latest.log` is the game's own and may already have been rotated.
async fn read_session_log(cluster: &Cluster) -> Option<SessionLog> {
	let mut candidates: Vec<PathBuf> = Vec::new();
	if let Ok(path) = crate::logs::cluster_output_log(cluster) {
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
