use std::collections::HashSet;

use oneclient_core::{BundleArchive, BundleFile};

use crate::hooks::ClusterBundles;

pub fn pkg_key(cluster_id: i64, bundle_name: &str, package_id: &str) -> String {
    format!("{cluster_id}|{bundle_name}|{package_id}")
}

pub fn is_default_bundle(archive: &BundleArchive) -> bool {
    archive.manifest.enabled
}

pub fn is_default_file(file: &BundleFile) -> bool {
    file.enabled && !file.hidden
}

pub fn is_optional_file(file: &BundleFile) -> bool {
    !file.enabled && !file.hidden
}

pub fn default_selection(
    items: &[ClusterBundles],
    opted_in_categories: Option<&[String]>,
) -> HashSet<String> {
    let mut selected = HashSet::new();

    for cb in items {
        for archive in &cb.archives {
            if !is_default_bundle(archive) && !category_opted_in(archive, opted_in_categories) {
                continue;
            }
            for file in &archive.manifest.files {
                if is_default_file(file) {
                    selected.insert(pkg_key(
                        cb.cluster.id,
                        &archive.manifest.name,
                        &file.kind.package_id(),
                    ));
                }
            }
        }
    }

    selected
}

fn category_opted_in(archive: &BundleArchive, opted_in: Option<&[String]>) -> bool {
    let Some(categories) = opted_in else {
        return false;
    };
    let category = archive.manifest.category.trim();
    if category.is_empty() {
        return false;
    }
    categories.iter().any(|c| c.eq_ignore_ascii_case(category))
}

pub fn archive_selected(
    cluster_id: i64,
    archive: &BundleArchive,
    selected: &HashSet<String>,
) -> bool {
    let mut any = false;
    for file in archive.manifest.files.iter().filter(|f| is_default_file(f)) {
        any = true;
        if !selected.contains(&pkg_key(
            cluster_id,
            &archive.manifest.name,
            &file.kind.package_id(),
        )) {
            return false;
        }
    }
    any
}

pub fn set_archive_selected(
    cluster_id: i64,
    archive: &BundleArchive,
    on: bool,
    selected: &mut HashSet<String>,
) {
    for file in archive.manifest.files.iter().filter(|f| is_default_file(f)) {
        let key = pkg_key(
            cluster_id,
            &archive.manifest.name,
            &file.kind.package_id(),
        );
        if on {
            selected.insert(key);
        } else {
            selected.remove(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::onboarding::test_support::{archive, cluster, file};

    fn catalog() -> Vec<ClusterBundles> {
        vec![ClusterBundles {
            cluster: cluster(1),
            archives: vec![
                archive("QoL", true, vec![file("qol-a", true, false)]),
                archive(
                    "Utility",
                    true,
                    vec![file("util-a", true, false), file("util-opt", false, false)],
                ),
                archive("PvP", false, vec![file("pvp-a", true, false)]),
                archive("SkyBlock", false, vec![file("sb-a", true, false)]),
            ],
        }]
    }

    #[test]
    fn opt_in_bundles_are_not_selected_by_default() {
        let selected = default_selection(&catalog(), None);

        assert!(!selected.iter().any(|k| k.contains("[PvP]")));
        assert!(!selected.iter().any(|k| k.contains("[SkyBlock]")));
    }

    #[test]
    fn default_bundles_are_selected_by_default() {
        let selected = default_selection(&catalog(), None);

        assert!(selected.contains(&pkg_key(1, "OneClient 1.21.11 Fabric [QoL]", "qol-a")));
        assert!(selected.contains(&pkg_key(1, "OneClient 1.21.11 Fabric [Utility]", "util-a")));
    }

    #[test]
    fn files_the_bundle_ships_off_are_not_selected_by_default() {
        let selected = default_selection(&catalog(), None);

        assert!(!selected.contains(&pkg_key(
            1,
            "OneClient 1.21.11 Fabric [Utility]",
            "util-opt"
        )));
    }

    #[test]
    fn migrated_categories_opt_into_their_bundles_without_losing_defaults() {
        let cats = vec!["SkyBlock".to_string()];
        let selected = default_selection(&catalog(), Some(&cats));

        assert!(selected.contains(&pkg_key(1, "OneClient 1.21.11 Fabric [SkyBlock]", "sb-a")));
        assert!(selected.contains(&pkg_key(1, "OneClient 1.21.11 Fabric [QoL]", "qol-a")));
        assert!(!selected.iter().any(|k| k.contains("[PvP]")));
    }

    #[test]
    fn migrated_category_match_ignores_case() {
        let cats = vec!["skyblock".to_string()];
        let selected = default_selection(&catalog(), Some(&cats));

        assert!(selected.contains(&pkg_key(1, "OneClient 1.21.11 Fabric [SkyBlock]", "sb-a")));
    }

    #[test]
    fn hidden_files_are_never_selected() {
        let items = vec![ClusterBundles {
            cluster: cluster(1),
            archives: vec![archive(
                "QoL",
                true,
                vec![file("shown", true, false), file("dep", true, true)],
            )],
        }];
        let selected = default_selection(&items, None);

        assert!(selected.contains(&pkg_key(1, "OneClient 1.21.11 Fabric [QoL]", "shown")));
        assert!(!selected.contains(&pkg_key(1, "OneClient 1.21.11 Fabric [QoL]", "dep")));
    }

    #[test]
    fn selection_is_per_cluster() {
        let items = vec![
            ClusterBundles {
                cluster: cluster(1),
                archives: vec![archive("SkyBlock", false, vec![file("sb-a", true, false)])],
            },
            ClusterBundles {
                cluster: cluster(2),
                archives: vec![archive("SkyBlock", false, vec![file("sb-a", true, false)])],
            },
        ];
        let mut selected = HashSet::new();
        let sb = &items[0].archives[0];
        set_archive_selected(1, sb, true, &mut selected);

        assert!(archive_selected(1, sb, &selected));
        assert!(!archive_selected(2, &items[1].archives[0], &selected));
    }

    #[test]
    fn set_archive_selected_round_trips() {
        let items = catalog();
        let pvp = &items[0].archives[2];
        let mut selected = HashSet::new();

        set_archive_selected(1, pvp, true, &mut selected);
        assert!(archive_selected(1, pvp, &selected));

        set_archive_selected(1, pvp, false, &mut selected);
        assert!(!archive_selected(1, pvp, &selected));
        assert!(selected.is_empty());
    }

    #[test]
    fn archive_with_no_default_files_is_not_selected() {
        let a = archive("Odd", true, vec![file("opt", false, false)]);
        assert!(!archive_selected(1, &a, &HashSet::new()));
    }
}
