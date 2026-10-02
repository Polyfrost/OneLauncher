use std::collections::{HashMap, HashSet};

use oneclient_common::version::{VersionKey, parse_mc_version};

const DEFAULT_ART_KEY: &str = "default";

pub type ArtsManifest = HashMap<String, String>;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct VersionArts {
    by_patch: HashMap<(u32, u32, u32), String>,
    by_minor: HashMap<(u32, u32), String>,
    by_major: HashMap<u32, String>,
    default: Option<String>,
}

impl VersionArts {
    #[must_use]
    pub fn new(manifest: &ArtsManifest, meta_url_base: &str) -> Self {
        let mut arts = Self::default();

        for (version, path) in manifest {
            let url = format!("{meta_url_base}{path}");

            if version == DEFAULT_ART_KEY {
                arts.default = Some(url);
                continue;
            }

            let Some(parsed) = parse_mc_version(version) else {
                tracing::warn!("ignoring version art under an unparseable key {version}");
                continue;
            };

            match (parsed.minor, parsed.patch) {
                (Some(minor), Some(patch)) => {
                    arts.by_patch.insert((parsed.major, minor, patch), url);
                }
                (Some(minor), None) => {
                    arts.by_minor.insert((parsed.major, minor), url);
                }
                (None, _) => {
                    arts.by_major.insert(parsed.major, url);
                }
            }
        }

        arts
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_patch.is_empty()
            && self.by_minor.is_empty()
            && self.by_major.is_empty()
            && self.default.is_none()
    }

    #[must_use]
    pub fn gallery(&self) -> Vec<String> {
        let mut ordered: Vec<(u32, Option<u32>, Option<u32>, &str)> = self
            .by_major
            .iter()
            .map(|(major, url)| (*major, None, None, url.as_str()))
            .chain(
                self.by_minor
                    .iter()
                    .map(|((major, minor), url)| (*major, Some(*minor), None, url.as_str())),
            )
            .chain(self.by_patch.iter().map(|((major, minor, patch), url)| {
                (*major, Some(*minor), Some(*patch), url.as_str())
            }))
            .collect();

        ordered.sort_unstable_by(|a, b| {
            let rank = |v: &(u32, Option<u32>, Option<u32>, &str)| {
                (v.0, v.1.unwrap_or(u32::MAX), v.2.unwrap_or(u32::MAX))
            };
            rank(b).cmp(&rank(a))
        });

        let mut seen = HashSet::new();
        let mut urls: Vec<String> = ordered
            .into_iter()
            .filter(|(_, _, _, url)| seen.insert(*url))
            .map(|(_, _, _, url)| url.to_string())
            .collect();

        if let Some(default) = &self.default
            && seen.insert(default.as_str())
        {
            urls.push(default.clone());
        }

        urls
    }

    #[must_use]
    pub fn specific_art_url(&self, major: Option<u32>, key: Option<VersionKey>) -> Option<String> {
        let major = major?;
        let (minor, patch) = key?;

        patch
            .and_then(|patch| self.by_patch.get(&(major, minor, patch)))
            .or_else(|| self.by_minor.get(&(major, minor)))
            .cloned()
    }

    #[must_use]
    pub fn fallback_art_url(&self, major: Option<u32>) -> Option<String> {
        major
            .and_then(|major| self.by_major.get(&major))
            .or(self.default.as_ref())
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "https://example.invalid";

    fn manifest() -> ArtsManifest {
        [
            ("1.9", "/art/combat.webp"),
            ("1.16", "/art/nether.webp"),
            ("1.21", "/art/trials.webp"),
            ("1.21.4", "/art/garden.webp"),
            ("1.21.11", "/art/mounts.webp"),
            ("26.1", "/art/tiny.webp"),
            ("26.1.3", "/art/chaos.webp"),
            (DEFAULT_ART_KEY, "/art/dirt.webp"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
    }

    fn resolve(version: &str) -> Option<String> {
        let arts = VersionArts::new(&manifest(), BASE);
        let parsed = parse_mc_version(version)?;
        arts.specific_art_url(Some(parsed.major), parsed.key())
            .or_else(|| arts.fallback_art_url(Some(parsed.major)))
    }

    #[test]
    fn a_patch_key_does_not_collide_with_its_minor_line() {
        assert_eq!(
            resolve("26.1.3").as_deref(),
            Some("https://example.invalid/art/chaos.webp")
        );
        assert_eq!(
            resolve("26.1").as_deref(),
            Some("https://example.invalid/art/tiny.webp")
        );
        assert_eq!(
            resolve("26.1.9").as_deref(),
            Some("https://example.invalid/art/tiny.webp")
        );
    }

    #[test]
    fn minor_specific_art_wins_over_its_line() {
        assert_eq!(
            resolve("1.21.4").as_deref(),
            Some("https://example.invalid/art/garden.webp")
        );
        assert_eq!(
            resolve("1.21.11").as_deref(),
            Some("https://example.invalid/art/mounts.webp")
        );
    }

    #[test]
    fn minor_without_its_own_art_falls_back_to_the_line() {
        assert_eq!(
            resolve("1.21.2").as_deref(),
            Some("https://example.invalid/art/trials.webp")
        );
        assert_eq!(
            resolve("1.16.5").as_deref(),
            Some("https://example.invalid/art/nether.webp")
        );
    }

    #[test]
    fn bare_line_resolves_to_itself() {
        assert_eq!(
            resolve("1.9").as_deref(),
            Some("https://example.invalid/art/combat.webp")
        );
    }

    #[test]
    fn unknown_versions_and_missing_majors_use_the_default() {
        assert_eq!(
            resolve("1.20.1").as_deref(),
            Some("https://example.invalid/art/dirt.webp")
        );

        let arts = VersionArts::new(&manifest(), BASE);
        assert_eq!(
            arts.fallback_art_url(None).as_deref(),
            Some("https://example.invalid/art/dirt.webp")
        );
    }

    #[test]
    fn a_manifest_without_a_default_resolves_to_nothing() {
        let manifest: ArtsManifest = [("1.9".to_string(), "/art/combat.webp".to_string())]
            .into_iter()
            .collect();
        let arts = VersionArts::new(&manifest, BASE);
        assert_eq!(arts.specific_art_url(Some(20), Some((1, None))), None);
        assert_eq!(arts.fallback_art_url(Some(20)), None);
    }

    #[test]
    fn the_gallery_is_deduped_newest_first_and_ends_with_the_default() {
        let mut manifest = manifest();
        manifest.insert("1.21.5".to_string(), "/art/garden.webp".to_string());

        let arts = VersionArts::new(&manifest, BASE);

        assert_eq!(
            arts.gallery(),
            vec![
                "https://example.invalid/art/tiny.webp",
                "https://example.invalid/art/chaos.webp",
                "https://example.invalid/art/trials.webp",
                "https://example.invalid/art/mounts.webp",
                "https://example.invalid/art/garden.webp",
                "https://example.invalid/art/nether.webp",
                "https://example.invalid/art/combat.webp",
                "https://example.invalid/art/dirt.webp",
            ]
        );
    }

    #[test]
    fn unparseable_keys_are_skipped() {
        let manifest: ArtsManifest = [("snapshot".to_string(), "/art/snap.webp".to_string())]
            .into_iter()
            .collect();
        assert!(VersionArts::new(&manifest, BASE).is_empty());
    }
}
