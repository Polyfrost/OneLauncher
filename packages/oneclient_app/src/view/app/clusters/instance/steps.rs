use std::collections::HashMap;

use freya::prelude::*;
use oneclient_cluster::naming::{MAX_NAME_CHARS, validate_instance_name};
use oneclient_core::BundleArchive;
use oneclient_core::clusters::DirTarget;

use super::cards::{
    BUNDLE_CARD_H, BUNDLE_COLUMNS, CARD_RADIUS, CellCard, LOADER_CARD_H, LOADER_COLUMNS,
    LOADER_MARK_SIZE, VERSION_ROW_H, VERSION_ROW_SPACING, WideCard, cell_card, loader_mark,
    version_row, wide_card,
};
use super::data::Picks;
use super::details::details_body;
use super::import::ImportStep;
use super::import_review::review_step;
use super::model::{
    FolderChoice, ImportMode, ImportSource, LoaderChoice, ModpackOrigin, Step, TypeChoice, Wizard,
};
use super::model::{kind_bit, version_kinds};
use super::rail::version_art;
use crate::components::{
    Button, Dropdown, GRID_GAP, Icon, IconType, ScrollArea, TextInput, centered_spinner,
};
use crate::hooks::{
    ClusterAction, settled_or_loading, use_cluster_mutation, use_clusters, use_dispatch,
    use_shared_groups,
};
use crate::i18n::{t, tf};
use crate::theme::colors;
use crate::ui::{border_all_color, centered_note, divider, fixed_grid, note};
use crate::utils::bundle_display_name;
use oneclient_events::{Choice, Prompt};

pub fn body(wizard: Wizard, picks: &Picks) -> Element {
    let (key, inner) = match picks.step {
        Step::Type => ("step-type", type_step(wizard, picks)),
        Step::Folder => (
            "step-folder",
            FolderStep {
                folder: wizard.folder,
                selected: picks.folder.clone(),
            }
            .into_element(),
        ),
        Step::Loader => ("step-loader", loader_step(wizard, picks)),
        Step::Version => ("step-version", version_step(wizard, picks)),
        Step::Bundles => ("step-bundles", bundles_step(wizard, picks)),
        Step::Modpack => ("step-modpack", modpack_step(wizard, picks)),
        Step::ImportSource => ("step-import-source", import_source_step(wizard, picks)),
        Step::ImportMode => ("step-import-mode", import_mode_step(wizard, picks)),
        Step::ImportReview => ("step-import-review", review_step(wizard, picks)),
        Step::Import => (
            "step-import",
            ImportStep {
                chosen: wizard.import_chosen,
                extra: wizard.import_extra,
            }
            .into_element(),
        ),
        Step::Customize => (
            "step-details",
            details_body(
                wizard.details,
                picks.suggested.clone(),
                None,
                version_art(picks.versions.chosen.as_deref(), picks.loader.chosen),
                validate_instance_name(&picks.name).err(),
                Some(MAX_NAME_CHARS),
            ),
        ),
    };

    rect()
        .key(key)
        .width(Size::fill())
        .height(
            if matches!(
                picks.step,
                Step::Folder | Step::Version | Step::Import | Step::ImportReview
            ) {
                Size::fill()
            } else {
                Size::auto()
            },
        )
        .child(inner)
        .into_element()
}

