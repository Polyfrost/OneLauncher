use std::collections::{HashMap, HashSet};

use freya::prelude::*;
use oneclient_content::packages::ProviderId;

use crate::components::ScrollArea;
use crate::hooks::{
    ClusterBundles, onboarding_bundles_items, package_meta_batch, query_error, query_is_loading,
    use_onboarding_bundles, use_onboarding_selection, use_package_meta_batch,
};
use crate::i18n::{t, tf};

type MetaMap = HashMap<String, oneclient_content::packages::CachedPackageMeta>;
use crate::routes::Route;
use crate::theme::{colors, rounded, text_size};
use crate::ui::{border_all_color, fixed_grid};
use crate::view::onboarding::{
    bundle_taken, onboarding_nav, onboarding_slide, pkg_key, step_heading,
};

mod row;
use row::{CARD_GRID_H, GRID_GAP, OnboardingModCard, empty_hint};

const MOD_GRID_COLS: usize = 3;

#[derive(Clone)]
struct OptionalMod {
    package_id: String,
    metadata_id: String,
    provider: ProviderId,
    github_hosted: bool,
    fallback_name: String,
    size: u64,
    /// `(cluster_id, bundle_name)` pairs this mod is available from
    locations: Vec<(i64, String)>,
}

impl OptionalMod {
    fn keys(&self) -> Vec<String> {
        self.locations
            .iter()
            .map(|(cluster_id, bundle)| pkg_key(*cluster_id, bundle, &self.package_id))
            .collect()
    }

    fn selected(&self, selected: &HashSet<String>) -> bool {
        self.keys().iter().any(|k| selected.contains(k))
    }
}

fn optional_mods(clusters: &[ClusterBundles], selected: &HashSet<String>) -> Vec<OptionalMod> {
    let mut order: Vec<String> = Vec::new();
    let mut map: HashMap<String, OptionalMod> = HashMap::new();

    for cb in clusters {
        for archive in &cb.archives {
            if !bundle_taken(cb.cluster.id, archive, selected) {
                continue;
            }
            for file in archive
                .manifest
                .files
                .iter()
                .filter(|f| f.is_optional_offer())
            {
                let package_id = file.kind.package_id();
                let entry = map.entry(package_id.clone()).or_insert_with(|| {
                    order.push(package_id.clone());
                    OptionalMod {
                        package_id: package_id.clone(),
                        metadata_id: file.kind.metadata_id(),
                        provider: file.kind.metadata_provider(),
                        github_hosted: file.is_github_hosted(),
                        fallback_name: file.display_name(),
                        size: file.size,
                        locations: Vec::new(),
                    }
                });
                entry
                    .locations
                    .push((cb.cluster.id, archive.manifest.name.clone()));
            }
        }
    }

    order.into_iter().filter_map(|n| map.remove(&n)).collect()
}

fn toggle_optional_mod(
    mut selected: State<HashSet<String>>,
    mut user_touched: State<bool>,
    entry: &OptionalMod,
) {
    let mut set = selected.peek().clone();
    let on = entry.selected(&set);
    for key in entry.keys() {
        if on {
            set.remove(&key);
        } else {
            set.insert(key);
        }
    }
    user_touched.set(true);
    selected.set(set);
}

#[derive(PartialEq)]
pub struct OnboardingBundles;

