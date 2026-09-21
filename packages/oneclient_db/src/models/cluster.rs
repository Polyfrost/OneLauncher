use serde::{Deserialize, Serialize};
use sqlx::FromRow;

pub type ClusterId = i64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(i64)]
pub enum ClusterKind {
    #[default]
    OneClient = 0,
    Vanilla = 1,
    Modded = 2,
}

impl ClusterKind {
    pub fn as_i64(self) -> i64 {
        self as i64
    }

    pub fn from_repr(value: i64) -> Option<Self> {
        match value {
            0 => Some(Self::OneClient),
            1 => Some(Self::Vanilla),
            2 => Some(Self::Modded),
            _ => None,
        }
    }
}

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
    pub kind: i64,
    pub user_created: i64,
    pub description: Option<String>,
    pub tags: String,
    pub cover_path: Option<String>,
}

impl ClusterRow {
    pub fn kind(&self) -> ClusterKind {
        ClusterKind::from_repr(self.kind).unwrap_or_default()
    }

    pub fn is_user_created(&self) -> bool {
        self.user_created != 0
    }
}

pub struct NewCluster<'a> {
    pub name: &'a str,
    pub folder_name: &'a str,
    pub mc_version: &'a str,
    pub mc_loader: i64,
    pub mc_loader_version: Option<&'a str>,
    pub setting_profile_name: Option<&'a str>,
    pub stage: i64,
    pub kind: i64,
    pub user_created: i64,
    pub description: Option<&'a str>,
    pub tags: &'a str,
    pub cover_path: Option<&'a str>,
}

#[derive(Debug, Clone, Default)]
pub struct ClusterPatch {
    pub name: Option<String>,
    pub setting_profile_name: Option<Option<String>>,
    pub mc_loader_version: Option<Option<String>>,
    pub linked_modpack_hash: Option<Option<String>>,
    pub description: Option<Option<String>>,
    pub tags: Option<String>,
    pub cover_path: Option<Option<String>>,
}
