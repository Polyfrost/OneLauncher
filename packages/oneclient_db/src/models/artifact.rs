use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct ArtifactRow {
	pub hash: String,
	pub content_type: i64,
	pub path: String,
	pub file_name: String,
	pub size_bytes: Option<i64>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ProviderReleaseRow {
	pub provider: i64,
	pub project_id: String,
	pub version_id: String,
	pub hash: String,
	pub display_name: String,
	pub display_version: String,
	pub published_at: Option<String>,
	pub mc_versions: String,
	pub mc_loaders: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct ClusterArtifactRow {
	pub cluster_id: i64,
	pub hash: String,
	pub cluster_file_name: String,
	pub enabled: i64,
}
