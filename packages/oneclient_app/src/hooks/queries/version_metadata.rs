use freya::query::QueryStateData;
use oneclient_core::VersionKey;
use oneclient_core::VersionMetadata;
use oneclient_core::packages::domain::GameLoader;

use super::use_versions;

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
    let list = match &*state {
        QueryStateData::Settled { res: Ok(list), .. } => list,
        QueryStateData::Loading {
            res: Some(Ok(list)),
        } => list,
        _ => return None,
    };

    pick_version_metadata(list, major, key, loader)
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
