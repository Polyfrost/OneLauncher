use std::collections::HashMap;

use freya::prelude::*;
use oneclient_cluster::naming::{MAX_NAME_CHARS, validate_instance_name};
use oneclient_core::BundleArchive;

use super::cards::{
    BUNDLE_CARD_H, BUNDLE_COLUMNS, CARD_RADIUS, CellCard, LOADER_CARD_H, LOADER_COLUMNS,
    LOADER_MARK_SIZE, VERSION_ROW_H, VERSION_ROW_SPACING, WideCard, cell_card, loader_mark,
    version_row, wide_card,
};
use super::data::Picks;
use super::details::details_body;
use super::model::{LoaderChoice, ModpackOrigin, Step, TypeChoice, Wizard};
use super::model::{VERSION_KINDS, kind_bit};
use super::rail::version_art;
use crate::components::{
    Dropdown, GRID_GAP, Icon, IconType, ScrollArea, TextInput, centered_spinner,
};
use crate::theme::colors;
use crate::ui::{border_all_color, centered_note, fixed_grid, note};
use crate::utils::bundle_display_name;

pub fn body(wizard: Wizard, picks: &Picks) -> Element {
    let (key, inner) = match picks.step {
        Step::Type => ("step-type", type_step(wizard, picks)),
        Step::Loader => ("step-loader", loader_step(wizard, picks)),
        Step::Version => ("step-version", version_step(wizard, picks)),
        Step::Bundles => ("step-bundles", bundles_step(wizard, picks)),
        Step::Modpack => ("step-modpack", modpack_step(wizard, picks)),
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
            Some("Recommended"),
            "The most bleeding edge performance, QoL mods and world hosting in one instance.",
            "Shares configs, worlds, and packs with your other OneClient instances.",
        ),
        (
            TypeChoice::Scratch,
            IconType::Sliders04,
            "Start from scratch",
            None,
            "Pick a loader and version, then add mods yourself.",
            "Has its own game folder.",
        ),
        (
            TypeChoice::Modpack,
            IconType::DownloadCloud02,
            "Modpack",
            None,
            "Install a pack from Modrinth or CurseForge, or a file you downloaded.",
            "Has its own game folder.",
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
            "Browse modpacks",
            "Search Modrinth and CurseForge. Installing a pack from there creates its instance.",
            "Opens the browser on the Modpacks tab.",
        ),
        (
            ModpackOrigin::File,
            IconType::File02,
            "Import a file",
            "Use a modpack you already downloaded or got from someone else.",
            "Modrinth .mrpack or CurseForge .zip.",
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
            })
        }))
        .into_element()
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
                meta: (!offered).then(|| {
                    let version = picks.versions.chosen.as_deref();
                    format!("No build for {}", version.unwrap_or("this version"))
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
        .maybe_child((picks.choice != TypeChoice::OneClient).then(|| {
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
                        filter_summary(filter),
                        VERSION_KINDS
                            .iter()
                            .map(|(name, _)| (*name).to_string())
                            .collect(),
                    )
                    .checked(
                        VERSION_KINDS
                            .iter()
                            .map(|(_, kind)| filter & kind_bit(*kind) != 0)
                            .collect(),
                    )
                    .width(Size::px(168.))
                    .height(Size::px(32.))
                    .on_select(move |index: usize| {
                        if let Some((_, kind)) = VERSION_KINDS.get(index) {
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
    let picked: Vec<&str> = VERSION_KINDS
        .iter()
        .filter(|(_, kind)| filter & kind_bit(*kind) != 0)
        .map(|(name, _)| *name)
        .collect();
    match picked.as_slice() {
        [] => "None".to_string(),
        [only] => (*only).to_string(),
        all if all.len() == VERSION_KINDS.len() => "All versions".to_string(),
        [first, rest @ ..] => format!("{first} +{}", rest.len()),
    }
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
            format!("Could not load the version list. {message}"),
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
        0 if head.is_empty() => "Configuration only.".to_string(),
        0 => head,
        rest => format!("{head} and {rest} more"),
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
                title: bundle_display_name(archive),
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
