use super::*;

use freya::animation::{AnimNum, Ease, Function, OnCreation, OnFinish, use_animation};

use crate::theme::{colors, rounded};
use crate::ui::border_all_color;

fn skeleton_block(width: Size, height: f32) -> impl IntoElement {
    rect()
        .width(width)
        .height(Size::px(height))
        .corner_radius(rounded(6.))
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
            .corner_radius(rounded(8.))
            .background(colors::component_bg())
            .border(border_all_color(1., colors::component_border()))
            .overflow(Overflow::Clip)
            .opacity(pulse)
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .height(Size::px(BANNER_H))
                    .cross_align(Alignment::Center)
                    .content(Content::Flex)
                    .spacing(10.)
                    .padding(Gaps::new_symmetric(0., 14.))
                    .child(skeleton_block(Size::px(CARD_ICON + 4.), CARD_ICON + 4.))
                    .child(
                        rect()
                            .vertical()
                            .width(Size::flex(1.0))
                            .spacing(6.)
                            .child(skeleton_block(Size::percent(65.), 14.))
                            .child(skeleton_block(Size::percent(45.), 10.)),
                    ),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .padding(Gaps::new(10., 14., 12., 14.))
                    .spacing(6.)
                    .child(skeleton_block(Size::fill(), 10.))
                    .child(skeleton_block(Size::percent(90.), 10.))
                    .child(skeleton_block(Size::percent(65.), 10.))
                    .child(rect().width(Size::fill()).height(Size::flex(1.0)))
                    .child(skeleton_block(Size::px(76.), 28.)),
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
            .corner_radius(rounded(8.))
            .background(colors::component_bg())
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

pub(super) fn skeleton_grid_row(cols: usize) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::px(CARD_H))
        .spacing(GRID_SPACING)
        .content(Content::Flex)
        .children((0..cols).map(|_| SkeletonCard.into_element()))
        .into_element()
}
