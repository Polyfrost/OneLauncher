use std::collections::HashMap;

use freya::prelude::*;
use oneclient_core::BundleArchive;

use super::create::{
    FILTERS, LoaderChoice, LoaderMark, Picks, Step, TypeChoice, VersionRow, Wizard,
};
use super::details::details_body;
use crate::AppAssets;
use crate::components::{
    AssetImage, Dropdown, GRID_GAP, Icon, IconType, ScrollArea, TextInput, centered_spinner,
    filled_pill, pill,
};
use crate::theme::colors;
use crate::ui::{border_all_color, centered_note, fixed_grid, note};

const CARD_RADIUS: f32 = 12.;
const MARKER_SIZE: f32 = 18.;
const MARKER_MARK: f32 = 12.;
const MARKER_DOT: f32 = 7.;
const MARKER_RADIUS: f32 = 5.;
const VERSION_ROW_H: f32 = 48.;
const VERSION_ROW_SPACING: f32 = 4.;
const LOADER_MARK_SIZE: f32 = 22.;
const LOADER_ROW_MARK_SIZE: f32 = 34.;
const BUNDLE_COLUMNS: usize = 3;
const BUNDLE_CARD_H: f32 = 132.;
const LOADER_COLUMNS: usize = 2;
const LOADER_CARD_H: f32 = 124.;

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

    let mut root = rect()
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
        );

    if chosen != LoaderChoice::Vanilla && !picks.loader_versions.is_empty() {
        let options = picks.loader_versions.clone();
        let selected = picks
            .loader_version
            .clone()
            .or_else(|| options.first().cloned())
            .unwrap_or_default();

        root = root.child(
            rect()
                .key("loader-version")
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
                ),
        );
    }

    root.into_element()
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

fn version_step(mut wizard: Wizard, picks: &Picks) -> Element {
    let oneclient = picks.choice == Some(TypeChoice::OneClient);
    let filter = *wizard.filter.read();
    let chosen = picks.version.clone();

    let controls = rect()
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
        .maybe_child((!oneclient).then(|| {
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
        }));

    let list: Element = if let Some(error) = &picks.versions_error {
        centered_failure(error)
    } else if !picks.versions_loaded {
        centered_spinner("Loading versions...")
    } else if picks.versions.is_empty() {
        centered_note("No versions match that search.")
    } else {
        let versions = picks.versions.clone();
        let count = versions.len();
        let selected = chosen.clone();

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
    };

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
                .child(controls),
        )
        .child(
            rect()
                .key("version-list")
                .width(Size::fill())
                .height(Size::flex(1.0))
                .child(list),
        )
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
    picks.filter.hash(&mut hasher);
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
    if picks.archives.is_empty() {
        return centered_note(if picks.bundles_loaded {
            "No bundles ship for this version yet."
        } else {
            "Loading bundles..."
        });
    }

    let cards: Vec<Element> = picks
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
                blurb: bundle_blurb(archive, &picks.package_names),
                meta: Some(match count {
                    1 => "1 package".to_string(),
                    many => format!("{many} packages"),
                }),
                corner: None,
                selected: !picks.declined.contains(&name),
                checkbox: true,
                on_press: {
                    let defaults = picks.declined.clone();
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

#[derive(PartialEq)]
struct SelectCard {
    id: String,
    selected: bool,
    height: Option<f32>,
    padding: Gaps,
    content: Element,
    on_press: EventHandler<()>,
}

impl Component for SelectCard {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);
        let hovered = *hovering.read();
        let on_press = self.on_press.clone();

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let focused = focus().is_focused();

        let ring = if self.selected || focused {
            colors::brand()
        } else if hovered {
            colors::component_border_hover()
        } else {
            colors::component_border()
        };

        rect()
            .key(self.id.clone())
            .width(Size::fill())
            .height(self.height.map_or_else(Size::auto, Size::px))
            .padding(self.padding)
            .corner_radius(CornerRadius::new_all(CARD_RADIUS))
            .overflow(Overflow::Clip)
            .background(if hovered || focused {
                colors::component_bg_hover()
            } else {
                colors::component_bg()
            })
            .border(border_all_color(1., ring))
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .cursor(CursorIcon::Pointer)
            .on_press(move |_| on_press.call(()))
            .on_pointer_enter(move |_| hovering.set(true))
            .on_pointer_leave(move |_| hovering.set(false))
            .child(self.content.clone())
    }
}

fn loader_mark(choice: LoaderChoice, selected: bool, size: f32) -> Option<Element> {
    let mark = choice.mark()?;

    let drawn = match mark {
        LoaderMark::Tinted(icon) => {
            AppAssets::get_bytes(icon.path()).map(|_| Icon::new(icon).size(size).into_element())
        }
        LoaderMark::Image(path) => {
            AppAssets::get_bytes(path).map(|_| AssetImage::new(path, size).into_element())
        }
    };

    if let Some(drawn) = drawn {
        return Some(
            rect()
                .width(Size::px(size))
                .height(Size::px(size))
                .corner_radius(CornerRadius::new_all(5.))
                .overflow(Overflow::Clip)
                .opacity(if selected { 1.0 } else { 0.7 })
                .child(drawn)
                .into_element(),
        );
    }

    let color = if selected {
        colors::fg_primary()
    } else {
        colors::fg_secondary()
    };

    Some(
        rect()
            .width(Size::px(size))
            .height(Size::px(size))
            .center()
            .corner_radius(CornerRadius::new_all(5.))
            .border(border_all_color(1., colors::component_border()))
            .child(
                label()
                    .text(choice.short())
                    .font_size(9.)
                    .font_weight(FontWeight::SEMI_BOLD)
                    .letter_spacing(0.4)
                    .color(color),
            )
            .into_element(),
    )
}

