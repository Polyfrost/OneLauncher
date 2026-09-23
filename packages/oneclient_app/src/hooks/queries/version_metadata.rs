use oneclient_common::VersionKey;
use oneclient_common::domain::GameLoader;
use oneclient_core::VersionMetadata;

use freya::query::QueriesStorage;

use super::use_versions;
use super::versions::{VersionArtsKeys, VersionArtsQuery, use_version_arts};

pub fn pick_version_metadata(
    list: &[VersionMetadata],
    major: u32,
    key: Option<VersionKey>,
    loader: Option<GameLoader>,
) -> Option<VersionMetadata> {
    let loader_matches = |m: &VersionMetadata| match (&m.loader, loader) {
        (Some(s), Some(l)) => l.to_string().eq_ignore_ascii_case(s),
        _ => false,
    };

    if let Some(key) = key {
        if let Some(hit) = list
            .iter()
            .find(|m| m.major_version == major && m.key() == Some(key) && loader_matches(m))
        {
            return Some(hit.clone());
        }
        if let Some(hit) = list
            .iter()
            .find(|m| m.major_version == major && m.key() == Some(key))
        {
            return Some(hit.clone());
        }
        let (minor, _) = key;
        if let Some(hit) = list
            .iter()
            .find(|m| m.major_version == major && m.minor_version == Some(minor))
        {
            return Some(hit.clone());
        }
    }

    list.iter()
        .find(|m| m.major_version == major && m.minor_version.is_none())
        .cloned()
}

pub fn use_version_metadata(
    major: Option<u32>,
    key: Option<VersionKey>,
    loader: Option<GameLoader>,
) -> Option<VersionMetadata> {
    let versions_query = use_versions();

    let major = major?;
    let reader = versions_query.read();
    let state = reader.state();
    let list = state.ok()?;

    pick_version_metadata(list, major, key, loader)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VersionArtCandidates {
    pub minor: Option<String>,
    pub fallback: Option<String>,
}

pub fn use_version_art(major: Option<u32>, key: Option<VersionKey>) -> VersionArtCandidates {
    let arts_query = use_version_arts();
    let reader = arts_query.read();
    let state = reader.state();

    let Some(arts) = state.ok() else {
        return VersionArtCandidates::default();
    };

    VersionArtCandidates {
        minor: arts.specific_art_url(major, key),
        fallback: arts.fallback_art_url(major),
    }
}

pub async fn refresh_version_art_gallery() {
    QueriesStorage::<VersionArtsQuery>::invalidate_matching(VersionArtsKeys).await;
}

pub fn use_version_art_gallery() -> Vec<String> {
    let arts_query = use_version_arts();
    let reader = arts_query.read();
    let state = reader.state();

    state.ok().map(|arts| arts.gallery()).unwrap_or_default()
}

pub fn resolve_art_url(
    curated: Option<&VersionMetadata>,
    candidates: &VersionArtCandidates,
) -> Option<String> {
    let curated_minor = curated
        .filter(|m| m.minor_version.is_some())
        .and_then(|m| m.art_url.clone());

    curated_minor
        .or_else(|| candidates.minor.clone())
        .or_else(|| curated.and_then(|m| m.art_url.clone()))
        .or_else(|| candidates.fallback.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(
        major: u32,
        minor: Option<u32>,
        patch: Option<u32>,
        loader: Option<&str>,
        name: &str,
    ) -> VersionMetadata {
        VersionMetadata {
            major_version: major,
            minor_version: minor,
            patch_version: patch,
            loader: loader.map(str::to_string),
            name: name.to_string(),
            art_url: None,
            long_description: None,
            tags: Vec::new(),
            predownload: false,
        }
    }

    fn list() -> Vec<VersionMetadata> {
        vec![
            entry(26, None, None, None, "major"),
            entry(26, Some(1), None, Some("fabric"), "bare minor"),
            entry(26, Some(1), Some(2), Some("fabric"), "patched"),
        ]
    }

    fn candidates(minor: Option<&str>, fallback: Option<&str>) -> VersionArtCandidates {
        VersionArtCandidates {
            minor: minor.map(str::to_string),
            fallback: fallback.map(str::to_string),
        }
    }

    fn with_art(mut entry: VersionMetadata, art: &str) -> VersionMetadata {
        entry.art_url = Some(art.to_string());
        entry
    }

    #[test]
    fn a_minor_specific_art_beats_the_curated_cluster_row() {
        let cluster = with_art(entry(21, None, None, None, "major"), "trials");
        let resolved = resolve_art_url(Some(&cluster), &candidates(Some("garden"), Some("trials")));
        assert_eq!(resolved.as_deref(), Some("garden"));
    }

    #[test]
    fn a_curated_entry_art_beats_the_arts_manifest() {
        let curated = with_art(
            entry(21, Some(10), None, Some("fabric"), "copper"),
            "copper",
        );
        let resolved = resolve_art_url(Some(&curated), &candidates(Some("other"), Some("trials")));
        assert_eq!(resolved.as_deref(), Some("copper"));
    }

    #[test]
    fn the_cluster_row_still_wins_over_a_line_level_art() {
        let cluster = with_art(entry(21, None, None, None, "major"), "trials");
        let resolved = resolve_art_url(Some(&cluster), &candidates(None, Some("line")));
        assert_eq!(resolved.as_deref(), Some("trials"));
    }

    #[test]
    fn nothing_curated_falls_through_to_the_arts_manifest() {
        assert_eq!(
            resolve_art_url(None, &candidates(None, Some("dirt"))).as_deref(),
            Some("dirt")
        );
        assert_eq!(
            resolve_art_url(None, &candidates(Some("combat"), Some("dirt"))).as_deref(),
            Some("combat")
        );
        assert_eq!(resolve_art_url(None, &candidates(None, None)), None);
    }

    #[test]
    fn a_curated_row_without_art_does_not_shadow_the_manifest() {
        let cluster = entry(21, None, None, None, "major");
        let resolved = resolve_art_url(Some(&cluster), &candidates(None, Some("dirt")));
        assert_eq!(resolved.as_deref(), Some("dirt"));
    }

    #[test]
    fn patch_and_bare_minor_are_distinct() {
        let bare = pick_version_metadata(&list(), 26, Some((1, None)), Some(GameLoader::Fabric));
        assert_eq!(bare.unwrap().name, "bare minor");

        let patched =
            pick_version_metadata(&list(), 26, Some((1, Some(2))), Some(GameLoader::Fabric));
        assert_eq!(patched.unwrap().name, "patched");
    }

    #[test]
    fn unknown_patch_falls_back_to_its_minor_line() {
        let hit = pick_version_metadata(&list(), 26, Some((1, Some(9))), Some(GameLoader::Fabric));
        assert_eq!(hit.unwrap().name, "bare minor");
    }

    #[test]
    fn unknown_minor_falls_back_to_major() {
        let hit = pick_version_metadata(&list(), 26, Some((7, None)), Some(GameLoader::Fabric));
        assert_eq!(hit.unwrap().name, "major");
    }
}
