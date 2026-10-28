use freya::prelude::*;
use freya::router::RouterContext;

use crate::components::{Icon, IconType};
use crate::hooks::use_onboarding_selection;
use crate::routes::Route;
use crate::theme::{colors, rounded, text_size};
use crate::view::onboarding::{onboarding_illustration, onboarding_nav_action, onboarding_page};

const PURPLE: Color = Color::from_rgb(122, 100, 255);
const PINK: Color = Color::from_rgb(255, 148, 194);
const CORAL: Color = Color::from_rgb(238, 92, 86);
const GLOW_PX: f32 = 440.;

#[derive(PartialEq)]
pub struct OnboardingWelcome;

impl Component for OnboardingWelcome {
    fn render(&self) -> impl IntoElement {
        let next = if *use_onboarding_selection().picks_location.read() {
            Route::OnboardingLocation {}
        } else {
            Route::OnboardingTerms {}
        };

        onboarding_page(
            welcome_illustration(),
            welcome_content(),
            onboarding_nav_action(None, "Let's go", true, move |()| {
                let _ = RouterContext::get().replace(next.clone());
            }),
        )
    }
}

fn welcome_content() -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(28.)
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(10.)
                .child(
                    paragraph()
                        .width(Size::fill())
                        .font_size(text_size(40.))
                        .font_weight(FontWeight::BOLD)
                        .color(colors::fg_primary())
                        .span(Span::new("Welcome to "))
                        .span(
                            Span::new("OneClient").color(
                                LinearGradient::new()
                                    .angle(-90.)
                                    .stop((PURPLE, 45.))
                                    .stop((PINK, 75.))
                                    .stop((CORAL, 95.)),
                            ),
                        ),
                )
                .child(
                    label()
                        .text("We're glad you're here. Let's get you set up with the most advanced client.")
                        .font_size(text_size(16.))
                        .color(colors::fg_secondary()),
                ),
        )
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(10.)
                .child(highlight(
                    IconType::Globe01,
                    PURPLE,
                    "Pick your language",
                    "Use OneClient in the language you're most at home in.",
                ))
                .child(highlight(
                    IconType::Users01,
                    colors::brand(),
                    "Sign in",
                    "Connect your Microsoft account so you can play.",
                ))
                .child(highlight(
                    IconType::Rocket02,
                    CORAL,
                    "Get your first versions",
                    "We'll have everything ready so you can jump straight in.",
                )),
        )
        .child(
            label()
                .text("It only takes a minute.")
                .font_size(text_size(13.))
                .color(colors::fg_secondary()),
        )
    .into_element()
}

fn welcome_illustration() -> impl IntoElement {
    rect()
        .width(Size::px(GLOW_PX))
        .height(Size::px(GLOW_PX))
        .center()
        .child(
            rect()
                .position(Position::new_absolute())
                .width(Size::px(GLOW_PX))
                .height(Size::px(GLOW_PX))
                .background(
                    RadialGradient::new()
                        .stop((PURPLE.with_a(96), 0.))
                        .stop((PINK.with_a(40), 45.))
                        .stop((PINK.with_a(0), 72.)),
                ),
        )
        .child(onboarding_illustration(IconType::OnboardingWelcome))
}

fn highlight(icon: IconType, tint: Color, title: &str, description: &str) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(14.)
        .padding(Gaps::new_symmetric(12., 14.))
        .corner_radius(rounded(14.))
        .background(colors::page_elevated())
        .child(
            rect()
                .width(Size::px(40.))
                .height(Size::px(40.))
                .center()
                .corner_radius(rounded(12.))
                .background(tint.with_a(40))
                .child(Icon::new(icon).size(20.).color(tint)),
        )
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(2.)
                .child(
                    label()
                        .text(title.to_string())
                        .font_size(text_size(15.))
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(description.to_string())
                        .font_size(text_size(13.))
                        .color(colors::fg_secondary()),
                ),
        )
        .into_element()
}
