use std::collections::{HashMap, HashSet};

use freya::prelude::*;
use freya::query::QueryStateData;
use oneclient_core::clusters::Cluster;
use oneclient_core::packages::{ContentType, ProviderId};
use oneclient_core::{BundleArchive, BundleFile, BundleFileKind};

use crate::components::ScrollArea;
use crate::hooks::{
    ClusterBundles, onboarding_bundles_items,
    use_onboarding_bundles, use_onboarding_selection,
};
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::view::onboarding::{
    onboarding_nav, onboarding_slide, predownload_toggle_row, step_heading,
};

mod card;
mod popup;
use card::BundleCard;
use popup::{BundlePopup, empty_hint};

const CARD_BG: Color = Color::from_rgb(26, 34, 41);
const CARD_NAME: Color = Color::from_rgb(213, 219, 255);
const CARD_HEIGHT: f32 = 150.;
const GRID_COLS: usize = 2;

const SECTION_ORDER: [ContentType; 6] = [
    ContentType::Mod,
    ContentType::ResourcePack,
    ContentType::Shader,
    ContentType::DataPack,
    ContentType::World,
    ContentType::Modpack,
];

fn pkg_key(cluster_id: i64, bundle_name: &str, package_id: &str) -> String {
    format!("{cluster_id}|{bundle_name}|{package_id}")
}

fn file_provider(file: &BundleFile) -> ProviderId {
    match &file.kind {
        BundleFileKind::Managed { provider, .. } => *provider,
        BundleFileKind::External(_) => ProviderId::Local,
    }
}

