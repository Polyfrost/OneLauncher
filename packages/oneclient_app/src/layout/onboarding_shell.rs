use std::collections::HashSet;

use freya::prelude::*;
use freya::router::{Outlet, use_route};

use crate::Route;
use crate::components::OnboardingNavbar;
use crate::hooks::{
    OnboardingSelectionState, has_migration_data, onboarding_bundles_items, use_launcher,
    use_migration, use_onboarding_bundles, use_provide_onboarding_selection, use_settings_snapshot,
};
use crate::theme::{colors, text_size};
use crate::view::onboarding::{default_selection, onboarding_step_index, onboarding_total};

#[derive(PartialEq)]
pub struct OnboardingShell;

impl Component for OnboardingShell {
    fn render(&self) -> impl IntoElement {
        let mut selected = use_state(HashSet::new);
        let user_touched = use_state(|| false);
        let migrated_categories = use_state(|| None::<Vec<String>>);
        let language = use_state(|| "English".to_string());
        let reduce_motion = use_state(|| false);
        let import_folder = use_state(|| None::<String>);
        let import_dedicated = use_state(|| false);
        let picks_location = use_state({
            let fresh = use_launcher().needs_location;
            move || fresh
        });
        let route = use_route::<Route>();
        let chrome_opacity = use_state({
            let welcome = matches!(route, Route::OnboardingWelcome {});
            move || if welcome { 0. } else { 1. }
        });
        use_provide_onboarding_selection(OnboardingSelectionState {
            selected,
            user_touched,
            migrated_categories,
            language,
            reduce_motion,
            import_folder,
            import_dedicated,
            picks_location,
            chrome_opacity,
        });

        let migration_query = use_migration();
        let detected_migration = has_migration_data(&migration_query);

        let choosing_location = *picks_location.read();
        let step_index = onboarding_step_index(&route, detected_migration, choosing_location);

        let onboarded = use_settings_snapshot().settings.seen_onboarding;
        let reconsent = onboarded && matches!(&route, Route::OnboardingTerms {});

        let bundles = use_onboarding_bundles();

        use_side_effect(move || {
            if *user_touched.peek() {
                return;
            }
            let Some(items) = onboarding_bundles_items(&bundles) else {
                return;
            };
            let categories = migrated_categories.read().clone();
            let defaults = default_selection(&items, categories.as_deref());
            if *selected.peek() != defaults {
                selected.set(defaults);
            }
        });

        let chrome = *chrome_opacity.read();

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .background(colors::page())
            .color(colors::fg_primary())
            .overflow(Overflow::Clip)
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .height(Size::fill())
                    .content(Content::Flex)
                    .layer(Layer::Relative(30))
                    .maybe_child((!reconsent).then(|| {
                        rect()
                            .width(Size::fill())
                            .opacity(chrome)
                            .child(progress_bar(
                                step_index,
                                onboarding_total(detected_migration, choosing_location),
                            ))
                            .into_element()
                    }))
                    .child(
                        rect()
                            .width(Size::fill())
                            .opacity(chrome)
                            .child(OnboardingNavbar),
                    )
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::flex(1.0))
                            .overflow(Overflow::Clip)
                            .child(Outlet::<Route>::new())
                            .child(copyright(chrome)),
                    ),
            )
    }
}

fn progress_bar(step_index: usize, total: usize) -> impl IntoElement {
    let filled = ((step_index + 1) as f32 / total as f32) * 100.;

    rect()
        .width(Size::fill())
        .height(Size::px(3.))
        .background(colors::ghost_overlay_hover())
        .child(
            rect()
                .width(Size::percent(filled))
                .height(Size::fill())
                .background(colors::brand()),
        )
}

fn copyright(opacity: f32) -> impl IntoElement {
    rect()
        .position(Position::new_absolute().bottom(16.).left(24.))
        .interactive(false)
        .opacity(opacity)
        .child(
            label()
                .text("Design © 2026 Polyfrost. All rights reserved.")
                .font_size(text_size(11.))
                .color(colors::fg_secondary()),
        )
}
