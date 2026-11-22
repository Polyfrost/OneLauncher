use freya::prelude::*;

use crate::components::{IconType, toggle};
use crate::hooks::{use_dispatch, use_onboarding_selection, use_settings_snapshot};
use crate::i18n::t;
use crate::routes::Route;
use crate::theme::{colors, text_size};
use crate::view::app::settings::appearance_picker::{section, style_picker, theme_mode_picker};
use crate::view::app::settings::settings_row;
use crate::view::onboarding::parallax_grid::ParallaxGrid;
use crate::view::onboarding::{
    onboarding_compact, onboarding_dense, onboarding_illustration, onboarding_nav, onboarding_page,
    step_heading,
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

        let dense = onboarding_dense();

        let content = rect()
            .vertical()
            .width(Size::fill())
            .spacing(if dense { 12. } else { 24. })
            // Short windows keep just the title so Style and Theme both fit before scrolling
            .child(if dense {
                label()
                    .text(t("oneclient.onboarding.preferences.title"))
                    .font_size(text_size(28.))
                    .font_weight(FontWeight::BOLD)
                    .color(colors::fg_primary())
                    .into_element()
            } else {
                step_heading(
                    t("oneclient.onboarding.preferences.title"),
                    t("oneclient.onboarding.preferences.subtitle"),
                )
                .into_element()
            })
            .child(section(
                t("oneclient.onboarding.preferences.theme"),
                theme_mode_picker(&settings.theme, &dispatch),
            ))
            .child(section(
                t("oneclient.onboarding.preferences.style"),
                style_picker(&settings.theme, dense, false),
            ))
            .child(section(
                t("oneclient.onboarding.preferences.motion"),
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
            ));

        // Capped beside the illustration so the previews keep their proportions when maximized;
        // the narrow layout has no illustration, so there it spans the page like the nav does
        let content = if onboarding_compact() {
            content
        } else {
            content.max_width(Size::px(640.))
        }
        .into_element();

        // The dot field sits behind the illustration so the parallax toggle can be felt here
        let illustration = rect()
            .width(Size::fill())
            .height(Size::fill())
            .center()
            .child(
                rect()
                    .position(Position::new_absolute())
                    .width(Size::fill())
                    .height(Size::fill())
                    .child(ParallaxGrid {
                        enabled: *parallax_on.read(),
                    }),
            )
            // Its own layer keeps the swatches above the dots, which otherwise paint on top
            .child(
                rect()
                    .layer(Layer::Relative(1))
                    .child(onboarding_illustration(IconType::OnboardingPreferences)),
            );

        onboarding_page(
            illustration,
            content,
            onboarding_nav(
                Some(Route::OnboardingBundles {}),
                Route::OnboardingSummary {},
                true,
            ),
        )
    }
}
