use std::collections::HashSet;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use oneclient_common::domain::GameLoader;
use oneclient_common::version::{VersionKey, format_mc_version};

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
    /// Default for every entry in this cluster an entry's own key wins
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
    #[serde(default)]
    pub allow_without_bundles: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MigrationSource {
    pub mc_version: String,
    pub loader: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MigrationTarget {
    pub mc_version: String,
    #[serde(default)]
    pub loader: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MigrationNode {
    pub mc_version: String,
    pub loader: GameLoader,
}

impl RemoteMigration {
    #[must_use]
    pub fn endpoints(&self) -> Option<(MigrationNode, MigrationNode)> {
        let from_loader = GameLoader::from_str(&self.from.loader).ok()?;
        let to_loader = match &self.to.loader {
            Some(loader) => GameLoader::from_str(loader).ok()?,
            None => from_loader,
        };

        Some((
            MigrationNode {
                mc_version: self.from.mc_version.clone(),
                loader: from_loader,
            },
            MigrationNode {
                mc_version: self.to.mc_version.clone(),
                loader: to_loader,
            },
        ))
    }

    #[must_use]
    pub fn changes_loader(&self) -> bool {
        self.endpoints()
            .is_some_and(|(from, to)| from.loader != to.loader)
    }
}

#[must_use]
pub fn cyclic_migration_ids(rules: &[RemoteMigration]) -> HashSet<String> {
    let edges: Vec<(MigrationNode, MigrationNode)> = rules
        .iter()
        .filter_map(RemoteMigration::endpoints)
        .filter(|(from, to)| from != to)
        .collect();

    rules
        .iter()
        .filter(|rule| {
            let Some((from, to)) = rule.endpoints() else {
                return false;
            };
            from != to && reaches(&edges, &to, &from)
        })
        .map(|rule| rule.id.clone())
        .collect()
}

fn reaches(
    edges: &[(MigrationNode, MigrationNode)],
    start: &MigrationNode,
    goal: &MigrationNode,
) -> bool {
    let mut seen: HashSet<&MigrationNode> = HashSet::new();
    let mut stack = vec![start];

    while let Some(node) = stack.pop() {
        if node == goal {
            return true;
        }
        if !seen.insert(node) {
            continue;
        }
        stack.extend(
            edges
                .iter()
                .filter(|(from, _)| from == node)
                .map(|(_, to)| to),
        );
    }

    false
}

#[must_use]
pub fn resolve_migration_chain(
    mc_version: &str,
    loader: GameLoader,
    rules: &[RemoteMigration],
) -> (String, GameLoader) {
    let mut current = MigrationNode {
        mc_version: mc_version.to_string(),
        loader,
    };

    for _ in 0..=rules.len() {
        let Some((_, to)) = rules
            .iter()
            .filter_map(RemoteMigration::endpoints)
            .find(|(from, _)| *from == current)
        else {
            break;
        };

        if to == current {
            break;
        }

        current = to;
    }

    (current.mc_version, current.loader)
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
    /// Fetched up front during onboarding rather than on first launch
    pub predownload: bool,
}

impl VersionMetadata {
    #[must_use]
    pub fn key(&self) -> Option<VersionKey> {
        Some((self.minor_version?, self.patch_version))
    }

    /// `None` for the synthetic cluster-level row which has no minor version
    #[must_use]
    pub fn mc_version(&self) -> Option<String> {
        Some(format_mc_version(
            self.major_version,
            self.minor_version?,
            self.patch_version,
        ))
    }
}

fn art_url(path: &Option<String>, meta_url_base: &str) -> Option<String> {
    path.as_ref().map(|p| format!("{meta_url_base}{p}"))
}

impl VersionsManifest {
    pub fn metadata(&self, meta_url_base: &str) -> Vec<VersionMetadata> {
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
                art_url: art_url(&cluster.art, meta_url_base),
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
                    art_url: art_url(&entry.art, meta_url_base)
                        .or_else(|| art_url(&cluster.art, meta_url_base)),
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

        let metadata = manifest.metadata("https://example.test");
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

        let metadata = manifest.metadata("https://example.test");
        let flag = |major: u32, minor: Option<u32>| {
            metadata
                .iter()
                .find(|m| m.major_version == major && m.minor_version == minor)
                .expect("row present")
                .predownload
        };

        assert!(flag(26, None));
        assert!(flag(26, Some(1)));
        assert!(!flag(26, Some(2)));
        assert!(flag(21, Some(11)));
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

        let metadata = manifest.metadata("https://example.test");
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
                loader: None,
            },
            allow_without_bundles: false,
        }
    }

    fn loader_rule(id: &str, version: &str, from: &str, to: &str) -> RemoteMigration {
        let mut rule = rule(id, version, from, version);
        rule.to.loader = Some(to.to_string());
        rule
    }

    fn fabric(version: &str) -> (String, GameLoader) {
        (version.to_string(), GameLoader::Fabric)
    }

    #[test]
    fn chain_resolves_single_hop() {
        let rules = [rule("a", "26.1", "fabric", "26.1.2")];
        assert_eq!(
            resolve_migration_chain("26.1", GameLoader::Fabric, &rules),
            fabric("26.1.2")
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
            fabric("26.1.3")
        );
    }

    #[test]
    fn chain_requires_matching_loader() {
        let rules = [rule("a", "26.1", "forge", "26.1.2")];
        assert_eq!(
            resolve_migration_chain("26.1", GameLoader::Fabric, &rules),
            fabric("26.1")
        );
    }

    #[test]
    fn chain_returns_input_when_no_rule_matches() {
        let rules = [rule("a", "21.1", "fabric", "21.1.2")];
        assert_eq!(
            resolve_migration_chain("26.1", GameLoader::Fabric, &rules),
            fabric("26.1")
        );
    }

    #[test]
    fn chain_terminates_on_self_and_cyclic_rules() {
        let self_rule = [rule("a", "26.1", "fabric", "26.1")];
        assert_eq!(
            resolve_migration_chain("26.1", GameLoader::Fabric, &self_rule),
            fabric("26.1")
        );

        let cycle = [
            rule("a", "26.1", "fabric", "26.2"),
            rule("b", "26.2", "fabric", "26.1"),
        ];
        let out = resolve_migration_chain("26.1", GameLoader::Fabric, &cycle);
        assert!(out == fabric("26.1") || out == fabric("26.2"));
    }

    #[test]
    fn chain_switches_loader() {
        let rules = [
            rule("a", "26.1", "fabric", "26.1.2"),
            loader_rule("b", "26.1.2", "fabric", "neoforge"),
        ];
        assert_eq!(
            resolve_migration_chain("26.1", GameLoader::Fabric, &rules),
            ("26.1.2".to_string(), GameLoader::NeoForge)
        );
    }

    #[test]
    fn missing_target_loader_keeps_the_source_loader() {
        let (from, to) = rule("a", "26.1", "forge", "26.1.2").endpoints().unwrap();
        assert_eq!(from.loader, GameLoader::Forge);
        assert_eq!(to.loader, GameLoader::Forge);
        assert!(!rule("a", "26.1", "forge", "26.1.2").changes_loader());
        assert!(loader_rule("a", "1.20.1", "forge", "neoforge").changes_loader());
    }

    #[test]
    fn unknown_target_loader_invalidates_the_rule() {
        let broken = loader_rule("a", "1.20.1", "forge", "rift");
        assert!(broken.endpoints().is_none());
        assert!(!broken.changes_loader());
    }

    #[test]
    fn opposite_loader_rules_are_a_cycle() {
        let rules = [
            loader_rule("there", "1.8.9", "fabric", "ornithe"),
            loader_rule("back", "1.8.9", "ornithe", "fabric"),
            loader_rule("other", "1.20.1", "forge", "neoforge"),
        ];
        let cyclic = cyclic_migration_ids(&rules);
        assert!(cyclic.contains("there"));
        assert!(cyclic.contains("back"));
        assert!(!cyclic.contains("other"));
    }

    #[test]
    fn a_three_loader_ring_is_a_cycle() {
        let rules = [
            loader_rule("a", "1.20.1", "forge", "neoforge"),
            loader_rule("b", "1.20.1", "neoforge", "fabric"),
            loader_rule("c", "1.20.1", "fabric", "forge"),
        ];
        assert_eq!(cyclic_migration_ids(&rules).len(), 3);
    }

    #[test]
    fn a_chain_is_not_a_cycle() {
        let rules = [
            rule("a", "26.1", "fabric", "26.1.2"),
            loader_rule("b", "26.1.2", "fabric", "quilt"),
            rule("self", "26.3", "fabric", "26.3"),
        ];
        assert!(cyclic_migration_ids(&rules).is_empty());
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
        assert_eq!(manifest.migrations[0].to.loader, None);
        assert!(!manifest.migrations[0].allow_without_bundles);
    }

    #[test]
    fn loader_migrations_parse() {
        let manifest: VersionsManifest = serde_json::from_str(
            r#"{"clusters":[],"migrations":[
                {"id":"x","from":{"mc_version":"1.20.1","loader":"forge"},
                 "to":{"mc_version":"1.20.1","loader":"neoforge"},
                 "allow_without_bundles":true}
            ]}"#,
        )
        .expect("should parse");

        assert_eq!(
            manifest.migrations[0].to.loader.as_deref(),
            Some("neoforge")
        );
        assert!(manifest.migrations[0].allow_without_bundles);
    }
}
