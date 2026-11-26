use std::collections::{HashMap, HashSet};

use freya::prelude::*;
use oneclient_content::packages::ProviderId;
use oneclient_core::BundleArchive;

use crate::components::{Button, CARD_GRID_H, GRID_GAP, OptionalModCard, ScrollArea};
use crate::hooks::{
    ClusterBundles, onboarding_bundles_items, package_meta_batch, query_error, query_is_loading,
    shared_mod_keys, toggle_keys, use_onboarding_bundles, use_onboarding_selection,
    use_package_meta_batch,
};
use crate::i18n::{t, tf};
use crate::routes::Route;
use crate::theme::{colors, rounded, text_size};
use crate::ui::{border_all_color, centered_note, fixed_grid};
use crate::utils::bundle_display_name;
use crate::view::app::{
    BUNDLE_CARD_H, BUNDLE_COLUMNS, CellCard, bundle_blurb, cell_card, file_keys, package_names,
};
use crate::view::app::cluster::package_manager::make_row;
use crate::view::onboarding::{
    UNCREATED_CLUSTER, bundle_taken, is_default_file, onboarding_nav, onboarding_slide,
    step_heading, version_chip,
};

type MetaMap = HashMap<String, oneclient_content::packages::CachedPackageMeta>;

const FPS_WARNING_BUNDLE: &str = "SkyBlock";
const MOD_COLUMNS: usize = 3;

fn fps_warning_runs() -> [(&'static str, bool, bool); 7] {
    [
        (
            t("oneclient.onboarding.bundles.fps_warning.your"),
            false,
            false,
        ),
        (
            t("oneclient.onboarding.bundles.fps_warning.fps"),
            true,
            false,
        ),
        (
            t("oneclient.onboarding.bundles.fps_warning.will_be"),
            false,
            false,
        ),
        (
            t("oneclient.onboarding.bundles.fps_warning.significantly_worse"),
            true,
            false,
        ),
        (
            t("oneclient.onboarding.bundles.fps_warning.with_skyblock_mods"),
            false,
            false,
        ),
        (
            t("oneclient.onboarding.bundles.fps_warning.do_not_install"),
            true,
            true,
        ),
        (
            t("oneclient.onboarding.bundles.fps_warning.unless_hypixel"),
            false,
            true,
        ),
    ]
}

fn fps_warning_banner() -> Element {
    let mut text = paragraph();
    for (run, bold, underlined) in &fps_warning_runs() {
        let mut span = Span::new(*run)
            .font_size(text_size(12.))
            .font_weight(if *bold {
                FontWeight::BOLD
            } else {
                FontWeight::NORMAL
            })
            .color(colors::code_warn());
        if *underlined {
            span = span.text_decoration(TextDecoration::Underline);
        }
        text = text.span(span);
    }

    rect()
        .width(Size::fill())
        .corner_radius(rounded(8.))
        .border(border_all_color(1., colors::code_warn()))
        .padding(Gaps::new_all(8.))
        .child(text)
        .into_element()
}

/// (cluster id, minecraft version, archive), one per instance offering this bundle
type Member = (i64, String, BundleArchive);

#[derive(Clone)]
struct BundleGroup {
    display_name: String,
    members: Vec<Member>,
}

impl BundleGroup {
    fn taken(&self, selected: &HashSet<String>) -> bool {
        self.members
            .iter()
            .any(|(cluster_id, _, archive)| bundle_taken(*cluster_id, archive, selected))
    }

    fn toggled(&self, selected: &HashSet<String>) -> HashSet<String> {
        let on = !self.taken(selected);
        let mut next = selected.clone();
        for (cluster_id, _, archive) in &self.members {
            set_member(*cluster_id, archive, on, &mut next);
        }
        next
    }
}

/// Turning a version off also drops its optional mods so it reads as unpicked
fn set_member(cluster_id: i64, archive: &BundleArchive, on: bool, next: &mut HashSet<String>) {
    let visible = file_keys(cluster_id, archive, |_| true);
    if !on {
        for key in &visible {
            next.remove(key);
        }
        return;
    }
    let defaults = file_keys(cluster_id, archive, is_default_file);
    next.extend(if defaults.is_empty() { visible } else { defaults });
}

fn bundle_groups(clusters: &[ClusterBundles]) -> Vec<BundleGroup> {
    let mut groups: Vec<BundleGroup> = Vec::new();
    for cb in clusters {
        for archive in &cb.archives {
            let name = bundle_display_name(archive);
            let member = (
                cb.cluster.id,
                cb.cluster.mc_version.clone(),
                archive.clone(),
            );
            match groups.iter_mut().find(|g| g.display_name == name) {
                Some(group) => group.members.push(member),
                None => groups.push(BundleGroup {
                    display_name: name,
                    members: vec![member],
                }),
            }
        }
    }
    for group in &mut groups {
        group.members.sort_by(|a, b| a.1.cmp(&b.1));
    }
    groups
}

fn toggle_group(
    mut selected: State<HashSet<String>>,
    mut user_touched: State<bool>,
    group: &BundleGroup,
) {
    let next = group.toggled(&selected.peek());
    user_touched.set(true);
    selected.set(next);
}

fn toggle_member(
    mut selected: State<HashSet<String>>,
    mut user_touched: State<bool>,
    cluster_id: i64,
    archive: &BundleArchive,
) {
    let mut next = selected.peek().clone();
    let on = !bundle_taken(cluster_id, archive, &next);
    set_member(cluster_id, archive, on, &mut next);
    user_touched.set(true);
    selected.set(next);
}

/// Per-version control for a picked bundle offered by more than one instance
fn version_picker(
    group: &BundleGroup,
    selected_set: &HashSet<String>,
    selected: State<HashSet<String>>,
    user_touched: State<bool>,
) -> Element {
    let chips = rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Wrap {
            wrap_spacing: Some(8.),
        })
        .spacing(8.)
        .children(group.members.iter().map(|(cluster_id, version, archive)| {
            let active = bundle_taken(*cluster_id, archive, selected_set);
            let cluster_id = *cluster_id;
            let archive = archive.clone();
            version_chip(version, active, move |()| {
                toggle_member(selected, user_touched, cluster_id, &archive)
            })
        }));

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(8.)
        .child(
            label()
                .text(tf(
                    "oneclient.onboarding.bundles.opt_in.versions",
                    &[&group.display_name],
                ))
                .font_size(text_size(13.))
                .color(colors::fg_secondary()),
        )
        .child(chips)
        .into_element()
}

