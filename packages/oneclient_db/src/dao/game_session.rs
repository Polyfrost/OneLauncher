use chrono::Utc;
use sqlx::SqlitePool;

use crate::models::{
	GameSessionRow, GameSessionServerRow, NewGameSession, ServerJoinCount, SessionSpan,
	UnfinishedSession,
};

pub async fn insert_session(
	pool: &SqlitePool,
	new: &NewGameSession<'_>,
) -> Result<GameSessionRow, sqlx::Error> {
	sqlx::query_as!(
		GameSessionRow,
		r#"
		INSERT INTO game_sessions (
			cluster_id, started_at, ram_allocated_mb, mods_enabled, java_vendor, java_version
		)
		VALUES (?, ?, ?, ?, ?, ?)
		RETURNING
			cluster_id, started_at, ended_at, exit_code,
			ram_allocated_mb, mods_enabled, java_vendor, java_version
		"#,
		new.cluster_id,
		new.started_at,
		new.ram_allocated_mb,
		new.mods_enabled,
		new.java_vendor,
		new.java_version
	)
	.fetch_one(pool)
	.await
}

pub async fn finish_session(
	pool: &SqlitePool,
	started_at: &str,
	exit_code: Option<i64>,
) -> Result<(), sqlx::Error> {
	finish_session_at(pool, started_at, &Utc::now().to_rfc3339(), exit_code).await
}

/// Close a session at an explicit time. Used when the exit time is inferred
/// from the game's log rather than observed live.
pub async fn finish_session_at(
	pool: &SqlitePool,
	started_at: &str,
	ended_at: &str,
	exit_code: Option<i64>,
) -> Result<(), sqlx::Error> {
	sqlx::query!(
		r#"
		UPDATE game_sessions
		SET ended_at = ?, exit_code = ?
		WHERE started_at = ? AND ended_at IS NULL
		"#,
		ended_at,
		exit_code,
		started_at
	)
	.execute(pool)
	.await?;
	Ok(())
}

pub async fn set_session_process(
	pool: &SqlitePool,
	started_at: &str,
	pid: Option<i64>,
	pid_started_at: Option<i64>,
) -> Result<(), sqlx::Error> {
	sqlx::query!(
		r#"
		UPDATE game_sessions
		SET pid = ?, pid_started_at = ?
		WHERE started_at = ?
		"#,
		pid,
		pid_started_at,
		started_at
	)
	.execute(pool)
	.await?;
	Ok(())
}

pub async fn unfinished_sessions(pool: &SqlitePool) -> Result<Vec<UnfinishedSession>, sqlx::Error> {
	sqlx::query_as!(
		UnfinishedSession,
		r#"
		SELECT cluster_id, started_at, pid, pid_started_at
		FROM game_sessions
		WHERE ended_at IS NULL
		ORDER BY started_at ASC
		"#
	)
	.fetch_all(pool)
	.await
}

pub async fn insert_server_join(
	pool: &SqlitePool,
	session_started_at: &str,
	address: &str,
	port: Option<i64>,
) -> Result<String, sqlx::Error> {
	insert_server_join_at(pool, session_started_at, address, port, &Utc::now().to_rfc3339()).await
}

/// Record a join at an explicit time, for spans replayed out of a game log.
pub async fn insert_server_join_at(
	pool: &SqlitePool,
	session_started_at: &str,
	address: &str,
	port: Option<i64>,
	joined_at: &str,
) -> Result<String, sqlx::Error> {
	sqlx::query!(
		r#"
		INSERT INTO game_session_servers (session_started_at, address, port, joined_at)
		VALUES (?, ?, ?, ?)
		"#,
		session_started_at,
		address,
		port,
		joined_at
	)
	.execute(pool)
	.await?;
	Ok(joined_at.to_string())
}

pub async fn finish_server(pool: &SqlitePool, joined_at: &str) -> Result<(), sqlx::Error> {
	finish_server_at(pool, joined_at, &Utc::now().to_rfc3339()).await
}

/// Close a server span at an explicit time, for spans replayed out of a log.
pub async fn finish_server_at(
	pool: &SqlitePool,
	joined_at: &str,
	disconnected_at: &str,
) -> Result<(), sqlx::Error> {
	sqlx::query!(
		r#"
		UPDATE game_session_servers
		SET disconnected_at = ?
		WHERE joined_at = ? AND disconnected_at IS NULL
		"#,
		disconnected_at,
		joined_at
	)
	.execute(pool)
	.await?;
	Ok(())
}

