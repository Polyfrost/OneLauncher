use freya::prelude::*;

use crate::components::{IconType, toggle};
use crate::hooks::{use_dispatch, use_onboarding_selection, use_settings_snapshot};
use crate::i18n::t;
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
        use_side_effect(move || {
            let parallax = *parallax_on.read();
            let animations = *animations_on.read();
            if *first.peek() {
                first.set(false);
                return;
            }
            let mut next = settings.clone();
            next.dynamic_background_enabled = parallax;
            next.animations_enabled = animations;
            dispatch.set_settings(next);
        });

        let content = rect()
            .vertical()
            .width(Size::fill())
            .spacing(20.)
            .child(step_heading(
                t("oneclient.onboarding.preferences.title"),
                t("oneclient.onboarding.preferences.subtitle"),
            ))
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .spacing(8.)
                    .child(settings_row(
                        IconType::Play,
                        t("oneclient.onboarding.preferences.animations.title"),
                        t("oneclient.onboarding.preferences.animations.description"),
                        toggle(animations_on),
                    ))
                    .child(settings_row(
                        IconType::Eye,
                        t("oneclient.onboarding.preferences.parallax.title"),
                        t("oneclient.onboarding.preferences.parallax.description"),
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
