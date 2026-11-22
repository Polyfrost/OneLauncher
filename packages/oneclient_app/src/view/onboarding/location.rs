use std::path::PathBuf;

use freya::prelude::*;
use freya::router::RouterContext;

use oneclient_core::settings::data_dir::{self, DataDirCheck};

use crate::components::{IconType, onedrive_warning};
use crate::hooks::{use_dispatch, use_launcher, use_onboarding_selection};
use crate::i18n::t;
use crate::routes::Route;
use crate::state::AppChannel;
use crate::theme::{colors, text_size};
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
        let mut onedrive = use_state(|| None::<DataDirCheck>);
        let mut default_accepted = use_state(|| false);

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

        let default_path = oneclient_common::paths::config_dir()
            .ok()
            .map(PathBuf::from);
        let default_dir = default_path
            .as_ref()
            .map(|dir| dir.display().to_string())
            .unwrap_or_default();
        let default_flag = default_path.clone();

        let confirm = move |()| {
            if *waiting.peek() {
                return;
            }

            if settled {
                let _ = RouterContext::get().replace(Route::OnboardingTerms {});
                return;
            }

            let chosen = picked.peek().clone();

            // The default folder can also be redirected into OneDrive, so it gets the same warning
            if !answered
                && chosen.is_none()
                && !*default_accepted.peek()
                && let Some(path) = default_flag.clone().filter(|p| data_dir::in_onedrive(p))
            {
                onedrive.set(Some(DataDirCheck {
                    path,
                    warning: None,
                }));
                return;
            }

            let actions = actions.clone();
            let station = actions.station();
            let events = actions.events();

            waiting.set(true);
            error.set(None);

            spawn_in_window(async move {
                if !answered {
                    if let Err(message) = data_dir::apply(chosen).await {
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
                t("oneclient.onboarding.location.title"),
                t("oneclient.onboarding.location.subtitle"),
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
                .child(choice_row(
                    t("oneclient.onboarding.location.your_folder"),
                    &folder,
                    true,
                    |()| {},
                ))
                .child(
                    label()
                        .text(t("oneclient.onboarding.location.answered"))
                        .font_size(text_size(13.))
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
                        t("oneclient.onboarding.location.this_pc"),
                        &default_dir,
                        chosen.is_none(),
                        move |()| {
                            picked.set(None);
                            warning.set(None);
                            error.set(None);
                        },
                    ))
                    .child(choice_row(
                        t("oneclient.onboarding.location.another_drive"),
                        &chosen
                            .as_ref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| {
                                t("oneclient.onboarding.location.pick_folder").to_string()
                            }),
                        chosen.is_some(),
                        move |()| browse(picked, warning, error, onedrive),
                    )),
            )
        };

        if let Some(message) = warning.read().clone() {
            content = content.child(note(message, colors::code_warn()));
        }

        if let Some(flagged) = onedrive.read().clone() {
            let mut dismiss = onedrive;
            let mut back = onedrive;
            let mut proceed = onedrive;
            let flagged_path = flagged.path.clone();
            let default_path = default_path.clone();
            content = content.child(onedrive_warning(
                &flagged.path,
                move |()| dismiss.set(None),
                move |()| {
                    back.set(None);
                    browse(picked, warning, error, onedrive);
                },
                move |()| {
                    if Some(&flagged_path) == default_path.as_ref() {
                        proceed.set(None);
                        default_accepted.set(true);
                    } else if let Some(checked) = proceed.write().take() {
                        error.set(None);
                        warning.set(checked.warning);
                        picked.set(Some(checked.path));
                    }
                },
            ));
        }

        let failure = error.read().clone().or_else(|| launcher.error.clone());

        if let Some(message) = failure {
            content = content.child(note(message, colors::danger()));
        }

        let next_label = if running {
            t("oneclient.onboarding.location.setting_up")
        } else if answered && !settled {
            t("oneclient.common.try_again")
        } else {
            t("oneclient.common.next")
        };

        onboarding_page(
            onboarding_illustration(IconType::OnboardingFolder),
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
    mut onedrive: State<Option<DataDirCheck>>,
) {
    spawn(async move {
        let Some(handle) = rfd::AsyncFileDialog::new()
            .set_title(t("oneclient.onboarding.location.picker_title"))
            .pick_folder()
            .await
        else {
            return;
        };

        match data_dir::check(handle.path()).await {
            Ok(checked) if data_dir::in_onedrive(&checked.path) => onedrive.set(Some(checked)),
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
