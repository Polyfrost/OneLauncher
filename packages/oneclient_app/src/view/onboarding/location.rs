use std::path::PathBuf;

use freya::prelude::*;
use freya::router::RouterContext;

use crate::components::IconType;
use crate::hooks::{use_dispatch, use_launcher, use_onboarding_selection};
use crate::routes::Route;
use crate::state::AppChannel;
use crate::theme::colors;
use crate::ui::note;
use crate::view::onboarding::{
    choice_row, onboarding_illustration, onboarding_nav_action, onboarding_page, step_heading,
};

#[derive(PartialEq)]
pub struct OnboardingLocation;

impl Component for OnboardingLocation {
    fn render(&self) -> impl IntoElement {
        let actions = use_dispatch();
        let launcher = use_launcher();
        let selection = use_onboarding_selection();

        let mut picked = use_state(|| None::<PathBuf>);
        let mut warning = use_state(|| None::<String>);
        let mut error = use_state(|| None::<String>);
        let mut waiting = use_state(|| false);

        let answered = !launcher.needs_location;
        let settled = answered && launcher.ready;

        if !*selection.picks_location.read() && settled {
            let _ = RouterContext::get().replace(Route::OnboardingTerms {});
            return rect().into_element();
        }

        if *waiting.read() && launcher.ready {
            let _ = RouterContext::get().replace(Route::OnboardingTerms {});
            return rect().into_element();
        }

        let default_dir = oneclient_common::paths::config_dir()
            .map(|dir| dir.display().to_string())
            .unwrap_or_default();

        let confirm = move |()| {
            if *waiting.peek() {
                return;
            }

            if settled {
                let _ = RouterContext::get().replace(Route::OnboardingTerms {});
                return;
            }

            let chosen = picked.peek().clone();
            let actions = actions.clone();
            let station = actions.station();
            let events = actions.events();

            waiting.set(true);
            error.set(None);

            spawn_forever(async move {
                if !answered {
                    if let Err(message) = oneclient_core::settings::data_dir::apply(chosen).await {
                        error.set(Some(message));
                        waiting.set(false);
                        return;
                    }

                    let mut station = station;
                    let mut guard = station.write_channel(AppChannel::Launcher);
                    guard.launcher.needs_location = false;
                }

                match crate::events::start_launcher(station, events).await {
                    Ok(()) => actions.sync_bundles(),
                    Err(err) => {
                        crate::events::report_startup_failure(&station, &err);
                        waiting.set(false);
                    }
                }
            });
        };

        let chosen = picked.read().clone();
        let running = *waiting.read();

        let mut content = rect()
            .vertical()
            .width(Size::fill())
            .spacing(16.)
            .child(step_heading(
                "Where should OneClient live?",
                "All your data including mods, worlds and game settings gets stored here. This can be changed later from Settings.",
            ));

        content = if answered {
            let folder = if launcher.data_dir.is_empty() {
                chosen
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| default_dir.clone())
            } else {
                launcher.data_dir.clone()
            };

            content
                .child(choice_row("Your folder", &folder, true, |()| {}))
                .child(
                    label()
                        .text(
                            "This is where OneClient is set up. Settings can move it once you are \
                             through setup.",
                        )
                        .font_size(13.)
                        .max_lines(2)
                        .color(colors::fg_secondary()),
                )
        } else {
            content.child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .spacing(10.)
                    .child(choice_row(
                        "This PC",
                        &default_dir,
                        chosen.is_none(),
                        move |()| {
                            picked.set(None);
                            warning.set(None);
                            error.set(None);
                        },
                    ))
                    .child(choice_row(
                        "Another drive",
                        &chosen
                            .as_ref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| "Click to pick a folder…".to_string()),
                        chosen.is_some(),
                        move |()| browse(picked, warning, error),
                    )),
            )
        };

        if let Some(message) = warning.read().clone() {
            content = content.child(note(message, colors::code_warn()));
        }

        let failure = error.read().clone().or_else(|| launcher.error.clone());

        if let Some(message) = failure {
            content = content.child(note(message, colors::danger()));
        }

        let next_label = if running {
            "Setting up…"
        } else if answered && !settled {
            "Try again"
        } else {
            "Next"
        };

        onboarding_page(
            onboarding_illustration(IconType::Folder),
            content.into_element(),
            onboarding_nav_action(
                Some(Route::OnboardingWelcome {}),
                next_label,
                !running,
                confirm,
            ),
        )
        .into_element()
    }
}

fn browse(
    mut picked: State<Option<PathBuf>>,
    mut warning: State<Option<String>>,
    mut error: State<Option<String>>,
) {
    spawn(async move {
        let Some(handle) = rfd::AsyncFileDialog::new()
            .set_title("Choose where OneClient stores game data")
            .pick_folder()
            .await
        else {
            return;
        };

        match oneclient_core::settings::data_dir::check(handle.path()).await {
            Ok(checked) => {
                error.set(None);
                warning.set(checked.warning);
                picked.set(Some(checked.path));
            }
            Err(message) => {
                picked.set(None);
                warning.set(None);
                error.set(Some(message));
            }
        }
    });
}
