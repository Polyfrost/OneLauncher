use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::{BundleArchive, BundleFile, ImportTarget, MigrationSource, SentryExclusion};
use oneclient_db::models::OverrideType;

use crate::hooks::{
    Actions, ClusterBundles, invalidate_cluster_queries, migration_detection,
    onboarding_bundles_items, try_default_account, use_current_account, use_dispatch,
    use_migration, use_onboarding_bundles, use_onboarding_selection, use_settings_snapshot,
};
use crate::routes::Route;
use crate::view::onboarding::{matching_new_cluster_id, pkg_key};

mod view;
use view::{SummaryView, summary_view};

#[derive(Clone, PartialEq)]
struct ClusterPlan {
    cluster_id: i64,
    /// `(bundle_name, package_id, override)` for every file whose fate differs from the manifest default
    overrides: Vec<(String, String, OverrideType)>,
}

#[derive(PartialEq)]
pub struct OnboardingSummary;

impl Component for OnboardingSummary {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let bundles_query = use_onboarding_bundles();
        let migration_query = use_migration();
        let selection = use_onboarding_selection();
        let settings = use_settings_snapshot().settings;
        let account_query = use_current_account();

        let selected_state = selection.selected;
        let mut attempted = use_state(|| false);
        let mut finishing = use_state(|| false);
        let mut failure = use_state(|| None::<String>);

        let items = onboarding_bundles_items(&bundles_query).unwrap_or_default();
        let selected = selected_state.read().clone();
        let language = selection.language.read().clone();
        let reduce_motion = *selection.reduce_motion.read();
        let account_name = try_default_account(&account_query)
            .map(|account| account.username.clone())
            .unwrap_or_else(|| "Not signed in".to_string());

        let import_dispatch = dispatch.clone();
        let import_folder = selection.import_folder;
        let import_dedicated = selection.import_dedicated;
        let import_detection = migration_detection(&migration_query);
        let import_items = items.clone();

        let migration_summary = import_detection.as_ref().map(|detection| {
            let source_name = detection.source.display_name().to_string();
            let is_vanilla = detection.source == MigrationSource::Vanilla;
            match import_folder.read().clone() {
                Some(folder) => {
                    let dedicated = !is_vanilla
                        && *import_dedicated.read()
                        && detection
                            .instances
                            .iter()
                            .find(|c| c.folder_name == folder)
                            .and_then(|inst| matching_new_cluster_id(inst, &items))
                            .is_some();
                    let target = if dedicated {
                        "This version only"
                    } else {
                        "Shared game directory"
                    };
                    let files = if is_vanilla {
                        "Your Minecraft folder".to_string()
                    } else {
                        folder
                    };
                    (source_name, files, target.to_string())
                }
                None => (source_name, "No files imported".to_string(), String::new()),
            }
        });

        let finish_items = items.clone();
        let finish_dispatch = dispatch.clone();
        let continue_items = items.clone();
        let continue_dispatch = dispatch;

        summary_view(
            SummaryView {
                items: &items,
                selected: &selected,
                language: &language,
                reduce_motion,
                parallax: settings.dynamic_background_enabled,
                account_name,
                migration: migration_summary,
                finishing: *finishing.read(),
                attempted: *attempted.read(),
                failure: failure.read().clone(),
            },
            move |_| {
                if *finishing.peek() {
                    return;
                }

                if !*attempted.peek() {
                    if let (Some(detection), Some(folder)) =
                        (import_detection.as_ref(), import_folder.peek().clone())
                    {
                        let target = if detection.source != MigrationSource::Vanilla
                            && *import_dedicated.peek()
                        {
                            detection
                                .instances
                                .iter()
                                .find(|c| c.folder_name == folder)
                                .and_then(|inst| matching_new_cluster_id(inst, &import_items))
                                .map(|new_cluster_id| ImportTarget::Dedicated { new_cluster_id })
                                .unwrap_or(ImportTarget::Shared)
                        } else {
                            ImportTarget::Shared
                        };
                        import_dispatch.import_launcher(detection.source, folder, target);
                    }
                    attempted.set(true);
                }

                failure.set(None);
                finishing.set(true);
                finish_setup(
                    build_plans(&finish_items, &selected_state.peek().clone()),
                    seen_versions(&finish_items),
                    finish_dispatch.clone(),
                    finishing,
                    failure,
                );
            },
            move |_| {
                if *finishing.peek() {
                    return;
                }
                finishing.set(true);
                // Leaves the manifest defaults in place the picks are already lost
                finish_setup(
                    Vec::new(),
                    seen_versions(&continue_items),
                    continue_dispatch.clone(),
                    finishing,
                    failure,
                );
            },
        )
        .into_element()
    }
}

