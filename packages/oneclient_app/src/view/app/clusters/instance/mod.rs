use std::collections::HashMap;

use oneclient_content::packages::{CachedPackageMeta, ProviderId};
use oneclient_core::{BundleArchive, BundleFile, BundleFileKind};

mod cards;
mod copy;
mod create;
mod data;
mod details;
mod edit;
mod model;
mod rail;
mod shell;
mod steps;

pub use create::CreateInstanceModal;
pub use edit::{EditInstanceModal, InstanceFacts};

pub fn file_provider(file: &BundleFile) -> ProviderId {
    match &file.kind {
        BundleFileKind::Managed { provider, .. } => *provider,
        BundleFileKind::External(_) => ProviderId::Local,
    }
}

pub fn tidy_file_name(raw: &str) -> String {
    let stem = raw
        .rsplit_once('.')
        .filter(|(head, ext)| !head.is_empty() && ext.len() <= 8)
        .map_or(raw, |(head, _)| head);

    let bytes = stem.as_bytes();
    let cut = bytes.iter().enumerate().position(|(index, byte)| {
        matches!(byte, b'-' | b'_' | b'+') && bytes.get(index + 1).is_some_and(u8::is_ascii_digit)
    });

    match cut {
        Some(0) | None => stem.to_string(),
        Some(cut) => stem[..cut].to_string(),
    }
}

pub fn package_names(
    archives: &[BundleArchive],
    modrinth: &HashMap<String, CachedPackageMeta>,
    curseforge: &HashMap<String, CachedPackageMeta>,
) -> HashMap<String, String> {
    let mut names = HashMap::new();

    for file in archives.iter().flat_map(|archive| &archive.manifest.files) {
        let package_id = file.kind.package_id();
        let meta = match file_provider(file) {
            ProviderId::Modrinth => modrinth.get(&package_id),
            ProviderId::CurseForge => curseforge.get(&package_id),
            ProviderId::Local => None,
        };

        let name = meta
            .map(|meta| meta.name.clone())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| tidy_file_name(&file.display_name()));

        names.insert(package_id, name);
    }

    names
}

#[cfg(test)]
mod tests {
    use super::tidy_file_name;

    #[test]
    fn a_versioned_jar_keeps_only_its_name() {
        assert_eq!(
            tidy_file_name("sodium-fabric-0.5.11+mc1.20.1.jar"),
            "sodium-fabric"
        );
        assert_eq!(tidy_file_name("EvergreenHUD-3.0.0.jar"), "EvergreenHUD");
        assert_eq!(tidy_file_name("Sodium-13.303x012"), "Sodium");
    }

    #[test]
    fn a_name_without_a_version_survives_whole() {
        assert_eq!(tidy_file_name("OneConfig.jar"), "OneConfig");
        assert_eq!(tidy_file_name("PolyBlur"), "PolyBlur");
    }

    #[test]
    fn a_leading_separator_is_not_a_version_cut() {
        assert_eq!(tidy_file_name("-1abc"), "-1abc");
    }

    #[test]
    fn a_dotted_name_is_not_mistaken_for_an_extension() {
        assert_eq!(
            tidy_file_name("com.example.longextension"),
            "com.example.longextension"
        );
    }
}
