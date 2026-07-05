use serde::{Deserialize, Serialize};

use crate::packages::domain::GameLoader;
use crate::patch::Patch;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateClusterOptions {
	pub name: String,
	pub mc_version: String,
	pub mc_loader: GameLoader,
	pub mc_loader_version: Option<String>,
	pub mem_max: Option<u32>,
}

impl CreateClusterOptions {
	pub fn new(name: impl Into<String>, mc_version: impl Into<String>, mc_loader: GameLoader) -> Self {
		Self {
			name: name.into(),
			mc_version: mc_version.into(),
			mc_loader,
			mc_loader_version: None,
			mem_max: None,
		}
	}

	pub fn mem_max(mut self, megabytes: u32) -> Self {
		self.mem_max = Some(megabytes);
		self
	}

	pub fn loader_version(mut self, version: impl Into<String>) -> Self {
		self.mc_loader_version = Some(version.into());
		self
	}
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClusterUpdate {
	pub name: Option<String>,
	pub setting_profile_name: Patch<String>,
	pub mc_loader_version: Patch<String>,
	pub linked_modpack_hash: Patch<String>,
}

impl ClusterUpdate {
	pub fn setting_profile(mut self, name: impl Into<String>) -> Self {
		self.setting_profile_name = Patch::Set(name.into());
		self
	}

	pub fn loader_version(mut self, version: impl Into<String>) -> Self {
		self.mc_loader_version = Patch::Set(version.into());
		self
	}
}
