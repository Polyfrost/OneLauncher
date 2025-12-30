use std::fmt::Display;

use sea_orm::{DeriveActiveEnum, EnumIter};
use serde::{Deserialize, Serialize};

#[onelauncher_macro::specta]
#[derive(
	Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum,
)]
#[serde(rename_all = "lowercase")]
#[sea_orm(rs_type = "u8", db_type = "Integer")]
pub enum PackageType {
	#[default]
	Mod = 0,
	ResourcePack = 1,
	Shader = 2,
	DataPack = 3,
	ModPack = 4,
}

impl PackageType {
	#[must_use]
	pub fn folder_name(&self) -> String {
		match self {
			Self::Mod => "mods",
			Self::ResourcePack => "resourcepacks",
			Self::Shader => "shaders",
			Self::DataPack => "datapacks",
			Self::ModPack => "modpacks",
		}
		.to_string()
	}
}

impl From<&str> for PackageType {
	fn from(value: &str) -> Self {
		match value.to_lowercase().as_str() {
			"mod" | "mods" => Self::Mod,
			"resourcepack" | "resourcepacks" => Self::ResourcePack,
			"shader" | "shaders" => Self::Shader,
			"datapack" | "datapacks" => Self::DataPack,
			"modpack" => Self::ModPack,
			_ => Self::Mod, // default case
		}
	}
}

impl From<String> for PackageType {
	fn from(value: String) -> Self {
		Self::from(value.as_str())
	}
}

impl ToString for PackageType {
	fn to_string(&self) -> String {
		match self {
			&Self::Mod => "mod",
			&Self::ResourcePack => "resourcepack",
			&Self::Shader => "shader",
			&Self::DataPack => "datapack",
			&Self::ModPack => "modpack",
		}
		.to_string()
	}
}

#[onelauncher_macro::specta]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "u8", db_type = "Integer")]
pub enum Provider {
	Modrinth = 0,
	CurseForge = 1,
	SkyClient = 2,
	Local = 3,
}

impl Display for Provider {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.name())
	}
}

impl Provider {
	#[must_use]
	pub const fn name(&self) -> &str {
		match self {
			Self::Modrinth => "Modrinth",
			Self::CurseForge => "CurseForge",
			Self::SkyClient => "SkyClient",
			Self::Local => "Local",
		}
	}

	/// Get the URL of the provider with a trailing slash
	#[must_use]
	pub const fn website(&self) -> &str {
		match self {
			Self::Modrinth => "https://modrinth.com/",
			Self::CurseForge => "https://curseforge.com/",
			Self::SkyClient => "https://skyclient.co/",
			Self::Local => "",
		}
	}

	#[must_use]
	pub const fn get_providers() -> &'static [Self] {
		&[Self::Modrinth, Self::CurseForge]
	}
}
