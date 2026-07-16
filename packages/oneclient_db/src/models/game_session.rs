use sqlx::FromRow;

pub type GameSessionId = String;

#[derive(Debug, Clone, FromRow)]
pub struct GameSessionRow {
	pub cluster_id: i64,
	pub started_at: String,
	pub ended_at: Option<String>,
	pub exit_code: Option<i64>,
	pub ram_allocated_mb: i64,
	pub mods_enabled: i64,
	pub java_vendor: Option<String>,
	pub java_version: Option<String>,
}

pub struct NewGameSession<'a> {
	pub cluster_id: i64,
	pub started_at: &'a str,
	pub ram_allocated_mb: i64,
	pub mods_enabled: i64,
	pub java_vendor: Option<&'a str>,
	pub java_version: Option<&'a str>,
}

#[derive(Debug, Clone, FromRow)]
pub struct UnfinishedSession {
	pub cluster_id: i64,
	pub started_at: String,
	pub pid: Option<i64>,
	pub pid_started_at: Option<i64>,
}

#[derive(Debug, Clone, FromRow)]
pub struct GameSessionServerRow {
	pub session_started_at: String,
	pub address: String,
	pub port: Option<i64>,
	pub joined_at: String,
	pub disconnected_at: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ServerJoinCount {
	pub address: String,
	pub joins: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct SessionSpan {
	pub started_at: String,
	pub ended_at: Option<String>,
}
