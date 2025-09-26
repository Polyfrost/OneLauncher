use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use crate::cluster_stage::ClusterStage;
use crate::icon::Icon;
use crate::loader::GameLoader;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "clusters")]
#[serde(rename = "ClusterModel")]
#[onelauncher_macro::specta]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i64,
	pub folder_name: String,
	pub stage: ClusterStage,
	pub created_at: DateTimeUtc,
	pub group_id: Option<i64>,
	pub name: String,
	pub mc_version: String,
	pub mc_loader: GameLoader,
	#[sea_orm(nullable)]
	pub mc_loader_version: Option<String>,
	pub last_played: Option<DateTimeUtc>,
	pub overall_played: Option<i64>,
	#[sea_orm(nullable)]
	pub icon_url: Option<Icon>,
	#[sea_orm(nullable)]
	pub setting_profile_name: Option<String>,
	#[sea_orm(nullable)]
	pub linked_modpack_hash: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(
		belongs_to = "super::cluster_groups::Entity",
		from = "Column::GroupId",
		to = "super::cluster_groups::Column::Id",
		on_update = "NoAction",
		on_delete = "NoAction"
	)]
	ClusterGroups,
	#[sea_orm(has_many = "super::cluster_packages::Entity")]
	ClusterPackages,
	#[sea_orm(
		belongs_to = "super::setting_profiles::Entity",
		from = "Column::SettingProfileName",
		to = "super::setting_profiles::Column::Name",
		on_update = "NoAction",
		on_delete = "NoAction"
	)]
	SettingProfiles,
}

impl Related<super::cluster_groups::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::ClusterGroups.def()
	}
}

impl Related<super::cluster_packages::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::ClusterPackages.def()
	}
}

impl Related<super::setting_profiles::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::SettingProfiles.def()
	}
}

impl Related<super::packages::Entity> for Entity {
	fn to() -> RelationDef {
		super::cluster_packages::Relation::Packages.def()
	}
	fn via() -> Option<RelationDef> {
		Some(super::cluster_packages::Relation::Clusters.def().rev())
	}
}

impl ActiveModelBehavior for ActiveModel {}
