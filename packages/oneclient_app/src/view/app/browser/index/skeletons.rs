use super::*;

use freya::animation::{
    AnimNum, Ease, Function, OnCreation, OnFinish, use_animation,
};

use crate::theme::colors;
use crate::ui::border_all_color;

fn skeleton_block(width: Size, height: f32) -> impl IntoElement {
    rect()
        .width(width)
        .height(Size::px(height))
        .corner_radius(CornerRadius::new_all(6.))
        .background(colors::component_bg())
        .into_element()
}

fn use_skeleton_pulse() -> f32 {
    let pulse = use_animation(|conf| {
        conf.on_creation(OnCreation::Run);
        conf.on_finish(OnFinish::reverse());
        AnimNum::new(0.4, 0.9)
            .time(800)
            .ease(Ease::InOut)
            .function(Function::Sine)
    });
    pulse.read().value()
}

#[derive(PartialEq)]
struct SkeletonCard;

impl Component for SkeletonCard {
    fn render(&self) -> impl IntoElement {
        let pulse = use_skeleton_pulse();
        rect()
            .vertical()
            .width(Size::flex(1.0))
            .height(Size::px(CARD_H))
            .corner_radius(CornerRadius::new_all(10.))
            .background(CARD_BG)
            .border(border_all_color(1., colors::component_border()))
            .overflow(Overflow::Clip)
            .opacity(pulse)
            .child(skeleton_block(Size::fill(), BANNER_H))
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .padding(Gaps::new_all(12.))
                    .spacing(8.)
                    .child(skeleton_block(Size::percent(70.), 16.))
                    .child(skeleton_block(Size::percent(40.), 10.))
                    .child(skeleton_block(Size::fill(), 10.))
                    .child(skeleton_block(Size::percent(85.), 10.))
                    .child(rect().width(Size::fill()).height(Size::flex(1.0)))
                    .child(skeleton_block(Size::percent(30.), 10.)),
            )
    }
}

#[derive(PartialEq)]
pub(super) struct SkeletonListRow;

impl Component for SkeletonListRow {
    fn render(&self) -> impl IntoElement {
        let pulse = use_skeleton_pulse();
        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(LIST_ROW_H))
            .cross_align(Alignment::Center)
            .spacing(12.)
            .padding(Gaps::new_all(10.))
            .corner_radius(CornerRadius::new_all(10.))
            .background(CARD_BG)
            .border(border_all_color(1., colors::component_border()))
            .opacity(pulse)
            .child(skeleton_block(Size::px(48.), 48.))
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(6.)
                    .child(skeleton_block(Size::percent(45.), 14.))
                    .child(skeleton_block(Size::percent(80.), 10.)),
            )
    }
}

pub(super) fn skeleton_grid_row() -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::px(CARD_H))
        .spacing(GRID_SPACING)
        .content(Content::Flex)
        .children((0..GRID_COLUMNS).map(|_| SkeletonCard.into_element()))
        .into_element()
}
