use freya::prelude::*;

use crate::components::{Button, Icon, IconType, OverlayPopup};
use crate::hooks::use_link_confirm;
use crate::theme::colors;
use crate::ui::border_all_color;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);

#[derive(PartialEq)]
pub struct ConfirmLinkOverlay;

impl Component for ConfirmLinkOverlay {
    fn render(&self) -> impl IntoElement {
        let mut pending = use_link_confirm();
        let Some(url) = pending.read().clone() else {
            return rect().into_element();
        };

        let open_url = url.clone();

        OverlayPopup::new()
            .on_close(move |_| pending.set(None))
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(440.))
                            .max_width(Size::window_percent(90.))
                            .spacing(14.)
                            .padding(Gaps::new_all(20.))
                            .corner_radius(CornerRadius::new_all(14.))
                            .background(CARD_BG)
                            .border(border_all_color(1., colors::component_border()))
                            .child(
                                rect()
                                    .horizontal()
                                    .cross_align(Alignment::Center)
                                    .spacing(10.)
                                    .child(
                                        Icon::new(IconType::AlertTriangle)
                                            .size(20.)
                                            .color(colors::code_warn()),
                                    )
                                    .child(
                                        label()
                                            .text("Open external link?")
                                            .font_size(16.)
                                            .font_weight(FontWeight::SEMI_BOLD)
                                            .color(colors::fg_primary()),
                                    ),
                            )
                            .child(
                                label()
                                    .text("This link was provided by a third party. Only open it if you trust the source.")
                                    .font_size(12.)
                                    .color(colors::fg_secondary()),
                            )
                            .child(
                                rect()
                                    .width(Size::fill())
                                    .padding(Gaps::new_all(10.))
                                    .corner_radius(CornerRadius::new_all(8.))
                                    .background(colors::component_bg())
                                    .border(border_all_color(1., colors::component_border()))
                                    .child(
                                        label()
                                            .text(url.clone())
                                            .font_size(12.)
                                            .max_lines(4)
                                            .width(Size::fill())
                                            .color(colors::code_info()),
                                    ),
                            )
                            .child(
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .main_align(Alignment::End)
                                    .spacing(8.)
                                    .child(
                                        Button::new()
                                            .secondary()
                                            .on_press(move |_| pending.set(None))
                                            .text("Cancel"),
                                    )
                                    .child(
                                        Button::new()
                                            .primary()
                                            .on_press(move |_| {
                                                crate::platform::open_url(&open_url);
                                                pending.set(None);
                                            })
                                            .child(Icon::new(IconType::LinkExternal01).size(14.))
                                            .text("Open link"),
                                    ),
                            ),
                    ),
            )
            .into_element()
    }
}