fn type_step(mut wizard: Wizard, picks: &Picks) -> Element {
    let cards = [
        (
            TypeChoice::OneClient,
            IconType::IconLogo,
            "OneClient",
            Some(t("oneclient.clusters.instance.steps.recommended")),
            t("oneclient.clusters.instance.steps.type.oneclient.blurb"),
            t("oneclient.clusters.instance.steps.type.oneclient.meta"),
        ),
        (
            TypeChoice::Scratch,
            IconType::Sliders04,
            t("oneclient.clusters.instance.steps.type.scratch.title"),
            None,
            t("oneclient.clusters.instance.steps.type.scratch.blurb"),
            t("oneclient.clusters.instance.steps.own_folder"),
        ),
        (
            TypeChoice::Modpack,
            IconType::DownloadCloud02,
            t("oneclient.clusters.instance.steps.type.modpack.title"),
            None,
            t("oneclient.clusters.instance.steps.type.modpack.blurb"),
            t("oneclient.clusters.instance.steps.own_folder"),
        ),
        (
            TypeChoice::Import,
            IconType::FolderDownload,
            t("oneclient.clusters.instance.steps.type.import.title"),
            None,
            t("oneclient.clusters.instance.steps.type.import.blurb"),
            t("oneclient.clusters.instance.steps.own_folder"),
        ),
    ];

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .children(
            cards
                .into_iter()
                .map(|(choice, icon, title, badge, blurb, meta)| {
                    wide_card(WideCard {
                        icon,
                        title: title.to_string(),
                        badge: badge.map(str::to_string),
                        blurb: blurb.to_string(),
                        meta: meta.to_string(),
                        selected: picks.choice == choice,
                        on_press: (move |()| {
                            wizard.choice.set(choice);
                            wizard.version.set(None);
                            wizard.declined.set(None);
                        })
                        .into(),
                        trailing: None,
                    })
                }),
        )
        .into_element()
}

fn modpack_step(wizard: Wizard, picks: &Picks) -> Element {
    let cards = [
        (
            ModpackOrigin::Browse,
            IconType::Globe01,
            t("oneclient.clusters.instance.steps.modpack.browse.title"),
            t("oneclient.clusters.instance.steps.modpack.browse.blurb"),
            t("oneclient.clusters.instance.steps.modpack.browse.meta"),
        ),
        (
            ModpackOrigin::File,
            IconType::File02,
            t("oneclient.clusters.instance.steps.modpack.file.title"),
            t("oneclient.clusters.instance.steps.modpack.file.blurb"),
            t("oneclient.clusters.instance.steps.modpack.file.meta"),
        ),
    ];

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .children(cards.into_iter().map(|(origin, icon, title, blurb, meta)| {
            let mut chosen = wizard.modpack_origin;
            wide_card(WideCard {
                icon,
                title: title.to_string(),
                badge: None,
                blurb: blurb.to_string(),
                meta: meta.to_string(),
                selected: picks.modpack_origin == origin,
                on_press: (move |()| chosen.set(origin)).into(),
                trailing: None,
            })
        }))
        .into_element()
}

fn import_source_step(wizard: Wizard, picks: &Picks) -> Element {
    let cards = [
        (
            ImportSource::File,
            IconType::File02,
            t("oneclient.clusters.instance.steps.import_source.file.title"),
            t("oneclient.clusters.instance.steps.import_source.file.blurb"),
            t("oneclient.clusters.instance.steps.import_source.file.meta"),
        ),
        (
            ImportSource::Launcher,
            IconType::FolderDownload,
            t("oneclient.clusters.instance.steps.import_source.launcher.title"),
            t("oneclient.clusters.instance.steps.import_source.launcher.blurb"),
            t("oneclient.clusters.instance.steps.import_source.launcher.meta"),
        ),
    ];

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .children(cards.into_iter().map(|(source, icon, title, blurb, meta)| {
            let mut chosen = wizard.import_source;
            wide_card(WideCard {
                icon,
                title: title.to_string(),
                badge: None,
                blurb: blurb.to_string(),
                meta: meta.to_string(),
                selected: picks.import_source == source,
                on_press: (move |()| chosen.set(source)).into(),
                trailing: None,
            })
        }))
        .into_element()
}