fn build_plans(
    items: &[ClusterBundles],
    selected: &std::collections::HashSet<String>,
) -> Vec<ClusterPlan> {
    items
        .iter()
        .map(|cb| {
            let kept = kept_package_ids(cb.cluster.id, &cb.archives, selected);
            let mut overrides = Vec::new();
            for archive in &cb.archives {
                overrides.extend(archive_overrides(cb.cluster.id, archive, selected, &kept));
            }
            ClusterPlan {
                cluster_id: cb.cluster.id,
                overrides,
            }
        })
        .collect()
}

fn seen_versions(items: &[ClusterBundles]) -> Vec<String> {
    let mut versions: Vec<String> = items
        .iter()
        .map(|cb| cb.cluster.mc_version.clone())
        .collect();

    versions.sort();
    versions.dedup();
    versions
}

/// True once anything visible in the bundle was taken including one opted-in extra that would otherwise lose its dependencies
fn bundle_taken(
    cluster_id: i64,
    archive: &BundleArchive,
    selected: &std::collections::HashSet<String>,
) -> bool {
    archive.manifest.files.iter().any(|file| {
        !file.hidden
            && selected.contains(&pkg_key(
                cluster_id,
                &archive.manifest.name,
                &file.kind.package_id(),
            ))
    })
}

/// Bundles overlap deciding each archive alone recorded a removal for a package an accepted bundle was about to install
fn kept_package_ids(
    cluster_id: i64,
    archives: &[BundleArchive],
    selected: &std::collections::HashSet<String>,
) -> std::collections::HashSet<String> {
    let mut kept = std::collections::HashSet::new();

    for archive in archives {
        let takes_hidden = bundle_taken(cluster_id, archive, selected);
        for file in &archive.manifest.files {
            let package_id = file.kind.package_id();
            let wanted = if file.hidden {
                takes_hidden
            } else {
                selected.contains(&pkg_key(cluster_id, &archive.manifest.name, &package_id))
            };
            if wanted {
                kept.insert(package_id);
            }
        }
    }

    kept
}

fn archive_overrides(
    cluster_id: i64,
    archive: &BundleArchive,
    selected: &std::collections::HashSet<String>,
    kept_elsewhere: &std::collections::HashSet<String>,
) -> Vec<(String, String, OverrideType)> {
    let bundle_name = &archive.manifest.name;
    let wants = |file: &BundleFile| {
        selected.contains(&pkg_key(cluster_id, bundle_name, &file.kind.package_id()))
    };

    // Hidden files follow the bundle kept if anything from it was taken dropped if not
    let takes_hidden = bundle_taken(cluster_id, archive, selected);

    let mut overrides = Vec::new();
    for file in &archive.manifest.files {
        let wanted = if file.hidden {
            takes_hidden
        } else {
            wants(file)
        };

        let override_type = match (wanted, file.enabled) {
            // Matches the manifest default nothing to record
            (true, true) | (false, false) => continue,
            // Opting into a mod the bundle ships turned off
            (true, false) => OverrideType::Enabled,
            // Only really declined if no bundle the user took ships the same package
            (false, true) => {
                if kept_elsewhere.contains(&file.kind.package_id()) {
                    continue;
                }
                OverrideType::Removed
            }
        };

        overrides.push((bundle_name.clone(), file.kind.package_id(), override_type));
    }
    overrides
}

/// The bundle picks are written nowhere else so a failure has to stop the run rather than drop them silently
fn finish_setup(
    plans: Vec<ClusterPlan>,
    versions: Vec<String>,
    dispatch: Actions,
    mut finishing: State<bool>,
    mut failure: State<Option<String>>,
) {
    spawn(async move {
        for plan in plans.iter().filter(|p| !p.overrides.is_empty()) {
            if let Err(err) = apply_overrides(plan).await {
                // Expected failures (a busy or unwritable database) are not crashes warn! is a breadcrumb error! reports to Sentry
                if err.is_sentry_excluded() {
                    tracing::warn!(
                        cluster_id = plan.cluster_id,
                        "saving package choices failed (expected): {err}"
                    );
                } else {
                    tracing::error!(
                        cluster_id = plan.cluster_id,
                        "saving package choices failed: {err}"
                    );
                }
                failure.set(Some(err.to_string()));
                finishing.set(false);
                return;
            }
        }

        invalidate_cluster_queries().await;

        for version in versions {
            dispatch.record_seen_version(version);
        }

        dispatch.mark_onboarding_seen();
        let _ = RouterContext::get().replace(Route::Home {});
    });
}

