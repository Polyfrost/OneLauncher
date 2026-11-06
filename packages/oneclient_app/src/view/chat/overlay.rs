use freya::prelude::*;

use crate::components::{OVERLAY_BASE_LEVEL, OverlayPopup, popup_surface, use_popup_intro};
use crate::hooks::{use_chat_allowed, use_dispatch, use_social_open, use_social_popped_notice};
use crate::i18n::t;
use crate::theme::{self, colors, rounded, text_size};

use super::common::{ROW_RADIUS_PX, inset_border};
use super::panel::{PanelMode, SocialPanel};
use super::window::{MAIN_FOCUSED, track_focus};

const DOCKED_WIDTH_PX: f32 = 348.;
const DOCKED_EDGE_PX: f32 = 12.;
pub const DOCKED_INSET_PX: f32 = DOCKED_EDGE_PX + DOCKED_WIDTH_PX;

fn docked_panel_height() -> f32 {
    let window_height = Platform::get().current_window().root_size.read().height;
    (window_height - theme::NAVBAR_HEIGHT_PX - DOCKED_EDGE_PX).max(0.)
}

#[derive(PartialEq)]
pub struct SocialOverlay;

impl Component for SocialOverlay {
    fn render(&self) -> impl IntoElement {
        let open = use_social_open();
        let notice = use_social_popped_notice();
        let allowed = use_chat_allowed();
        let dispatch = use_dispatch();

        track_focus(&dispatch, &MAIN_FOCUSED);

        if !allowed || (!open && !notice) {
            return rect().into_element();
        }

        if open {
            DockedPanel.into_element()
        } else {
            PoppedNotice.into_element()
        }
    }
}

#[derive(PartialEq)]
struct DockedPanel;

impl Component for DockedPanel {
    fn render(&self) -> impl IntoElement {
        let height = docked_panel_height();
        let dispatch = use_dispatch();

        OverlayPopup::new()
            .position(
                Position::new_global()
                    .top(theme::NAVBAR_HEIGHT_PX)
                    .right(DOCKED_EDGE_PX),
            )
            .on_close(move |()| dispatch.close_social())
            .child(
                popup_surface(use_popup_intro())
                    .width(Size::px(DOCKED_WIDTH_PX))
                    .height(Size::px(height))
                    .overflow(Overflow::Clip)
                    .child(SocialPanel {
                        mode: PanelMode::Launcher,
                        expanded: false,
                    }),
            )
    }
}

#[derive(PartialEq)]
struct PoppedNotice;

impl Component for PoppedNotice {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();

        rect()
            .position(
                Position::new_global()
                    .top(theme::NAVBAR_HEIGHT_PX + 8.)
                    .right(DOCKED_EDGE_PX),
            )
            .layer(Layer::OverlayLevel(OVERLAY_BASE_LEVEL))
            .horizontal()
            .height(Size::px(40.))
            .padding(Gaps::new(0., 8., 0., 14.))
            .spacing(12.)
            .cross_align(Alignment::Center)
            .corner_radius(rounded(ROW_RADIUS_PX))
            .background(colors::page().with_a(209))
            .backdrop_blur(40.)
            .border(inset_border(colors::component_border()))
            .child(
                label()
                    .text(t("oneclient.chat.overlay.popped_out"))
                    .font_size(text_size(12.))
                    .color(colors::fg_primary()),
            )
            .child(
                rect()
                    .height(Size::px(28.))
                    .padding(Gaps::new_symmetric(0., 10.))
                    .center()
                    .corner_radius(rounded(ROW_RADIUS_PX))
                    .background(colors::component_bg())
                    .border(inset_border(colors::component_border()))
                    .cursor(CursorIcon::Pointer)
                    .on_press(move |_| dispatch.dock_social())
                    .child(
                        label()
                            .text(t("oneclient.chat.overlay.dock_back"))
                            .font_size(text_size(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .color(colors::fg_primary()),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use freya_testing::prelude::*;

    use super::*;

    fn app() -> impl IntoElement {
        label().text(docked_panel_height().to_string())
    }

    fn height_at(scale: f64) -> Option<String> {
        let (test, _) = TestingRunner::new(app, Size2D::new(1200., 1200.), |_| {}, scale);
        test.find(|_, element| Label::try_downcast(element).map(|label| label.text.to_string()))
    }

    #[test]
    fn the_docked_panel_fills_the_window_at_any_display_scale() {
        assert_eq!(height_at(1.).as_deref(), Some("1108"));
        assert_eq!(height_at(1.5).as_deref(), Some("708"));
        assert_eq!(height_at(2.).as_deref(), Some("508"));
    }
}
