use std::fmt::Display;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use strum::{Display as StrumDisplay, EnumIter, FromRepr};

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    EnumIter,
    FromRepr,
    StrumDisplay,
)]
#[repr(u8)]
pub enum ContentType {
    #[serde(rename = "mod")]
    Mod = 0,
    #[serde(rename = "resourcepack")]
    ResourcePack = 1,
    #[serde(rename = "shader")]
    Shader = 2,
    #[serde(rename = "datapack")]
    DataPack = 3,
    #[serde(rename = "world")]
    World = 4,
    #[serde(rename = "modpack")]
    Modpack = 5,
}

impl ContentType {
    pub fn modrinth_type(self) -> &'static str {
        match self {
            Self::Mod => "mod",
            Self::ResourcePack => "resourcepack",
            Self::Shader => "shader",
            Self::DataPack => "datapack",
            Self::World => "world",
            Self::Modpack => "modpack",
        }
    }

    pub fn folder_name(self) -> &'static str {
        match self {
            Self::Mod => "mods",
            Self::ResourcePack => "resourcepacks",
            Self::Shader => "shaderpacks",
            Self::DataPack => "datapacks",
            Self::World => "worlds",
            Self::Modpack => "modpacks",
        }
    }

    pub fn from_folder_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "mods" | "mod" => Some(Self::Mod),
            "resourcepacks" | "resourcepack" => Some(Self::ResourcePack),
            "shaderpacks" | "shaders" | "shader" => Some(Self::Shader),
            "datapacks" | "datapack" => Some(Self::DataPack),
            "worlds" | "world" | "saves" => Some(Self::World),
            "modpacks" | "modpack" => Some(Self::Modpack),
            _ => None,
        }
    }
}

impl From<&str> for ContentType {
    fn from(value: &str) -> Self {
        Self::from_folder_name(value).unwrap_or(Self::Mod)
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    EnumIter,
    FromRepr,
    StrumDisplay,
)]
#[repr(u8)]
pub enum ProviderId {
    Modrinth = 0,
    CurseForge = 1,
    Local = 2,
}

impl ProviderId {
    pub const REMOTE_PROVIDERS: &[Self] = &[Self::Modrinth, Self::CurseForge];
    pub const REMOTE_PROVIDERS_STR: &[&str] = &["Modrinth", "CurseForge"];

    pub fn dir_name(self) -> &'static str {
        match self {
            Self::Modrinth => "modrinth",
            Self::CurseForge => "curseforge",
            Self::Local => "local",
        }
    }

    pub fn website(self) -> &'static str {
        match self {
            Self::Modrinth => "https://modrinth.com/",
            Self::CurseForge => "https://www.curseforge.com/",
            Self::Local => "",
        }
    }

    pub const fn remote_providers() -> &'static [Self] {
        &[Self::Modrinth, Self::CurseForge]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Sha1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default, FromRepr)]
#[repr(u8)]
pub enum GameLoader {
    #[default]
    #[serde(rename = "minecraft")]
    Vanilla = 0,
    Forge = 1,
    NeoForge = 2,
    Quilt = 3,
    Fabric = 4,
    LegacyFabric = 5,
}

impl GameLoader {
    pub const fn is_modded(self) -> bool {
        !matches!(self, Self::Vanilla)
    }

    pub const fn get_format_version(self) -> usize {
        match self {
            Self::Vanilla => interfrost::api::minecraft::CURRENT_FORMAT_VERSION,
            Self::Forge => interfrost::api::modded::CURRENT_FORGE_FORMAT_VERSION,
            Self::NeoForge => interfrost::api::modded::CURRENT_NEOFORGE_FORMAT_VERSION,
            Self::Quilt => interfrost::api::modded::CURRENT_QUILT_FORMAT_VERSION,
            Self::Fabric => interfrost::api::modded::CURRENT_FABRIC_FORMAT_VERSION,
            Self::LegacyFabric => interfrost::api::modded::CURRENT_LEGACY_FABRIC_FORMAT_VERSION,
        }
    }

    pub fn get_format_name(self) -> String {
        match self {
            Self::Vanilla => String::from("minecraft"),
            Self::NeoForge => String::from("neo"),
            _ => self.to_string().to_lowercase().replace(['_', '.', ' '], ""),
        }
    }

    pub const fn modded_loaders() -> &'static [Self] {
        &[Self::Forge, Self::NeoForge, Self::Quilt, Self::Fabric]
    }

    pub fn compatible_with(self, other: Self) -> bool {
        match self {
            Self::Vanilla => other == Self::Vanilla,
            Self::Forge => other == Self::Forge,
            Self::NeoForge => other == Self::NeoForge,
            Self::Quilt => matches!(other, Self::Quilt | Self::Fabric),
            Self::Fabric => other == Self::Fabric,
            Self::LegacyFabric => other == Self::LegacyFabric,
        }
    }

    pub fn modrinth_name(self) -> &'static str {
        match self {
            Self::Vanilla => "minecraft",
            Self::Forge => "forge",
            Self::NeoForge => "neoforge",
            Self::Quilt => "quilt",
            Self::Fabric => "fabric",
            Self::LegacyFabric => "legacy-fabric",
        }
    }
}

impl FromStr for GameLoader {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(
            match s.to_lowercase().replace(['_', '.', ' '], "").as_str() {
                "vanilla" | "minecraft" => Self::Vanilla,
                "forge" => Self::Forge,
                "neoforge" | "neo" => Self::NeoForge,
                "quilt" | "quiltloader" => Self::Quilt,
                "fabric" | "fabricloader" => Self::Fabric,
                "legacyfabric" => Self::LegacyFabric,
                _ => return Err(format!("'{s}' is not a valid game loader")),
            },
        )
    }
}

impl Display for GameLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Vanilla => "Vanilla",
            Self::Forge => "Forge",
            Self::NeoForge => "NeoForge",
            Self::Quilt => "Quilt",
            Self::Fabric => "Fabric",
            Self::LegacyFabric => "Legacy Fabric",
        })
    }
}