async fn apply_overrides(plan: &ClusterPlan) -> oneclient_core::LauncherResult<()> {
    let state = crate::launcher::state()?;
    oneclient_core::set_bundle_package_overrides(
        plan.cluster_id,
        &plan.overrides,
        &state.services.content(),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::onboarding::test_support::{archive, cluster, file};
    use std::collections::HashSet;

    fn skyblock() -> BundleArchive {
        archive(
            "SkyBlock",
            false,
            vec![
                file("skyblock-main", true, false),
                file("skycubed", false, false),
                file("sb-dep", true, true),
            ],
        )
    }

    fn items(archives: Vec<BundleArchive>) -> Vec<ClusterBundles> {
        vec![ClusterBundles {
            cluster: cluster(1),
            archives,
        }]
    }

    fn keys(archive: &BundleArchive, pids: &[&str]) -> HashSet<String> {
        pids.iter()
            .map(|p| pkg_key(1, &archive.manifest.name, p))
            .collect()
    }

    #[test]
    fn declining_a_bundle_removes_its_visible_and_hidden_files() {
        let sb = skyblock();
        let plans = build_plans(&items(vec![sb.clone()]), &HashSet::new());

        let ov = &plans[0].overrides;
        // The hidden dependency must be dropped too or it installs anyway
        assert!(ov.contains(&(
            sb.manifest.name.clone(),
            "sb-dep".to_string(),
            OverrideType::Removed
        )));
        assert!(ov.contains(&(
            sb.manifest.name.clone(),
            "skyblock-main".to_string(),
            OverrideType::Removed
        )));
        assert!(!ov.iter().any(|(_, pid, _)| pid == "skycubed"));
    }

    #[test]
    fn accepting_a_bundle_records_nothing_for_its_defaults() {
        let sb = skyblock();
        let selected = keys(&sb, &["skyblock-main"]);
        let plans = build_plans(&items(vec![sb.clone()]), &selected);

        assert!(plans[0].overrides.is_empty());
    }

    #[test]
    fn opting_into_an_extra_writes_enabled() {
        let sb = skyblock();
        let selected = keys(&sb, &["skyblock-main", "skycubed"]);
        let plans = build_plans(&items(vec![sb.clone()]), &selected);

        assert_eq!(
            plans[0].overrides,
            vec![(
                sb.manifest.name.clone(),
                "skycubed".to_string(),
                OverrideType::Enabled
            )]
        );
    }

    #[test]
    fn an_extra_alone_still_keeps_hidden_dependencies() {
        let sb = skyblock();
        let selected = keys(&sb, &["skycubed"]);
        let plans = build_plans(&items(vec![sb.clone()]), &selected);
        let ov = &plans[0].overrides;

        assert!(ov.contains(&(
            sb.manifest.name.clone(),
            "skycubed".to_string(),
            OverrideType::Enabled
        )));
        assert!(!ov.iter().any(|(_, pid, _)| pid == "sb-dep"));
        assert!(ov.contains(&(
            sb.manifest.name.clone(),
            "skyblock-main".to_string(),
            OverrideType::Removed
        )));
    }

    #[test]
    fn rejections_are_removed_never_disabled() {
        let plans = build_plans(&items(vec![skyblock()]), &HashSet::new());

        assert!(
            !plans[0]
                .overrides
                .iter()
                .any(|(_, _, ty)| *ty == OverrideType::Disabled)
        );
    }

    #[test]
    fn per_cluster_choices_are_independent() {
        let sb = skyblock();
        let both = vec![
            ClusterBundles {
                cluster: cluster(1),
                archives: vec![sb.clone()],
            },
            ClusterBundles {
                cluster: cluster(2),
                archives: vec![sb.clone()],
            },
        ];
        let selected: HashSet<String> = [pkg_key(1, &sb.manifest.name, "skyblock-main")].into();
        let plans = build_plans(&both, &selected);

        assert!(plans[0].overrides.is_empty());
        assert!(
            plans[1]
                .overrides
                .iter()
                .any(|(_, pid, ty)| pid == "skyblock-main" && *ty == OverrideType::Removed)
        );
    }

    #[test]
    fn every_version_is_recorded_once() {
        let mut second = cluster(2);
        second.mc_version = "1.20.1".to_string();
        let items = vec![
            ClusterBundles {
                cluster: cluster(1),
                archives: Vec::new(),
            },
            ClusterBundles {
                cluster: second,
                archives: Vec::new(),
            },
            ClusterBundles {
                cluster: cluster(3),
                archives: Vec::new(),
            },
        ];

        assert_eq!(seen_versions(&items), vec!["1.20.1", "1.21.11"]);
    }
}
