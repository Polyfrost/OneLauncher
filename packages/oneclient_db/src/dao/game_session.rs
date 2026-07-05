use chrono::Utc;
use sqlx::SqlitePool;

use crate::models::{
	GameSessionRow, GameSessionServerRow, NewGameSession, ServerJoinCount, SessionSpan,
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
	let ended_at = Utc::now().to_rfc3339();
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

pub async fn insert_server_join(
	pool: &SqlitePool,
	session_started_at: &str,
	address: &str,
	port: Option<i64>,
) -> Result<String, sqlx::Error> {
	let joined_at = Utc::now().to_rfc3339();
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
	Ok(joined_at)
}

pub async fn finish_server(pool: &SqlitePool, joined_at: &str) -> Result<(), sqlx::Error> {
	let disconnected_at = Utc::now().to_rfc3339();
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