impl Component for OnboardingBundles {
    fn render(&self) -> impl IntoElement {
        let bundles_query = use_onboarding_bundles();
        let selection = use_onboarding_selection();
        let selected = selection.selected;
        let user_touched = selection.user_touched;

        let clusters = onboarding_bundles_items(&bundles_query).unwrap_or_default();
        let bundles_loaded = onboarding_bundles_items(&bundles_query).is_some();
        let catalog_msg = match query_error(&bundles_query) {
            Some(err) => tf("oneclient.onboarding.bundles.load_failed", &[&err]),
            None if query_is_loading(&bundles_query) => {
                t("oneclient.onboarding.bundles.loading").to_string()
            }
            None => t("oneclient.onboarding.bundles.extras_empty").to_string(),
        };

        let selected_set = selected.read().clone();
        let extras = optional_mods(&clusters, &selected_set);

        // Hooks so they run every render regardless of whether there are extras yet
        let (mr_ids, cf_ids, local_ids) = collect_ids(&extras);
        let mr_meta = package_meta_batch(&use_package_meta_batch(ProviderId::Modrinth, mr_ids));
        let cf_meta = package_meta_batch(&use_package_meta_batch(ProviderId::CurseForge, cf_ids));
        let local_meta = package_meta_batch(&use_package_meta_batch(ProviderId::Local, local_ids));

        let mut sections: Vec<Element> = Vec::new();

        if !extras.is_empty() {
            sections.push(extras_section(
                &extras,
                &selected_set,
                selected,
                user_touched,
                &mr_meta,
                &cf_meta,
                &local_meta,
            ));
        }

        let nothing_to_ask = extras.is_empty();

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .content(Content::Flex)
            .child(onboarding_slide(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .height(Size::fill())
                    .content(Content::Flex)
                    .padding(Gaps::new(40., 80., 12., 80.))
                    .spacing(16.)
                    .child(step_heading(
                        t("oneclient.onboarding.bundles.title"),
                        t("oneclient.onboarding.bundles.subtitle"),
                    ))
                    .child(
                        rect()
                            .vertical()
                            .width(Size::fill())
                            .height(Size::flex(1.0))
                            .content(Content::Flex)
                            .corner_radius(rounded(12.))
                            .background(colors::page_elevated())
                            .border(border_all_color(1., colors::component_border()))
                            .padding(Gaps::new_all(12.))
                            .child(
                                ScrollArea::new()
                                    .width(Size::fill())
                                    .height(Size::flex(1.0))
                                    .spacing(20.)
                                    .children(sections),
                            )
                            .maybe_child(nothing_to_ask.then(|| empty_hint(&catalog_msg))),
                    ),
            ))
            .child(onboarding_nav(
                Some(Route::OnboardingBundlePicker {}),
                Route::OnboardingPreferences {},
                bundles_loaded,
            ))
    }
}

fn extras_section(
    extras: &[OptionalMod],
    selected_set: &HashSet<String>,
    selected: State<HashSet<String>>,
    user_touched: State<bool>,
    mr_meta: &MetaMap,
    cf_meta: &MetaMap,
    local_meta: &MetaMap,
) -> Element {
    let cards: Vec<Element> = extras
        .iter()
        .map(|entry| {
            let meta = match entry.provider {
                ProviderId::Modrinth => mr_meta.get(&entry.metadata_id),
                ProviderId::CurseForge => cf_meta.get(&entry.metadata_id),
                ProviderId::Local => local_meta.get(&entry.metadata_id),
            };
            let name = meta
                .map(|m| m.name.clone())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| entry.fallback_name.clone());
            let author = meta.map(|m| m.author.clone()).unwrap_or_default();
            let description = meta.map(|m| m.summary.clone()).unwrap_or_default();
            let icon_url = meta.and_then(|m| m.icon_url.clone());
            let toggle_entry = entry.clone();

            OnboardingModCard {
                provider: entry.provider,
                github_hosted: entry.github_hosted,
                name,
                author,
                description,
                icon_url,
                size: entry.size,
                enabled: entry.selected(selected_set),
                on_toggle: (move |()| toggle_optional_mod(selected, user_touched, &toggle_entry))
                    .into(),
            }
            .into_element()
        })
        .collect();

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .child(
            label()
                .text(t("oneclient.onboarding.bundles.extras"))
                .font_size(text_size(15.))
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        )
        .child(fixed_grid(cards, MOD_GRID_COLS, CARD_GRID_H, GRID_GAP))
        .into_element()
}

fn collect_ids(extras: &[OptionalMod]) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut mr = Vec::new();
    let mut cf = Vec::new();
    let mut local = Vec::new();
    for entry in extras {
        match entry.provider {
            ProviderId::Modrinth => mr.push(entry.metadata_id.clone()),
            ProviderId::CurseForge => cf.push(entry.metadata_id.clone()),
            ProviderId::Local => local.push(entry.metadata_id.clone()),
        }
    }
    (mr, cf, local)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::onboarding::test_support::{archive, cluster, file};

    #[test]
    fn only_picked_bundles_offer_their_extras() {
        let items = vec![ClusterBundles {
            cluster: cluster(1),
            archives: vec![
                archive(
                    "PvP",
                    false,
                    vec![file("pvp-a", true, false), file("pvp-opt", false, false)],
                ),
                archive(
                    "SkyBlock",
                    false,
                    vec![file("sb-a", true, false), file("sb-opt", false, false)],
                ),
            ],
        }];
        let selected = HashSet::from([pkg_key(1, "OneClient 1.21.11 Fabric [PvP]", "pvp-a")]);

        let offered: Vec<String> = optional_mods(&items, &selected)
            .into_iter()
            .map(|m| m.package_id)
            .collect();

        assert_eq!(offered.len(), 1);
        assert!(offered[0].contains("pvp-opt"));
    }
}
