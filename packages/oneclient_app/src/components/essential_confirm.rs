use freya::prelude::*;

use crate::components::{
    Button, Icon, IconType, Markdown, MarkdownStyle, OverlayPopup, ScrollArea,
};
use crate::hooks::{EssentialGuardKind, use_cluster_mutation, use_essential_guard};
use crate::theme::colors;
use crate::ui::border_all_color;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);
const BODY_MAX_H: f32 = 400.;

#[derive(PartialEq)]
pub struct EssentialConfirmOverlay;

impl Component for EssentialConfirmOverlay {
    fn render(&self) -> impl IntoElement {
        let mut pending = use_essential_guard();
        let cluster = use_cluster_mutation();

        let Some(request) = pending.read().clone() else {
            return rect().into_element();
        };

        let (title, confirm_label) = match request.kind {
            EssentialGuardKind::Disable => (format!("Turn off {}?", request.name), "Turn it off"),
            EssentialGuardKind::Remove => (format!("Remove {}?", request.name), "Remove anyway"),
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
                            .child(WarningBody {
                                body: request.body.clone(),
                            })
                            .child(
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .main_align(Alignment::End)
                                    .spacing(8.)
                                    .child(
                                        Button::new()
                                            .secondary()
                                            .on_press(move |_| {
                                                cluster.mutate(action.clone());
                                                pending.set(None);
                                            })
                                            .text(confirm_label),
                                    )
                                    .child(
                                        Button::new()
                                            .primary()
                                            .on_press(move |_| pending.set(None))
                                            .text("Keep it"),
                                    ),
                            ),
                    ),
            )
            .into_element()
    }
}

#[derive(PartialEq)]
struct WarningBody {
    body: String,
}

impl Component for WarningBody {
    fn render(&self) -> impl IntoElement {
        let mut content_h = use_state(|| 0f32);
        let height = content_h.read().min(BODY_MAX_H);

        ScrollArea::new()
            .width(Size::fill())
            .height(Size::px(height))
            .scrollbar_gutter(true)
            .child(
                rect()
                    .width(Size::fill())
                    .on_sized(move |e: Event<SizedEventData>| {
                        let h = e.area.height();
                        if (*content_h.read() - h).abs() > 0.5 {
                            content_h.set(h);
                        }
                    })
                    .child(
                        rect()
                            .width(Size::fill())
                            .padding(Gaps::new_all(12.))
                            .corner_radius(CornerRadius::new_all(8.))
                            .background(colors::component_bg())
                            .border(border_all_color(1., colors::component_border()))
                            .child(
                                Markdown::new(self.body.clone())
                                    .width(Size::fill())
                                    .style(MarkdownStyle {
                                        color: Color::WHITE,
                                        color_link: colors::code_info(),
                                        color_code: Color::WHITE,
                                        background_code: CARD_BG,
                                        background_blockquote: CARD_BG,
                                        border_blockquote: colors::component_border(),
                                        background_divider: colors::component_border(),
                                        headings: [16., 14., 13., 12., 12., 12.],
                                        paragraph_size: 12.,
                                        code_font_size: 11.,
                                        ..MarkdownStyle::default()
                                    }),
                            ),
                    ),
            )
    }
}
