use serde::{Deserialize, Serialize};

use crate::api_config::meta_url_base;

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct VersionsManifest {
    #[serde(default)]
    pub clusters: Vec<RemoteCluster>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteCluster {
    pub major_version: u32,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub art: Option<String>,
    #[serde(default)]
    pub long_description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub entries: Vec<RemoteEntry>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteEntry {
    pub minor_version: u32,
    #[serde(default)]
    pub loader: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub art: Option<String>,
    #[serde(default)]
    pub long_description: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VersionMetadata {
    pub major_version: u32,
    pub minor_version: Option<u32>,
    pub loader: Option<String>,
    pub name: String,
    pub art_url: Option<String>,
    pub long_description: Option<String>,
    pub tags: Vec<String>,
}

fn art_url(path: &Option<String>) -> Option<String> {
    path.as_ref()
        .map(|p| format!("{}{p}", meta_url_base()))
}

impl VersionsManifest {
    pub fn metadata(&self) -> Vec<VersionMetadata> {
        let mut out = Vec::new();

        for cluster in &self.clusters {
            let cluster_name = cluster
                .name
                .clone()
                .unwrap_or_else(|| format!("1.{}", cluster.major_version));

            out.push(VersionMetadata {
                major_version: cluster.major_version,
                minor_version: None,
                loader: None,
                name: cluster_name.clone(),
                art_url: art_url(&cluster.art),
                long_description: cluster.long_description.clone(),
                tags: cluster.tags.clone(),
            });

            for entry in &cluster.entries {
                out.push(VersionMetadata {
                    major_version: cluster.major_version,
                    minor_version: Some(entry.minor_version),
                    loader: entry.loader.clone(),
                    name: entry.name.clone().unwrap_or_else(|| cluster_name.clone()),
                    art_url: art_url(&entry.art).or_else(|| art_url(&cluster.art)),
                    long_description: entry
                        .long_description
                        .clone()
                        .or_else(|| cluster.long_description.clone()),
                    tags: entry.tags.clone().unwrap_or_else(|| cluster.tags.clone()),
                });
            }
        }

        out
    }
}
