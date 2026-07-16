use std::collections::{HashMap, HashSet};

use freya::prelude::*;
use freya::query::QueryStateData;
use oneclient_core::clusters::Cluster;
use oneclient_core::packages::ProviderId;
use oneclient_core::{BundleArchive, BundleFile, BundleFileKind};

use crate::components::ScrollArea;
use crate::hooks::{
    ClusterBundles, onboarding_bundles_items, package_meta_batch, use_onboarding_bundles,
    use_onboarding_selection, use_package_meta_batch,
};

type MetaMap = HashMap<String, oneclient_core::packages::CachedPackageMeta>;
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::view::onboarding::{
    archive_selected, choice_row_sized, is_default_bundle, is_optional_file, onboarding_nav,
    onboarding_slide, pkg_key, predownload_toggle_row, set_archive_selected, step_heading,
    version_chip,
};

mod row;
use row::{CARD_GRID_H, GRID_GAP, OnboardingModCard, empty_hint};

const CHIPS_PER_ROW: usize = 4;

const MOD_GRID_COLS: usize = 3;

const ANSWER_H: f32 = 48.;

fn file_provider(file: &BundleFile) -> ProviderId {
    match &file.kind {
        BundleFileKind::Managed { provider, .. } => *provider,
        BundleFileKind::External(_) => ProviderId::Local,
    }
}

fn bundle_display_name(archive: &BundleArchive) -> String {
    let category = archive.manifest.category.trim();
    if category.is_empty() {
        archive.manifest.name.clone()
    } else {
        category.to_string()
    }
}

#[derive(Clone)]
struct OptInBundle {
    display_name: String,
    members: Vec<(Cluster, BundleArchive)>,
}

impl OptInBundle {
    fn chosen(&self, selected: &HashSet<String>) -> Vec<i64> {
        self.members
            .iter()
            .filter(|(cluster, archive)| archive_selected(cluster.id, archive, selected))
            .map(|(cluster, _)| cluster.id)
            .collect()
    }

    fn wanted(&self, selected: &HashSet<String>) -> bool {
        !self.chosen(selected).is_empty()
    }

    fn mod_count(&self) -> usize {
        self.members
            .first()
            .map(|(_, a)| {
                a.manifest
                    .files
                    .iter()
                    .filter(|f| f.enabled && !f.hidden)
                    .count()
            })
            .unwrap_or(0)
    }
}

fn opt_in_bundles(clusters: &[ClusterBundles]) -> Vec<OptInBundle> {
    let mut order: Vec<String> = Vec::new();
    let mut map: HashMap<String, OptInBundle> = HashMap::new();

    for cb in clusters {
        for archive in &cb.archives {
            if is_default_bundle(archive) {
                continue;
            }
            let name = bundle_display_name(archive);
            let entry = map.entry(name.clone()).or_insert_with(|| {
                order.push(name.clone());
                OptInBundle {
                    display_name: name.clone(),
                    members: Vec::new(),
                }
            });
            entry.members.push((cb.cluster.clone(), archive.clone()));
        }
    }

    for bundle in map.values_mut() {
        bundle
            .members
            .sort_by(|(a, _), (b, _)| a.mc_version.cmp(&b.mc_version));
    }

    order.into_iter().filter_map(|n| map.remove(&n)).collect()
}

