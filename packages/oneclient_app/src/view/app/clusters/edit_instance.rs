use std::path::PathBuf;

use freya::prelude::*;

use crate::components::{ART_PREVIEW_EDGE, Button, LocalImage, OverlayPopup, TextInput};
use crate::hooks::{ClusterAction, use_cluster_mutation};
use crate::theme::colors;
use crate::ui::border_all_color;

const DIALOG_WIDTH: f32 = 460.;
const PREVIEW_HEIGHT: f32 = 120.;

#[derive(PartialEq)]
pub struct EditInstanceModal {
    cluster_id: i64,
    name: String,
    description: String,
    tags: String,
    cover: Option<PathBuf>,
    on_close: EventHandler<()>,
}

impl EditInstanceModal {
    pub fn new(
        cluster_id: i64,
        name: String,
        description: Option<String>,
        tags: Vec<String>,
        cover: Option<PathBuf>,
        on_close: impl Into<EventHandler<()>>,
    ) -> Self {
        Self {
            cluster_id,
            name,
            description: description.unwrap_or_default(),
            tags: tags.join(", "),
            cover,
            on_close: on_close.into(),
        }
    }
}

impl Component for EditInstanceModal {
    fn render(&self) -> impl IntoElement {
        let mutation = use_cluster_mutation();
        let cluster_id = self.cluster_id;

        let name = use_state({
            let initial = self.name.clone();
            move || initial.clone()
        });
        let description = use_state({
            let initial = self.description.clone();
            move || initial.clone()
        });
        let tags = use_state({
            let initial = self.tags.clone();
            move || initial.clone()
        });
        let picked = use_state(|| None::<PathBuf>);
        let cleared = use_state(|| false);

        let existing = self.cover.clone();
        let chosen = picked.read().clone();
        let from_disk = chosen.is_some();
        let preview = if *cleared.read() {
            None
        } else {
            chosen.or_else(|| existing.clone())
        };

        let has_cover = preview.is_some();
        let ready = !name.read().trim().is_empty();

        let close_scrim = self.on_close.clone();
        let close_cancel = self.on_close.clone();
        let close_save = self.on_close.clone();

        OverlayPopup::new()
            .on_close(move |()| close_scrim.call(()))
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(DIALOG_WIDTH))
                            .max_width(Size::window_percent(92.))
                            .spacing(16.)
                            .padding(Gaps::new_all(20.))
                            .corner_radius(CornerRadius::new_all(16.))
                            .background(colors::page_elevated())
                            .border(border_all_color(1., colors::component_border()))
                            .child(
                                label()
                                    .text("Edit instance")
                                    .font_size(18.)
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .color(colors::fg_primary()),
                            )
                            .child(field(
                                "Name",
                                TextInput::new(name).width(Size::fill()).into_element(),
                            ))
                            .child(field(
                                "Description",
                                TextInput::new(description)
                                    .placeholder("What is this instance for?")
                                    .multiline(true)
                                    .width(Size::fill())
                                    .height(Size::px(64.))
                                    .into_element(),
                            ))
                            .child(field(
                                "Tags",
                                TextInput::new(tags)
                                    .placeholder("pvp, skyblock")
                                    .width(Size::fill())
                                    .into_element(),
                            ))
                            .child(field(
                                "Cover",
                                cover_control(preview, from_disk, picked, cleared, has_cover),
                            ))
                            .child(
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .main_align(Alignment::End)
                                    .spacing(8.)
                                    .child(
                                        Button::new()
                                            .ghost()
                                            .on_press(move |_| close_cancel.call(()))
                                            .text("Cancel"),
                                    )
                                    .child(
                                        Button::new()
                                            .primary()
                                            .enabled(ready)
                                            .on_press(move |_| {
                                                let description =
                                                    description.read().trim().to_string();
                                                mutation.mutate(ClusterAction::UpdateInstance {
                                                    cluster_id,
                                                    name: name.read().trim().to_string(),
                                                    description: (!description.is_empty())
                                                        .then_some(description),
                                                    tags: parse_tags(&tags.read()),
                                                    cover_source: picked.read().clone(),
                                                    clear_cover: *cleared.read(),
                                                });
                                                close_save.call(());
                                            })
                                            .text("Save"),
                                    ),
                            ),
                    ),
            )
            .into_element()
    }
}

fn cover_control(
    preview: Option<PathBuf>,
    from_disk: bool,
    mut picked: State<Option<PathBuf>>,
    mut cleared: State<bool>,
    has_cover: bool,
) -> Element {
    let pick = move |_| {
        spawn(async move {
            if let Some(handle) = rfd::AsyncFileDialog::new()
                .set_title("Choose a cover image")
                .add_filter("Image", &["png", "jpg", "jpeg", "webp", "gif"])
                .pick_file()
                .await
            {
                picked.set(Some(handle.path().to_path_buf()));
                cleared.set(false);
            }
        });
    };

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(8.)
        .maybe_child(preview.map(|path| {
            rect()
                .width(Size::fill())
                .height(Size::px(PREVIEW_HEIGHT))
                .corner_radius(CornerRadius::new_all(10.))
                .overflow(Overflow::Clip)
                .border(border_all_color(1., colors::component_border()))
                .child(
                    LocalImage::new(path, ART_PREVIEW_EDGE, true)
                        .picked(from_disk)
                        .skeleton(true),
                )
                .into_element()
        }))
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .spacing(8.)
                .child(
                    Button::new()
                        .secondary()
                        .small()
                        .on_press(pick)
                        .text("Choose image"),
                )
                .maybe_child(has_cover.then(|| {
                    Button::new()
                        .ghost()
                        .small()
                        .on_press(move |_| {
                            picked.set(None);
                            cleared.set(true);
                        })
                        .text("Remove")
                        .into_element()
                })),
        )
        .into_element()
}

fn parse_tags(raw: &str) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    for tag in raw.split(',') {
        let tag = tag.trim();
        if !tag.is_empty() && !tags.iter().any(|existing| existing == tag) {
            tags.push(tag.to_string());
        }
    }
    tags
}

fn field(title: &'static str, control: Element) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(6.)
        .child(
            label()
                .text(title)
                .font_size(11.)
                .font_weight(FontWeight::MEDIUM)
                .color(colors::fg_secondary()),
        )
        .child(control)
        .into_element()
}
