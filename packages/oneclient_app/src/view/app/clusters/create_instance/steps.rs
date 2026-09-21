use std::collections::HashMap;
use std::path::PathBuf;

use freya::prelude::*;
use oneclient_core::BundleArchive;

use super::{FILTERS, LoaderChoice, Picks, Step, TypeChoice, VersionRow, Wizard};
use crate::components::{
    ART_PREVIEW_EDGE, Button, Dropdown, Icon, IconType, LocalImage, TextInput,
};
use crate::theme::colors;
use crate::ui::{border_all_color, centered_note};

const CARD_RADIUS: f32 = 12.;
const BUNDLE_COLUMNS: usize = 3;
const BUNDLE_GAP: f32 = 10.;
const BUNDLE_CARD_H: f32 = 132.;
const PREVIEW_W: f32 = 132.;
const PREVIEW_H: f32 = 74.;

pub fn body(wizard: Wizard, picks: &Picks) -> Element {
    match picks.step {
        Step::Type => type_step(wizard, picks),
        Step::Loader => loader_step(wizard, picks),
        Step::Version => version_step(wizard, picks),
        Step::Bundles => bundles_step(wizard, picks),
        Step::Customize => customize_step(wizard, picks),
    }
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
                    ChoiceCard {
                        id: title.to_string(),
                        icon: Some(icon),
                        title: title.to_string(),
                        badge: badge.map(str::to_string),
                        blurb: blurb.to_string(),
                        meta: Some(meta.to_string()),
                        selected: picks.choice == Some(choice),
                        on_press: (move |()| {
                            wizard.choice.set(Some(choice));
                            wizard.version.set(None);
                            wizard.declined.set(None);
                        })
                        .into(),
                    }
                    .into_element()
                }),
        )
        .into_element()
}

fn loader_step(mut wizard: Wizard, picks: &Picks) -> Element {
    let chosen = *wizard.loader.read();

    let cards: Vec<Element> = LoaderChoice::ALL
        .into_iter()
        .map(|choice| {
            ChoiceCard {
                id: choice.name().to_string(),
                icon: None,
                title: choice.name().to_string(),
                badge: None,
                blurb: choice.blurb().to_string(),
                meta: None,
                selected: chosen == choice,
                on_press: (move |()| {
                    wizard.loader.set(choice);
                    wizard.loader_version.set(None);
                    wizard.version.set(None);
                })
                .into(),
            }
            .into_element()
        })
        .collect();

    let mut root = rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .children(cards);

    if chosen != LoaderChoice::Vanilla && !picks.loader_versions.is_empty() {
        let options = picks.loader_versions.clone();
        let selected = picks
            .loader_version
            .clone()
            .or_else(|| options.first().cloned())
            .unwrap_or_default();

        root = root.child(
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
                ),
        );
    }

    root.into_element()
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

    let list: Element = if picks.versions.is_empty() {
        if picks.versions_loaded {
            centered_note("No versions match that search.")
        } else {
            centered_note("Loading versions...")
        }
    } else {
        rect()
            .vertical()
            .width(Size::fill())
            .spacing(4.)
            .children(picks.versions.iter().map(|row| {
                let id = row.id.clone();
                VersionRowItem {
                    row: VersionRow {
                        id: row.id.clone(),
                        meta: row.meta.clone(),
                        badge: row.badge.clone(),
                        date: row.date.clone(),
                    },
                    selected: chosen.as_deref() == Some(row.id.as_str()),
                    on_press: (move |()| {
                        wizard.version.set(Some(id.clone()));
                        wizard.loader_version.set(None);
                    })
                    .into(),
                }
                .into_element()
            }))
            .into_element()
    };

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(12.)
        .child(controls)
        .child(list)
        .into_element()
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

            BundleCard {
                id: name.clone(),
                title: bundle_title(archive),
                blurb: bundle_blurb(archive, &picks.package_names),
                count: match count {
                    1 => "1 package".to_string(),
                    many => format!("{many} packages"),
                },
                selected: !picks.declined.contains(&name),
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
            }
            .into_element()
        })
        .collect();

    let mut grid = rect().vertical().width(Size::fill()).spacing(BUNDLE_GAP);

    for chunk in cards.chunks(BUNDLE_COLUMNS) {
        let mut row = rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(BUNDLE_CARD_H))
            .content(Content::Flex)
            .spacing(BUNDLE_GAP);

        for card in chunk {
            row = row.child(
                rect()
                    .width(Size::flex(1.0))
                    .height(Size::fill())
                    .child(card.clone()),
            );
        }
        for _ in chunk.len()..BUNDLE_COLUMNS {
            row = row.child(rect().width(Size::flex(1.0)).height(Size::fill()));
        }

        grid = grid.child(row);
    }

    grid.into_element()
}

