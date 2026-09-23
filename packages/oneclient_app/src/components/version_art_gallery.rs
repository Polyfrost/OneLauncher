use freya::prelude::*;
use oneclient_core::images::DEFAULT_IMAGE_EDGE;

use crate::components::overlay_popup::OVERLAY_MAX_LEVEL;
use crate::components::spinner::Spinner;
use crate::components::{
    ART_PREVIEW_EDGE, Button, DynamicArt, Icon, IconType, OverlayPopup, ScrollArea,
};
use crate::hooks::{refresh_version_art_gallery, use_version_art_gallery};
use crate::theme::colors;
use crate::ui::border_all_color;

const DIALOG_WIDTH: f32 = 880.;
const DIALOG_HEIGHT: f32 = 620.;
const PANE_PADDING: f32 = 24.;
const TILE_W: f32 = 248.;
const TILE_H: f32 = 140.;
const TILE_GAP: f32 = 14.;
const COLUMNS: usize = 3;

#[derive(PartialEq, Clone)]
pub struct VersionArtGallery {
    pending: State<Option<String>>,
    error: State<Option<String>>,
    on_pick: EventHandler<String>,
    on_close: EventHandler<()>,
}

impl VersionArtGallery {
    pub fn new(
        pending: State<Option<String>>,
        error: State<Option<String>>,
        on_pick: impl Into<EventHandler<String>>,
        on_close: impl Into<EventHandler<()>>,
    ) -> Self {
        Self {
            pending,
            error,
            on_pick: on_pick.into(),
            on_close: on_close.into(),
        }
    }
}

impl Component for VersionArtGallery {
    fn render(&self) -> impl IntoElement {
        let urls = use_version_art_gallery();
        let mut pending = self.pending;
        let mut error = self.error;
        let in_flight = pending.read().clone();
        let failure = error.read().clone();

        let scrim_close = self.on_close.clone();
        let header_close = self.on_close.clone();
        let footer_close = self.on_close.clone();
        let picked = self.on_pick.clone();
        let on_pick: EventHandler<String> = EventHandler::from(move |url: String| {
            if pending.read().is_some() {
                return;
            }
            error.set(None);
            pending.set(Some(url.clone()));
            picked.call(url);
        });

        let rows = urls.len().div_ceil(COLUMNS);

        let scroll = ScrollArea::new()
            .width(Size::fill())
            .height(Size::flex(1.0))
            .padding(Gaps::new(0., PANE_PADDING, 8., PANE_PADDING))
            .scrollbar_gutter(true);

        let scroll = if urls.is_empty() {
            scroll.child(empty_state())
        } else {
            scroll.lazy(rows, TILE_H, TILE_GAP, move |index| {
                let start = index * COLUMNS;
                let end = (start + COLUMNS).min(urls.len());
                tile_row(&urls[start..end], &on_pick, in_flight.as_deref())
            })
        };

        OverlayPopup::new()
            .overlay_level(OVERLAY_MAX_LEVEL)
            .on_close(move |()| scrim_close.call(()))
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(DIALOG_WIDTH))
                            .height(Size::px(DIALOG_HEIGHT))
                            .max_width(Size::window_percent(94.))
                            .max_height(Size::window_percent(92.))
                            .content(Content::Flex)
                            .corner_radius(CornerRadius::new_all(16.))
                            .overflow(Overflow::Clip)
                            .background(colors::page_elevated())
                            .border(border_all_color(1., colors::component_border()))
                            .child(header(move |_| header_close.call(())))
                            .child(scroll)
                            .child(footer(failure, move |_| footer_close.call(()))),
                    ),
            )
            .into_element()
    }
}

fn tile_row(row: &[String], on_pick: &EventHandler<String>, in_flight: Option<&str>) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .spacing(TILE_GAP)
        .children(
            row.iter()
                .map(|url| {
                    GalleryTile {
                        busy: in_flight == Some(url.as_str()),
                        dimmed: in_flight.is_some(),
                        url: url.clone(),
                        on_pick: on_pick.clone(),
                    }
                    .into_element()
                })
                .collect::<Vec<_>>(),
        )
        .into_element()
}

