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

	pub(crate) async fn observe(&self, line: &str) {
		let Some(join) = parse_server_join(line) else {
			return;
		};

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

	pub(crate) async fn finish(self, exit_code: Option<i64>) {
		if let Some(open) = self.open_server.lock().await.take()
			&& let Err(err) = session_dao::finish_server(&self.db, &open).await
		{
			tracing::warn!(error = %err, "failed to close open server on exit");
		}

		if let Err(err) =
			session_dao::finish_session(&self.db, &self.session_started_at, exit_code).await
		{
			tracing::warn!(session = %self.session_started_at, error = %err, "failed to finish game session");
		}
	}
}

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