fn customize_step(mut wizard: Wizard, picks: &Picks) -> Element {
    let cover = wizard.cover.read().clone();
    let tags = wizard.tags.read().clone();
    let tag_open = *wizard.tag_open.read();

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(18.)
        .child(field(
            "Name",
            TextInput::new(wizard.name)
                .placeholder(picks.suggested.clone())
                .width(Size::fill())
                .on_validate(move |_| {
                    wizard.name_touched.set(true);
                })
                .into_element(),
        ))
        .child(field(
            "Description",
            TextInput::new(wizard.description)
                .placeholder("What is this instance for?")
                .multiline(true)
                .width(Size::fill())
                .height(Size::px(76.))
                .into_element(),
        ))
        .child(field("Tags", tag_field(wizard, &tags, tag_open)))
        .child(field("Background image", cover_field(wizard, cover)))
        .into_element()
}

fn tag_field(mut wizard: Wizard, tags: &[String], open: bool) -> Element {
    let mut commit = move |()| {
        let draft = wizard.tag_draft.read().trim().to_string();
        if !draft.is_empty() {
            let mut next = wizard.tags.read().clone();
            if !next.iter().any(|tag| tag == &draft) {
                next.push(draft);
                wizard.tags.set(next);
            }
        }
        wizard.tag_draft.set(String::new());
        wizard.tag_open.set(false);
    };

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::wrap_spacing(8.))
        .cross_align(Alignment::Center)
        .spacing(8.)
        .children(tags.iter().map(|tag| {
            let removed = tag.clone();
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(8.)
                .height(Size::px(28.))
                .padding(Gaps::new_symmetric(0., 10.))
                .corner_radius(CornerRadius::new_all(8.))
                .background(colors::brand())
                .cursor(CursorIcon::Pointer)
                .on_press(move |_| {
                    let next: Vec<String> = wizard
                        .tags
                        .read()
                        .iter()
                        .filter(|tag| *tag != &removed)
                        .cloned()
                        .collect();
                    wizard.tags.set(next);
                })
                .child(
                    label()
                        .text(tag.clone())
                        .font_size(12.)
                        .font_weight(FontWeight::MEDIUM)
                        .color(Color::WHITE),
                )
                .child(Icon::new(IconType::XClose).size(10.).color(Color::WHITE))
                .into_element()
        }))
        .child(if open {
            rect()
                .width(Size::px(150.))
                .child(
                    TextInput::new(wizard.tag_draft)
                        .placeholder("Add a tag")
                        .auto_focus(true)
                        .width(Size::fill())
                        .on_submit(move |_| commit(()))
                        .into_element(),
                )
                .into_element()
        } else {
            Button::new()
                .secondary()
                .icon()
                .alt("Add a tag")
                .width(Size::px(28.))
                .height(Size::px(28.))
                .on_press(move |_| {
                    wizard.tag_draft.set(String::new());
                    wizard.tag_open.set(true);
                })
                .child(Icon::new(IconType::Plus).size(14.))
                .into_element()
        })
        .into_element()
}