fn import_mode_step(wizard: Wizard, picks: &Picks) -> Element {
    let cards = [
        (
            ImportMode::Improve,
            IconType::IconLogo,
            t("oneclient.clusters.instance.steps.import_mode.improve.title"),
            Some(t("oneclient.clusters.instance.steps.recommended")),
            t("oneclient.clusters.instance.steps.import_mode.improve.blurb"),
            t("oneclient.clusters.instance.steps.import_mode.improve.meta"),
        ),
        (
            ImportMode::AsIs,
            IconType::FolderDownload,
            t("oneclient.clusters.instance.steps.import_mode.as_is.title"),
            None,
            t("oneclient.clusters.instance.steps.import_mode.as_is.blurb"),
            t("oneclient.clusters.instance.steps.import_mode.as_is.meta"),
        ),
    ];

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .children(
            cards
                .into_iter()
                .map(|(mode, icon, title, badge, blurb, meta)| {
                    let mut chosen = wizard.import_mode;
                    wide_card(WideCard {
                        icon,
                        title: title.to_string(),
                        badge: badge.map(str::to_string),
                        blurb: blurb.to_string(),
                        meta: meta.to_string(),
                        selected: picks.import_mode == mode,
                        on_press: (move |()| chosen.set(mode)).into(),
                        trailing: None,
                    })
                }),
        )
        .into_element()
}

#[derive(PartialEq)]
struct FolderStep {
    folder: State<FolderChoice>,
    selected: FolderChoice,
}

impl Component for FolderStep {
    fn render(&self) -> impl IntoElement {
        let groups = settled_or_loading(&use_shared_groups()).unwrap_or_default();
        let clusters = settled_or_loading(&use_clusters()).unwrap_or_default();
        let mutation = use_cluster_mutation();
        let dispatch = use_dispatch();
        let cards = [
            (
                FolderChoice::Shared,
                IconType::Link03,
                t("oneclient.clusters.instance.steps.folder.shared.title").to_string(),
                Some(t("oneclient.clusters.instance.steps.recommended")),
                t("oneclient.clusters.instance.steps.folder.shared.blurb").to_string(),
                t("oneclient.clusters.instance.steps.folder.shared.meta").to_string(),
            ),
            (
                FolderChoice::Dedicated,
                IconType::Folder,
                t("oneclient.clusters.instance.steps.folder.dedicated.title").to_string(),
                None,
                t("oneclient.clusters.instance.steps.folder.dedicated.blurb").to_string(),
                t("oneclient.clusters.instance.steps.folder.dedicated.meta").to_string(),
            ),
        ];
        let custom = groups.into_iter().map(|group| {
            let target = DirTarget::Shared(Some(group.clone()));
            let members = clusters
                .iter()
                .filter(|cluster| cluster.dir_target == target)
                .count();
            let used_by = match members {
                0 => t("oneclient.clusters.instance.steps.folder.custom.used_by.none").to_string(),
                1 => t("oneclient.clusters.instance.steps.folder.custom.used_by.one").to_string(),
                n => tf(
                    "oneclient.clusters.instance.steps.folder.custom.used_by.other",
                    &[&n],
                ),
            };
            (
                (
                    FolderChoice::Custom(group.clone()),
                    IconType::Link03,
                    group,
                    None,
                    t("oneclient.clusters.instance.steps.folder.custom.blurb").to_string(),
                    used_by,
                ),
                members == 0,
            )
        });

        let folder = self.folder;
        let selected = self.selected.clone();
        let row = move |(choice, icon, title, badge, blurb, meta): (
            FolderChoice,
            IconType,
            String,
            Option<&str>,
            String,
            String,
        ),
                        trailing: Option<Element>| {
            let mut chosen = folder;
            wide_card(WideCard {
                icon,
                title,
                badge: badge.map(str::to_string),
                blurb,
                meta,
                selected: selected == choice,
                on_press: (move |()| chosen.set(choice.clone())).into(),
                trailing,
            })
        };
        let custom: Vec<Element> = custom
            .map(|(card, empty)| {
                let FolderChoice::Custom(group) = card.0.clone() else {
                    return row(card, None);
                };
                let dispatch = dispatch.clone();
                let selected = self.selected.clone();
                let delete = Button::new()
                    .secondary()
                    .icon()
                    .tooltip(if empty {
                        tf(
                            "oneclient.clusters.instance.steps.folder.custom.delete",
                            &[&group],
                        )
                    } else {
                        t("oneclient.clusters.instance.steps.folder.custom.delete_in_use")
                            .to_string()
                    })
                    .enabled(empty)
                    .width(Size::px(38.))
                    .height(Size::px(38.))
                    .on_press(move |e: Event<PressEventData>| {
                        e.stop_propagation();
                        let dispatch = dispatch.clone();
                        let group = group.clone();
                        let mut folder = folder;
                        let picked = selected == FolderChoice::Custom(group.clone());
                        spawn_forever(async move {
                            let linked = match oneclient_common::paths::shared_group_dir(Some(&group)) {
                                Ok(dir) => polyio::symlink_metadata(dir)
                                    .await
                                    .is_ok_and(|meta| meta.file_type().is_symlink()),
                                Err(_) => false,
                            };
                            let body = if linked {
                                t("oneclient.clusters.instance.steps.folder.custom.delete_prompt.link")
                            } else {
                                t("oneclient.clusters.instance.steps.folder.custom.delete_prompt.body")
                            };
                            let title = tf(
                                "oneclient.clusters.instance.steps.folder.custom.delete_prompt.title",
                                &[&group],
                            );
                            let prompt = Prompt::new(title, body)
                                .option(
                                    Choice::danger(
                                        "shared_group.delete",
                                        t("oneclient.common.delete"),
                                    ),
                                    (),
                                )
                                .dismiss(t("oneclient.common.cancel"));
                            if !matches!(dispatch.events().ask(prompt).await, Ok(Some(_))) {
                                return;
                            }
                            if picked {
                                folder.set(FolderChoice::Shared);
                            }
                            mutation.mutate(ClusterAction::DeleteSharedGroup { name: group });
                        });
                    })
                    .child(Icon::new(IconType::Trash01).size(16.));
                row(card, Some(delete.into_element()))
            })
            .collect();

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .content(Content::Flex)
            .spacing(10.)
            .children(cards.into_iter().map(|card| row(card, None)))
            .maybe_child((!custom.is_empty()).then(divider))
            .maybe_child((!custom.is_empty()).then(|| {
                ScrollArea::new()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .scrollbar_gutter(true)
                    .spacing(10.)
                    .children(custom)
            }))
            .into_element()
    }
}

