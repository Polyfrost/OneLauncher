use std::path::{Path, PathBuf};

use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::relocate::RelocationPlan;
use oneclient_core::storage::format_bytes;

use super::{section_header, settings_page, settings_row};
use crate::Route;
use crate::components::{Button, Icon, IconType, OverlayPopup, open_folder_button, toggle};
use crate::hooks::{
    Actions, DiscardLeftoversKeys, mutation_error, mutation_is_running, try_leftovers,
    use_discard_leftovers, use_dispatch, use_launcher, use_leftovers, use_settings_snapshot,
};
use crate::theme::colors;
use crate::ui::{border_all_color, note, path_block};

#[derive(PartialEq)]
pub struct SettingsLauncher;

impl Component for SettingsLauncher {
    fn render(&self) -> impl IntoElement {
        let settings = use_settings_snapshot().settings;
        let dispatch = use_dispatch();

        let discord_rpc = use_state({
            let v = settings.discord_enabled;
            move || v
        });

        let crash_reporting = use_state({
            let v = settings.crash_reporting;
            move || v
        });

        let mut first = use_state(|| true);
        {
            let settings = settings.clone();
            use_side_effect(move || {
                let discord = *discord_rpc.read();
                let crash = *crash_reporting.read();
                if *first.peek() {
                    first.set(false);
                    return;
                }
                let mut next = settings.clone();
                next.discord_enabled = discord;
                next.crash_reporting = crash;
                dispatch.set_settings(next);
            });
        }

        settings_page()
            .child(section_header("GENERAL"))
            .child(settings_row(
                IconType::Link03,
                "Discord RPC",
                "Enable Discord Rich Presence.",
                toggle(discord_rpc),
            ))
            .child(settings_row(
                IconType::AlertTriangle,
                "Crash Reporting",
                "Send anonymous crash and error reports to help fix bugs. Applies on restart.",
                toggle(crash_reporting),
            ))
            .child(section_header("FOLDERS AND FILES"))
            .child(DataFolder.into_element())
            .into_element()
    }
}

#[derive(PartialEq)]
struct DataFolder;

impl Component for DataFolder {
    fn render(&self) -> impl IntoElement {
        let discard = use_discard_leftovers();
        let leftovers_query = use_leftovers();
        let data_dir = use_launcher().data_dir;
        let actions = use_dispatch();

        let pending = use_state(|| None::<RelocationPlan>);
        let error = use_state(|| None::<String>);
        let checking = use_state(|| false);

        let checking_now = *checking.read();

        let change = Button::new()
            .secondary()
            .small()
            .disabled(checking_now)
            .on_press(move |_| browse(pending, error, checking))
            .text(if checking_now { "Checking…" } else { "Change…" });

        let mut section = rect()
            .vertical()
            .width(Size::fill())
            .spacing(4.)
            .child(settings_row(
                IconType::Folder,
                "Launcher Folder",
                data_dir.clone(),
                row_actions(PathBuf::from(data_dir), change),
            ));

        if let Some(message) = error.read().clone() {
            section = section.child(note(message, colors::danger()));
        }

        if let Some(left) = try_leftovers(&leftovers_query) {
            let clearing = mutation_is_running(&discard);

            let remove = Button::new()
                .danger()
                .small()
                .disabled(clearing)
                .on_press(move |_| {
                    discard.mutate(DiscardLeftoversKeys);
                })
                .text(if clearing { "Removing…" } else { "Remove" });

            section = section.child(settings_row(
                IconType::Database01,
                "Old location",
                format!(
                    "{} still sitting in {}. Nothing uses it any more.",
                    format_bytes(left.bytes),
                    left.path.display()
                ),
                row_actions(left.path, remove),
            ));
        }

        if let Some(message) = mutation_error(&discard) {
            section = section.child(note(message, colors::danger()));
        }

        if let Some(planned) = pending.read().clone() {
            section = section.child(confirm_move(planned, pending, actions));
        }

        section.into_element()
    }
}

fn browse(
    mut pending: State<Option<RelocationPlan>>,
    mut error: State<Option<String>>,
    mut checking: State<bool>,
) {
    if *checking.peek() {
        return;
    }

    spawn(async move {
        let mut dialog =
            rfd::AsyncFileDialog::new().set_title("Choose where OneClient should store game data");

        if let Some(start) = oneclient_common::paths::picker_start_dir() {
            dialog = dialog.set_directory(start);
        }

        let Some(handle) = dialog.pick_folder().await else {
            return;
        };

        checking.set(true);
        error.set(None);

        match plan_move(handle.path()).await {
            Ok(planned) => pending.set(Some(planned)),
            Err(message) => error.set(Some(message)),
        }

        checking.set(false);
    });
}

async fn plan_move(picked: &Path) -> Result<RelocationPlan, String> {
    let state = crate::launcher::state().map_err(|err| err.to_string())?;
    oneclient_core::relocate::plan(&state, picked).await
}

fn row_actions(folder: PathBuf, action: Button) -> impl IntoElement {
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(8.)
        .child(open_folder_button(folder))
        .child(action)
}

fn confirm_move(
    planned: RelocationPlan,
    mut pending: State<Option<RelocationPlan>>,
    actions: Actions,
) -> Element {
    let start = planned.clone();

    let free_after = planned
        .available
        .map(|available| available.saturating_sub(planned.bytes));

    let mut card = rect()
        .vertical()
        .width(Size::px(460.))
        .max_width(Size::window_percent(90.))
        .spacing(14.)
        .padding(Gaps::new_all(20.))
        .corner_radius(CornerRadius::new_all(14.))
        .background(colors::page_elevated())
        .border(border_all_color(1., colors::component_border()))
        .child(
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(10.)
                .child(Icon::new(IconType::FolderDownload).size(20.))
                .child(
                    label()
                        .text("Move game data?")
                        .font_size(16.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                ),
        )
        .child(path_block("From", &planned.from))
        .child(path_block("To", &planned.to))
        .child(
            label()
                .text(match free_after {
                    Some(free) => format!(
                        "{} to copy, leaving {} free on the new drive.",
                        format_bytes(planned.bytes),
                        format_bytes(free)
                    ),
                    None => format!("{} to copy.", format_bytes(planned.bytes)),
                })
                .font_size(12.)
                .color(colors::fg_secondary()),
        );

    if let Some(warning) = planned.warning.clone() {
        card = card.child(note(warning, colors::code_warn()));
    }

    card = card
        .child(
            label()
                .text(
                    "Your settings and sign-in stay where they are. The old copy is kept until \
                     you remove it, and OneClient has to restart before it uses the new folder. \
                     The launcher shows the move on its own screen until it is done.",
                )
                .font_size(12.)
                .max_lines(4)
                .color(colors::fg_secondary()),
        )
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .main_align(Alignment::End)
                .spacing(8.)
                .child(
                    Button::new()
                        .secondary()
                        .on_press(move |_| pending.set(None))
                        .text("Cancel"),
                )
                .child(
                    Button::new()
                        .primary()
                        .on_press(move |_| {
                            actions.relocate(start.clone());
                            pending.set(None);
                            let _ = RouterContext::get().replace(Route::Relocating {});
                        })
                        .text("Move"),
                ),
        );

    OverlayPopup::new()
        .on_close(move |_| pending.set(None))
        .child(
            rect()
                .width(Size::window_percent(100.))
                .height(Size::window_percent(100.))
                .center()
                .child(card),
        )
        .into_element()
}
