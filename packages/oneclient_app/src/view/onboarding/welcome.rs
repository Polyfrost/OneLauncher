use freya::prelude::*;

use crate::components::IconType;
use crate::hooks::{has_migration_data, use_migration};
use crate::routes::Route;
use crate::theme::colors;
use crate::view::onboarding::{
    onboarding_illustration, onboarding_nav, onboarding_page, step_heading,
};

#[derive(PartialEq)]
pub struct OnboardingWelcome;

impl Component for OnboardingWelcome {
    fn render(&self) -> impl IntoElement {
        let migration_query = use_migration();

		let next = if has_migration_data(&migration_query) {
            Route::OnboardingMigration {}
        } else {
            Route::OnboardingLanguage {}
        };

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
            onboarding_nav(None, next, true),
        )
    }
}