fn marker(selected: bool, checkbox: bool) -> Element {
    let radius = if checkbox { MARKER_RADIUS } else { 999. };

    rect()
        .width(Size::px(MARKER_SIZE))
        .height(Size::px(MARKER_SIZE))
        .center()
        .corner_radius(CornerRadius::new_all(radius))
        .background(if selected {
            colors::brand()
        } else {
            colors::page_elevated()
        })
        .border(border_all_color(
            1.,
            if selected {
                colors::brand()
            } else {
                colors::component_border()
            },
        ))
        .maybe_child(selected.then(|| {
            if checkbox {
                Icon::new(IconType::Check)
                    .size(MARKER_MARK)
                    .color(Color::WHITE)
                    .into_element()
            } else {
                rect()
                    .width(Size::px(MARKER_DOT))
                    .height(Size::px(MARKER_DOT))
                    .corner_radius(CornerRadius::new_all(999.))
                    .background(Color::WHITE)
                    .into_element()
            }
        }))
        .into_element()
}

struct WideCard {
    icon: IconType,
    title: String,
    badge: Option<String>,
    blurb: String,
    meta: String,
    selected: bool,
    on_press: EventHandler<()>,
}

fn wide_card(card: WideCard) -> Element {
    let WideCard {
        icon,
        title,
        badge,
        blurb,
        meta,
        selected,
        on_press,
    } = card;
    let title_id = title.clone();

    let content = rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Start)
        .spacing(16.)
        .child(
            rect()
                .width(Size::px(44.))
                .height(Size::px(44.))
                .center()
                .corner_radius(CornerRadius::new_all(10.))
                .background(if selected {
                    colors::brand()
                } else {
                    colors::page_elevated()
                })
                .border(border_all_color(1., colors::component_border()))
                .child(Icon::new(icon).size(22.).color(if selected {
                    Color::WHITE
                } else {
                    colors::fg_primary()
                })),
        )
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(7.)
                .child(
                    rect()
                        .horizontal()
                        .cross_align(Alignment::Center)
                        .spacing(10.)
                        .child(
                            label()
                                .text(title)
                                .font_size(15.)
                                .font_weight(FontWeight::MEDIUM)
                                .color(colors::fg_primary()),
                        )
                        .maybe_child(
                            badge.map(|badge| filled_pill(badge, colors::brand(), Color::WHITE)),
                        ),
                )
                .child(
                    label()
                        .text(blurb)
                        .font_size(12.)
                        .line_height(1.45)
                        .color(colors::fg_secondary()),
                )
                .child(
                    label()
                        .text(meta)
                        .font_size(11.)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(marker(selected, false))
        .into_element();

    SelectCard {
        id: title_id,
        selected,
        height: None,
        padding: Gaps::new_all(16.),
        content,
        on_press,
    }
    .into_element()
}

struct CellCard {
    id: String,
    title: String,
    blurb: String,
    meta: Option<String>,
    corner: Option<Element>,
    selected: bool,
    checkbox: bool,
    on_press: EventHandler<()>,
}

fn cell_card(card: CellCard) -> Element {
    let CellCard {
        id,
        title,
        blurb,
        meta,
        corner,
        selected,
        checkbox,
        on_press,
    } = card;

    let content = rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .main_align(Alignment::SpaceBetween)
        .spacing(8.)
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(8.)
                .child(
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .content(Content::Flex)
                        .cross_align(Alignment::Center)
                        .spacing(10.)
                        .child(marker(selected, checkbox))
                        .child(
                            label()
                                .text(title)
                                .width(Size::flex(1.0))
                                .font_size(14.)
                                .font_weight(FontWeight::MEDIUM)
                                .max_lines(1)
                                .color(colors::fg_primary()),
                        )
                        .maybe_child(corner),
                )
                .child(
                    label()
                        .text(blurb)
                        .width(Size::fill())
                        .font_size(12.)
                        .line_height(1.4)
                        .max_lines(3)
                        .color(colors::fg_secondary()),
                ),
        )
        .maybe_child(meta.map(|meta| {
            label()
                .text(meta)
                .font_size(11.)
                .max_lines(1)
                .color(colors::fg_secondary())
                .into_element()
        }))
        .into_element();

    SelectCard {
        id,
        selected,
        height: None,
        padding: Gaps::new_all(14.),
        content,
        on_press,
    }
    .into_element()
}

fn version_row(row: &VersionRow, selected: bool, on_press: EventHandler<()>) -> Element {
    let content = rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(14.)
        .child(marker(selected, false))
        .child(
            label()
                .text(row.id.clone())
                .width(Size::px(110.))
                .font_size(14.)
                .font_weight(FontWeight::MEDIUM)
                .max_lines(1)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text(row.meta.clone())
                .width(Size::flex(1.0))
                .font_size(12.)
                .max_lines(1)
                .text_align(TextAlign::Right)
                .color(colors::fg_secondary()),
        )
        .maybe_child(
            row.badge
                .as_ref()
                .map(|badge| pill(None, badge.clone(), colors::fg_secondary())),
        )
        .maybe_child((!row.date.is_empty()).then(|| {
            label()
                .text(row.date.clone())
                .width(Size::px(84.))
                .font_size(12.)
                .max_lines(1)
                .text_align(TextAlign::Right)
                .color(colors::fg_secondary())
                .into_element()
        }))
        .into_element();

    SelectCard {
        id: row.id.clone(),
        selected,
        height: Some(VERSION_ROW_H),
        padding: Gaps::new_symmetric(0., 14.),
        content,
        on_press,
    }
    .into_element()
}
