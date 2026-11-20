use freya::prelude::*;

use super::MarkdownStyle;
use super::embed::Embed;
use crate::components::{Icon, IconType};
use crate::hooks::{LinkConfirmState, loaded_image, use_cached_image};
use crate::i18n::tf;
use crate::theme::{colors, rounded, text_size};
use crate::ui::ImageFallbackExt;

const MAX_WIDTH: f32 = 720.;
const MIN_HEIGHT: f32 = 112.;

/// Draw the provider preview natively and use the existing link confirmation to open playback.
#[derive(PartialEq)]
pub struct EmbedCard {
    embed: Embed,
    style: MarkdownStyle,
    key: DiffKey,
}

impl EmbedCard {
    pub fn new(embed: Embed, style: MarkdownStyle) -> Self {
        Self {
            embed,
            style,
            key: DiffKey::None,
        }
    }
}

impl KeyExt for EmbedCard {
    fn key_mut(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for EmbedCard {
    fn render(&self) -> impl IntoElement {
        let pending = try_consume_root_context::<LinkConfirmState>();
        let mut hovered = use_state(|| false);
        let mut width = use_state(|| 0.0f32);
        let id = use_a11y();
        let focus = use_focus(id);
        let hot = hovered() || focus() == Focus::Keyboard;

        let provider = self.embed.provider.name();
        let watch = tf("oneclient.components.markdown.watch_on", &[&provider]);
        let open_url = self.embed.open_url.clone();
        let image_query = use_cached_image(self.embed.thumbnail.clone(), 768);
        let image = loaded_image(self.embed.thumbnail.as_deref(), &image_query);
        let height = if self.embed.thumbnail.is_some() {
            (*width.read() * 9. / 16.).max(MIN_HEIGHT)
        } else {
            MIN_HEIGHT
        };

        rect()
            .width(Size::fill())
            .max_width(Size::px(MAX_WIDTH))
            .height(Size::px(height))
            .corner_radius(rounded(10.))
            .overflow(Overflow::Clip)
            .background(self.style.background_code)
            .a11y_id(id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Link)
            .a11y_alt(watch.clone())
            .cursor(CursorIcon::Pointer)
            .on_sized(move |event: Event<SizedEventData>| {
                let measured = event.data().area.width();
                if (measured - *width.peek()).abs() > 0.5 {
                    width.set(measured);
                }
            })
            .on_pointer_enter(move |_| hovered.set_if_modified(true))
            .on_pointer_leave(move |_| hovered.set_if_modified(false))
            .on_press(move |_| {
                if let Some(mut pending) = pending {
                    pending.set(Some(open_url.clone()));
                }
            })
            .map(image, |el, (url, bytes)| {
                el.child(
                    rect()
                        .width(Size::fill())
                        .height(Size::fill())
                        .position(Position::new_absolute().top(0.).left(0.))
                        .child(
                            ImageViewer::new((url, bytes))
                                .width(Size::fill())
                                .height(Size::fill())
                                .aspect_ratio(AspectRatio::Max)
                                .image_cover(ImageCover::Center)
                                .fallback(rect()),
                        ),
                )
            })
            .child(
                rect()
                    .position(Position::new_absolute().top(0.).left(0.))
                    .width(Size::fill())
                    .height(Size::fill())
                    .layer(Layer::Relative(3))
                    .center()
                    .border(
                        Border::new()
                            .width(if focus() == Focus::Keyboard { 2. } else { 1. })
                            .fill(if hot {
                                colors::brand()
                            } else {
                                self.style.background_divider
                            })
                            .alignment(BorderAlignment::Inner),
                    )
                    .child(
                        rect()
                            .width(Size::px(58.))
                            .height(Size::px(42.))
                            .center()
                            .corner_radius(rounded(10.))
                            .background(if hot {
                                colors::brand()
                            } else {
                                Color::from_argb(225, 10, 13, 19)
                            })
                            .child(Icon::new(IconType::Play).size(22.).color(Color::WHITE)),
                    )
                    .child(
                        rect()
                            .position(Position::new_absolute().bottom(10.).right(10.))
                            .horizontal()
                            .cross_align(Alignment::Center)
                            .spacing(5.)
                            .padding(Gaps::new_symmetric(6., 9.))
                            .corner_radius(rounded(5.))
                            .background(Color::from_argb(225, 10, 13, 19))
                            .child(
                                label()
                                    .text(watch)
                                    .font_size(text_size(11.))
                                    .color(Color::WHITE),
                            )
                            .child(
                                Icon::new(IconType::LinkExternal01)
                                    .size(12.)
                                    .color(Color::WHITE),
                            ),
                    )
                    .maybe_child(self.embed.title.clone().map(|title| {
                        rect()
                            .position(Position::new_absolute().top(10.).left(10.))
                            .max_width(Size::percent(90.))
                            .padding(Gaps::new_symmetric(6., 9.))
                            .corner_radius(rounded(5.))
                            .background(Color::from_argb(225, 10, 13, 19))
                            .child(
                                label()
                                    .max_lines(1)
                                    .text_overflow(TextOverflow::Ellipsis)
                                    .font_size(text_size(13.))
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .text(title)
                                    .color(Color::WHITE),
                            )
                    })),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}
