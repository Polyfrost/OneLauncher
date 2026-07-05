use std::collections::HashSet;

use freya::prelude::*;
use freya::router::{Outlet, use_route};

use crate::Route;
use crate::components::window_controls;
use crate::hooks::{
    OnboardingSelectionState, onboarding_bundles_items, use_onboarding_bundles,
    use_provide_onboarding_selection,
};
use crate::theme::{self, colors};
use crate::view::onboarding::{LoadingBackdrop, ONBOARDING_TOTAL, onboarding_step_index};

#[derive(PartialEq)]
pub struct OnboardingShell;

impl Component for OnboardingShell {
    fn render(&self) -> impl IntoElement {
        let selected = use_state(HashSet::new);
        let seeded = use_state(|| false);
        let language = use_state(|| "English".to_string());
        let reduce_motion = use_state(|| false);
        let predownload = use_state(|| true);
        let setup_started = use_state(|| false);
        use_provide_onboarding_selection(OnboardingSelectionState {
            selected,
            seeded,
            language,
            reduce_motion,
            predownload,
            setup_started,
        });

        let route = use_route::<Route>();
        let step_index = onboarding_step_index(&route);

        let bundles = use_onboarding_bundles();
        let show_backdrop =
            matches!(&route, Route::OnboardingDownloading {}) && *setup_started.read();
        let backdrop = show_backdrop.then(|| {
            let clusters = onboarding_bundles_items(&bundles)
                .map(|items| items.into_iter().map(|cb| cb.cluster).collect())
                .unwrap_or_default();
            LoadingBackdrop { clusters }.into_element()
        });

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .background(colors::page())
            .color(colors::fg_primary())
            .overflow(Overflow::Clip)
            .maybe_child(backdrop)
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .height(Size::fill())
                    .content(Content::Flex)
                    .layer(Layer::Relative(30))
                    .child(progress_bar(step_index))
                    .child(header())
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::flex(1.0))
                            .overflow(Overflow::Clip)
                            .child(Outlet::<Route>::new())
                            .child(copyright()),
                    ),
            )
    }
}

fn progress_bar(step_index: usize) -> impl IntoElement {
    let filled = ((step_index + 1) as f32 / ONBOARDING_TOTAL as f32) * 100.;

    rect()
        .width(Size::fill())
        .height(Size::px(3.))
        .background(Color::WHITE.with_a(28))
        .child(
            rect()
                .width(Size::percent(filled))
                .height(Size::fill())
                .background(colors::brand()),
        )
}

fn header() -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::px(theme::NAVBAR_HEIGHT_PX))
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .padding(Gaps::new_symmetric(0., 24.))
        .window_drag()
        .child(logo())
        .child(rect().width(Size::flex(1.0)).height(Size::fill()))
        .child(window_controls())
}

fn logo() -> impl IntoElement {
    let bytes = use_memo(|| crate::AppAssets::get_bytes("logo.svg").unwrap_or_default());

    svg(bytes.read().cloned())
        .height(Size::px(36.))
        .width(Size::px(170.))
        .color(colors::fg_primary())
}

fn copyright() -> impl IntoElement {
    rect()
        .position(Position::new_absolute().bottom(16.).left(24.))
        .interactive(false)
        .child(
            label()
                .text("© Polyfrost. All rights reserved.")
                .font_size(11.)
                .color(colors::fg_secondary()),
        )
}
