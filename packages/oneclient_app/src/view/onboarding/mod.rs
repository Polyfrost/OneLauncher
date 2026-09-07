mod account;
mod bundles;
mod downloading;
mod language;
mod location;
mod migration;
mod preferences;
mod selection;
mod terms;
#[cfg(test)]
pub(crate) mod test_support;
mod welcome;

pub(crate) use selection::{
    archive_selected, default_selection, is_default_bundle, is_optional_file, pkg_key,
    set_archive_selected,
};

pub use account::OnboardingAccount;
pub use bundles::OnboardingBundles;
pub use downloading::{LoadingBackdrop, OnboardingDownloading};
pub use language::OnboardingLanguage;
pub use location::OnboardingLocation;
pub use migration::OnboardingMigration;
pub(crate) use migration::matching_new_cluster_id;
pub use preferences::OnboardingPreferences;
pub use terms::OnboardingTerms;
pub use welcome::OnboardingWelcome;

use std::sync::atomic::{AtomicUsize, Ordering};

use freya::animation::*;
use freya::prelude::*;
use freya::router::{RouterContext, use_route};

use crate::Route;
use crate::components::{Button, Icon, IconType, toggle};
use crate::hooks::{has_migration_data, use_migration, use_onboarding_selection};
use crate::theme::colors;
use crate::ui::{border_all_color, entrance_motion_layer};

pub fn onboarding_total(has_migration: bool, picks_location: bool) -> usize {
    7 + usize::from(has_migration) + usize::from(picks_location)
}

pub fn onboarding_step_index(route: &Route, has_migration: bool, picks_location: bool) -> usize {
    let location = usize::from(picks_location);
    let shift = location + usize::from(has_migration);

    match route {
        Route::OnboardingWelcome {} => 0,
        Route::OnboardingLocation {} => 1,
        Route::OnboardingTerms {} => 1 + location,
        Route::OnboardingMigration {} => 2 + location,
        Route::OnboardingLanguage {} => 2 + shift,
        Route::OnboardingAccount {} => 3 + shift,
        Route::OnboardingBundles {} => 4 + shift,
        Route::OnboardingPreferences {} => 5 + shift,
        Route::OnboardingDownloading {} => 6 + shift,
        _ => 0,
    }
}

static LAST_STEP: AtomicUsize = AtomicUsize::new(0);

const SLIDE_DISTANCE: f32 = 44.;

pub(crate) fn onboarding_slide(content: impl IntoElement) -> impl IntoElement {
    let route = use_route::<Route>();
    let migration_query = use_migration();
    let selection = use_onboarding_selection();
    let step = onboarding_step_index(
        &route,
        has_migration_data(&migration_query),
        *selection.picks_location.read(),
    );
    let reduce_motion = selection.reduce_motion;

    let direction = use_hook(|| {
        let prev = LAST_STEP.swap(step, Ordering::Relaxed);
        if step >= prev { 1.0_f32 } else { -1.0 }
    });

    let anim = use_animation(|conf| {
        conf.on_creation(OnCreation::Run);
        AnimNum::new(0., 1.)
            .time(320)
            .ease(Ease::Out)
            .function(Function::Cubic)
    });

    let anim_finished = *anim.has_run_yet().read() && !*anim.is_running().read();
    let motion = !*reduce_motion.read();

    use_side_effect_with_deps(&anim_finished, move |&finished| {
        if finished {
            Platform::get().send(UserEvent::RequestRedraw);
        }
    });

    let p = if anim_finished {
        1.0
    } else {
        anim.get().value()
    };
    let slide_x = if motion {
        direction * (1. - p) * SLIDE_DISTANCE
    } else {
        0.
    };
    let opacity = if motion && !anim_finished { p } else { 1. };

    rect()
        .width(Size::fill())
        .height(Size::flex(1.0))
        .child(entrance_motion_layer(slide_x, 0., opacity, content))
        .into_element()
}

pub(crate) fn onboarding_page(
    illustration: impl IntoElement,
    content: impl IntoElement,
    nav: impl IntoElement,
) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .content(Content::Flex)
        .child(onboarding_slide(
            rect()
                .horizontal()
                .width(Size::fill())
                .height(Size::fill())
                .content(Content::Flex)
                .child(
                    rect()
                        .width(Size::flex(1.0))
                        .height(Size::fill())
                        .center()
                        .padding(Gaps::new_all(48.))
                        .child(illustration),
                )
                .child(
                    rect()
                        .vertical()
                        .width(Size::flex(1.0))
                        .height(Size::fill())
                        .main_align(Alignment::Center)
                        .padding(Gaps::new(48., 80., 24., 24.))
                        .spacing(24.)
                        .child(content),
                ),
        ))
        .child(nav)
        .into_element()
}

pub(crate) fn onboarding_illustration(icon: IconType) -> impl IntoElement {
    Icon::new(icon).size(240.).color(Color::WHITE)
}

pub(crate) fn onboarding_nav(
    back: Option<Route>,
    next: Route,
    next_enabled: bool,
) -> impl IntoElement {
    onboarding_nav_action(back, "Next", next_enabled, move |()| {
        let _ = RouterContext::get().replace(next.clone());
    })
}

