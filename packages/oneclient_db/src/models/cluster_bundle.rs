use sqlx::FromRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverrideType {
    Removed,
    Disabled,
}

impl OverrideType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Removed => "removed",
            Self::Disabled => "disabled",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "removed" => Some(Self::Removed),
            "disabled" => Some(Self::Disabled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct ClusterBundleOverrideRow {
    pub id: i64,
    pub cluster_id: i64,
    pub bundle_name: String,
    pub package_id: String,
    pub override_type: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct BundleTrackedArtifactRow {
    pub cluster_id: i64,
    pub hash: String,
    pub cluster_file_name: String,
    pub enabled: i64,
    pub bundle_name: Option<String>,
    pub bundle_version_id: Option<String>,
    pub package_id: Option<String>,
    pub installed_at: Option<String>,
}