#[derive(Clone, PartialEq)]
struct Editing {
    name: String,
    versions: Vec<i64>,
}

fn picked_versions(group: &BundleGroup, selected: &HashSet<String>) -> Vec<i64> {
    group
        .members
        .iter()
        .filter(|(cluster_id, _, archive)| bundle_taken(*cluster_id, archive, selected))
        .map(|(cluster_id, _, _)| *cluster_id)
        .collect()
}

fn edit_scope<'a>(
    group: &'a BundleGroup,
    selected: &HashSet<String>,
    remembered: &[i64],
) -> Vec<&'a Member> {
    let picked = picked_versions(group, selected);
    let wanted = if picked.is_empty() { remembered } else { &picked };
    let members: Vec<&Member> = group
        .members
        .iter()
        .filter(|(cluster_id, _, _)| wanted.contains(cluster_id))
        .collect();
    if members.is_empty() {
        group.members.iter().collect()
    } else {
        members
    }
}

fn mod_keys(
    clusters: &[ClusterBundles],
    scope: &[&Member],
    selected: &HashSet<String>,
    package_id: &str,
) -> Vec<String> {
    scope
        .iter()
        .flat_map(|(cluster_id, _, archive)| {
            let archives = clusters
                .iter()
                .find(|cb| cb.cluster.id == *cluster_id)
                .map_or(&[][..], |cb| &cb.archives[..]);
            shared_mod_keys(*cluster_id, archives, &archive.manifest.name, package_id, selected)
        })
        .collect()
}

fn toggle_mod(
    mut selected: State<HashSet<String>>,
    mut user_touched: State<bool>,
    mut editing: State<Option<Editing>>,
    edit: Editing,
    keys: &[String],
) {
    let mut next = selected.peek().clone();
    toggle_keys(&mut next, keys);
    user_touched.set(true);
    selected.set(next);
    editing.set(Some(edit));
}

