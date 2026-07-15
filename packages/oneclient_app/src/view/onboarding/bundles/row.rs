use freya::prelude::*;
use freya::query::QueryStateData;
use oneclient_core::packages::ProviderId;

use crate::components::{Icon, provider_badge};
use crate::hooks::use_cached_image;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::utils::format_size;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);
const CARD_NAME: Color = Color::from_rgb(213, 219, 255);

pub(super) const CARD_GRID_H: f32 = 148.;
pub(super) const GRID_GAP: f32 = 10.;

#[derive(PartialEq)]
pub(super) struct OnboardingModCard {
    pub(super) provider: ProviderId,
    pub(super) name: String,
    pub(super) author: String,
    pub(super) description: String,
    pub(super) icon_url: Option<String>,
    pub(super) size: u64,
    pub(super) enabled: bool,
    pub(super) on_toggle: EventHandler<()>,
}

impl Component for OnboardingModCard {
    fn render(&self) -> impl IntoElement {
        let icon_query = use_cached_image(self.icon_url.clone(), 128);
        let loaded = {
            let reader = icon_query.read();
            match (&self.icon_url, &*reader.state()) {
                (Some(url), QueryStateData::Settled { res: Ok(bytes), .. })
                | (
                    Some(url),
                    QueryStateData::Loading {
                        res: Some(Ok(bytes)),
                    },
                ) => Some((url.clone(), bytes.clone())),
                _ => None,
            }
        };

        let icon = match loaded {
            Some((url, bytes)) => ImageViewer::new((url, bytes))
                .width(Size::px(44.))
                .height(Size::px(44.))
                .aspect_ratio(AspectRatio::Min)
                .corner_radius(CornerRadius::new_all(8.))
                .into_element(),

            None => icon_box(self.provider).into_element(),
        };

        let (bg, border, alpha) = if self.enabled {
            (colors::brand().with_a(38), colors::brand(), 255)
        } else {
            (CARD_BG, colors::component_border(), 140)
        };

        let on_toggle = self.on_toggle.clone();
        let size = self.size;

        let header = rect()
            .horizontal()
            .width(Size::fill())
            .cross_align(Alignment::Center)
            .spacing(10.)
            .content(Content::Flex)
            .child(icon)
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(3.)
                    .child(
                        label()
                            .text(self.name.clone())
                            .font_size(14.)
                            .font_weight(FontWeight::MEDIUM)
                            .max_lines(1)
                            .width(Size::fill())
                            .color(CARD_NAME.with_a(alpha)),
                    )
                    .maybe(!self.author.is_empty(), |el| {
                        el.child(
                            label()
                                .text(format!("by {}", self.author))
                                .font_size(10.)
                                .max_lines(1)
                                .width(Size::fill())
                                .color(colors::fg_secondary().with_a(alpha)),
                        )
                    })
                    .child(provider_badge(self.provider)),
            );

        let description = (!self.description.is_empty()).then(|| {
            label()
                .text(self.description.clone())
                .font_size(11.)
                .max_lines(3)
                .width(Size::fill())
                .color(colors::fg_secondary().with_a(alpha))
                .into_element()
        });

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .spacing(8.)
            .padding(Gaps::new_all(12.))
            .corner_radius(CornerRadius::new_all(8.))
            .background(bg)
            .border(border_all_color(1.5, border))
            .content(Content::Flex)
            .a11y_role(AccessibilityRole::Button)
            .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
            .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
            .on_press(move |_| on_toggle.call(()))
            .child(header)
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .maybe_child(description),
            )
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .main_align(Alignment::End)
                    .maybe_child((size > 0).then(|| {
                        label()
                            .text(format_size(size))
                            .font_size(11.)
                            .color(colors::fg_secondary().with_a(alpha))
                            .into_element()
                    })),
            )
    }
}

fn icon_box(provider: ProviderId) -> impl IntoElement {
    rect()
        .center()
        .width(Size::px(44.))
        .height(Size::px(44.))
        .corner_radius(CornerRadius::new_all(8.))
        .background(colors::component_bg())
        .child(Icon::new(provider).size(20.).color(colors::fg_secondary()))
        .into_element()
}

pub(super) fn empty_hint(text: &str) -> impl IntoElement {
    rect()
        .width(Size::fill())
        .padding(Gaps::new_all(24.))
        .center()
        .child(
            label()
                .text(text.to_string())
                .font_size(13.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}
