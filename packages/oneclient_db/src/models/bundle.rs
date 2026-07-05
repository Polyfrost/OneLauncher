use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct BundleRow {
    pub remote_path: String,
    pub mc_version: String,
    pub mc_loader: i64,
    pub file_name: String,
    pub name: Option<String>,
    pub version_id: Option<String>,
    pub category: Option<String>,
    pub loader_version: Option<String>,
    pub disk_path: String,
    pub hidden: i64,
    pub etag: Option<String>,
    pub synced_at: Option<String>,
}

pub struct NewBundle<'a> {
    pub remote_path: &'a str,
    pub mc_version: &'a str,
    pub mc_loader: i64,
    pub file_name: &'a str,
    pub name: Option<&'a str>,
    pub version_id: Option<&'a str>,
    pub category: Option<&'a str>,
    pub loader_version: Option<&'a str>,
    pub disk_path: &'a str,
    pub hidden: bool,
    pub etag: Option<&'a str>,
    pub synced_at: Option<&'a str>,
}
