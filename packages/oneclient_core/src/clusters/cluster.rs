use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use oneclient_db::models::ClusterId;
use oneclient_db::models::ClusterRow;
use serde::{Deserialize, Serialize};

use crate::packages::domain::GameLoader;
use crate::paths;
use crate::LauncherResult;

use super::error::ClusterError;
use super::stage::ClusterStage;

pub const DEDICATED_MARKER: &str = ".dedicated_directory";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cluster {
	pub id: ClusterId,
	pub name: String,
	pub folder_name: String,
	pub setting_profile_name: Option<String>,
	pub mc_version: String,
	pub mc_loader: GameLoader,
	pub mc_loader_version: Option<String>,
	pub stage: ClusterStage,
	pub created_at: Option<DateTime<Utc>>,
	pub last_played: Option<DateTime<Utc>>,
	#[serde(with = "serde_duration_secs")]
	pub overall_played: Duration,
	pub linked_modpack_hash: Option<String>,
}

impl PartialEq for Cluster {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

mod serde_duration_secs {
	use serde::{Deserialize, Deserializer, Serialize, Serializer};
	use std::time::Duration;

	pub fn serialize<S: Serializer>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
		duration.as_secs().serialize(serializer)
	}

	pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
		Ok(Duration::from_secs(u64::deserialize(deserializer)?))
	}
}

impl Cluster {
	pub fn try_from_row(row: ClusterRow) -> Result<Self, ClusterError> {
		let mc_loader = GameLoader::from_repr(row.mc_loader as u8)
			.ok_or(ClusterError::InvalidLoader(row.mc_loader))?;
		let stage = ClusterStage::from_repr(row.stage)
			.ok_or(ClusterError::InvalidStage(row.stage))?;

		Ok(Self {
			id: row.id,
			name: row.name,
			folder_name: row.folder_name,
			setting_profile_name: row.setting_profile_name,
			mc_version: row.mc_version,
			mc_loader,
			mc_loader_version: row.mc_loader_version,
			stage,
			created_at: parse_timestamp(row.created_at),
			last_played: parse_timestamp(row.last_played),
			overall_played: Duration::from_secs(row.overall_played.unwrap_or(0).max(0) as u64),
			linked_modpack_hash: row.linked_modpack_hash,
		})
	}

	pub fn dir(&self) -> LauncherResult<PathBuf> {
		Ok(paths::clusters_dir()?.join(&self.folder_name))
	}

	pub fn dedicated_marker(&self) -> LauncherResult<PathBuf> {
		Ok(self.dir()?.join(DEDICATED_MARKER))
	}

	pub fn uses_dedicated_dir(&self) -> bool {
		self.dedicated_marker().map(|p| p.exists()).unwrap_or(false)
	}

	pub fn game_dir(&self) -> LauncherResult<PathBuf> {
		if self.uses_dedicated_dir() {
			self.dir()
		} else {
			paths::shared_minecraft_dir()
		}
	}

	pub fn as_link_target(&self) -> ClusterLinkTarget<'_> {
		ClusterLinkTarget {
			id: self.id,
			folder_name: &self.folder_name,
			mc_version: &self.mc_version,
			mc_loader: self.mc_loader,
		}
	}
}

#[derive(Debug, Clone)]
pub struct ClusterLinkTarget<'a> {
	pub id: ClusterId,
	pub folder_name: &'a str,
	pub mc_version: &'a str,
	pub mc_loader: GameLoader,
}

fn parse_timestamp(value: Option<String>) -> Option<DateTime<Utc>> {
	value.and_then(|text| DateTime::parse_from_rfc3339(&text).ok().map(|dt| dt.with_timezone(&Utc)))
}
