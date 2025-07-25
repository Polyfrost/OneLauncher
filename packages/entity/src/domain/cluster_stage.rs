use std::fmt::Display;

use sea_orm::{DeriveActiveEnum, EnumIter};
use serde::{Deserialize, Serialize};

#[onelauncher_macro::specta]
#[derive(
	Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default, EnumIter, DeriveActiveEnum,
)]
#[sea_orm(rs_type = "u8", db_type = "Integer")]
#[serde(rename_all = "lowercase")]
pub enum ClusterStage {
	#[default]
	NotReady = 0,
	Downloading = 1,
	Repairing = 2,
	Ready = 3,
}

impl ClusterStage {
	#[must_use]
	pub const fn is_downloading(&self) -> bool {
		matches!(self, Self::Downloading | Self::Repairing)
	}
}

impl Display for ClusterStage {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{}",
			match self {
				Self::NotReady => "Not Ready",
				Self::Downloading => "Downloading",
				Self::Repairing => "Repairing",
				Self::Ready => "Ready",
			}
		)
	}
}
