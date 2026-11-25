use freya::prelude::*;

use crate::components::{
    Button, Icon, IconType, Markdown, MarkdownStyle, OVERLAY_BASE_LEVEL, OverlayPopup, ScrollArea,
};
use crate::disable;
use crate::hooks::{
    EssentialGuardKind, Guarded, use_cluster_mutation, use_disable_confirm, use_dispatch,
    use_essential_guard,
};
use crate::i18n::{t, tf};
use crate::theme::{colors, rounded, text_size};
use crate::ui::border_all_color;

const BODY_MAX_H: f32 = 400.;
/// Seconds before a warned mod can be turned off, so the warning gets read first
const CONFIRM_DELAY: u32 = 5;

#[derive(PartialEq)]
pub struct EssentialConfirmOverlay;

impl Component for EssentialConfirmOverlay {
    fn render(&self) -> impl IntoElement {
        let mut pending = use_essential_guard();
        let cluster = use_cluster_mutation();
        let pending_disable = use_disable_confirm();
        let dispatch = use_dispatch();

        let Some(request) = pending.read().clone() else {
            return rect().into_element();
        };

        let (title, confirm_label) = match request.kind {
            EssentialGuardKind::Disable => (
                tf(
                    "oneclient.components.essential_confirm.disable.title",
                    &[&request.name],
                ),
                t("oneclient.components.essential_confirm.disable.confirm"),
            ),
            EssentialGuardKind::Remove => (
                tf(
                    "oneclient.components.essential_confirm.remove.title",
                    &[&request.name],
                ),
                t("oneclient.components.essential_confirm.remove.confirm"),
            ),
        };

        let then = request.then.clone();

        OverlayPopup::new()
            .overlay_level(OVERLAY_BASE_LEVEL + 3)
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
                            .corner_radius(rounded(14.))
                            .background(colors::card())
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
                                            .font_size(text_size(16.))
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
                                    .child(CountdownButton {
                                        label: confirm_label.to_string(),
                                        on_press: EventHandler::new(move |_| {
                                            match then.clone() {
                                                Guarded::Mutate(action) => cluster.mutate(action),
                                                Guarded::Disable(request) => disable::request(
                                                    request,
                                                    pending_disable,
                                                    dispatch.clone(),
                                                ),
                                            }
                                            pending.set(None);
                                        }),
                                    })
                                    .child(
                                        Button::new()
                                            .primary()
                                            .on_press(move |_| pending.set(None))
                                            .text(t("oneclient.components.essential_confirm.keep")),
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
                            .corner_radius(rounded(8.))
                            .background(colors::component_bg())
                            .border(border_all_color(1., colors::component_border()))
                            .child(Markdown::new(self.body.clone()).width(Size::fill()).style(
                                MarkdownStyle {
                                    color: colors::fg_primary(),
                                    color_link: colors::code_info(),
                                    color_code: colors::fg_primary(),
                                    background_code: colors::card(),
                                    background_blockquote: colors::card(),
                                    border_blockquote: colors::component_border(),
                                    background_divider: colors::component_border(),
                                    headings: [16., 14., 13., 12., 12., 12.],
                                    paragraph_size: 12.,
                                    code_font_size: 11.,
                                    ..MarkdownStyle::default()
                                },
                            )),
                    ),
            )
    }
}

/// Starts disabled with a countdown in its label and unlocks at zero. It mounts
/// fresh with each warning, so every warning gets its own countdown
#[derive(PartialEq)]
struct CountdownButton {
    label: String,
    on_press: EventHandler<Event<PressEventData>>,
}

impl Component for CountdownButton {
    fn render(&self) -> impl IntoElement {
        let mut left = use_state(|| CONFIRM_DELAY);
        use_hook(move || {
            spawn(async move {
                while *left.peek() > 0 {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    let next = left.peek().saturating_sub(1);
                    left.set(next);
                }
            });
        });
        let left = *left.read();
        let on_press = self.on_press.clone();
        Button::new()
            .secondary()
            .enabled(left == 0)
            .on_press(move |e| on_press.call(e))
            .text(if left == 0 {
                self.label.clone()
            } else {
                format!("{} ({left})", self.label)
            })
    }
}