fn cover_field(mut wizard: Wizard, cover: Option<PathBuf>) -> Element {
    let pick = move |_| {
        spawn(async move {
            if let Some(handle) = rfd::AsyncFileDialog::new()
                .set_title("Choose a background image")
                .add_filter("Image", &["png", "jpg", "jpeg", "webp", "gif"])
                .pick_file()
                .await
            {
                wizard.cover.set(Some(handle.path().to_path_buf()));
            }
        });
    };

    let has_cover = cover.is_some();

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(14.)
        .padding(Gaps::new_all(12.))
        .corner_radius(CornerRadius::new_all(CARD_RADIUS))
        .background(colors::component_bg())
        .border(border_all_color(1., colors::component_border()))
        .child(
            rect()
                .width(Size::px(PREVIEW_W))
                .height(Size::px(PREVIEW_H))
                .corner_radius(CornerRadius::new_all(8.))
                .overflow(Overflow::Clip)
                .border(border_all_color(1., colors::component_border()))
                .center()
                .child(match cover {
                    Some(path) => LocalImage::new(path, ART_PREVIEW_EDGE, true)
                        .picked(true)
                        .skeleton(true)
                        .into_element(),
                    None => label()
                        .text("Version art")
                        .font_size(11.)
                        .color(colors::fg_secondary())
                        .into_element(),
                }),
        )
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(8.)
                .child(
                    label()
                        .text("Shown behind the instance on the home page and on its card. PNG or JPG, 1280x720 or larger.")
                        .font_size(12.)
                        .line_height(1.4)
                        .color(colors::fg_secondary()),
                )
                .child(
                    rect()
                        .horizontal()
                        .spacing(8.)
                        .cross_align(Alignment::Center)
                        .child(
                            Button::new()
                                .secondary()
                                .small()
                                .on_press(pick)
                                .text("Choose file"),
                        )
                        .maybe_child(has_cover.then(|| {
                            Button::new()
                                .ghost()
                                .small()
                                .on_press(move |_| wizard.cover.set(None))
                                .text("Use version art")
                                .into_element()
                        })),
                ),
        )
        .into_element()
}

fn field(title: &'static str, control: Element) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(8.)
        .child(
            label()
                .text(title)
                .font_size(12.)
                .font_weight(FontWeight::MEDIUM)
                .color(colors::fg_primary()),
        )
        .child(control)
        .into_element()
}

#[derive(PartialEq)]
struct ChoiceCard {
    id: String,
    icon: Option<IconType>,
    title: String,
    badge: Option<String>,
    blurb: String,
    meta: Option<String>,
    selected: bool,
    on_press: EventHandler<()>,
}

impl Component for ChoiceCard {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);
        let hovered = *hovering.read();
        let selected = self.selected;
        let on_press = self.on_press.clone();

        let ring = if selected {
            colors::brand()
        } else if hovered {
            colors::component_border_hover()
        } else {
            colors::component_border()
        };
        let background = if hovered {
            colors::component_bg_hover()
        } else {
            colors::component_bg()
        };

        rect()
            .key(self.id.clone())
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .cross_align(Alignment::Start)
            .spacing(16.)
            .padding(Gaps::new_all(16.))
            .corner_radius(CornerRadius::new_all(CARD_RADIUS))
            .background(background)
            .border(border_all_color(1., ring))
            .cursor(CursorIcon::Pointer)
            .on_press(move |_| on_press.call(()))
            .on_pointer_enter(move |_| hovering.set(true))
            .on_pointer_leave(move |_| hovering.set(false))
            .maybe_child(self.icon.map(|icon| {
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
                    }))
                    .into_element()
            }))
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
                                    .text(self.title.clone())
                                    .font_size(15.)
                                    .font_weight(FontWeight::MEDIUM)
                                    .color(colors::fg_primary()),
                            )
                            .maybe_child(self.badge.as_ref().map(|badge| {
                                rect()
                                    .padding(Gaps::new_symmetric(2., 8.))
                                    .corner_radius(CornerRadius::new_all(6.))
                                    .background(colors::brand())
                                    .child(
                                        label()
                                            .text(badge.clone())
                                            .font_size(10.)
                                            .font_weight(FontWeight::SEMI_BOLD)
                                            .letter_spacing(1.)
                                            .color(Color::WHITE),
                                    )
                                    .into_element()
                            })),
                    )
                    .child(
                        label()
                            .text(self.blurb.clone())
                            .font_size(12.)
                            .line_height(1.45)
                            .color(colors::fg_secondary()),
                    )
                    .maybe_child(self.meta.as_ref().map(|meta| {
                        label()
                            .text(meta.clone())
                            .font_size(11.)
                            .color(colors::fg_secondary())
                            .into_element()
                    })),
            )
            .child(marker(selected, false))
    }
}

fn marker(selected: bool, checkbox: bool) -> Element {
    let radius = if checkbox { 6. } else { 999. };

    rect()
        .width(Size::px(20.))
        .height(Size::px(20.))
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
                    .size(12.)
                    .color(Color::WHITE)
                    .into_element()
            } else {
                rect()
                    .width(Size::px(8.))
                    .height(Size::px(8.))
                    .corner_radius(CornerRadius::new_all(999.))
                    .background(Color::WHITE)
                    .into_element()
            }
        }))
        .into_element()
}

