use std::collections::HashMap;

use freya::prelude::*;
use oneclient_core::BundleArchive;

use super::cards::{
    BUNDLE_CARD_H, BUNDLE_COLUMNS, CARD_RADIUS, CellCard, LOADER_CARD_H, LOADER_COLUMNS,
    LOADER_MARK_SIZE, LOADER_ROW_MARK_SIZE, SelectCard, VERSION_ROW_H, VERSION_ROW_SPACING,
    WideCard, cell_card, loader_mark, marker, version_row, wide_card,
};
use super::data::Picks;
use super::details::details_body;
use super::model::FILTERS;
use super::model::{LoaderChoice, Step, TypeChoice, Wizard};
use crate::components::{
    Dropdown, GRID_GAP, Icon, IconType, ScrollArea, TextInput, centered_spinner,
};
use crate::theme::colors;
use crate::ui::{border_all_color, centered_note, fixed_grid, note};

pub fn body(wizard: Wizard, picks: &Picks) -> Element {
    let (key, inner) = match picks.step {
        Step::Type => ("step-type", type_step(wizard, picks)),
        Step::Loader => ("step-loader", loader_step(wizard, picks)),
        Step::Version => ("step-version", version_step(wizard, picks)),
        Step::Bundles => ("step-bundles", bundles_step(wizard, picks)),
        Step::Customize => (
            "step-details",
            details_body(wizard.details, picks.suggested.clone(), None),
        ),
    };

    rect()
        .key(key)
        .width(Size::fill())
        .height(if picks.step == Step::Version {
            Size::fill()
        } else {
            Size::auto()
        })
        .child(inner)
        .into_element()
}

fn type_step(mut wizard: Wizard, picks: &Picks) -> Element {
    let cards = [
        (
            TypeChoice::OneClient,
            IconType::Rocket02,
            "OneClient",
            Some("RECOMMENDED"),
            "Polyfrost's client. OneConfig, the performance stack and cosmetics are wired up, so it is playable the moment it finishes downloading.",
            "Shares the game folder, worlds and packs with your other OneClient instances.",
        ),
        (
            TypeChoice::Scratch,
            IconType::Sliders04,
            "Start from scratch",
            None,
            "Pick a loader and a version and build the instance yourself. Packages can be added once it exists.",
            "Fabric, Forge, NeoForge, Quilt, or no loader at all. Keeps its own game folder.",
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
                        selected: picks.choice == Some(choice),
                        on_press: (move |()| {
                            wizard.choice.set(Some(choice));
                            wizard.version.set(None);
                            wizard.declined.set(None);
                        })
                        .into(),
                    })
                }),
        )
        .into_element()
}

fn loader_step(mut wizard: Wizard, picks: &Picks) -> Element {
    let chosen = *wizard.loader.read();

    let cards: Vec<Element> = LoaderChoice::MODDED
        .into_iter()
        .map(|choice| {
            cell_card(CellCard {
                id: choice.name().to_string(),
                title: choice.name().to_string(),
                blurb: choice.blurb().to_string(),
                meta: None,
                corner: loader_mark(choice, chosen == choice, LOADER_MARK_SIZE),
                selected: chosen == choice,
                checkbox: false,
                on_press: (move |()| {
                    wizard.loader.set(choice);
                    wizard.loader_version.set(None);
                    wizard.version.set(None);
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
        .child(
            rect()
                .key("loader-vanilla")
                .width(Size::fill())
                .child(loader_row(wizard, LoaderChoice::Vanilla, chosen)),
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
    let selected = picks
        .loader
        .version
        .clone()
        .or_else(|| options.first().cloned())
        .unwrap_or_default();

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
                        .text(format!("{} version", chosen.name()))
                        .font_size(14.)
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text("The newest build is picked for you. Change it if a package needs an older one.")
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

fn loader_row(mut wizard: Wizard, choice: LoaderChoice, chosen: LoaderChoice) -> Element {
    let selected = chosen == choice;

    let content = rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(8.)
                .child(
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .content(Content::Flex)
                        .cross_align(Alignment::Center)
                        .spacing(10.)
                        .child(marker(selected, false))
                        .child(
                            label()
                                .text(choice.name())
                                .width(Size::flex(1.0))
                                .font_size(14.)
                                .font_weight(FontWeight::MEDIUM)
                                .max_lines(1)
                                .color(colors::fg_primary()),
                        ),
                )
                .child(
                    label()
                        .text(choice.blurb())
                        .width(Size::fill())
                        .font_size(12.)
                        .line_height(1.4)
                        .max_lines(2)
                        .color(colors::fg_secondary()),
                ),
        )
        .maybe_child(loader_mark(choice, selected, LOADER_ROW_MARK_SIZE))
        .into_element();

    SelectCard {
        id: choice.name().to_string(),
        selected,
        height: None,
        padding: Gaps::new_symmetric(14., 14.),
        content,
        on_press: (move |()| {
            wizard.loader.set(choice);
            wizard.loader_version.set(None);
            wizard.version.set(None);
        })
        .into(),
    }
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
                    .placeholder("Search versions")
                    .width(Size::fill())
                    .leading(Icon::new(IconType::SearchMd).size(14.)),
            ),
        )
        .maybe_child((picks.choice != Some(TypeChoice::OneClient)).then(|| {
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(10.)
                .child(
                    label()
                        .text("Show")
                        .font_size(12.)
                        .color(colors::fg_secondary()),
                )
                .child(
                    Dropdown::new(
                        FILTERS[filter.min(FILTERS.len() - 1)],
                        FILTERS.iter().map(|f| (*f).to_string()).collect(),
                    )
                    .width(Size::px(168.))
                    .height(Size::px(32.))
                    .on_select(move |index: usize| {
                        wizard.filter.set(index);
                        wizard.version.set(None);
                    }),
                )
                .into_element()
        }))
        .into_element()
}

fn version_list(mut wizard: Wizard, picks: &Picks) -> Element {
    if let Some(error) = &picks.versions.error {
        return centered_failure(error);
    }
    if !picks.versions.loaded {
        return centered_spinner("Loading versions...");
    }
    if picks.versions.list.is_empty() {
        return centered_note("No versions match that search.");
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
                (move |()| {
                    wizard.version.set(Some(id.clone()));
                    wizard.loader_version.set(None);
                })
                .into(),
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
            format!("Could not load the version list. {message}"),
            colors::danger(),
        ))
        .into_element()
}

fn list_reset_key(picks: &Picks, needle: &str) -> u64 {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    matches!(picks.choice, Some(TypeChoice::OneClient)).hash(&mut hasher);
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
        0 if head.is_empty() => "Configuration only.".to_string(),
        0 => head,
        rest => format!("{head} and {rest} more"),
    }
}

fn bundle_title(archive: &BundleArchive) -> String {
    let category = archive.manifest.category.trim();
    if category.is_empty() {
        archive.manifest.name.clone()
    } else {
        category.to_string()
    }
}

fn bundles_step(mut wizard: Wizard, picks: &Picks) -> Element {
    if picks.bundles.archives.is_empty() {
        return centered_note(if picks.bundles.loaded {
            "No bundles ship for this version yet."
        } else {
            "Loading bundles..."
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
                title: bundle_title(archive),
                blurb: bundle_blurb(archive, &picks.bundles.names),
                meta: Some(match count {
                    1 => "1 package".to_string(),
                    many => format!("{many} packages"),
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