struct Catalog<'a> {
    clusters: &'a [ClusterBundles],
    modrinth: &'a MetaMap,
    curseforge: &'a MetaMap,
    names: &'a HashMap<String, String>,
}

fn bundle_editor(
    group: &BundleGroup,
    selected_set: &HashSet<String>,
    catalog: &Catalog,
    selected: State<HashSet<String>>,
    user_touched: State<bool>,
    remembered: &[i64],
    mut editing: State<Option<Editing>>,
) -> Element {
    let scope = edit_scope(group, selected_set, remembered);
    let edit = Editing {
        name: group.display_name.clone(),
        versions: scope.iter().map(|(cluster_id, _, _)| *cluster_id).collect(),
    };
    let mut seen = HashSet::new();
    let cards: Vec<Element> = scope
        .iter()
        .flat_map(|(_, _, archive)| {
            archive
                .manifest
                .files
                .iter()
                .filter(|f| is_default_file(f))
                .map(move |file| (archive, file))
        })
        .filter(|(_, file)| seen.insert(file.kind.package_id()))
        .map(|(archive, file)| {
            let package_id = file.kind.package_id();
            let provider = file.kind.metadata_provider();
            let meta = match provider {
                ProviderId::Modrinth => catalog.modrinth.get(&package_id),
                ProviderId::CurseForge => catalog.curseforge.get(&package_id),
                ProviderId::Local | ProviderId::Hangar => None,
            };
            let keys = mod_keys(catalog.clusters, &scope, selected_set, &package_id);
            let enabled = keys.iter().any(|k| selected_set.contains(k));
            let entry = make_row(
                package_id.clone(),
                Some(archive.manifest.name.clone()),
                provider,
                file.size,
                Vec::new(),
                enabled,
                file.enabled,
                file.is_github_hosted(),
                None,
                meta,
                catalog
                    .names
                    .get(&package_id)
                    .cloned()
                    .unwrap_or_else(|| file.display_name()),
                false,
                false,
                true,
            );

            OptionalModCard {
                entry,
                cluster_id: UNCREATED_CLUSTER,
                on_toggle: {
                    let edit = edit.clone();
                    (move |()| toggle_mod(selected, user_touched, editing, edit.clone(), &keys))
                        .into()
                },
            }
            .into_element()
        })
        .collect();

    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .content(Content::Flex)
        .spacing(12.)
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .cross_align(Alignment::Center)
                .spacing(10.)
                .child(
                    Button::new()
                        .secondary()
                        .small()
                        .text(t("oneclient.common.back"))
                        .on_press(move |_: Event<PressEventData>| editing.set(None)),
                )
                .child(
                    label()
                        .text(group.display_name.clone())
                        .font_size(text_size(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .max_lines(1)
                        .color(colors::fg_primary()),
                ),
        )
        .maybe_child(
            (group.members.len() > 1)
                .then(|| version_picker(group, selected_set, selected, user_touched)),
        )
        .child(
            ScrollArea::new()
                .width(Size::fill())
                .height(Size::flex(1.0))
                .scrollbar_gutter(true)
                .child(fixed_grid(cards, MOD_COLUMNS, CARD_GRID_H, GRID_GAP)),
        )
        .into_element()
}

fn group_card(
    group: &BundleGroup,
    selected_set: &HashSet<String>,
    names: &HashMap<String, String>,
    selected: State<HashSet<String>>,
    user_touched: State<bool>,
    mut editing: State<Option<Editing>>,
) -> Element {
    let archive = &group.members[0].2;
    let offered = || archive.manifest.files.iter().filter(|f| is_default_file(f));
    let count = offered().count();
    let toggle = group.clone();
    let open = Editing {
        name: group.display_name.clone(),
        versions: picked_versions(group, selected_set),
    };
    let edit = Button::new()
        .secondary()
        .small()
        .text(t("oneclient.common.edit"))
        .on_press(move |e: Event<PressEventData>| {
            e.stop_propagation();
            editing.set(Some(open.clone()));
        })
        .into_element();

    cell_card(CellCard {
        id: group.display_name.clone(),
        title: group.display_name.clone(),
        blurb: bundle_blurb(offered(), names),
        meta: Some(match count {
            1 => t("oneclient.clusters.instance.steps.bundle.packages.one").to_string(),
            many => tf(
                "oneclient.clusters.instance.steps.bundle.packages.other",
                &[&many],
            ),
        }),
        corner: Some(edit),
        selected: group.taken(selected_set),
        checkbox: true,
        on_press: (move |()| toggle_group(selected, user_touched, &toggle)).into(),
    })
}

#[derive(PartialEq)]
pub struct OnboardingBundlePicker;

impl Component for OnboardingBundlePicker {
    fn render(&self) -> impl IntoElement {
        let bundles_query = use_onboarding_bundles();
        let selection = use_onboarding_selection();
        let selected = selection.selected;
        let user_touched = selection.user_touched;
        let editing = use_state(|| None::<Editing>);

        let clusters = onboarding_bundles_items(&bundles_query).unwrap_or_default();
        let bundles_loaded = onboarding_bundles_items(&bundles_query).is_some();
        let catalog_msg = match query_error(&bundles_query) {
            Some(err) => tf("oneclient.onboarding.bundles.load_failed", &[&err]),
            None if query_is_loading(&bundles_query) => {
                t("oneclient.onboarding.bundles.loading").to_string()
            }
            None => t("oneclient.onboarding.bundles.empty").to_string(),
        };

        let groups = bundle_groups(&clusters);
        let shown: Vec<BundleArchive> = groups
            .iter()
            .flat_map(|g| g.members.iter().map(|(_, _, archive)| archive.clone()))
            .collect();

        let mut mr_ids = Vec::new();
        let mut cf_ids = Vec::new();
        for file in shown.iter().flat_map(|a| &a.manifest.files) {
            match file.kind.metadata_provider() {
                ProviderId::Modrinth => mr_ids.push(file.kind.package_id()),
                ProviderId::CurseForge => cf_ids.push(file.kind.package_id()),
                ProviderId::Local | ProviderId::Hangar => {}
            }
        }
        let mr_meta = package_meta_batch(&use_package_meta_batch(ProviderId::Modrinth, mr_ids));
        let cf_meta = package_meta_batch(&use_package_meta_batch(ProviderId::CurseForge, cf_ids));
        let names = package_names(&shown, &mr_meta, &cf_meta);

        let selected_set = selected.read().clone();
        let edit_state = editing.read().clone();
        let edited = edit_state.as_ref().and_then(|edit| {
            groups
                .iter()
                .find(|g| g.display_name == edit.name)
                .map(|g| (g, &edit.versions))
        });
        let cards: Vec<Element> = groups
            .iter()
            .map(|g| group_card(g, &selected_set, &names, selected, user_touched, editing))
            .collect();
        let fps_warning = groups.iter().any(|g| {
            g.display_name.eq_ignore_ascii_case(FPS_WARNING_BUNDLE) && g.taken(&selected_set)
        });

        let body = if let Some((group, remembered)) = edited {
            let catalog = Catalog {
                clusters: &clusters,
                modrinth: &mr_meta,
                curseforge: &cf_meta,
                names: &names,
            };
            bundle_editor(
                group,
                &selected_set,
                &catalog,
                selected,
                user_touched,
                remembered,
                editing,
            )
        } else if cards.is_empty() {
            centered_note(&catalog_msg)
        } else {
            ScrollArea::new()
                .width(Size::fill())
                .height(Size::flex(1.0))
                .spacing(12.)
                .children(
                    fps_warning
                        .then(fps_warning_banner)
                        .into_iter()
                        .chain([fixed_grid(cards, BUNDLE_COLUMNS, BUNDLE_CARD_H, GRID_GAP)])
                        .collect::<Vec<_>>(),
                )
                .into_element()
        };

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
                        t("oneclient.onboarding.bundle_picker.title"),
                        t("oneclient.onboarding.bundle_picker.subtitle"),
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
                            .child(body),
                    ),
            ))
            .child(onboarding_nav(
                Some(Route::OnboardingAccount {}),
                Route::OnboardingBundles {},
                bundles_loaded,
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::onboarding::pkg_key;
    use crate::view::onboarding::test_support::{archive, cluster, file};

    fn catalog() -> Vec<ClusterBundles> {
        let sb = || {
            archive(
                "SkyBlock",
                false,
                vec![file("sb-a", true, false), file("sb-opt", false, false)],
            )
        };
        vec![
            ClusterBundles {
                cluster: cluster(1),
                archives: vec![sb()],
            },
            ClusterBundles {
                cluster: cluster(2),
                archives: vec![sb()],
            },
        ]
    }

    #[test]
    fn picking_a_bundle_enables_its_defaults_in_every_cluster_and_unpicking_clears_it() {
        let groups = bundle_groups(&catalog());
        assert_eq!(groups.len(), 1);
        let name = "OneClient 1.21.11 Fabric [SkyBlock]";

        let mut on = groups[0].toggled(&HashSet::new());
        assert_eq!(
            on,
            HashSet::from([pkg_key(1, name, "sb-a"), pkg_key(2, name, "sb-a")])
        );

        on.insert(pkg_key(1, name, "sb-opt"));
        assert!(groups[0].toggled(&on).is_empty());
    }

    #[test]
    fn editing_a_mod_only_touches_picked_versions() {
        let clusters = catalog();
        let groups = bundle_groups(&clusters);
        let name = "OneClient 1.21.11 Fabric [SkyBlock]";

        let keys = |selected: &HashSet<String>, remembered: &[i64]| {
            let scope = edit_scope(&groups[0], selected, remembered);
            mod_keys(&clusters, &scope, selected, "sb-a")
        };

        assert_eq!(
            keys(&HashSet::new(), &[]),
            vec![pkg_key(1, name, "sb-a"), pkg_key(2, name, "sb-a")]
        );

        let only_second = HashSet::from([pkg_key(2, name, "sb-a")]);
        assert_eq!(keys(&only_second, &[]), vec![pkg_key(2, name, "sb-a")]);
        assert_eq!(keys(&HashSet::new(), &[2]), vec![pkg_key(2, name, "sb-a")]);
    }

    #[test]
    fn disabling_a_shared_mod_disables_it_in_every_picked_bundle() {
        use oneclient_db::models::OverrideType;

        let clusters = vec![ClusterBundles {
            cluster: cluster(1),
            archives: vec![
                archive("QoL", true, vec![file("polyplus", true, false), file("qol-a", true, false)]),
                archive("HUD", true, vec![file("polyplus", true, false), file("hud-a", true, false)]),
                archive("SkyBlock", false, vec![file("polyplus", true, false), file("sb-a", true, false)]),
            ],
        }];
        let groups = bundle_groups(&clusters);
        let mut selected = crate::view::onboarding::default_selection(&clusters, None);

        let keys = mod_keys(&clusters, &edit_scope(&groups[0], &selected, &[]), &selected, "polyplus");
        toggle_keys(&mut selected, &keys);

        let (overrides, _) = crate::hooks::bundle_plan(1, &clusters[0].archives, &selected);
        let polyplus: Vec<(&str, OverrideType)> = overrides
            .iter()
            .filter(|(_, id, _)| id.contains("polyplus"))
            .map(|(bundle, _, kind)| (bundle.as_str(), *kind))
            .collect();
        assert_eq!(
            polyplus,
            vec![
                ("OneClient 1.21.11 Fabric [QoL]", OverrideType::Disabled),
                ("OneClient 1.21.11 Fabric [HUD]", OverrideType::Disabled),
            ]
        );
    }

    #[test]
    fn a_version_can_be_dropped_without_touching_the_others() {
        let groups = bundle_groups(&catalog());
        let name = "OneClient 1.21.11 Fabric [SkyBlock]";
        let mut on = groups[0].toggled(&HashSet::new());
        on.insert(pkg_key(2, name, "sb-opt"));

        set_member(1, &groups[0].members[0].2, false, &mut on);
        assert_eq!(
            on,
            HashSet::from([pkg_key(2, name, "sb-a"), pkg_key(2, name, "sb-opt")])
        );
        assert!(groups[0].taken(&on));
    }
}
