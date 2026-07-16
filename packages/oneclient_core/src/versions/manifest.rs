use serde::{Deserialize, Serialize};

use crate::api_config::meta_url_base;
use crate::version::VersionKey;

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct VersionsManifest {
    #[serde(default)]
    pub clusters: Vec<RemoteCluster>,
    #[serde(default)]
    pub migrations: Vec<RemoteMigration>,
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
    pub patch_version: Option<u32>,
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
pub struct RemoteMigration {
    pub id: String,
    pub from: MigrationSource,
    pub to: MigrationTarget,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MigrationSource {
    pub mc_version: String,
    pub loader: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MigrationTarget {
    pub mc_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VersionMetadata {
    pub major_version: u32,
    pub minor_version: Option<u32>,
    pub patch_version: Option<u32>,
    pub loader: Option<String>,
    pub name: String,
    pub art_url: Option<String>,
    pub long_description: Option<String>,
    pub tags: Vec<String>,
}

impl VersionMetadata {
    #[must_use]
    pub fn key(&self) -> Option<VersionKey> {
        Some((self.minor_version?, self.patch_version))
    }
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
                patch_version: None,
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
                    patch_version: entry.patch_version,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_without_migrations_key_parses() {
        let manifest: VersionsManifest =
            serde_json::from_str(r#"{"clusters": []}"#).expect("should parse");
        assert!(manifest.migrations.is_empty());
    }

    #[test]
    fn entry_without_patch_key_parses() {
        let manifest: VersionsManifest = serde_json::from_str(
            r#"{"clusters":[{"major_version":26,"entries":[{"minor_version":1}]}]}"#,
        )
        .expect("should parse");
        assert_eq!(manifest.clusters[0].entries[0].patch_version, None);
    }

    #[test]
    fn patch_version_reaches_metadata() {
        let manifest: VersionsManifest = serde_json::from_str(
            r#"{"clusters":[{"major_version":26,"entries":[
                {"minor_version":1,"patch_version":2,"loader":"fabric"}
            ]}]}"#,
        )
        .expect("should parse");

        let metadata = manifest.metadata();
        let entry = metadata
            .iter()
            .find(|m| m.minor_version == Some(1))
            .expect("entry present");
        assert_eq!(entry.patch_version, Some(2));
        assert_eq!(entry.key(), Some((1, Some(2))));
    }

    #[test]
    fn migrations_parse() {
        let manifest: VersionsManifest = serde_json::from_str(
            r#"{"clusters":[],"migrations":[
                {"id":"x","from":{"mc_version":"26.1","loader":"fabric"},
                 "to":{"mc_version":"26.1.2"}}
            ]}"#,
        )
        .expect("should parse");

        assert_eq!(manifest.migrations[0].id, "x");
        assert_eq!(manifest.migrations[0].from.mc_version, "26.1");
        assert_eq!(manifest.migrations[0].to.mc_version, "26.1.2");
    }
}