#[derive(Clone)]
struct OptionalMod {
    package_id: String,
    provider: ProviderId,
    fallback_name: String,
    size: u64,
    /// `(cluster_id, bundle_name)` pairs this mod is available from.
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

fn optional_mods(clusters: &[ClusterBundles]) -> Vec<OptionalMod> {
    let mut order: Vec<String> = Vec::new();
    let mut map: HashMap<String, OptionalMod> = HashMap::new();

    for cb in clusters {
        for archive in &cb.archives {
            for file in archive
                .manifest
                .files
                .iter()
                .filter(|f| is_optional_file(f))
            {
                let package_id = file.kind.package_id();
                let entry = map.entry(package_id.clone()).or_insert_with(|| {
                    order.push(package_id.clone());
                    OptionalMod {
                        package_id: package_id.clone(),
                        provider: file_provider(file),
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

fn set_bundle_versions(
    mut selected: State<HashSet<String>>,
    mut user_touched: State<bool>,
    bundle: &OptInBundle,
    clusters: &[i64],
) {
    let mut set = selected.peek().clone();
    for (cluster, archive) in &bundle.members {
        let on = clusters.contains(&cluster.id);
        set_archive_selected(cluster.id, archive, on, &mut set);
    }
    user_touched.set(true);
    selected.set(set);
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
        let predownload = selection.predownload;

        let clusters = onboarding_bundles_items(&bundles_query).unwrap_or_default();
        let bundles_loaded = onboarding_bundles_items(&bundles_query).is_some();
        let catalog_msg = {
            let reader = bundles_query.read();
            match &*reader.state() {
                QueryStateData::Settled { res: Err(err), .. } => {
                    format!("Couldn't load bundles: {err}")
                }
                QueryStateData::Pending | QueryStateData::Loading { res: None } => {
                    "Loading bundles...".to_string()
                }
                _ => "No optional bundles available".to_string(),
            }
        };

        let selected_set = selected.read().clone();
        let opt_ins = opt_in_bundles(&clusters);
        let extras = optional_mods(&clusters);

        // These are hooks, so they run every render regardless of whether there
        // are any extras to show yet.
        let (mr_ids, cf_ids) = collect_ids(&extras);
        let mr_meta = package_meta_batch(&use_package_meta_batch(ProviderId::Modrinth, mr_ids));
        let cf_meta = package_meta_batch(&use_package_meta_batch(ProviderId::CurseForge, cf_ids));

        let mut sections: Vec<Element> = Vec::new();

        for bundle in &opt_ins {
            sections
                .push(opt_in_card(bundle, &selected_set, selected, user_touched).into_element());
        }

        if !extras.is_empty() {
            sections.push(extras_section(
                &extras,
                &selected_set,
                selected,
                user_touched,
                &mr_meta,
                &cf_meta,
            ));
        }

        let nothing_to_ask = opt_ins.is_empty() && extras.is_empty();

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
                        "Mods",
                        "Everything you need is already included. Pick anything extra you want.",
                    ))
                    .child(
                        rect()
                            .vertical()
                            .width(Size::fill())
                            .height(Size::flex(1.0))
                            .content(Content::Flex)
                            .corner_radius(CornerRadius::new_all(12.))
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
                    )
                    .child(predownload_toggle_row(predownload)),
            ))
            .child(onboarding_nav(
                Some(Route::OnboardingAccount {}),
                Route::OnboardingPreferences {},
                bundles_loaded,
            ))
    }
}

fn opt_in_card(
    bundle: &OptInBundle,
    selected_set: &HashSet<String>,
    selected: State<HashSet<String>>,
    user_touched: State<bool>,
) -> impl IntoElement {
    let name = bundle.display_name.clone();
    let wanted = bundle.wanted(selected_set);
    let chosen = bundle.chosen(selected_set);
    let all_ids: Vec<i64> = bundle.members.iter().map(|(c, _)| c.id).collect();

    let yes_bundle = bundle.clone();
    let yes_ids = all_ids.clone();
    let no_bundle = bundle.clone();

    let mut card = rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .child(
            label()
                .text(format!("Do you want to install {name} mods?"))
                .font_size(15.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        )
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .spacing(8.)
                .content(Content::Flex)
                .child(
                    rect()
                        .width(Size::flex(1.0))
                        .height(Size::px(ANSWER_H))
                        .child(choice_row_sized(
                            "Yes",
                            &format!("{} mods", yes_bundle.mod_count()),
                            wanted,
                            Size::fill(),
                            move |()| {
                                if wanted {
                                    return;
                                }
                                set_bundle_versions(selected, user_touched, &yes_bundle, &yes_ids)
                            },
                        )),
                )
                .child(
                    rect()
                        .width(Size::flex(1.0))
                        .height(Size::px(ANSWER_H))
                        .child(choice_row_sized(
                            "No",
                            "",
                            !wanted,
                            Size::fill(),
                            move |()| set_bundle_versions(selected, user_touched, &no_bundle, &[]),
                        )),
                ),
        );

    if wanted {
        let mut rows: Vec<Element> = Vec::new();
        for chunk in bundle.members.chunks(CHIPS_PER_ROW) {
            let mut row = rect().horizontal().spacing(8.);
            for (cluster, _) in chunk {
                let active = chosen.contains(&cluster.id);
                let chip_bundle = bundle.clone();
                let chosen_now = chosen.clone();
                let cluster_id = cluster.id;
                row = row.child(version_chip(&cluster.mc_version, active, move |()| {
                    let mut next = chosen_now.clone();
                    if active {
                        next.retain(|id| *id != cluster_id);
                    } else {
                        next.push(cluster_id);
                    }
                    set_bundle_versions(selected, user_touched, &chip_bundle, &next);
                }));
            }
            rows.push(row.into_element());
        }

        card = card.child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(8.)
                .padding(Gaps::new(4., 0., 0., 0.))
                .child(
                    label()
                        .text(format!("What versions do you play {name} on?"))
                        .font_size(13.)
                        .color(colors::fg_secondary()),
                )
                .child(rect().vertical().spacing(8.).children(rows)),
        );
    }

    card
}

fn extras_section(
    extras: &[OptionalMod],
    selected_set: &HashSet<String>,
    selected: State<HashSet<String>>,
    user_touched: State<bool>,
    mr_meta: &MetaMap,
    cf_meta: &MetaMap,
) -> Element {
    let cards: Vec<Element> = extras
        .iter()
        .map(|entry| {
            let meta = match entry.provider {
                ProviderId::Modrinth => mr_meta.get(&entry.package_id),
                ProviderId::CurseForge => cf_meta.get(&entry.package_id),
                ProviderId::Local => None,
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

    let mut rows: Vec<Element> = Vec::new();
    for (index, chunk) in cards.chunks(MOD_GRID_COLS).enumerate() {
        let mut row = rect()
            .key(index)
            .horizontal()
            .width(Size::fill())
            .height(Size::px(CARD_GRID_H))
            .spacing(GRID_GAP)
            .content(Content::Flex);
        for card in chunk {
            row = row.child(
                rect()
                    .width(Size::flex(1.0))
                    .height(Size::fill())
                    .child(card.clone()),
            );
        }
        for _ in chunk.len()..MOD_GRID_COLS {
            row = row.child(rect().width(Size::flex(1.0)).height(Size::fill()));
        }
        rows.push(row.into_element());
    }

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .child(
            label()
                .text("Try out these additional mods:")
                .font_size(15.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        )
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(GRID_GAP)
                .children(rows),
        )
        .into_element()
}

fn collect_ids(extras: &[OptionalMod]) -> (Vec<String>, Vec<String>) {
    let mut mr = Vec::new();
    let mut cf = Vec::new();
    for entry in extras {
        match entry.provider {
            ProviderId::Modrinth => mr.push(entry.package_id.clone()),
            ProviderId::CurseForge => cf.push(entry.package_id.clone()),
            ProviderId::Local => {}
        }
    }
    (mr, cf)
}
