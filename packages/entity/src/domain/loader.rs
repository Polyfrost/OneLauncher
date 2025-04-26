use std::{fmt::Display, str::FromStr};

use sea_orm::{DeriveActiveEnum, EnumIter};
use serde::{Deserialize, Serialize};

#[onelauncher_macro::specta]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[serde(rename_all = "lowercase")]
#[sea_orm(rs_type = "u8", db_type = "Integer")]
pub enum GameLoader {
	#[default]
	#[serde(alias = "minecraft")]
	Vanilla = 0,
	Forge = 1,
	NeoForge = 2,
	Quilt = 3,
	Fabric = 4,
	LegacyFabric = 5,
}

impl GameLoader {

	#[must_use]
	pub const fn is_modded(&self) -> bool {
		!matches!(self, Self::Vanilla)
	}

	pub const fn get_format_version(&self) -> usize {
		match self {
			Self::Vanilla => interpulse::api::minecraft::CURRENT_FORMAT_VERSION,
			Self::Forge => interpulse::api::modded::CURRENT_FORGE_FORMAT_VERSION,
			Self::NeoForge => interpulse::api::modded::CURRENT_NEOFORGE_FORMAT_VERSION,
			Self::Quilt => interpulse::api::modded::CURRENT_QUILT_FORMAT_VERSION,
			Self::Fabric => interpulse::api::modded::CURRENT_FABRIC_FORMAT_VERSION,
			Self::LegacyFabric => interpulse::api::modded::CURRENT_LEGACY_FABRIC_FORMAT_VERSION,
		}
	}

	pub fn get_format_name(&self) -> String {
		match self {
			Self::Vanilla => String::from("minecraft"),
			Self::NeoForge => String::from("neo"), // TODO(metadata): change to neoforge
			_ => self.to_string().to_lowercase().replace(['_', '.', ' '], "")
		}
	}

	pub fn compatible_with(&self, other: &Self) -> bool {
		match self {
			Self::Vanilla => matches!(other, Self::Vanilla),
			Self::Forge => matches!(other, Self::Forge),
			Self::NeoForge => matches!(other, Self::NeoForge),
			Self::Quilt => matches!(other, Self::Quilt | Self::Fabric),
			Self::Fabric => matches!(other, Self::Fabric),
			Self::LegacyFabric => matches!(other, Self::LegacyFabric),
		}
	}

}

impl TryFrom<String> for GameLoader {
	type Error = String;

	fn try_from(s: String) -> Result<GameLoader, String> {
		Ok(match s.to_lowercase().replace(['_', '.', ' '], "").as_str() {
			"vanilla" | "minecraft" => Self::Vanilla,
			"forge" => Self::Forge,
			"neoforge" | "neo" => Self::NeoForge,
			"quilt" => Self::Quilt,
			"fabric" => Self::Fabric,
			"legacyfabric" => Self::LegacyFabric,
			_ => return Err(format!("'{s}' is not a valid game loader")),
		})
	}
}

impl FromStr for GameLoader {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Self::try_from(s.to_string())
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
			}
		)
	}
}
