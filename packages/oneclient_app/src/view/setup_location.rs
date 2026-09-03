use std::path::PathBuf;

use freya::prelude::*;
use freya::router::RouterContext;

use crate::components::Button;
use crate::hooks::{use_dispatch, use_launcher};
use crate::routes::Route;
use crate::state::AppChannel;
use crate::theme::colors;
use crate::ui::note;
use crate::view::onboarding::choice_row;

#[derive(PartialEq)]
pub struct SetupLocation;

impl Component for SetupLocation {
    fn render(&self) -> impl IntoElement {
        let actions = use_dispatch();
        let launcher = use_launcher();

        let mut picked = use_state(|| None::<PathBuf>);
        let mut warning = use_state(|| None::<String>);
        let mut error = use_state(|| None::<String>);
        let mut busy = use_state(|| false);

        if !launcher.needs_location {
            let _ = RouterContext::get().replace(Route::Startup {});
            return rect().into_element();
        }

        let default_dir = oneclient_common::paths::config_dir()
            .map(|dir| dir.display().to_string())
            .unwrap_or_default();

        let confirm = move |_| {
            if *busy.peek() {
                return;
            }

            let chosen = picked.peek().clone();
            let actions = actions.clone();
            let station = actions.station();
            let events = actions.events();

            busy.set(true);
            error.set(None);

            spawn_forever(async move {
                if let Err(message) =
                    oneclient_core::settings::data_dir::apply(chosen).await
                {
                    error.set(Some(message));
                    busy.set(false);
                    return;
                }

                {
                    let mut station = station;
                    let mut guard = station.write_channel(AppChannel::Launcher);
                    guard.launcher.needs_location = false;
                }

                match crate::events::start_launcher(station, events).await {
                    Ok(()) => actions.sync_bundles(),
                    Err(err) => crate::events::report_startup_failure(&station, &err),
                }
            });
        };

        let chosen = picked.read().clone();
        let running = *busy.read();

        let mut content = rect()
            .vertical()
            .width(Size::px(520.))
            .spacing(20.)
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .spacing(8.)
                    .child(
                        label()
                            .text("Choose a folder location for OneClient")
                            .font_size(26.)
                            .font_weight(FontWeight::BOLD)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text(
                                "Any content downloaded via OneClient (such as Minecraft versions, mods, your worlds) will go in this folder. Make sure you have sufficient storage in your chosen disk.",
                            )
                            .font_size(14.)
                            .color(colors::fg_secondary()),
                    ),
            )
            .child(
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
            );

        if let Some(message) = warning.read().clone() {
            content = content.child(note(message, colors::code_warn()));
        }

        if let Some(message) = error.read().clone() {
            content = content.child(note(message, colors::danger()));
        }

        content = content.child(
            Button::new()
                .primary()
                .width(Size::px(160.))
                .enabled(!running)
                .on_press(confirm)
                .text(if running { "Setting up…" } else { "Continue" }),
        );

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .center()
            .background(colors::page())
            .window_drag()
            .child(content)
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
