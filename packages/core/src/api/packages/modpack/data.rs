use onelauncher_entity::loader::GameLoader;
use serde::{Deserialize, Serialize};

use crate::api::packages::data::{ExternalPackage, ManagedPackage, ManagedVersion};

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
pub enum ModpackFile {
	Managed((ManagedPackage, ManagedVersion)),
	External(ExternalPackage),
}
