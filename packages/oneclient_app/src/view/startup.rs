use freya::animation::{AnimNum, Ease, Function, OnCreation, OnFinish, use_animation};
use freya::{prelude::*, router::*};

use crate::AppAssets;
use crate::Route;
use crate::hooks::{
    terms_document, terms_is_loading, use_launcher, use_notifications_snapshot, use_settings_snapshot,
    use_splash, use_terms,
};
use crate::hooks::use_dispatch;
use crate::components::Button;
use crate::theme::colors;

#[derive(PartialEq)]
struct RecoveryActions {
    snapshots: usize,
}

impl Component for RecoveryActions {
    fn render(&self) -> impl IntoElement {
        let actions = use_dispatch();
        let snapshots = self.snapshots;

        let restore = actions.clone();

        let mut row = rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(8.)
            .child(
                Button::new()
                    .secondary()
                    .small()
                    .on_press(|_| crate::recovery::open_data_folder())
                    .text("Open data folder"),
            );

        if snapshots > 0 {
            row = row.child(
                Button::new()
                    .primary()
                    .small()
                    .on_press(move |_| crate::recovery::restore_latest(&restore))
                    .text("Restore last backup"),
            );
        }

        row.child(
            Button::new()
                .danger()
                .small()
                .on_press(move |_| crate::recovery::reset(&actions))
                .text("Reset database"),
        )
    }
}

#[derive(PartialEq)]
pub struct Startup;

impl Component for Startup {
    fn render(&self) -> impl IntoElement {
        let launcher = use_launcher();
        let settings = use_settings_snapshot();
        let notifications = use_notifications_snapshot();
        let terms = use_terms();
        let splash = use_splash();

        let intro = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            AnimNum::new(0., 1.)
                .time(620)
                .ease(Ease::Out)
                .function(Function::Cubic)
        });

        let pulse = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            conf.on_finish(OnFinish::reverse());
            AnimNum::new(0.82, 1.0)
                .time(1600)
                .ease(Ease::InOut)
                .function(Function::Sine)
        });

        let logo = use_memo(|| AppAssets::get_bytes("logo.svg").unwrap_or_default());

        if launcher.ready && !terms_is_loading(&terms) {
            let document = terms_document(&terms);
            let required_terms = document.as_ref().map(|doc| doc.version).unwrap_or(1);
            let required_privacy = document
                .as_ref()
                .map(|doc| doc.privacy_version())
                .unwrap_or(1);

            let stale = settings.settings.accepted_tos_version < required_terms
                || settings.settings.accepted_privacy_version < required_privacy;

            let destination = if settings.settings.declined_tos {
                Route::Home {}
            } else if !settings.settings.seen_onboarding {
                Route::OnboardingWelcome {}
            } else if stale {
                Route::OnboardingTerms {}
            } else {
                Route::Home {}
            };

            // Onboarding needs the bundle catalog before it can render its steps so gate it on the fetch
            let heading_to_onboarding = matches!(
                destination,
                Route::OnboardingWelcome { .. } | Route::OnboardingTerms { .. }
            );
            if !heading_to_onboarding || !launcher.fetching {
                // Raise the splash curtain so the hard route swap is hidden until Home settles onboarding animates itself
                if !heading_to_onboarding {
                    let mut active = splash.active;
                    let mut home_ready = splash.home_ready;
                    home_ready.set(false);
                    active.set(true);
                }
                let _ = RouterContext::get().replace(destination);
                return rect().into_element();
            }
        }

        let active = notifications.inbox.iter().find(|entry| entry.is_loading);

        let (message, detail): (String, Option<String>) =
            if let Some(err) = launcher.error.as_deref() {
                (
                    "Couldn't start OneClient".to_string(),
                    Some(err.to_string()),
                )
            } else if let Some(entry) = active {
                let detail = entry
                    .tasks
                    .iter()
                    .find(|t| t.total == 0 || t.current < t.total)
                    .map(|t| t.label.clone())
                    .or_else(|| entry.progress.map(|(c, t)| format!("{c} / {t}")));
                (entry.title.clone(), detail)
            } else if launcher.ready {
                ("Fetching versions and bundles...".to_string(), None)
            } else {
                ("Starting OneClient...".to_string(), None)
            };

        let is_error = launcher.error.is_some();

        let appear = intro.get().value();
        let rise = (1.0 - appear) * 16.;
        let breath = pulse.read().value();

        let dot_color = if is_error {
            colors::danger()
        } else {
            colors::brand()
        };

        let loading_row = rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(9.)
            .child(
                rect()
                    .width(Size::px(8.))
                    .height(Size::px(8.))
                    .corner_radius(CornerRadius::new_all(4.))
                    .background(dot_color),
            )
            .child(
                label()
                    .text(message)
                    .font_size(14.)
                    .color(colors::fg_primary().with_a(180)),
            );

        let mut content = rect()
            .center()
            .vertical()
            .spacing(14.)
            .child(
                SvgViewer::new(("logo.svg", logo.read().cloned()))
                    .show_loader(false)
                    .width(Size::px(288.))
                    .height(Size::px(60.))
                    .color(colors::fg_primary()),
            )
            .child(
                label()
                    .text("The only Minecraft launcher you'll need.")
                    .font_size(15.)
                    .color(colors::fg_primary().with_a(140)),
            )
            .child(rect().height(Size::px(14.)))
            .child(loading_row);

        if let Some(detail) = detail {
            content = content.child(
                label()
                    .text(detail)
                    .font_size(12.)
                    .max_lines(if is_error { 4 } else { 1 })
                    .color(colors::fg_secondary().with_a(180)),
            );
        }

        if is_error {
            content = content
                .child(rect().height(Size::px(6.)))
                .child(RecoveryActions { snapshots: launcher.snapshots })
                .child(
                    label()
                        .text(format!(
                            "Your data stays in {}. Nothing here deletes it.",
                            launcher.data_dir
                        ))
                        .font_size(11.)
                        .max_lines(2)
                        .color(colors::fg_secondary().with_a(130)),
                );
        }

        let content = content
            .position(Position::new_absolute())
            .width(Size::fill())
            .height(Size::fill())
            .center()
            .opacity(appear * breath)
            .margin(Gaps::new(rise, 0., 0., 0.));

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .background(colors::page())
            .window_drag()
            .child(content)
            .into_element()
    }
}
