mod account;
mod bundles;
mod downloading;
mod language;
mod preferences;
mod welcome;

pub use account::OnboardingAccount;
pub use bundles::OnboardingBundles;
pub use downloading::{LoadingBackdrop, OnboardingDownloading};
pub use language::OnboardingLanguage;
pub use preferences::OnboardingPreferences;
pub use welcome::OnboardingWelcome;

use std::sync::atomic::{AtomicUsize, Ordering};

use freya::animation::*;
use freya::prelude::*;
use freya::router::{RouterContext, use_route};

use crate::Route;
use crate::components::{Button, Icon, IconType, toggle};
use crate::hooks::use_onboarding_selection;
use crate::theme::colors;
use crate::ui::{border_all_color, entrance_motion_layer};

pub const ONBOARDING_TOTAL: usize = 6;

pub fn onboarding_step_index(route: &Route) -> usize {
    match route {
        Route::OnboardingWelcome {} => 0,
        Route::OnboardingLanguage {} => 1,
        Route::OnboardingAccount {} => 2,
        Route::OnboardingBundles {} => 3,
        Route::OnboardingPreferences {} => 4,
        Route::OnboardingDownloading {} => 5,
        _ => 0,
    }
}

static LAST_STEP: AtomicUsize = AtomicUsize::new(0);

const SLIDE_DISTANCE: f32 = 44.;

pub(crate) fn onboarding_slide(content: impl IntoElement) -> impl IntoElement {
    let route = use_route::<Route>();
    let step = onboarding_step_index(&route);
    let reduce_motion = use_onboarding_selection().reduce_motion;

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

    let p = if anim_finished { 1.0 } else { anim.get().value() };
    let slide_x = if motion { direction * (1. - p) * SLIDE_DISTANCE } else { 0. };
    let opacity = if motion && !anim_finished { p } else { 1. };

    rect()
        .width(Size::fill())
        .height(Size::flex(1.0))
        .child(entrance_motion_layer(slide_x, 0., opacity, content))
        .into_element()
}

pub(crate) fn onboarding_page(illustration: impl IntoElement, content: impl IntoElement, nav: impl IntoElement) -> impl IntoElement {
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

pub(crate) fn onboarding_nav(back: Option<Route>, next: Route, next_enabled: bool) -> impl IntoElement {
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
                .on_press(move |_| {
                    let _ = RouterContext::get().replace(next.clone());
                })
                .text("Next")
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
                            "Fetch your selected versions and mods during setup. Turn off to \
                             download each version the first time you launch it.",
                        )
                        .font_size(11.)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(toggle(predownload))
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
