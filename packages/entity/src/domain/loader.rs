use std::{fmt::Display, str::FromStr};

use sea_orm::{DeriveActiveEnum, EnumIter};
use serde::{Deserialize, Serialize};

#[onelauncher_macro::specta]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[serde(rename_all = "lowercase")]
#[sea_orm(rs_type = "u8", db_type = "Integer")]
pub enum GameLoader {
	#[default]
	Vanilla = 0,
	Unknown = 1,
	Forge = 2,
	NeoForge = 3,
	Quilt = 4,
	Fabric = 5,
	LegacyFabric = 6,
}

impl GameLoader {

	#[must_use]
	pub const fn is_mod_loader(&self) -> bool {
		matches!(self, Self::Forge | Self::NeoForge | Self::Quilt | Self::Fabric | Self::LegacyFabric)
	}

}

impl From<String> for GameLoader {
	fn from(s: String) -> Self {
		match s.to_lowercase().replace(['_', '.', ' '], "").as_str() {
			"vanilla" => Self::Vanilla,
			"forge" => Self::Forge,
			"neoforge" => Self::NeoForge,
			"quilt" => Self::Quilt,
			"fabric" => Self::Fabric,
			"legacyfabric" => Self::LegacyFabric,
			_ => Self::Unknown,
		}
	}
}

impl FromStr for GameLoader {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(Self::from(s.to_string()))
	}
}

impl Display for GameLoader {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(
			match self {
				Self::Vanilla => "Vanilla",
				Self::Forge => "Forge",
				Self::NeoForge => "NeoForge",
				Self::Quilt => "Quilt",
				Self::Fabric => "Fabric",
				Self::LegacyFabric => "LegacyFabric",
				Self::Unknown => "Unknown",
			}
		)
	}
}
