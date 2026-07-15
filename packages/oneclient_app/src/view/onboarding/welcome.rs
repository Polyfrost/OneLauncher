use freya::prelude::*;

use crate::components::IconType;
use crate::routes::Route;
use crate::theme::colors;
use crate::view::onboarding::{
    onboarding_illustration, onboarding_nav, onboarding_page, step_heading,
};

#[derive(PartialEq)]
pub struct OnboardingWelcome;

impl Component for OnboardingWelcome {
    fn render(&self) -> impl IntoElement {
        let content = rect()
            .vertical()
            .width(Size::fill())
            .spacing(16.)
            .child(step_heading(
                "OneClient",
                "Let's get you all set-up with the most advanced client.",
            ))
            .child(
                label()
                    .text("This quick setup will pick your language, sign you in, and prepare your first versions. It only takes a minute.")
                    .font_size(15.)
                    .color(colors::fg_secondary()),
            )
            .into_element();

        onboarding_page(
            onboarding_illustration(IconType::OnboardingWelcome),
            content,
            onboarding_nav(None, Route::OnboardingTerms {}, true),
        )
    }
}