fn select_loader(mut wizard: Wizard, choice: LoaderChoice) {
    wizard.loader.set(choice);
    wizard.loader_version.set(None);
}

fn loader_step(wizard: Wizard, picks: &Picks) -> Element {
    let chosen = *wizard.loader.read();

    let cards: Vec<Element> = LoaderChoice::ALL
        .into_iter()
        .map(|choice| {
            let offered = picks
                .loader
                .available
                .as_ref()
                .is_none_or(|available| choice.resolve(available).is_some());
            cell_card(CellCard {
                id: choice.name(),
                title: choice.name(),
                blurb: choice.blurb().to_string(),
                meta: (!offered).then(|| match picks.versions.chosen.as_deref() {
                    Some(version) => tf(
                        "oneclient.clusters.instance.steps.loader.no_build",
                        &[&version],
                    ),
                    None => t("oneclient.clusters.instance.steps.loader.no_build_any").to_string(),
                }),
                corner: loader_mark(choice, chosen == choice, LOADER_MARK_SIZE),
                selected: offered && chosen == choice,
                checkbox: false,
                on_press: (move |()| {
                    if offered {
                        select_loader(wizard, choice);
                    }
                })
                .into(),
            })
        })
        .collect();

    let shows_versions = chosen != LoaderChoice::Vanilla && !picks.loader.versions.is_empty();

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(GRID_GAP)
        .child(
            rect()
                .key("loader-grid")
                .width(Size::fill())
                .child(fixed_grid(cards, LOADER_COLUMNS, LOADER_CARD_H, GRID_GAP)),
        )
        .maybe_child(shows_versions.then(|| {
            rect()
                .key("loader-version")
                .width(Size::fill())
                .child(loader_version_card(wizard, picks, chosen))
                .into_element()
        }))
        .into_element()
}