#[derive(PartialEq)]
struct BundleCard {
    id: String,
    title: String,
    blurb: String,
    count: String,
    selected: bool,
    on_press: EventHandler<()>,
}

impl Component for BundleCard {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);
        let hovered = *hovering.read();
        let selected = self.selected;
        let on_press = self.on_press.clone();

        let ring = if selected {
            colors::brand()
        } else if hovered {
            colors::component_border_hover()
        } else {
            colors::component_border()
        };

        rect()
            .key(self.id.clone())
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .main_align(Alignment::SpaceBetween)
            .spacing(8.)
            .padding(Gaps::new_all(14.))
            .corner_radius(CornerRadius::new_all(CARD_RADIUS))
            .overflow(Overflow::Clip)
            .background(if hovered {
                colors::component_bg_hover()
            } else {
                colors::component_bg()
            })
            .border(border_all_color(1., ring))
            .cursor(CursorIcon::Pointer)
            .on_press(move |_| on_press.call(()))
            .on_pointer_enter(move |_| hovering.set(true))
            .on_pointer_leave(move |_| hovering.set(false))
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .spacing(10.)
                    .child(marker(selected, true))
                    .child(
                        label()
                            .text(self.title.clone())
                            .width(Size::flex(1.0))
                            .font_size(14.)
                            .font_weight(FontWeight::MEDIUM)
                            .max_lines(1)
                            .color(colors::fg_primary()),
                    ),
            )
            .child(
                label()
                    .text(self.blurb.clone())
                    .width(Size::fill())
                    .font_size(12.)
                    .line_height(1.4)
                    .max_lines(3)
                    .color(colors::fg_secondary()),
            )
            .child(
                label()
                    .text(self.count.clone())
                    .font_size(11.)
                    .max_lines(1)
                    .color(colors::fg_secondary()),
            )
    }
}

#[derive(PartialEq)]
struct VersionRowItem {
    row: VersionRow,
    selected: bool,
    on_press: EventHandler<()>,
}

impl PartialEq for VersionRow {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.meta == other.meta
            && self.badge == other.badge
            && self.date == other.date
    }
}

impl Component for VersionRowItem {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);
        let hovered = *hovering.read();
        let selected = self.selected;
        let on_press = self.on_press.clone();

        let ring = if selected {
            colors::brand()
        } else if hovered {
            colors::component_border_hover()
        } else {
            colors::component_border()
        };

        rect()
            .key(self.row.id.clone())
            .horizontal()
            .width(Size::fill())
            .height(Size::px(48.))
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(14.)
            .padding(Gaps::new_symmetric(0., 14.))
            .corner_radius(CornerRadius::new_all(10.))
            .background(if hovered {
                colors::component_bg_hover()
            } else {
                colors::component_bg()
            })
            .border(border_all_color(1., ring))
            .cursor(CursorIcon::Pointer)
            .on_press(move |_| on_press.call(()))
            .on_pointer_enter(move |_| hovering.set(true))
            .on_pointer_leave(move |_| hovering.set(false))
            .child(marker(selected, false))
            .child(
                label()
                    .text(self.row.id.clone())
                    .width(Size::px(110.))
                    .font_size(14.)
                    .font_weight(FontWeight::MEDIUM)
                    .max_lines(1)
                    .color(colors::fg_primary()),
            )
            .child(
                label()
                    .text(self.row.meta.clone())
                    .width(Size::flex(1.0))
                    .font_size(12.)
                    .max_lines(1)
                    .text_align(TextAlign::Right)
                    .color(colors::fg_secondary()),
            )
            .maybe_child(self.row.badge.as_ref().map(|badge| {
                rect()
                    .padding(Gaps::new_symmetric(2., 7.))
                    .corner_radius(CornerRadius::new_all(6.))
                    .background(colors::page_elevated())
                    .border(border_all_color(1., colors::component_border()))
                    .child(
                        label()
                            .text(badge.clone())
                            .font_size(10.)
                            .font_weight(FontWeight::MEDIUM)
                            .letter_spacing(0.8)
                            .color(colors::fg_secondary()),
                    )
                    .into_element()
            }))
            .maybe_child((!self.row.date.is_empty()).then(|| {
                label()
                    .text(self.row.date.clone())
                    .width(Size::px(84.))
                    .font_size(12.)
                    .max_lines(1)
                    .text_align(TextAlign::Right)
                    .color(colors::fg_secondary())
                    .into_element()
            }))
    }
}
