use std::path::PathBuf;

use freya::prelude::*;

use crate::components::{ART_PREVIEW_EDGE, Button, Icon, IconType, LocalImage, TextInput};
use crate::theme::colors;
use crate::ui::border_all_color;

const CARD_RADIUS: f32 = 12.;
const PREVIEW_W: f32 = 132.;
const PREVIEW_H: f32 = 74.;

#[derive(Clone, Copy)]
pub struct DetailsState {
    pub name: State<String>,
    pub name_touched: State<bool>,
    pub description: State<String>,
    pub tags: State<Vec<String>>,
    pub tag_draft: State<String>,
    pub tag_open: State<bool>,
    pub cover: State<Option<PathBuf>>,
    pub cover_cleared: State<bool>,
}

impl DetailsState {
    pub fn blank() -> Self {
        Self {
            name: use_state(String::new),
            name_touched: use_state(|| false),
            description: use_state(String::new),
            tags: use_state(Vec::new),
            tag_draft: use_state(String::new),
            tag_open: use_state(|| false),
            cover: use_state(|| None::<PathBuf>),
            cover_cleared: use_state(|| false),
        }
    }

    pub fn seeded(name: &str, description: Option<&str>, tags: &[String]) -> Self {
        let name = name.to_string();
        let description = description.unwrap_or_default().to_string();
        let tags = tags.to_vec();

        Self {
            name: use_state(move || name.clone()),
            name_touched: use_state(|| true),
            description: use_state(move || description.clone()),
            tags: use_state(move || tags.clone()),
            tag_draft: use_state(String::new),
            tag_open: use_state(|| false),
            cover: use_state(|| None::<PathBuf>),
            cover_cleared: use_state(|| false),
        }
    }

    pub fn effective_name(&self, suggested: &str) -> String {
        let typed = self.typed_name();
        if *self.name_touched.read() && !typed.is_empty() {
            typed
        } else {
            suggested.to_string()
        }
    }

    pub fn typed_name(&self) -> String {
        self.name.read().trim().to_string()
    }

    pub fn description_value(&self) -> Option<String> {
        let text = self.description.read().trim().to_string();
        (!text.is_empty()).then_some(text)
    }

    pub fn preview_cover(&self, existing: Option<PathBuf>) -> Option<(PathBuf, bool)> {
        if let Some(picked) = self.cover.read().clone() {
            return Some((picked, true));
        }
        if *self.cover_cleared.read() {
            return None;
        }
        existing.map(|path| (path, false))
    }
}

pub fn details_body(
    state: DetailsState,
    placeholder: String,
    existing_cover: Option<PathBuf>,
) -> Element {
    let mut touched = state.name_touched;
    let tags = state.tags.read().clone();
    let tag_open = *state.tag_open.read();
    let preview = state.preview_cover(existing_cover);

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(18.)
        .child(field(
            "Name",
            TextInput::new(state.name)
                .placeholder(placeholder)
                .width(Size::fill())
                .on_validate(move |_| {
                    touched.set(true);
                })
                .into_element(),
        ))
        .child(field(
            "Description",
            TextInput::new(state.description)
                .placeholder("What is this instance for?")
                .multiline(true)
                .width(Size::fill())
                .height(Size::px(76.))
                .into_element(),
        ))
        .child(field("Tags", tag_field(state, &tags, tag_open)))
        .child(field("Background image", cover_field(state, preview)))
        .into_element()
}

fn commit_tag(state: DetailsState) {
    let mut tag_draft = state.tag_draft;
    let mut tag_open = state.tag_open;
    let mut all_tags = state.tags;

    let draft = tag_draft.read().trim().to_string();
    if !draft.is_empty() {
        let mut next = all_tags.read().clone();
        if !next.iter().any(|tag| tag == &draft) {
            next.push(draft);
            all_tags.set(next);
        }
    }
    tag_draft.set(String::new());
    tag_open.set(false);
}

fn tag_field(state: DetailsState, tags: &[String], open: bool) -> Element {
    let mut tag_draft = state.tag_draft;
    let mut tag_open = state.tag_open;
    let mut all_tags = state.tags;

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
                    let next: Vec<String> = all_tags
                        .read()
                        .iter()
                        .filter(|tag| *tag != &removed)
                        .cloned()
                        .collect();
                    all_tags.set(next);
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
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(6.)
                .child(
                    rect().width(Size::px(150.)).child(
                        TextInput::new(tag_draft)
                            .placeholder("Add a tag")
                            .auto_focus(true)
                            .width(Size::fill())
                            .on_submit(move |_| commit_tag(state))
                            .into_element(),
                    ),
                )
                .child(
                    Button::new()
                        .primary()
                        .icon()
                        .alt("Add this tag")
                        .width(Size::px(28.))
                        .height(Size::px(28.))
                        .on_press(move |_| commit_tag(state))
                        .child(Icon::new(IconType::Check).size(14.)),
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
                    tag_draft.set(String::new());
                    tag_open.set(true);
                })
                .child(Icon::new(IconType::Plus).size(14.))
                .into_element()
        })
        .into_element()
}

fn cover_field(state: DetailsState, preview: Option<(PathBuf, bool)>) -> Element {
    let mut cover = state.cover;
    let mut cleared = state.cover_cleared;

    let pick = move |_| {
        spawn(async move {
            if let Some(handle) = rfd::AsyncFileDialog::new()
                .set_title("Choose a background image")
                .add_filter("Image", &["png", "jpg", "jpeg", "webp", "gif"])
                .pick_file()
                .await
            {
                cover.set(Some(handle.path().to_path_buf()));
                cleared.set(false);
            }
        });
    };

    let has_cover = preview.is_some();

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
                .child(match preview {
                    Some((path, picked)) => LocalImage::new(path, ART_PREVIEW_EDGE, true)
                        .picked(picked)
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
                                .on_press(move |_| {
                                    cover.set(None);
                                    cleared.set(true);
                                })
                                .text("Use version art")
                                .into_element()
                        })),
                ),
        )
        .into_element()
}

pub fn field(title: &'static str, control: Element) -> Element {
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
