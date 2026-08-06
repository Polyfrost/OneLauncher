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

/// One entry of a release's declared dependency list.
///
/// Either id may be empty: the providers pin a project, a version, or both.
#[derive(Debug, Clone, FromRow)]
pub struct ReleaseDependencyRow {
	pub dependency_project_id: String,
	pub dependency_version_id: String,
	pub kind: String,
}

/// A dependency declared by an artifact the cluster has installed.
///
/// The edge points the way the provider wrote it — `hash` needs
/// `dependency_project_id` — so a caller after the reverse graph inverts it.
#[derive(Debug, Clone, FromRow)]
pub struct ClusterDependencyEdgeRow {
	/// The installed artifact that declares the dependency.
	pub hash: String,
	pub provider: i64,
	pub dependency_project_id: String,
	pub dependency_version_id: String,
	pub kind: String,
}

/// A release the cluster has installed whose dependency list was never
/// recorded.
#[derive(Debug, Clone, FromRow)]
pub struct UnsyncedReleaseRow {
	pub provider: i64,
	pub project_id: String,
	pub version_id: String,
}