#[derive(PartialEq, Clone)]
struct GalleryTile {
    url: String,
    busy: bool,
    dimmed: bool,
    on_pick: EventHandler<String>,
}

impl Component for GalleryTile {
    fn render(&self) -> impl IntoElement {
        let a11y_id = use_a11y();
        let mut hovering = use_state(|| false);
        let hovered = *hovering.read();

        let on_pick = self.on_pick.clone();
        let picked = self.url.clone();

        let border_color = if hovered || self.busy {
            colors::brand()
        } else {
            colors::component_border()
        };

        let opacity = if self.dimmed && !self.busy { 0.45 } else { 1.0 };

        rect()
            .width(Size::px(TILE_W))
            .height(Size::px(TILE_H))
            .corner_radius(CornerRadius::new_all(12.))
            .overflow(Overflow::Clip)
            .opacity(opacity)
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .on_press(move |_| on_pick.call(picked.clone()))
            .on_pointer_enter(move |_| hovering.set(true))
            .on_pointer_leave(move |_| hovering.set(false))
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .position(Position::new_absolute())
                    .child(
                        DynamicArt::for_url(self.url.clone())
                            .max_edge(ART_PREVIEW_EDGE)
                            .skeleton(true),
                    ),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .layer(Layer::Relative(3))
                    .corner_radius(CornerRadius::new_all(12.))
                    .border(border_all_color(2., border_color).alignment(BorderAlignment::Inner))
                    .center()
                    .maybe_child(self.busy.then(|| Spinner::new(22.).into_element())),
            )
            .into_element()
    }
}

fn empty_state() -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::px(220.))
        .center()
        .cross_align(Alignment::Center)
        .spacing(12.)
        .child(
            label()
                .text("The art gallery could not be loaded. Check your connection.")
                .font_size(13.)
                .color(colors::fg_secondary()),
        )
        .child(
            Button::new()
                .secondary()
                .small()
                .on_press(move |_| {
                    spawn(async move {
                        refresh_version_art_gallery().await;
                    });
                })
                .text("Try again"),
        )
        .into_element()
}

fn header(on_close: impl FnMut(Event<PressEventData>) + 'static) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Start)
        .spacing(16.)
        .padding(Gaps::new(22., PANE_PADDING, 16., PANE_PADDING))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(5.)
                .child(
                    label()
                        .text("Art gallery")
                        .font_size(20.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text("Pick any of the official version artworks as the background for this instance.")
                        .font_size(13.)
                        .line_height(1.35)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(
            Button::new()
                .ghost()
                .icon()
                .alt("Close")
                .on_press(on_close)
                .child(Icon::new(IconType::XClose).size(16.)),
        )
        .into_element()
}

fn footer(
    failure: Option<String>,
    on_close: impl FnMut(Event<PressEventData>) + 'static,
) -> Element {
    let (note, note_color) = match failure {
        Some(message) => (message, colors::danger()),
        None => (
            "Choosing an artwork pins it to this instance, so it stays put if you change the version later."
                .to_string(),
            colors::fg_secondary(),
        ),
    };

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(20.)
        .padding(Gaps::new(16., PANE_PADDING, 16., PANE_PADDING))
        .border(
            Border::new()
                .fill(colors::component_border())
                .width(BorderWidth {
                    top: 1.,
                    right: 0.,
                    bottom: 0.,
                    left: 0.,
                }),
        )
        .child(
            label()
                .text(note)
                .width(Size::flex(1.0))
                .font_size(12.)
                .line_height(1.35)
                .max_lines(2)
                .color(note_color),
        )
        .child(Button::new().ghost().on_press(on_close).text("Cancel"))
        .into_element()
}

pub const GALLERY_COVER_EDGE: u32 = DEFAULT_IMAGE_EDGE;
