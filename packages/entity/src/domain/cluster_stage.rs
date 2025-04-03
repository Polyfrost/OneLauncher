use sea_orm::{DeriveActiveEnum, EnumIter};
use serde::{Deserialize, Serialize};

#[onelauncher_macro::specta]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "u8", db_type = "Integer")]
#[serde(rename_all = "lowercase")]
pub enum ClusterStage {
	#[default]
	NotReady = 0,
	Downloading = 1,
	Repairing = 2,
	Ready = 3,
}