fn loader_version_card(mut wizard: Wizard, picks: &Picks, chosen: LoaderChoice) -> Element {
    let options = picks.loader.versions.to_vec();
    let selected = picks.loader.version.clone().unwrap_or_default();

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(16.)
        .padding(Gaps::new_all(16.))
        .corner_radius(CornerRadius::new_all(CARD_RADIUS))
        .background(colors::component_bg())
        .border(border_all_color(1., colors::component_border()))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(4.)
                .child(
                    label()
                        .text(tf(
                            "oneclient.clusters.instance.steps.loader_version.title",
                            &[&chosen.name()],
                        ))
                        .font_size(14.)
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(t(
                            "oneclient.clusters.instance.steps.loader_version.description",
                        ))
                        .font_size(12.)
                        .line_height(1.4)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(
            Dropdown::new(selected, options.clone())
                .width(Size::px(200.))
                .height(Size::px(32.))
                .on_select(move |index: usize| {
                    if let Some(chosen) = options.get(index) {
                        wizard.loader_version.set(Some(chosen.clone()));
                    }
                }),
        )
        .into_element()
}

fn version_step(wizard: Wizard, picks: &Picks) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .content(Content::Flex)
        .spacing(12.)
        .child(
            rect()
                .key("version-controls")
                .width(Size::fill())
                .child(version_controls(wizard, picks)),
        )
        .child(
            rect()
                .key("version-list")
                .width(Size::fill())
                .height(Size::flex(1.0))
                .child(version_list(wizard, picks)),
        )
        .into_element()
}

fn version_controls(mut wizard: Wizard, picks: &Picks) -> Element {
    let filter = *wizard.filter.read();

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(16.)
        .child(
            rect().width(Size::flex(1.0)).child(
                TextInput::new(wizard.query)
                    .placeholder(t("oneclient.clusters.instance.steps.version.search"))
                    .width(Size::fill())
                    .leading(Icon::new(IconType::SearchMd).size(14.)),
            ),
        )
        .maybe_child((picks.choice != TypeChoice::OneClient).then(|| {
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(10.)
                .child(
                    label()
                        .text(t("oneclient.clusters.instance.steps.version.show"))
                        .font_size(12.)
                        .color(colors::fg_secondary()),
                )
                .child(
                    Dropdown::new(
                        filter_summary(filter),
                        version_kinds()
                            .iter()
                            .map(|(name, _)| (*name).to_string())
                            .collect(),
                    )
                    .checked(
                        version_kinds()
                            .iter()
                            .map(|(_, kind)| filter & kind_bit(*kind) != 0)
                            .collect(),
                    )
                    .width(Size::px(168.))
                    .height(Size::px(32.))
                    .on_select(move |index: usize| {
                        if let Some((_, kind)) = version_kinds().get(index) {
                            let mask = *wizard.filter.peek();
                            wizard.filter.set(mask ^ kind_bit(*kind));
                            wizard.version.set(None);
                        }
                    }),
                )
                .into_element()
        }))
        .into_element()
}

fn filter_summary(filter: u8) -> String {
    let picked: Vec<&str> = version_kinds()
        .iter()
        .filter(|(_, kind)| filter & kind_bit(*kind) != 0)
        .map(|(name, _)| *name)
        .collect();
    match picked.as_slice() {
        [] => t("oneclient.clusters.instance.steps.version.filter.none").to_string(),
        [only] => (*only).to_string(),
        all if all.len() == version_kinds().len() => {
            t("oneclient.clusters.instance.steps.version.filter.all").to_string()
        }
        [first, rest @ ..] => format!("{first} +{}", rest.len()),
    }
}

