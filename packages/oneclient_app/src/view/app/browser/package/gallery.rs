//! The package page's gallery tab.
//!
//! Laid out like the cluster's screenshots: a grid of tiles that open into a
//! full-window viewer with arrows between them. The difference is where the
//! pixels come from — these are remote images behind the shared image cache
//! rather than files on disk, so a tile and the opened view ask the cache for
//! the same url at two different sizes instead of sharing one decode.

use super::*;

use oneclient_content::packages::types::GalleryImage;

use crate::components::{Button, Icon, IconType, OverlayPopup};
use crate::hooks::{loaded_image, query_is_busy, use_cached_image};
use crate::theme::colors;
use crate::ui::{border_all_color, flow_grid, grid_columns_for_width};

/// How wide a tile is allowed to get before the grid takes another column.
const MAX_COL_W: f32 = 400.;
const GRID_GAP: f32 = 16.;
const TILE_PREVIEW_H: f32 = 168.;

/// Downscale ceilings handed to the image cache. A tile never shows more than
/// its own width, and asking for the full thing there was what made the grid
/// slow to fill; the opened view is the one that needs the detail.
const TILE_EDGE: u32 = 640;
const FULL_EDGE: u32 = 2048;

pub(super) fn gallery_panel(images: Vec<GalleryImage>) -> impl IntoElement {
    Gallery { images }
}

#[derive(PartialEq)]
struct Gallery {
    images: Vec<GalleryImage>,
}

impl Component for Gallery {
    fn render(&self) -> impl IntoElement {
        let grid_width = use_state(|| 0f32);
        let mut viewing = use_state(|| None::<usize>);

        let tiles: Vec<Element> = self
            .images
            .iter()
            .enumerate()
            .map(|(idx, image)| {
                let key = image.url.clone();
                GalleryTile {
                    image: image.clone(),
                    on_open: (move |()| viewing.set(Some(idx))).into(),
                    key: DiffKey::None,
                }
                .key(key)
                .into_element()
            })
            .collect();

        let cols = grid_columns_for_width(*grid_width.read(), MAX_COL_W, GRID_GAP);
        let images = self.images.clone();

        rect()
            .width(Size::fill())
            .child(flow_grid(tiles, cols, grid_width, GRID_GAP))
            .maybe_child((*viewing.read()).map(|start| {
                GalleryViewer {
                    images: images.clone(),
                    start,
                    on_close: (move |()| viewing.set(None)).into(),
                }
                .into_element()
            }))
    }
}

#[derive(PartialEq)]
struct GalleryTile {
    image: GalleryImage,
    on_open: EventHandler<()>,
    key: DiffKey,
}

