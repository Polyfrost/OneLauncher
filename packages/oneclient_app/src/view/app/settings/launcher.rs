use std::path::{Path, PathBuf};

use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::relocate::RelocationPlan;
use oneclient_core::settings::{LaunchBehaviour, LauncherSettings, data_dir};
use oneclient_core::storage::format_bytes;

use super::{resettable, section_header, settings_page, settings_row};
use crate::Route;
use crate::components::{
    Button, Dropdown, Icon, IconType, OverlayPopup, open_folder_button, toggle,
};
use crate::hooks::{
    Actions, DiscardLeftoversKeys, mutation_error, mutation_is_running, try_leftovers,
    use_discard_leftovers, use_dispatch, use_launcher, use_leftovers, use_settings_snapshot,
};
use crate::i18n::{t, tf};
use crate::theme::colors;
use crate::ui::{border_all_color, note, path_block};

#[derive(PartialEq)]
pub struct SettingsLauncher;

impl Component for SettingsLauncher {
    fn render(&self) -> impl IntoElement {
        let settings = use_settings_snapshot().settings;
        let defaults = LauncherSettings::default();
        let dispatch = use_dispatch();

        let discord_rpc = use_state({
            let v = settings.discord_enabled;
            move || v
        });

        let crash_reporting = use_state({
            let v = settings.crash_reporting;
            move || v
        });

        let start_maximized = use_state({
            let v = settings.start_maximized;
            move || v
        });

        let run_in_background = use_state({
            let v = settings.run_in_background;
            move || v
        });

        let show_tray_icon = use_state({
            let v = settings.show_tray_icon;
            move || v
        });

        let launch_behaviour = use_state({
            let v = settings.launch_behaviour;
            move || v
        });

        let open_game_log = use_state({
            let v = settings.open_game_log_on_launch;
            move || v
        });

        let mut first = use_state(|| true);
        use_side_effect(move || {
            let discord = *discord_rpc.read();
            let crash = *crash_reporting.read();
            let maximized = *start_maximized.read();
            let background = *run_in_background.read();
            let tray = *show_tray_icon.read();
            let behaviour = *launch_behaviour.read();
            let game_log = *open_game_log.read();
            if *first.peek() {
                first.set(false);
                return;
            }
            dispatch.edit_settings(|next| {
                next.discord_enabled = discord;
                next.crash_reporting = crash;
                next.start_maximized = maximized;
                next.run_in_background = background;
                next.show_tray_icon = tray;
                next.launch_behaviour = behaviour;
                next.open_game_log_on_launch = game_log;
            });
        });

        // The only way back for someone who declined during onboarding
        let consent_summary = if settings.declined_tos {
            t("oneclient.settings.launcher.terms.declined")
        } else {
            t("oneclient.settings.launcher.terms.accepted")
        };
        let review_terms = Button::new()
            .secondary()
            .small()
            .on_press(|_| {
                let _ = RouterContext::get().replace(Route::OnboardingTerms {});
            })
            .text(t("oneclient.settings.launcher.terms.review"));

        settings_page()
            .child(section_header(t(
                "oneclient.settings.launcher.section.general",
            )))
            .child(settings_row(
                IconType::Link03,
                t("oneclient.settings.launcher.discord_rpc.title"),
                t("oneclient.settings.launcher.discord_rpc.description"),
                resettable(toggle(discord_rpc), discord_rpc, defaults.discord_enabled),
            ))
            .child(settings_row(
                IconType::AlertTriangle,
                t("oneclient.settings.launcher.crash_reporting.title"),
                t("oneclient.settings.launcher.crash_reporting.description"),
                resettable(
                    toggle(crash_reporting),
                    crash_reporting,
                    defaults.crash_reporting,
                ),
            ))
            .child(settings_row(
                IconType::Maximize01,
                t("oneclient.settings.launcher.start_maximized.title"),
                t("oneclient.settings.launcher.start_maximized.description"),
                resettable(
                    toggle(start_maximized),
                    start_maximized,
                    defaults.start_maximized,
                ),
            ))
            .child(settings_row(
                IconType::File02,
                t("oneclient.settings.launcher.terms.title"),
                consent_summary,
                review_terms,
            ))
            .child(section_header(t(
                "oneclient.settings.launcher.section.system",
            )))
            .child(settings_row(
                IconType::Eye,
                t("oneclient.settings.launcher.launch_behaviour.title"),
                t("oneclient.settings.launcher.launch_behaviour.description"),
                resettable(
                    launch_behaviour_field(launch_behaviour),
                    launch_behaviour,
                    defaults.launch_behaviour,
                ),
            ))
            .child(settings_row(
                IconType::Terminal,
                t("oneclient.settings.launcher.open_game_log.title"),
                t("oneclient.settings.launcher.open_game_log.description"),
                resettable(
                    toggle(open_game_log),
                    open_game_log,
                    defaults.open_game_log_on_launch,
                ),
            ))
            .child(settings_row(
                IconType::Moon01,
                t("oneclient.settings.launcher.run_in_background.title"),
                t("oneclient.settings.launcher.run_in_background.description"),
                resettable(
                    toggle(run_in_background),
                    run_in_background,
                    defaults.run_in_background,
                ),
            ))
            .child(settings_row(
                IconType::LayoutTop,
                t("oneclient.settings.launcher.tray_icon.title"),
                t("oneclient.settings.launcher.tray_icon.description"),
                resettable(
                    toggle(show_tray_icon),
                    show_tray_icon,
                    defaults.show_tray_icon,
                ),
            ))
            .child(section_header(t(
                "oneclient.settings.launcher.section.folders",
            )))
            .child(DataFolder.into_element())
            .into_element()
    }
}

