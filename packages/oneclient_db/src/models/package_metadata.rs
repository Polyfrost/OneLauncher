use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct PackageMetadataRow {
	pub provider: i64,
	pub project_id: String,
	pub name: String,
	pub summary: String,
	pub author: String,
	pub icon_url: Option<String>,
}
