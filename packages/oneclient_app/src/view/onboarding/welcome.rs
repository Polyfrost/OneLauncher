use freya::prelude::*;

use crate::components::IconType;
use crate::hooks::use_onboarding_selection;
use crate::i18n::t;
use crate::routes::Route;
use crate::theme::colors;
use crate::view::onboarding::{
    onboarding_illustration, onboarding_nav, onboarding_page, step_heading,
};

#[derive(PartialEq)]
pub struct OnboardingWelcome;

impl Component for OnboardingWelcome {
    fn render(&self) -> impl IntoElement {
        let next = if *use_onboarding_selection().picks_location.read() {
            Route::OnboardingLocation {}
        } else {
            Route::OnboardingTerms {}
        };

        let content = rect()
            .vertical()
            .width(Size::fill())
            .spacing(16.)
            .child(step_heading(
                "OneClient",
                t("oneclient.onboarding.welcome.subtitle"),
            ))
            .child(
                label()
                    .text(t("oneclient.onboarding.welcome.description"))
                    .font_size(15.)
                    .color(colors::fg_secondary()),
            )
            .into_element();

        onboarding_page(
            onboarding_illustration(IconType::OnboardingWelcome),
            content,
            onboarding_nav(None, next, true),
        )
    }
}
