use sqlx::FromRow;

pub type ClusterId = i64;

#[derive(Debug, Clone, FromRow)]
pub struct ClusterRow {
	pub id: ClusterId,
	pub name: String,
	pub folder_name: String,
	pub setting_profile_name: Option<String>,
	pub mc_version: String,
	pub mc_loader: i64,
	pub stage: i64,
	pub mc_loader_version: Option<String>,
	pub created_at: Option<String>,
	pub last_played: Option<String>,
	pub overall_played: Option<i64>,
	pub linked_modpack_hash: Option<String>,
}

pub struct NewCluster<'a> {
	pub name: &'a str,
	pub folder_name: &'a str,
	pub mc_version: &'a str,
	pub mc_loader: i64,
	pub mc_loader_version: Option<&'a str>,
	pub setting_profile_name: Option<&'a str>,
	pub stage: i64,
}

#[derive(Debug, Clone, Default)]
pub struct ClusterPatch {
	pub name: Option<String>,
	pub setting_profile_name: Option<Option<String>>,
	pub mc_loader_version: Option<Option<String>>,
	pub linked_modpack_hash: Option<Option<String>>,
}
