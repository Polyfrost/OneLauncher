use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct ReleaseMigrationWaitlistRow {
	pub target_cluster_id: i64,
	pub provider: i64,
	pub project_id: String,
	pub content_type: i64,
	pub source_hash: String,
	pub display_name: String,
	pub added_at: String,
	pub expires_at: String,
	pub enabled: i64,
}