impl KeyExt for GalleryTile {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for GalleryTile {
    fn render(&self) -> impl IntoElement {
        let query = use_cached_image(Some(self.image.url.clone()), TILE_EDGE);
        let loaded = loaded_image(Some(&self.image.url), &query);

        let mut hovered = use_state(|| false);
        let on_open = self.on_open.clone();

        use_drop(|| {
            Cursor::set(CursorIcon::default());
        });

        let preview = rect()
            .width(Size::fill())
            .height(Size::px(TILE_PREVIEW_H))
            .center()
            .overflow(Overflow::Clip)
            .background(colors::component_bg())
            .maybe_child(loaded.map(|(url, bytes)| {
                ImageViewer::new((url, bytes))
                    .width(Size::fill())
                    .height(Size::fill())
                    .aspect_ratio(AspectRatio::Max)
                    .image_cover(ImageCover::Center)
                    .into_element()
            }));

        rect()
            .vertical()
            .width(Size::flex(1.0))
            .max_width(Size::px(MAX_COL_W))
            .corner_radius(CornerRadius::new_all(10.))
            .overflow(Overflow::Clip)
            .background(PANEL_BG)
            .border(
                border_all_color(
                    1.,
                    if *hovered.read() {
                        colors::component_border_hover()
                    } else {
                        colors::component_border()
                    },
                )
                .alignment(BorderAlignment::Inner),
            )
            .a11y_role(AccessibilityRole::Button)
            .on_pointer_enter(move |_| {
                hovered.set(true);
                Cursor::set(CursorIcon::Pointer);
            })
            .on_pointer_leave(move |_| {
                hovered.set(false);
                Cursor::set(CursorIcon::default());
            })
            .on_press(move |_| on_open.call(()))
            .child(preview)
            .maybe(self.image.title.is_some(), |el| {
                el.child(
                    rect()
                        .width(Size::fill())
                        .padding(Gaps::new_all(10.))
                        .child(
                            label()
                                .text(self.image.title.clone().unwrap_or_default())
                                .font_size(12.)
                                .max_lines(2)
                                .color(colors::fg_secondary()),
                        ),
                )
            })
    }
}

/// The opened image, filling the window with the rest of the gallery a key
/// press away.
#[derive(PartialEq)]
struct GalleryViewer {
    images: Vec<GalleryImage>,
    start: usize,
    on_close: EventHandler<()>,
}

impl Component for GalleryViewer {
    fn render(&self) -> impl IntoElement {
        let len = self.images.len();

        let mut index = use_state({
            let start = self.start.min(len.saturating_sub(1));
            move || start
        });

        if len == 0 {
            return rect().into_element();
        }
        let idx = (*index.read()).min(len - 1);
        let image = self.images[idx].clone();

        let query = use_cached_image(Some(image.url.clone()), FULL_EDGE);
        let loaded = loaded_image(Some(&image.url), &query);

        let close = self.on_close.clone();
        let scrim_close = self.on_close.clone();

        let has_prev = idx > 0;
        let has_next = idx + 1 < len;

        let preview = rect()
            .width(Size::flex(1.0))
            .height(Size::fill())
            .center()
            .overflow(Overflow::Clip)
            .maybe_child(loaded.map(|(url, bytes)| {
                ImageViewer::new((url, bytes))
                    .width(Size::fill())
                    .height(Size::fill())
                    .aspect_ratio(AspectRatio::Min)
                    .into_element()
            }))
            // The tile's smaller copy is already cached, but this one is a
            // fresh fetch at full size, so the first moment of the opened view
            // says so rather than showing an empty frame. Only while it is
            // still working: a fetch that failed is not loading.
            .maybe_child(query_is_busy(&query).then(|| {
                label()
                    .text("Loading image...")
                    .font_size(13.)
                    .color(colors::fg_secondary())
                    .into_element()
            }));

        OverlayPopup::new()
            .on_close(move |_| scrim_close.call(()))
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .on_global_key_down(move |e: Event<KeyboardEventData>| match &e.key {
                        Key::Named(NamedKey::ArrowLeft) if idx > 0 => index.set(idx - 1),
                        Key::Named(NamedKey::ArrowRight) if idx + 1 < len => index.set(idx + 1),
                        _ => {}
                    })
                    .child(
                        rect()
                            .vertical()
                            .width(Size::window_percent(88.))
                            .height(Size::window_percent(90.))
                            .spacing(12.)
                            .padding(Gaps::new_all(16.))
                            .content(Content::Flex)
                            .child(header_row(&image, idx, len, move |_| close.call(())))
                            .child(
                                rect()
                                    .width(Size::fill())
                                    .height(Size::flex(1.0))
                                    .horizontal()
                                    .cross_align(Alignment::Center)
                                    .content(Content::Flex)
                                    .spacing(12.)
                                    .overflow(Overflow::Clip)
                                    .child(chevron_btn(IconType::ArrowLeft, has_prev, move |_| {
                                        if idx > 0 {
                                            index.set(idx - 1);
                                        }
                                    }))
                                    .child(preview)
                                    .child(chevron_btn(
                                        IconType::ArrowRight,
                                        has_next,
                                        move |_| {
                                            if idx + 1 < len {
                                                index.set(idx + 1);
                                            }
                                        },
                                    )),
                            ),
                    ),
            )
            .into_element()
    }
}

fn chevron_btn(
    icon: IconType,
    enabled: bool,
    on_press: impl Into<EventHandler<Event<PressEventData>>>,
) -> impl IntoElement {
    let base = rect()
        .width(Size::px(44.))
        .height(Size::px(44.))
        .center()
        .corner_radius(CornerRadius::new_all(22.));

    if !enabled {
        return base
            .background(Color::from_argb(70, 0, 0, 0))
            .on_press(|_| {})
            .child(
                Icon::new(icon)
                    .size(26.)
                    .color(colors::fg_secondary().with_a(90)),
            )
            .into_element();
    }

    base.background(Color::from_argb(140, 0, 0, 0))
        .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
        .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
        .on_press(on_press)
        .child(Icon::new(icon).size(26.).color(colors::fg_primary()))
        .into_element()
}

fn header_row(
    image: &GalleryImage,
    idx: usize,
    len: usize,
    on_close: impl Into<EventHandler<Event<PressEventData>>>,
) -> impl IntoElement {
    let title = image
        .title
        .clone()
        .unwrap_or_else(|| format!("Image {}", idx + 1));

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(2.)
                .child(
                    label()
                        .text(title)
                        .font_size(15.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .max_lines(1)
                        .color(colors::fg_primary()),
                )
                .maybe(len > 1, |el| {
                    el.child(
                        label()
                            .text(format!("{} of {len}", idx + 1))
                            .font_size(11.)
                            .color(colors::fg_secondary()),
                    )
                }),
        )
        .child(
            Button::new()
                .ghost()
                .icon()
                .on_press(on_close)
                .child(Icon::new(IconType::XClose).size(18.)),
        )
        .into_element()
}
