use sqlx::FromRow;

/// A browser-installed package the last update check found a newer version for.
///
/// `hash` is the artifact currently linked into the cluster, which is also the
/// primary key the package manager renders by, so a row can never be attached
/// to a file the cluster no longer has.
#[derive(Debug, Clone, FromRow)]
pub struct BrowserPackageUpdateRow {
	pub cluster_id: i64,
	pub hash: String,
	pub provider: i64,
	pub project_id: String,
	pub installed_version_id: String,
	pub installed_version_name: String,
	pub latest_version_id: String,
	pub latest_version_name: String,
	pub display_name: String,
	pub checked_at: String,
	pub skipped: i64,
}