fn launch_behaviour_field(mut selected: State<LaunchBehaviour>) -> impl IntoElement {
    let options: Vec<String> = LaunchBehaviour::ALL
        .iter()
        .map(|behaviour| behaviour.label().to_string())
        .collect();

    Dropdown::new(selected.read().label(), options)
        .width(Size::px(220.))
        .height(Size::px(34.))
        .on_select(move |idx: usize| {
            if let Some(behaviour) = LaunchBehaviour::ALL.get(idx).copied() {
                selected.set(behaviour);
            }
        })
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
            .text(if checking_now {
                t("oneclient.settings.launcher.data_folder.checking")
            } else {
                t("oneclient.settings.launcher.data_folder.change")
            });

        let mut buttons = Vec::new();

        if !data_dir::is_default(Path::new(&data_dir)) {
            buttons.push(
                Button::new()
                    .secondary()
                    .small()
                    .disabled(checking_now)
                    .on_press(move |_| plan(pending, error, checking, Source::Default))
                    .text(t("oneclient.common.reset")),
            );
        }

        buttons.push(change);

        let mut section = rect()
            .vertical()
            .width(Size::fill())
            .spacing(4.)
            .child(settings_row(
                IconType::Folder,
                t("oneclient.settings.launcher.data_folder.title"),
                data_dir.clone(),
                row_actions(PathBuf::from(data_dir), buttons),
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
                .text(if clearing {
                    t("oneclient.settings.launcher.leftovers.removing")
                } else {
                    t("oneclient.common.remove")
                });

            section = section.child(settings_row(
                IconType::Database01,
                t("oneclient.settings.launcher.leftovers.title"),
                tf(
                    "oneclient.settings.launcher.leftovers.description",
                    &[&format_bytes(left.bytes), &left.path.display()],
                ),
                row_actions(left.path, vec![remove]),
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Picked,
    Default,
}

fn browse(
    pending: State<Option<RelocationPlan>>,
    error: State<Option<String>>,
    checking: State<bool>,
) {
    plan(pending, error, checking, Source::Picked);
}

fn plan(
    mut pending: State<Option<RelocationPlan>>,
    mut error: State<Option<String>>,
    mut checking: State<bool>,
    source: Source,
) {
    if *checking.peek() {
        return;
    }

    spawn(async move {
        let picked = match source {
            Source::Picked => {
                let mut dialog = rfd::AsyncFileDialog::new()
                    .set_title(t("oneclient.settings.launcher.data_folder.picker_title"));

                if let Some(start) = oneclient_common::paths::picker_start_dir() {
                    dialog = dialog.set_directory(start);
                }

                let Some(handle) = dialog.pick_folder().await else {
                    return;
                };

                Some(handle.path().to_path_buf())
            }
            Source::Default => None,
        };

        checking.set(true);
        error.set(None);

        match crate::launcher::off_ui(async move { plan_move(picked.as_deref()).await }).await {
            Ok(planned) => pending.set(Some(planned)),
            Err(message) => error.set(Some(message)),
        }

        checking.set(false);
    });
}

async fn plan_move(picked: Option<&Path>) -> Result<RelocationPlan, String> {
    let state = crate::launcher::state().map_err(|err| err.to_string())?;

    match picked {
        Some(picked) => oneclient_core::relocate::plan(&state, picked).await,
        None => oneclient_core::relocate::plan_default(&state).await,
    }
}

fn row_actions(folder: PathBuf, actions: Vec<Button>) -> impl IntoElement {
    let mut row = rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(8.)
        .child(open_folder_button(folder));

    for action in actions {
        row = row.child(action);
    }

    row
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
                        .text(t("oneclient.settings.launcher.move.title"))
                        .font_size(16.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                ),
        )
        .child(path_block(
            t("oneclient.settings.launcher.move.from"),
            &planned.from,
        ))
        .child(path_block(
            t("oneclient.settings.launcher.move.to"),
            &planned.to,
        ))
        .child(
            label()
                .text(match free_after {
                    Some(free) => tf(
                        "oneclient.settings.launcher.move.size_with_free",
                        &[&format_bytes(planned.bytes), &format_bytes(free)],
                    ),
                    None => tf(
                        "oneclient.settings.launcher.move.size",
                        &[&format_bytes(planned.bytes)],
                    ),
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
                .text(t("oneclient.settings.launcher.move.explanation"))
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
                        .text(t("oneclient.common.cancel")),
                )
                .child(
                    Button::new()
                        .primary()
                        .on_press(move |_| {
                            actions.relocate(start.clone());
                            pending.set(None);
                            let _ = RouterContext::get().replace(Route::Relocating {});
                        })
                        .text(t("oneclient.settings.launcher.move.confirm")),
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