fn version_list(mut wizard: Wizard, picks: &Picks) -> Element {
    if let Some(error) = &picks.versions.error {
        return centered_failure(error);
    }
    if !picks.versions.loaded {
        return centered_spinner(t("oneclient.clusters.instance.steps.version.loading"));
    }
    if picks.versions.list.is_empty() {
        return centered_note(t("oneclient.clusters.instance.steps.version.no_matches"));
    }

    let versions = picks.versions.list.clone();
    let count = versions.len();
    let selected = picks.versions.chosen.clone();

    ScrollArea::new()
        .width(Size::fill())
        .height(Size::fill())
        .scrollbar_gutter(true)
        .reset_key(list_reset_key(picks, &wizard.query.read()))
        .lazy(count, VERSION_ROW_H, VERSION_ROW_SPACING, move |index| {
            let Some(row) = versions.row(index) else {
                return rect().into_element();
            };
            let id = row.id.clone();
            let is_selected = selected.as_deref() == Some(row.id.as_str());

            version_row(
                &row,
                is_selected,
                (move |()| wizard.version.set(Some(id.clone()))).into(),
            )
        })
        .into_element()
}

fn centered_failure(message: &str) -> Element {
    rect()
        .width(Size::fill())
        .height(Size::fill())
        .center()
        .padding(Gaps::new_symmetric(0., 16.))
        .child(note(
            tf(
                "oneclient.clusters.instance.steps.version.load_failed",
                &[&message],
            ),
            colors::danger(),
        ))
        .into_element()
}

fn list_reset_key(picks: &Picks, needle: &str) -> u64 {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    matches!(picks.choice, TypeChoice::OneClient).hash(&mut hasher);
    picks.versions.filter.hash(&mut hasher);
    needle.hash(&mut hasher);
    hasher.finish()
}

fn bundle_blurb(archive: &BundleArchive, names: &HashMap<String, String>) -> String {
    let mut listed: Vec<String> = archive
        .manifest
        .files
        .iter()
        .filter(|file| file.enabled && !file.hidden)
        .map(|file| {
            let package_id = file.kind.package_id();
            names
                .get(&package_id)
                .cloned()
                .unwrap_or_else(|| super::tidy_file_name(&file.display_name()))
        })
        .collect();

    let total = listed.len();
    listed.truncate(3);
    let head = listed.join(", ");

    match total.saturating_sub(3) {
        0 if head.is_empty() => {
            t("oneclient.clusters.instance.steps.bundle.config_only").to_string()
        }
        0 => head,
        rest => tf(
            "oneclient.clusters.instance.steps.bundle.more",
            &[&head, &rest],
        ),
    }
}

fn bundles_step(mut wizard: Wizard, picks: &Picks) -> Element {
    if picks.bundles.archives.is_empty() {
        return centered_note(if picks.bundles.loaded {
            t("oneclient.clusters.instance.steps.bundles.none")
        } else {
            t("oneclient.clusters.instance.steps.bundles.loading")
        });
    }

    let cards: Vec<Element> = picks
        .bundles
        .archives
        .iter()
        .map(|archive| {
            let name = archive.manifest.name.clone();
            let count = archive
                .manifest
                .files
                .iter()
                .filter(|file| file.enabled && !file.hidden)
                .count();

            cell_card(CellCard {
                id: name.clone(),
                title: bundle_display_name(archive),
                blurb: bundle_blurb(archive, &picks.bundles.names),
                meta: Some(match count {
                    1 => t("oneclient.clusters.instance.steps.bundle.packages.one").to_string(),
                    many => tf(
                        "oneclient.clusters.instance.steps.bundle.packages.other",
                        &[&many],
                    ),
                }),
                corner: None,
                selected: !picks.bundles.declined.contains(&name),
                checkbox: true,
                on_press: {
                    let defaults = picks.bundles.declined.clone();
                    (move |()| {
                        let mut next = wizard
                            .declined
                            .read()
                            .clone()
                            .unwrap_or_else(|| defaults.clone());
                        if !next.remove(&name) {
                            next.insert(name.clone());
                        }
                        wizard.declined.set(Some(next));
                    })
                    .into()
                },
            })
        })
        .collect();

    fixed_grid(cards, BUNDLE_COLUMNS, BUNDLE_CARD_H, GRID_GAP)
}
