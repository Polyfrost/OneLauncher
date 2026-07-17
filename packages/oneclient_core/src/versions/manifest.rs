use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::api_config::meta_url_base;
use crate::packages::domain::GameLoader;
use crate::version::{VersionKey, format_mc_version};

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
    /// Default for every entry in this cluster; an entry's own key wins.
    #[serde(default)]
    pub predownload: Option<bool>,
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
    #[serde(default)]
    pub predownload: Option<bool>,
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

/// Follow the remote migration chain forward from `mc_version` for the given
/// loader, returning the final target version after every applicable rule has
/// been applied. Returns the input unchanged when no rule matches.
///
/// This mirrors what [`crate::clusters::apply_remote_migrations`] does to
/// existing cluster rows, so callers that only know an *old* version (e.g. the
/// OneClient v1 importer) can find the cluster the migrations have already moved
/// that version to. Multi-hop chains (`a -> b -> c`) are followed; the loop is
/// bounded by the rule count so a cyclic/self rule can't spin forever.
#[must_use]
pub fn resolve_migration_chain(
    mc_version: &str,
    loader: GameLoader,
    rules: &[RemoteMigration],
) -> String {
    let mut current = mc_version.to_string();

    for _ in 0..=rules.len() {
        let Some(rule) = rules.iter().find(|rule| {
            rule.from.mc_version == current
                && GameLoader::from_str(&rule.from.loader).is_ok_and(|l| l == loader)
        }) else {
            break;
        };

        if rule.to.mc_version == current {
            break;
        }

        current = rule.to.mc_version.clone();
    }

    current
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
    /// Whether this version is fetched up front during onboarding, rather than
    /// on first launch. Cluster rows carry the cluster's own default.
    pub predownload: bool,
}

impl VersionMetadata {
    #[must_use]
    pub fn key(&self) -> Option<VersionKey> {
        Some((self.minor_version?, self.patch_version))
    }

    /// The `mc_version` string this row maps to, or `None` for the synthetic
    /// cluster-level row (which has no minor version of its own).
    #[must_use]
    pub fn mc_version(&self) -> Option<String> {
        Some(format_mc_version(
            self.major_version,
            self.minor_version?,
            self.patch_version,
        ))
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
                predownload: cluster.predownload.unwrap_or(false),
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
                    predownload: entry.predownload.or(cluster.predownload).unwrap_or(false),
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
    fn predownload_inherits_from_cluster_and_entry_overrides() {
        let manifest: VersionsManifest = serde_json::from_str(
            r#"{"clusters":[
                {"major_version":26,"predownload":true,"entries":[
                    {"minor_version":1},
                    {"minor_version":2,"predownload":false}
                ]},
                {"major_version":21,"entries":[
                    {"minor_version":1},
                    {"minor_version":11,"predownload":true}
                ]}
            ]}"#,
        )
        .expect("should parse");

        let metadata = manifest.metadata();
        let flag = |major: u32, minor: Option<u32>| {
            metadata
                .iter()
                .find(|m| m.major_version == major && m.minor_version == minor)
                .expect("row present")
                .predownload
        };

        // Cluster default flows down to entries that stay silent.
        assert!(flag(26, None));
        assert!(flag(26, Some(1)));
        // An entry's own key wins over the cluster default, in both directions.
        assert!(!flag(26, Some(2)));
        assert!(flag(21, Some(11)));
        // Absent everywhere means off.
        assert!(!flag(21, None));
        assert!(!flag(21, Some(1)));
    }

    #[test]
    fn mc_version_skips_the_cluster_row() {
        let manifest: VersionsManifest = serde_json::from_str(
            r#"{"clusters":[{"major_version":26,"entries":[
                {"minor_version":1,"patch_version":2}
            ]}]}"#,
        )
        .expect("should parse");

        let metadata = manifest.metadata();
        assert_eq!(metadata[0].mc_version(), None);
        assert_eq!(metadata[1].mc_version().as_deref(), Some("26.1.2"));
    }

    fn rule(id: &str, from: &str, loader: &str, to: &str) -> RemoteMigration {
        RemoteMigration {
            id: id.to_string(),
            from: MigrationSource {
                mc_version: from.to_string(),
                loader: loader.to_string(),
            },
            to: MigrationTarget {
                mc_version: to.to_string(),
            },
        }
    }

    #[test]
    fn chain_resolves_single_hop() {
        let rules = [rule("a", "26.1", "fabric", "26.1.2")];
        assert_eq!(
            resolve_migration_chain("26.1", GameLoader::Fabric, &rules),
            "26.1.2"
        );
    }

    #[test]
    fn chain_follows_multiple_hops() {
        let rules = [
            rule("a", "26.1", "fabric", "26.1.2"),
            rule("b", "26.1.2", "fabric", "26.1.3"),
        ];
        assert_eq!(
            resolve_migration_chain("26.1", GameLoader::Fabric, &rules),
            "26.1.3"
        );
    }

    #[test]
    fn chain_requires_matching_loader() {
        let rules = [rule("a", "26.1", "forge", "26.1.2")];
        assert_eq!(
            resolve_migration_chain("26.1", GameLoader::Fabric, &rules),
            "26.1"
        );
    }

    #[test]
    fn chain_returns_input_when_no_rule_matches() {
        let rules = [rule("a", "21.1", "fabric", "21.1.2")];
        assert_eq!(
            resolve_migration_chain("26.1", GameLoader::Fabric, &rules),
            "26.1"
        );
    }

    #[test]
    fn chain_terminates_on_self_and_cyclic_rules() {
        let self_rule = [rule("a", "26.1", "fabric", "26.1")];
        assert_eq!(
            resolve_migration_chain("26.1", GameLoader::Fabric, &self_rule),
            "26.1"
        );

        let cycle = [
            rule("a", "26.1", "fabric", "26.2"),
            rule("b", "26.2", "fabric", "26.1"),
        ];
        // Must not loop forever; lands on one of the two, bounded by rule count.
        let out = resolve_migration_chain("26.1", GameLoader::Fabric, &cycle);
        assert!(out == "26.1" || out == "26.2");
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
