use freya::prelude::*;

use crate::components::{Button, Icon, IconType, OverlayPopup};
use crate::theme::colors;
use crate::ui::border_all_color;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);

#[derive(PartialEq)]
pub struct SharedPackageDeleteDialog {
    name: String,
    clusters: usize,
    on_cancel: EventHandler<()>,
    on_confirm: EventHandler<()>,
}

impl SharedPackageDeleteDialog {
    pub fn new(
        name: impl Into<String>,
        clusters: usize,
        on_cancel: impl Into<EventHandler<()>>,
        on_confirm: impl Into<EventHandler<()>>,
    ) -> Self {
        Self {
            name: name.into(),
            clusters,
            on_cancel: on_cancel.into(),
            on_confirm: on_confirm.into(),
        }
    }
}

impl Component for SharedPackageDeleteDialog {
    fn render(&self) -> impl IntoElement {
        let title = format!("Delete {} from every cluster?", self.name);
        let body = if self.clusters > 1 {
            format!(
                "Resource packs and shaders are shared, so this deletes it from all {} clusters that use it.",
                self.clusters
            )
        } else {
            "Resource packs and shaders are shared, so this deletes it from every cluster."
                .to_string()
        };
        let close = self.on_cancel.clone();
        let cancel = self.on_cancel.clone();
        let confirm = self.on_confirm.clone();

        OverlayPopup::new().on_close(move |_| close.call(())).child(
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
                                        .on_press(move |_| cancel.call(()))
                                        .text("Cancel"),
                                )
                                .child(
                                    Button::new()
                                        .danger()
                                        .on_press(move |_| confirm.call(()))
                                        .child(Icon::new(IconType::Trash01).size(14.))
                                        .text("Delete"),
                                ),
                        ),
                ),
        )
    }
}
