use serde::{Deserialize, Serialize};

use oneclient_common::domain::GameLoader;
use oneclient_common::patch::Patch;
use oneclient_db::models::ClusterKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateClusterOptions {
    pub name: String,
    pub mc_version: String,
    pub mc_loader: GameLoader,
    pub mc_loader_version: Option<String>,
    pub mem_max: Option<u32>,
    pub kind: ClusterKind,
    pub user_created: bool,
    pub description: Option<String>,
    pub tags: Vec<String>,
}

impl CreateClusterOptions {
    pub fn new(
        name: impl Into<String>,
        mc_version: impl Into<String>,
        mc_loader: GameLoader,
    ) -> Self {
        Self {
            name: name.into(),
            mc_version: mc_version.into(),
            mc_loader,
            mc_loader_version: None,
            mem_max: None,
            kind: ClusterKind::OneClient,
            user_created: false,
            description: None,
            tags: Vec::new(),
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

    pub fn kind(mut self, kind: ClusterKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn user_created(mut self, user_created: bool) -> Self {
        self.user_created = user_created;
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClusterUpdate {
    pub name: Option<String>,
    pub setting_profile_name: Patch<String>,
    pub mc_loader_version: Patch<String>,
    pub linked_modpack_hash: Patch<String>,
    pub description: Patch<String>,
    pub tags: Option<Vec<String>>,
    pub cover_path: Patch<String>,
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
