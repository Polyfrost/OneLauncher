use std::sync::Arc;

use oneclient_db::DbPool;
use oneclient_db::dao::game_session as session_dao;
use oneclient_db::models::NewGameSession;
use tokio::sync::Mutex;

use crate::java::JavaRuntime;
use crate::packages::{ContentType, PackageStore};
use crate::state::LauncherState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ServerJoin {
	pub host: String,
	pub port: Option<u16>,
}

/// The session row's identity — its `started_at`, which is the primary key.
pub(crate) type SessionId = String;

pub(crate) fn parse_server_join(line: &str) -> Option<ServerJoin> {
	const MARKER: &str = "Connecting to ";
	let idx = line.find(MARKER)?;
	let rest = line[idx + MARKER.len()..].trim();
	if rest.is_empty() {
		return None;
	}

	let (host, port) = match rest.split_once(", ") {
		Some((host, port)) => {
			let port = port.split_whitespace().next().and_then(|p| p.parse::<u16>().ok());
			(host.trim(), port)
		}
		None => (rest, None),
	};

	let host = host.trim();
	if host.is_empty() || host.chars().any(char::is_whitespace) {
		return None;
	}

	Some(ServerJoin {
		host: host.to_string(),
		port,
	})
}

#[derive(Clone)]
pub(crate) struct SessionRecorder {
	session_started_at: String,
	db: DbPool,
	open_server: Arc<Mutex<Option<String>>>,
}

impl SessionRecorder {
	#[tracing::instrument(skip(state, java), fields(cluster_id, ram_allocated_mb), level = "debug")]
	pub(crate) async fn start(
		state: &Arc<LauncherState>,
		cluster_id: i64,
		ram_allocated_mb: u32,
		java: &JavaRuntime,
	) -> Option<Self> {
		let mods_enabled = count_enabled_mods(state, cluster_id).await;

		let java_vendor = java.vendor.to_string();
		let started_at = chrono::Utc::now().to_rfc3339();
		let new = NewGameSession {
			cluster_id,
			started_at: &started_at,
			ram_allocated_mb: i64::from(ram_allocated_mb),
			mods_enabled: mods_enabled as i64,
			java_vendor: Some(java_vendor.as_str()),
			java_version: Some(java.version.as_str()),
		};

		let session = match session_dao::insert_session(&state.services.db, &new).await {
			Ok(row) => row,
			Err(err) => {
				tracing::warn!(cluster_id, error = %err, "failed to record game session");
				return None;
			}
		};

		Some(Self {
			session_started_at: session.started_at,
			db: state.services.db.clone(),
			open_server: Arc::new(Mutex::new(None)),
		})
	}

	/// Re-attach to a session row left open by a previous launcher run, so a
	/// game that outlived the launcher keeps recording into the same session.
	pub(crate) fn resume(db: DbPool, session_started_at: SessionId, open_server: Option<String>) -> Self {
		Self {
			session_started_at,
			db,
			open_server: Arc::new(Mutex::new(open_server)),
		}
	}

	/// When the session row says it began. Playtime is measured against this so
	/// it agrees with the session span analytics reads back out of the row.
	pub(crate) fn started_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
		chrono::DateTime::parse_from_rfc3339(&self.session_started_at)
			.ok()
			.map(|at| at.with_timezone(&chrono::Utc))
	}

	/// Remember which OS process backs this session. Without it, a launcher
	/// restart cannot tell a live game from one that exited unobserved.
	#[tracing::instrument(skip(self), level = "debug")]
	pub(crate) async fn record_process(&self, pid: u32, pid_started_at: Option<u64>) {
		if let Err(err) = session_dao::set_session_process(
			&self.db,
			&self.session_started_at,
			Some(i64::from(pid)),
			pid_started_at.map(|t| t as i64),
		)
		.await
		{
			tracing::warn!(pid, error = %err, "failed to record session process");
		}
	}

	pub(crate) async fn observe(&self, line: &str) {
		if let Some(join) = parse_server_join(line) {
			self.open(join).await;
		} else if super::log_replay::is_leave_marker(line) {
			self.close_open().await;
		}
	}

	async fn open(&self, join: ServerJoin) {
		let mut open = self.open_server.lock().await;
		if let Some(prev) = open.take()
			&& let Err(err) = session_dao::finish_server(&self.db, &prev).await
		{
			tracing::warn!(error = %err, "failed to close previous server row");
		}

		let port = join.port.map(i64::from);
		match session_dao::insert_server_join(
			&self.db,
			&self.session_started_at,
			&join.host,
			port,
		)
		.await
		{
			Ok(joined_at) => *open = Some(joined_at),
			Err(err) => tracing::warn!(error = %err, "failed to record server join"),
		}
	}

	async fn close_open(&self) {
		if let Some(prev) = self.open_server.lock().await.take()
			&& let Err(err) = session_dao::finish_server(&self.db, &prev).await
		{
			tracing::warn!(error = %err, "failed to close server row");
		}
	}

	/// Close the session. The time is explicit because an exit is not always
	/// observed as it happens — one recovered from a log ended in the past.
	#[tracing::instrument(skip(self), fields(exit_code), level = "debug")]
	pub(crate) async fn finish_at(self, ended_at: &str, exit_code: Option<i64>) {
		if let Some(open) = self.open_server.lock().await.take()
			&& let Err(err) = session_dao::finish_server_at(&self.db, &open, ended_at).await
		{
			tracing::warn!(error = %err, "failed to close open server on exit");
		}

		if let Err(err) =
			session_dao::finish_session_at(&self.db, &self.session_started_at, ended_at, exit_code)
				.await
		{
			tracing::warn!(session = %self.session_started_at, error = %err, "failed to finish game session");
		}
	}
}

#[tracing::instrument(skip(state), fields(cluster_id), level = "debug")]
async fn count_enabled_mods(state: &Arc<LauncherState>, cluster_id: i64) -> usize {
	match PackageStore::list_linked_artifacts(cluster_id, &state.services).await {
		Ok(linked) => linked
			.into_iter()
			.filter(|a| a.enabled && a.content_type == ContentType::Mod)
			.count(),
		Err(err) => {
			tracing::warn!(cluster_id, error = %err, "failed to count mods for session");
			0
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parses_host_and_port() {
		let join =
			parse_server_join("[12:00:00] [Render thread/INFO]: Connecting to mc.hypixel.net, 25565")
				.expect("should parse");
		assert_eq!(join.host, "mc.hypixel.net");
		assert_eq!(join.port, Some(25565));
	}

	#[test]
	fn parses_ip_and_port() {
		let join = parse_server_join("[Render thread/INFO]: Connecting to 192.168.1.10, 25577")
			.expect("should parse");
		assert_eq!(join.host, "192.168.1.10");
		assert_eq!(join.port, Some(25577));
	}

	#[test]
	fn parses_host_without_port() {
		let join = parse_server_join("Connecting to play.example.com").expect("should parse");
		assert_eq!(join.host, "play.example.com");
		assert_eq!(join.port, None);
	}

	#[test]
	fn ignores_unrelated_lines() {
		assert!(parse_server_join("[Render thread/INFO]: Loading world").is_none());
		assert!(parse_server_join("Connecting to ").is_none());
	}

	#[test]
	fn rejects_addresses_with_spaces() {
		assert!(
			parse_server_join(
				"[INFO]: Connecting to voice chat server: 'vc.example.com:24454'"
			)
			.is_none()
		);
	}
}
