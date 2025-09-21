use std::path::PathBuf;

use onelauncher_entity::loader::GameLoader;
use serde::{Deserialize, Serialize};

use crate::api::packages::data::{ExternalPackage, ManagedPackage, ManagedVersion};
use crate::api::packages::modpack::ModpackFormat;

#[onelauncher_macro::specta]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModpackArchive {
	pub manifest: ModpackManifest,
	pub path: PathBuf,
	pub format: ModpackFormat,
}

#[onelauncher_macro::specta]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModpackManifest {
	pub name: String,
	pub version: String,
	pub loader: GameLoader,
	pub loader_version: String,
	pub mc_version: String,
	pub files: Vec<ModpackFile>,
}

#[onelauncher_macro::specta]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModpackFile {
	pub enabled: bool,
	pub kind: ModpackFileKind,
}

#[onelauncher_macro::specta]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModpackFileKind {
	Managed((ManagedPackage, ManagedVersion)),
	External(ExternalPackage),
}