pub(crate) fn onboarding_nav_action(
    back: Option<Route>,
    next_label: &str,
    next_enabled: bool,
    on_next: impl FnMut(()) + 'static,
) -> impl IntoElement {
    let mut on_next = on_next;

    rect()
        .horizontal()
        .width(Size::fill())
        .main_align(Alignment::End)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .padding(Gaps::new(0., 40., 32., 40.))
        .maybe_child(back.map(|route| {
            Button::new()
                .secondary()
                .width(Size::px(128.))
                .on_press(move |_| {
                    let _ = RouterContext::get().replace(route.clone());
                })
                .text("Back")
                .into_element()
        }))
        .child(
            Button::new()
                .primary()
                .width(Size::px(140.))
                .enabled(next_enabled)
                .on_press(move |_| on_next(()))
                .text(next_label.to_string())
                .child(Icon::new(IconType::ArrowRight).size(16.)),
        )
        .into_element()
}

pub(crate) fn predownload_toggle_row(predownload: State<bool>) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(16.)
        .content(Content::Flex)
        .padding(Gaps::new_symmetric(12., 16.))
        .corner_radius(CornerRadius::new_all(12.))
        .background(colors::page_elevated())
        .border(border_all_color(1., colors::component_border()))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(3.)
                .child(
                    label()
                        .text("Download content now")
                        .font_size(14.)
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(
                            "Downloads Minecraft and your chosen mods now, so you can play as \
                             soon as setup finishes. Turn it off to download each version the \
                             first time you play it.",
                        )
                        .font_size(11.)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(toggle(predownload))
        .into_element()
}

pub(crate) fn choice_row(
    title: &str,
    subtitle: &str,
    active: bool,
    on_press: impl FnMut(()) + 'static,
) -> Element {
    choice_row_sized(title, subtitle, active, Size::auto(), on_press)
}

pub(crate) fn choice_row_sized(
    title: &str,
    subtitle: &str,
    active: bool,
    height: Size,
    on_press: impl FnMut(()) + 'static,
) -> Element {
    let mut on_press = on_press;
    let border_color = if active {
        colors::fg_primary()
    } else {
        colors::component_border()
    };

    rect()
        .horizontal()
        .width(Size::fill())
        .height(height)
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(10.)
        .padding(Gaps::new_symmetric(8., 12.))
        .corner_radius(CornerRadius::new_all(8.))
        .border(border_all_color(1., border_color))
        .a11y_role(AccessibilityRole::Button)
        .cursor(CursorIcon::Pointer)
        .on_press(move |_| on_press(()))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(2.)
                .child(
                    label()
                        .text(title.to_string())
                        .font_size(13.)
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_primary()),
                )
                .maybe_child((!subtitle.is_empty()).then(|| {
                    label()
                        .text(subtitle.to_string())
                        .font_size(11.)
                        .color(colors::fg_primary())
                        .into_element()
                })),
        )
        .maybe_child(active.then(|| Icon::new(IconType::Check).size(15.)))
        .into_element()
}

pub(crate) fn version_chip(
    text: &str,
    active: bool,
    on_press: impl FnMut(()) + 'static,
) -> Element {
    let mut on_press = on_press;
    let (bg, border, fg) = if active {
        (
            colors::brand().with_a(38),
            colors::brand(),
            colors::fg_primary(),
        )
    } else {
        (
            Color::TRANSPARENT,
            colors::component_border(),
            colors::fg_secondary(),
        )
    };

    rect()
        .horizontal()
        .height(Size::px(32.))
        .center()
        .padding(Gaps::new_symmetric(0., 14.))
        .corner_radius(CornerRadius::new_all(999.))
        .background(bg)
        .border(border_all_color(1.5, border))
        .a11y_role(AccessibilityRole::Button)
        .cursor(CursorIcon::Pointer)
        .on_press(move |_| on_press(()))
        .child(
            label()
                .text(text.to_string())
                .font_size(13.)
                .font_weight(FontWeight::MEDIUM)
                .color(fg),
        )
        .into_element()
}

pub(crate) fn step_heading(title: &str, subtitle: &str) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(8.)
        .child(
            label()
                .text(title.to_string())
                .font_size(36.)
                .font_weight(FontWeight::BOLD)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text(subtitle.to_string())
                .font_size(16.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flow(has_migration: bool, picks_location: bool) -> Vec<Route> {
        let mut routes = vec![Route::OnboardingWelcome {}];

        if picks_location {
            routes.push(Route::OnboardingLocation {});
        }

        routes.push(Route::OnboardingTerms {});

        if has_migration {
            routes.push(Route::OnboardingMigration {});
        }

        routes.extend([
            Route::OnboardingLanguage {},
            Route::OnboardingAccount {},
            Route::OnboardingBundles {},
            Route::OnboardingPreferences {},
            Route::OnboardingDownloading {},
        ]);

        routes
    }

    #[test]
    fn every_run_numbers_its_steps_one_after_another() {
        for has_migration in [false, true] {
            for picks_location in [false, true] {
                let routes = flow(has_migration, picks_location);
                let total = onboarding_total(has_migration, picks_location);

                assert_eq!(
                    routes.len(),
                    total,
                    "the bar is drawn out of {total} but the run has {} steps \
                     (migration: {has_migration}, folder: {picks_location})",
                    routes.len()
                );

                for (position, route) in routes.iter().enumerate() {
                    assert_eq!(
                        onboarding_step_index(route, has_migration, picks_location),
                        position,
                        "{route:?} is numbered off its place in the run \
                         (migration: {has_migration}, folder: {picks_location})"
                    );
                }
            }
        }
    }

    #[test]
    fn a_run_without_the_folder_step_is_numbered_as_it_always_was() {
        assert_eq!(onboarding_total(false, false), 7);
        assert_eq!(onboarding_total(true, false), 8);
        assert_eq!(
            onboarding_step_index(&Route::OnboardingTerms {}, false, false),
            1
        );
    }
}
