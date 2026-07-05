use freya::query::QueryStateData;
use oneclient_core::packages::domain::GameLoader;
use oneclient_core::VersionMetadata;

use super::use_versions;

pub fn pick_version_metadata(
    list: &[VersionMetadata],
    major: u32,
    minor: Option<u32>,
    loader: Option<GameLoader>,
) -> Option<VersionMetadata> {
    let loader_matches = |m: &VersionMetadata| match (&m.loader, loader) {
        (Some(s), Some(l)) => l.to_string().eq_ignore_ascii_case(s),
        _ => false,
    };

    if let Some(minor) = minor {
        if let Some(hit) = list.iter().find(|m| {
            m.major_version == major && m.minor_version == Some(minor) && loader_matches(m)
        }) {
            return Some(hit.clone());
        }
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
    minor: Option<u32>,
    loader: Option<GameLoader>,
) -> Option<VersionMetadata> {
    let versions_query = use_versions();

    let major = major?;
    let reader = versions_query.read();
    let state = reader.state();
    let list = match &*state {
        QueryStateData::Settled { res: Ok(list), .. } => list,
        QueryStateData::Loading { res: Some(Ok(list)) } => list,
        _ => return None,
    };

    pick_version_metadata(list, major, minor, loader)
}
