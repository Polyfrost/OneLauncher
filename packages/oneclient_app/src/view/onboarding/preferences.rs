use freya::prelude::*;

use crate::components::{IconType, toggle};
use crate::hooks::{use_dispatch, use_onboarding_selection, use_settings_snapshot};
use crate::routes::Route;
use crate::view::app::settings::settings_row;
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
            let v = !*reduce_motion.peek();
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
        use_side_effect(move || {
            let enabled = *parallax_on.read();
            if *first.peek() {
                first.set(false);
                return;
            }
            let mut next = settings.clone();
            next.dynamic_background_enabled = enabled;
            dispatch.set_settings(next);
        });

        let content = rect()
            .vertical()
            .width(Size::fill())
            .spacing(20.)
            .child(step_heading(
                "Accessibility",
                "Tune the launcher to your comfort. You can change these later in settings.",
            ))
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .spacing(8.)
                    .child(settings_row(
                        IconType::Play,
                        "Animations",
                        "Disable all launcher animations and transitions.",
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
                Route::OnboardingDownloading {},
                true,
            ),
        )
    }
}
