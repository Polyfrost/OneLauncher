use freya::prelude::*;

use crate::components::{Button, Icon, IconType, OverlayPopup};
use crate::hooks::{EssentialGuardKind, use_cluster_mutation, use_essential_guard};
use crate::theme::colors;
use crate::ui::border_all_color;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);

#[derive(PartialEq)]
pub struct EssentialConfirmOverlay;

impl Component for EssentialConfirmOverlay {
    fn render(&self) -> impl IntoElement {
        let mut pending = use_essential_guard();
        let cluster = use_cluster_mutation();

        let Some(request) = pending.read().clone() else {
            return rect().into_element();
        };

        let (title, confirm_label, body) = match request.kind {
            EssentialGuardKind::Disable => (
                format!("Turn off {}?", request.package.name),
                "Turn it off",
                request.package.disable_body,
            ),
            EssentialGuardKind::Remove => (
                format!("Remove {}?", request.package.name),
                "Remove anyway",
                request.package.remove_body,
            ),
        };

        let action = request.action.clone();

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
                                            .text(title)
                                            .font_size(16.)
                                            .font_weight(FontWeight::SEMI_BOLD)
                                            .color(colors::fg_primary()),
                                    ),
                            )
                            .child(
                                label()
                                    .text(body)
                                    .font_size(12.)
                                    .max_lines(6)
                                    .width(Size::fill())
                                    .color(colors::fg_secondary()),
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
                                            .text("Keep it"),
                                    )
                                    .child(
                                        Button::new()
                                            .danger()
                                            .on_press(move |_| {
                                                cluster.mutate(action.clone());
                                                pending.set(None);
                                            })
                                            .text(confirm_label),
                                    ),
                            ),
                    ),
            )
            .into_element()
    }
}
