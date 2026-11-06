use freya::animation::{AnimNum, Ease, OnCreation, use_animation};
use freya::prelude::*;

use crate::components::OverlayPopup;
use crate::hooks::{use_chat_allowed, use_dispatch, use_social_open, use_social_popped_notice};
use crate::i18n::t;
use crate::theme::{self, colors, rounded, text_size};

use super::common::{CARD_RADIUS_PX, ROW_RADIUS_PX, inset_border, popup_shadow};
use super::panel::{PanelMode, SocialPanel};
use super::window::{MAIN_FOCUSED, track_focus};

const DOCKED_WIDTH_PX: f32 = 348.;
const DOCKED_EDGE_PX: f32 = 12.;
const DOCKED_LEVEL: u8 = 3;
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

        rect()
            .position(Position::new_global().top(0.).left(0.))
            .width(Size::window_percent(100.))
            .height(Size::window_percent(100.))
            .layer(Layer::Overlay)
            .maybe_child(open.then(|| DockedPanel.into_element()))
            .maybe_child((notice && !open).then(|| PoppedNotice.into_element()))
            .into_element()
    }
}

#[derive(PartialEq)]
struct DockedPanel;

impl Component for DockedPanel {
    fn render(&self) -> impl IntoElement {
        let intro = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            AnimNum::new(0., 1.).time(90).ease(Ease::Out)
        });
        let progress = intro.read().value();

        let height = docked_panel_height();
        let dispatch = use_dispatch();

        OverlayPopup::new()
            .overlay_level(DOCKED_LEVEL)
            .position(
                Position::new_global()
                    .top(theme::NAVBAR_HEIGHT_PX)
                    .right(DOCKED_EDGE_PX),
            )
            .on_close(move |()| dispatch.close_social())
            .child(
                rect()
                    .width(Size::px(DOCKED_WIDTH_PX))
                    .height(Size::px(height))
                    .opacity(progress)
                    .corner_radius(rounded(CARD_RADIUS_PX))
                    .overflow(Overflow::Clip)
                    .background(colors::page().with_a(196))
                    .backdrop_blur(40.)
                    .border(inset_border(colors::component_border()))
                    .shadow(popup_shadow())
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
                Position::new_absolute()
                    .top(theme::NAVBAR_HEIGHT_PX + 8.)
                    .right(DOCKED_EDGE_PX),
            )
            .layer(Layer::Overlay)
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
