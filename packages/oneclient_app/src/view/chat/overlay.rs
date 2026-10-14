use freya::animation::{AnimNum, Ease, OnCreation, use_animation};
use freya::prelude::*;

use crate::hooks::{use_dispatch, use_social_open, use_social_popped_notice};
use crate::theme::{self, colors};

use super::common::{CARD_RADIUS_PX, ROW_RADIUS_PX, chip_bg, hairline, inset_border, popup_shadow};
use super::panel::{PanelMode, SocialPanel};

const PANEL_WIDTH_PX: f32 = 348.;
const EDGE_PX: f32 = 12.;

#[derive(PartialEq)]
pub struct SocialOverlay;

impl Component for SocialOverlay {
    fn render(&self) -> impl IntoElement {
        let open = use_social_open();
        let notice = use_social_popped_notice();

        if !open && !notice {
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
            AnimNum::new(0., 1.).time(200).ease(Ease::Out)
        });
        let progress = intro.read().value();

        let platform = Platform::get();
        let scale = match *platform.scale_factor.read() as f32 {
            scale if scale > 0. => scale,
            _ => 1.,
        };
        let window_height = platform.root_size.read().height / scale;
        let height = (window_height - theme::NAVBAR_HEIGHT_PX - EDGE_PX).max(0.);

        rect()
            .position(
                Position::new_absolute()
                    .top(theme::NAVBAR_HEIGHT_PX)
                    .right(EDGE_PX),
            )
            .layer(Layer::Overlay)
            .width(Size::px(PANEL_WIDTH_PX))
            .height(Size::px(height))
            .opacity(progress)
            .corner_radius(CornerRadius::new_all(CARD_RADIUS_PX))
            .overflow(Overflow::Clip)
            .background(colors::page().with_a(196))
            .backdrop_blur(40.)
            .border(inset_border(hairline()))
            .shadow(popup_shadow())
            .child(SocialPanel {
                mode: PanelMode::Launcher,
                expanded: false,
            })
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
                    .right(EDGE_PX),
            )
            .layer(Layer::Overlay)
            .horizontal()
            .height(Size::px(40.))
            .padding(Gaps::new(0., 8., 0., 14.))
            .spacing(12.)
            .cross_align(Alignment::Center)
            .corner_radius(CornerRadius::from(ROW_RADIUS_PX))
            .background(colors::page().with_a(209))
            .backdrop_blur(40.)
            .border(inset_border(hairline()))
            .child(
                label()
                    .text("Social is open in a separate window")
                    .font_size(12.)
                    .color(colors::fg_primary()),
            )
            .child(
                rect()
                    .height(Size::px(28.))
                    .padding(Gaps::new_symmetric(0., 10.))
                    .center()
                    .corner_radius(CornerRadius::from(ROW_RADIUS_PX))
                    .background(chip_bg())
                    .border(inset_border(hairline()))
                    .cursor(CursorIcon::Pointer)
                    .on_press(move |_| dispatch.dock_social())
                    .child(
                        label()
                            .text("Dock back")
                            .font_size(12.)
                            .font_weight(FontWeight::MEDIUM)
                            .color(colors::fg_primary()),
                    ),
            )
    }
}
