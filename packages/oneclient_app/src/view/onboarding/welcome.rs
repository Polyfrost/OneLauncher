use freya::animation::*;
use freya::prelude::*;

use crate::hooks::use_onboarding_selection;
use crate::routes::Route;
use crate::theme::{colors, text_size};
use crate::view::onboarding::onboarding_nav;

const SOFT_EDGE: f32 = 12.;
const TITLE_SHARE: f32 = 0.7;

#[derive(PartialEq)]
pub struct OnboardingWelcome;

impl Component for OnboardingWelcome {
    fn render(&self) -> impl IntoElement {
        let selection = use_onboarding_selection();
        let next = if *selection.picks_location.read() {
            Route::OnboardingLocation {}
        } else {
            Route::OnboardingTerms {}
        };

        let intro = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            AnimNum::new(0., 1.)
                .time(2400)
                .ease(Ease::InOut)
                .function(Function::Sine)
        });
        let progress = intro.get().value();
        let written = (progress / TITLE_SHARE).min(1.);
        let rest = ((progress - TITLE_SHARE) / (1. - TITLE_SHARE)).clamp(0., 1.);

        let mut chrome = selection.chrome_opacity;
        use_side_effect_with_deps(&rest, move |&rest| chrome.set_if_modified(rest));
        use_drop(move || chrome.set_if_modified(1.));

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .content(Content::Flex)
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .center()
                    .spacing(16.)
                    .child(
                        label()
                            .text("Welcome to OneClient")
                            .font_size(text_size(56.))
                            .font_weight(FontWeight::BOLD)
                            .color(write_on(colors::fg_primary(), written)),
                    )
                    .child(
                        label()
                            .text("Let's get you set up. It only takes a minute.")
                            .font_size(text_size(18.))
                            .color(colors::fg_secondary())
                            .opacity(rest),
                    ),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .opacity(rest)
                    .child(onboarding_nav(None, next, true)),
            )
    }
}

fn write_on(color: Color, written: f32) -> Fill {
    if written >= 1. {
        return color.into();
    }
    let edge = written * (100. + SOFT_EDGE);
    LinearGradient::new()
        .angle(-90.)
        .stop((color, (edge - SOFT_EDGE).max(0.)))
        .stop((colors::with_alpha(color, 0), edge.min(100.)))
        .into()
}
