use freya::prelude::*;

use crate::components::{IconType, toggle};
use crate::hooks::{use_dispatch, use_onboarding_selection, use_settings_snapshot};
use crate::routes::Route;
use crate::view::app::settings::{settings_row, theme_cards};
use crate::view::onboarding::{
    onboarding_illustration, onboarding_nav, onboarding_page, step_heading,
};

#[derive(PartialEq)]
pub struct OnboardingPreferences;

impl Component for OnboardingPreferences {
    fn render(&self) -> impl IntoElement {
        let settings = use_settings_snapshot().settings;
        let dispatch = use_dispatch();
        let mut reduce_motion = use_onboarding_selection().reduce_motion;

        let animations_on = use_state({
            let v = settings.animations_enabled;
            move || v
        });
        use_side_effect(move || {
            reduce_motion.set_if_modified(!*animations_on.read());
        });

        let parallax_on = use_state({
            let v = settings.dynamic_background_enabled;
            move || v
        });

        let mut first = use_state(|| true);
        let persist = dispatch.clone();
        use_side_effect(move || {
            let parallax = *parallax_on.read();
            let animations = *animations_on.read();
            if *first.peek() {
                first.set(false);
                return;
            }
            persist.edit_settings(|settings| {
                settings.dynamic_background_enabled = parallax;
                settings.animations_enabled = animations;
            });
        });

        let content = rect()
            .vertical()
            .width(Size::fill())
            .spacing(20.)
            .child(step_heading(
                "Appearance",
                "Pick a look you like. You can change it any time in settings.",
            ))
            .child(theme_cards(&settings.theme, dispatch))
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .spacing(8.)
                    .child(settings_row(
                        IconType::Play,
                        "Animations",
                        "Whether all launcher animations and transitions should be enabled.",
                        toggle(animations_on),
                    ))
                    .child(settings_row(
                        IconType::Eye,
                        "Parallax background",
                        "Make the background drift with your cursor. Turning it off disables the motion.",
                        toggle(parallax_on),
                    )),
            )
            .into_element();

        onboarding_page(
            onboarding_illustration(IconType::OnboardingPreferences),
            content,
            onboarding_nav(
                Some(Route::OnboardingBundles {}),
                Route::OnboardingSummary {},
                true,
            ),
        )
    }
}