pub async fn list_sessions_for_cluster(
	pool: &SqlitePool,
	cluster_id: i64,
) -> Result<Vec<GameSessionRow>, sqlx::Error> {
	sqlx::query_as!(
		GameSessionRow,
		r#"
		SELECT
			cluster_id, started_at, ended_at, exit_code,
			ram_allocated_mb, mods_enabled, java_vendor, java_version
		FROM game_sessions
		WHERE cluster_id = ?
		ORDER BY started_at DESC
		"#,
		cluster_id
	)
	.fetch_all(pool)
	.await
}

pub async fn list_session_servers(
	pool: &SqlitePool,
	session_started_at: &str,
) -> Result<Vec<GameSessionServerRow>, sqlx::Error> {
	sqlx::query_as!(
		GameSessionServerRow,
		r#"
		SELECT session_started_at, address, port, joined_at, disconnected_at
		FROM game_session_servers
		WHERE session_started_at = ?
		ORDER BY joined_at ASC
		"#,
		session_started_at
	)
	.fetch_all(pool)
	.await
}

/// Drop every server span of a session, so a log replay can rewrite them from scratch.
pub async fn delete_session_servers(
	pool: &SqlitePool,
	session_started_at: &str,
) -> Result<(), sqlx::Error> {
	sqlx::query!(
		r#"
		DELETE FROM game_session_servers
		WHERE session_started_at = ?
		"#,
		session_started_at
	)
	.execute(pool)
	.await?;
	Ok(())
}

pub async fn server_join_counts(
	pool: &SqlitePool,
	cluster_id: i64,
) -> Result<Vec<ServerJoinCount>, sqlx::Error> {
	sqlx::query_as!(
		ServerJoinCount,
		r#"
		SELECT s.address AS "address!", COUNT(*) AS "joins!: i64"
		FROM game_session_servers s
		JOIN game_sessions g ON g.started_at = s.session_started_at
		WHERE g.cluster_id = ?
		GROUP BY s.address
		ORDER BY COUNT(*) DESC
		"#,
		cluster_id
	)
	.fetch_all(pool)
	.await
}

pub async fn all_session_spans(pool: &SqlitePool) -> Result<Vec<SessionSpan>, sqlx::Error> {
	sqlx::query_as!(
		SessionSpan,
		r#"
		SELECT started_at, ended_at
		FROM game_sessions
		WHERE ended_at IS NOT NULL
		ORDER BY started_at ASC
		"#
	)
	.fetch_all(pool)
	.await
}

pub async fn session_spans_for_cluster(
	pool: &SqlitePool,
	cluster_id: i64,
) -> Result<Vec<SessionSpan>, sqlx::Error> {
	sqlx::query_as!(
		SessionSpan,
		r#"
		SELECT started_at, ended_at
		FROM game_sessions
		WHERE cluster_id = ? AND ended_at IS NOT NULL
		ORDER BY started_at ASC
		"#,
		cluster_id
	)
	.fetch_all(pool)
	.await
}

pub async fn all_session_servers(
	pool: &SqlitePool,
) -> Result<Vec<GameSessionServerRow>, sqlx::Error> {
	sqlx::query_as!(
		GameSessionServerRow,
		r#"
		SELECT session_started_at, address, port, joined_at, disconnected_at
		FROM game_session_servers
		ORDER BY joined_at ASC
		"#
	)
	.fetch_all(pool)
	.await
}

pub async fn session_servers_for_cluster(
	pool: &SqlitePool,
	cluster_id: i64,
) -> Result<Vec<GameSessionServerRow>, sqlx::Error> {
	sqlx::query_as!(
		GameSessionServerRow,
		r#"
		SELECT s.session_started_at, s.address, s.port, s.joined_at, s.disconnected_at
		FROM game_session_servers s
		JOIN game_sessions g ON g.started_at = s.session_started_at
		WHERE g.cluster_id = ?
		ORDER BY s.joined_at ASC
		"#,
		cluster_id
	)
	.fetch_all(pool)
	.await
}