fn section_label(ct: ContentType) -> &'static str {
    match ct {
        ContentType::Mod => "Mods",
        ContentType::ResourcePack => "Resource Packs",
        ContentType::Shader => "Shaders",
        ContentType::DataPack => "Data Packs",
        ContentType::World => "Worlds",
        ContentType::Modpack => "Modpacks",
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
struct UnifiedMember {
    cluster: Cluster,
    archive: BundleArchive,
}

#[derive(Clone)]
struct UnifiedBundle {
    display_name: String,
    members: Vec<UnifiedMember>,
}

impl UnifiedBundle {
    fn mod_count(&self) -> usize {
        self.members
            .first()
            .map(|m| {
                m.archive
                    .manifest
                    .files
                    .iter()
                    .filter(|f| f.enabled && !f.hidden)
                    .count()
            })
            .unwrap_or(0)
    }
}

fn unify_bundles(clusters: &[ClusterBundles]) -> Vec<UnifiedBundle> {
    let mut order: Vec<String> = Vec::new();
    let mut map: HashMap<String, UnifiedBundle> = HashMap::new();
    for cb in clusters {
        for archive in &cb.archives {
            let name = bundle_display_name(archive);
            let entry = map.entry(name.clone()).or_insert_with(|| {
                order.push(name.clone());
                UnifiedBundle {
                    display_name: name.clone(),
                    members: Vec::new(),
                }
            });
            entry.members.push(UnifiedMember {
                cluster: cb.cluster.clone(),
                archive: archive.clone(),
            });
        }
    }
    order.into_iter().filter_map(|n| map.remove(&n)).collect()
}

fn bundle_is_selected(bundle: &UnifiedBundle, selected: &HashSet<String>) -> bool {
    let Some(first) = bundle.members.first() else {
        return false;
    };
    let visible: Vec<&BundleFile> = first
        .archive
        .manifest
        .files
        .iter()
        .filter(|f| f.enabled && !f.hidden)
        .collect();
    !visible.is_empty()
        && visible.iter().all(|f| {
            selected.contains(&pkg_key(
                first.cluster.id,
                &first.archive.manifest.name,
                &f.kind.package_id(),
            ))
        })
}

fn toggle_bundle(mut selected: State<HashSet<String>>, bundle: &UnifiedBundle) {
    let currently = bundle_is_selected(bundle, &selected.peek());
    let mut set = selected.peek().clone();
    for member in &bundle.members {
        for file in &member.archive.manifest.files {
            if !file.enabled {
                continue;
            }
            let key = pkg_key(
                member.cluster.id,
                &member.archive.manifest.name,
                &file.kind.package_id(),
            );
            if currently {
                set.remove(&key);
            } else {
                set.insert(key);
            }
        }
    }
    selected.set(set);
}

fn flip(mut selected: State<HashSet<String>>, key: String) {
    let mut set = selected.peek().clone();
    if !set.remove(&key) {
        set.insert(key);
    }
    selected.set(set);
}

#[derive(PartialEq)]
pub struct OnboardingBundles;

impl Component for OnboardingBundles {
    fn render(&self) -> impl IntoElement {
        let bundles_query = use_onboarding_bundles();
        let selection = use_onboarding_selection();
        let mut selected = selection.selected;
        let mut seeded = selection.seeded;
        let predownload = selection.predownload;
        let mut open = use_state(|| None::<String>);

        use_side_effect(move || {
            if *seeded.peek() {
                return;
            }
            let Some(items) = onboarding_bundles_items(&bundles_query) else {
                return;
            };
            let mut defaults: HashSet<String> = HashSet::new();
            for cb in &items {
                for archive in &cb.archives {
                    for file in &archive.manifest.files {
                        if file.enabled && !file.hidden {
                            defaults.insert(pkg_key(
                                cb.cluster.id,
                                &archive.manifest.name,
                                &file.kind.package_id(),
                            ));
                        }
                    }
                }
            }
            selected.set(defaults);
            seeded.set(true);
        });

        let clusters = onboarding_bundles_items(&bundles_query).unwrap_or_default();
        let catalog_msg = {
            let reader = bundles_query.read();
            match &*reader.state() {
                QueryStateData::Settled { res: Err(err), .. } => {
                    format!("Couldn't load bundles: {err}")
                }
                QueryStateData::Pending | QueryStateData::Loading { res: None } => {
                    "Loading bundles...".to_string()
                }
                _ => "No bundles available".to_string(),
            }
        };

        let bundles = unify_bundles(&clusters);
        let selected_set = selected.read().clone();
        let any_selected = !selected_set.is_empty();

        let mut rows: Vec<Element> = Vec::new();
        for chunk in bundles.chunks(GRID_COLS) {
            let mut row = rect()
                .horizontal()
                .width(Size::fill())
                .height(Size::px(CARD_HEIGHT))
                .spacing(16.)
                .content(Content::Flex);
            for bundle in chunk {
                let is_selected = bundle_is_selected(bundle, &selected_set);
                let name = bundle.display_name.clone();
                let toggle_bundle_clone = bundle.clone();
                row = row.child(
                    rect()
                        .key(&bundle.display_name)
                        .width(Size::flex(1.0))
                        .height(Size::fill())
                        .child(BundleCard {
                            display_name: bundle.display_name.clone(),
                            art_cluster: bundle.members.first().map(|m| m.cluster.clone()),
                            mod_count: bundle.mod_count(),
                            selected: is_selected,
                            on_toggle: (move |()| toggle_bundle(selected, &toggle_bundle_clone))
                                .into(),
                            on_open: (move |()| open.set(Some(name.clone()))).into(),
                        }),
                );
            }
            for _ in chunk.len()..GRID_COLS {
                row = row.child(rect().width(Size::flex(1.0)).height(Size::fill()));
            }
            rows.push(row.into_element());
        }
        let grid_empty = bundles.is_empty();

        let popup = open.read().clone().and_then(|name| {
            bundles
                .iter()
                .find(|b| b.display_name == name)
                .cloned()
                .map(|bundle| {
                    BundlePopup {
                        bundle,
                        selected,
                        open,
                    }
                    .into_element()
                })
        });

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
                        "Bundles",
                        "Choose which bundles to install. Your selection is applied across every available version.",
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
                                    .spacing(16.)
                                    .children(rows),
                            )
                            .maybe_child(grid_empty.then(|| empty_hint(&catalog_msg))),
                    )
                    .child(predownload_toggle_row(predownload)),
            ))
            .child(onboarding_nav(
                Some(Route::OnboardingAccount {}),
                Route::OnboardingPreferences {},
                any_selected,
            ))
            .maybe_child(popup)
    }
}

