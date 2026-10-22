use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_common::domain::ContentType;
use oneclient_core::{BisectExit, BisectRole, BisectStatus};

use crate::components::{Button, CARD_BG, Icon, IconType, OverlayPopup, progress_track};
use freya::query::UseMutation;

use crate::Route;
use crate::hooks::{
    BisectAction, BisectMutation, cluster_content_items, invalidate_logs_queries, use_bisect,
    use_bisect_mutation, use_cluster_content, use_game_snapshot,
};
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::view::app::settings::settings_row;

use super::OpenGameOutput;
use super::folder_list::dialog;

#[derive(PartialEq)]
pub struct BisectBanner {
    pub cluster_id: i64,
    pub question_hidden: State<bool>,
}

impl Component for BisectBanner {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let status = use_bisect(cluster_id);
        let mutation = use_bisect_mutation();
        let game_active = use_game_snapshot().is_active(cluster_id);

        let Some(status) = status else {
            return rect().into_element();
        };

        let busy = crate::hooks::mutation_is_running(&mutation);
        let idle = !busy && !game_active;
        let total = status.mods.len().max(1);
        let suspects = status.suspect_count();
        let culprits: Vec<(String, String)> = status
            .culprits()
            .into_iter()
            .map(|m| (m.hash.clone(), m.name.clone()))
            .collect();

        let (title, body) = banner_text(&status, game_active);
        let note = status.retry_note.clone().filter(|_| !game_active);
        let warning = (status.mods_changed() && !status.finishing).then(|| changed_text(&status));

        let mut buttons: Vec<Element> = Vec::new();
        let mut question_hidden = self.question_hidden;
        if *question_hidden.read() && status.awaiting_answer() && !game_active {
            buttons.push(
                Button::new()
                    .primary()
                    .height(Size::px(34.))
                    .font_size(12.)
                    .on_press(move |_| question_hidden.set(false))
                    .text("Answer")
                    .into_element(),
            );
        }
        if status.finishing {
            buttons.push(
                Button::new()
                    .primary()
                    .height(Size::px(34.))
                    .font_size(12.)
                    .enabled(idle)
                    .on_press(move |_| {
                        mutation.mutate(BisectAction::Finish {
                            cluster_id,
                            disable: Vec::new(),
                        })
                    })
                    .text("Try again")
                    .into_element(),
            );
        }
        if warning.is_some() {
            buttons.push(
                Button::new()
                    .primary()
                    .height(Size::px(34.))
                    .font_size(12.)
                    .enabled(idle)
                    .tooltip("Put your mods back and start over with the mods the cluster has now")
                    .on_press(move |_| mutation.mutate(BisectAction::Restart { cluster_id }))
                    .text("Restart search")
                    .into_element(),
            );
        }
        if status.confirmed && !status.finishing && !culprits.is_empty() {
            let disable: Vec<String> = culprits.iter().map(|(hash, _)| hash.clone()).collect();
            let label = if culprits.len() == 1 {
                "Disable it and restore the rest"
            } else {
                "Disable these and restore the rest"
            };
            buttons.push(
                Button::new()
                    .primary()
                    .height(Size::px(34.))
                    .font_size(12.)
                    .enabled(idle)
                    .on_press(move |_| {
                        mutation.mutate(BisectAction::Finish {
                            cluster_id,
                            disable: disable.clone(),
                        })
                    })
                    .text(label)
                    .into_element(),
            );
        }
        if (status.round > 0 || status.confirmed) && !status.finishing {
            buttons.push(
                Button::new()
                    .secondary()
                    .height(Size::px(34.))
                    .font_size(12.)
                    .enabled(idle)
                    .tooltip("Go back one launch, in case you answered wrong")
                    .on_press(move |_| mutation.mutate(BisectAction::Undo { cluster_id }))
                    .child(Icon::new(IconType::ClockRewind).size(14.))
                    .text("Undo last answer")
                    .into_element(),
            );
        }
        if !status.finishing {
            buttons.push(
                Button::new()
                    .secondary()
                    .height(Size::px(34.))
                    .font_size(12.)
                    .enabled(idle)
                    .on_press(move |_| {
                        mutation.mutate(BisectAction::Finish {
                            cluster_id,
                            disable: Vec::new(),
                        })
                    })
                    .text(if status.finished {
                        "Restore everything"
                    } else {
                        "Stop and restore mods"
                    })
                    .into_element(),
            );
        }

        let cleared = total - suspects.min(total);
        let pct = if status.finished {
            100.
        } else {
            cleared as f32 / total as f32 * 100.
        };

        let accent = if status.finishing || warning.is_some() {
            colors::danger()
        } else if status.confirmed {
            colors::success()
        } else {
            colors::brand()
        };

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(10.)
            .padding(Gaps::new_all(14.))
            .corner_radius(CornerRadius::new_all(10.))
            .background(colors::page_elevated())
            .border(border_all_color(1., accent.with_a(120)))
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .cross_align(Alignment::Center)
                    .spacing(12.)
                    .content(Content::Flex)
                    .child(
                        Icon::new(if status.finishing || warning.is_some() {
                            IconType::AlertTriangle
                        } else if status.confirmed {
                            IconType::CheckCircle
                        } else {
                            IconType::SearchMd
                        })
                        .size(20.)
                        .color(accent),
                    )
                    .child(
                        rect()
                            .vertical()
                            .width(Size::flex(1.0))
                            .spacing(3.)
                            .child(
                                label()
                                    .text(title)
                                    .font_size(14.)
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .color(colors::fg_primary()),
                            )
                            .child(
                                label()
                                    .text(body)
                                    .font_size(12.)
                                    .color(colors::fg_secondary()),
                            )
                            .maybe_child(note.map(|note| {
                                label()
                                    .text(note)
                                    .font_size(12.)
                                    .font_weight(FontWeight::MEDIUM)
                                    .color(colors::brand())
                                    .into_element()
                            }))
                            .maybe_child(warning.map(|warning| {
                                label()
                                    .text(warning)
                                    .font_size(12.)
                                    .font_weight(FontWeight::MEDIUM)
                                    .color(colors::danger())
                                    .into_element()
                            })),
                    )
                    .child(
                        rect()
                            .horizontal()
                            .cross_align(Alignment::Center)
                            .spacing(8.)
                            .children(buttons),
                    ),
            )
            .child(progress_track(pct, 4., accent, colors::component_bg()))
            .into_element()
    }
}

fn banner_text(status: &BisectStatus, game_active: bool) -> (String, String) {
    if status.finishing {
        return (
            "Restoring your mods".to_string(),
            "Some mods couldn't be switched back yet. Close anything using the mod files, then try again.".to_string(),
        );
    }

    let names: Vec<&str> = status.suspects().map(|m| m.name.as_str()).collect();
    let culprits: Vec<&str> = status.culprits().iter().map(|m| m.name.as_str()).collect();
    let together = culprits.join(" + ");

    if status.finished {
        let playing = "Play until you're sure whether the problem happens, then close the game.";
        return match culprits.as_slice() {
            [] => (
                "Nothing left to test".to_string(),
                "Every mod in the search was removed. Restore your mods to finish.".to_string(),
            ),
            _ if status.confirmed => (
                format!("Found it: {together}"),
                if culprits.len() == 1 {
                    "The problem happens with only this mod and the mods it needs on.".to_string()
                } else {
                    "The problem happens with only these mods and the mods they need on."
                        .to_string()
                },
            ),
            _ if game_active => (format!("Checking {together}"), playing.to_string()),
            [only] => (
                format!("Probably {only}"),
                "Launch once more to make sure. Only this mod and the mods it needs are on."
                    .to_string(),
            ),
            many => (
                format!("Narrowed it down to {} mods", many.len()),
                format!(
                    "{} are on together. Launch once more to make sure the problem happens with only them.",
                    many.join(", ")
                ),
            ),
        };
    }

    let title = format!("Running mod diagnostics · launch {}", status.round + 1);
    let left = status.launches_left();
    let plural = if left == 1 { "launch" } else { "launches" };
    let body = if game_active {
        "Play until the problem shows up, or until you're sure it won't. Close the game when you're done.".to_string()
    } else {
        let partner = if culprits.is_empty() {
            String::new()
        } else {
            format!("Looking for a mod that causes it together with {together}. ")
        };
        format!(
            "{partner}{} of {} suspects are on. Launch the game and check if the problem happens, about {left} {plural} to go.",
            status.testing_count(),
            names.len(),
        )
    };
    (title, body)
}

fn changed_text(status: &BisectStatus) -> String {
    let mut parts = Vec::new();
    if !status.removed.is_empty() {
        parts.push(format!("{} left the cluster", status.removed.join(", ")));
    }
    if !status.added.is_empty() {
        parts.push(format!(
            "{} joined and stays on for every launch",
            status.added.join(", ")
        ));
    }
    format!(
        "Mods changed since the search started: {}. Restart the search so the result stays right.",
        parts.join("; ")
    )
}

#[derive(PartialEq)]
pub struct BisectQuestion {
    pub cluster_id: i64,
    pub hidden: State<bool>,
}

impl Component for BisectQuestion {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let status = use_bisect(cluster_id);
        let mutation = use_bisect_mutation();
        let game_active = use_game_snapshot().is_active(cluster_id);
        let mut picking = use_state(|| false);
        let mut picked = use_state(Vec::<String>::new);

        let hidden = self.hidden;
        let Some(status) =
            status.filter(|s| s.awaiting_answer() && !game_active && !*hidden.read())
        else {
            return rect().into_element();
        };
        let busy = crate::hooks::mutation_is_running(&mutation);

        let crashed = status.pending_exit == BisectExit::Crashed;
        if status.awaiting_confirmation() {
            return confirm_question(&status, crashed, busy, mutation, hidden);
        }
        let choosing = *picking.read();
        let suggestion = status.suggestion.clone().filter(|_| !choosing);
        let title = if choosing {
            "Which mod should stay on?"
        } else if suggestion.is_some() {
            "Did a missing mod cause this crash?"
        } else if crashed {
            "The game crashed. Is this the problem you're looking for?"
        } else {
            "Did the problem happen?"
        };
        let body = if choosing {
            "Pick the mods the crash says are missing. These are usually libraries, like a config or API mod. They will stay on whenever the mods being tested are on.".to_string()
        } else if let Some(suggestion) = &suggestion {
            let who = if suggestion.needed_by.is_empty() {
                "a mod that was on".to_string()
            } else {
                suggestion.needed_by.join(", ")
            };
            format!(
                "{} was off for this launch, and the crash looks like {who} needs it. If you keep it on, this launch doesn't count and you try again.",
                suggestion.missing.join(", ")
            )
        } else {
            format!(
                "{} of your mods were on for this launch. Your answer decides which half to test next.",
                status.testing_count()
            )
        };

        let mut reset = move || {
            picking.set(false);
            picked.set(Vec::new());
        };

        let content = if choosing {
            let chosen = picked.read().clone();
            let mut off: Vec<(String, String)> = status
                .off_this_round()
                .map(|m| (m.hash.clone(), m.name.clone()))
                .collect();
            off.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));

            let rows = off.into_iter().map(|(hash, name)| {
                let selected = chosen.contains(&hash);
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .cross_align(Alignment::Center)
                    .spacing(10.)
                    .padding(Gaps::new_symmetric(8., 10.))
                    .corner_radius(CornerRadius::new_all(8.))
                    .background(if selected {
                        colors::brand().with_a(40)
                    } else {
                        colors::component_bg()
                    })
                    .cursor(CursorIcon::Pointer)
                    .on_press(move |_| {
                        let mut list = picked.write();
                        if let Some(at) = list.iter().position(|h| *h == hash) {
                            list.remove(at);
                        } else {
                            list.push(hash.clone());
                        }
                    })
                    .child(
                        Icon::new(if selected {
                            IconType::CheckCircle
                        } else {
                            IconType::Plus
                        })
                        .size(14.)
                        .color(if selected {
                            colors::brand()
                        } else {
                            colors::fg_secondary()
                        }),
                    )
                    .child(
                        label()
                            .text(name)
                            .font_size(12.)
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .color(colors::fg_primary()),
                    )
                    .into_element()
            });

            let keep = chosen.clone();
            let cleared: Vec<String> = status
                .mods
                .iter()
                .filter(|m| m.role == BisectRole::Cleared)
                .map(|m| m.hash.clone())
                .collect();
            let unsure = Button::new()
                .ghost()
                .width(Size::fill())
                .enabled(!busy && !cleared.is_empty())
                .tooltip(if cleared.is_empty() {
                    "Nothing has been cleared yet. Pick the mods, or skip this launch."
                } else {
                    "Turns on every mod already ruled out, so a missing library is likely back"
                })
                .on_press(move |_| {
                    mutation.mutate(BisectAction::KeepOn {
                        cluster_id,
                        hashes: cleared.clone(),
                    });
                    reset();
                })
                .text("I don't know, turn on everything already cleared");
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(12.)
                .child(
                    ScrollView::new()
                        .width(Size::fill())
                        .height(Size::px(240.))
                        .child(
                            rect()
                                .vertical()
                                .width(Size::fill())
                                .spacing(6.)
                                .children(rows),
                        ),
                )
                .child(unsure)
                .child(
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .main_align(Alignment::End)
                        .spacing(8.)
                        .child(
                            Button::new()
                                .secondary()
                                .enabled(!busy)
                                .on_press(move |_| reset())
                                .text("Back"),
                        )
                        .child(
                            Button::new()
                                .primary()
                                .enabled(!busy && !keep.is_empty())
                                .on_press(move |_| {
                                    mutation.mutate(BisectAction::KeepOn {
                                        cluster_id,
                                        hashes: keep.clone(),
                                    });
                                    reset();
                                })
                                .text(format!("Keep {} on", chosen.len())),
                        ),
                )
                .into_element()
        } else if suggestion.is_some() {
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(8.)
                .child(
                    Button::new()
                        .primary()
                        .width(Size::fill())
                        .enabled(!busy)
                        .on_press(move |_| {
                            mutation.mutate(BisectAction::AcceptSuggestion { cluster_id })
                        })
                        .text("Yes, keep it on and try again"),
                )
                .child(
                    Button::new()
                        .secondary()
                        .width(Size::fill())
                        .enabled(!busy)
                        .on_press(move |_| {
                            mutation.mutate(BisectAction::DeclineSuggestion { cluster_id })
                        })
                        .text("No, this is the problem I'm looking for"),
                )
                .child(check_logs_button(cluster_id, busy, hidden))
                .into_element()
        } else {
            let answer = move |broken: bool| {
                move |_: Event<PressEventData>| {
                    mutation.mutate(BisectAction::Answer { cluster_id, broken })
                }
            };
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(8.)
                .child(
                    Button::new()
                        .danger()
                        .width(Size::fill())
                        .enabled(!busy)
                        .on_press(answer(true))
                        .text("Yes, it's still broken"),
                )
                .child(
                    Button::new()
                        .primary()
                        .width(Size::fill())
                        .enabled(!busy)
                        .on_press(answer(false))
                        .text("No, everything works"),
                )
                .child(
                    Button::new()
                        .secondary()
                        .width(Size::fill())
                        .enabled(!busy)
                        .on_press(move |_| picking.set(true))
                        .text("It crashed, but differently (a mod is missing)"),
                )
                .child(
                    Button::new()
                        .ghost()
                        .width(Size::fill())
                        .enabled(!busy)
                        .on_press(move |_| mutation.mutate(BisectAction::Skip { cluster_id }))
                        .text("I didn't get to test, ask me next time"),
                )
                .child(check_logs_button(cluster_id, busy, hidden))
                .into_element()
        };

        // Only the answers close it a stray click or Escape would skip the
        // launch without the user meaning to
        OverlayPopup::new()
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(440.))
                            .max_width(Size::window_percent(90.))
                            .spacing(14.)
                            .padding(Gaps::new_all(20.))
                            .corner_radius(CornerRadius::new_all(14.))
                            .background(CARD_BG)
                            .border(border_all_color(1., colors::component_border()))
                            .child(
                                rect()
                                    .horizontal()
                                    .cross_align(Alignment::Center)
                                    .spacing(10.)
                                    .maybe_child((crashed && !choosing).then(|| {
                                        Icon::new(IconType::AlertTriangle)
                                            .size(18.)
                                            .color(colors::danger())
                                            .into_element()
                                    }))
                                    .child(
                                        label()
                                            .text(title)
                                            .font_size(16.)
                                            .font_weight(FontWeight::SEMI_BOLD)
                                            .color(colors::fg_primary()),
                                    ),
                            )
                            .child(
                                label()
                                    .text(body)
                                    .font_size(12.)
                                    .color(colors::fg_secondary()),
                            )
                            .child(content),
                    ),
            )
            .into_element()
    }
}

fn confirm_question(
    status: &BisectStatus,
    crashed: bool,
    busy: bool,
    mutation: UseMutation<BisectMutation>,
    hidden: State<bool>,
) -> Element {
    let cluster_id = status.cluster_id;
    let names: Vec<&str> = status.culprits().iter().map(|m| m.name.as_str()).collect();
    let title = if crashed {
        "The game crashed. Is this the problem you're looking for?"
    } else {
        "Did the problem happen?"
    };
    let body = format!(
        "Only {} and the mods {} were on for this launch.",
        names.join(", "),
        if names.len() == 1 {
            "it needs"
        } else {
            "they need"
        },
    );
    let confirm = move |reproduced: bool| {
        move |_: Event<PressEventData>| {
            mutation.mutate(BisectAction::Confirm {
                cluster_id,
                reproduced,
            })
        }
    };

    let content = rect()
        .vertical()
        .width(Size::fill())
        .spacing(8.)
        .child(
            Button::new()
                .danger()
                .width(Size::fill())
                .enabled(!busy)
                .on_press(confirm(true))
                .text("Yes, it still happened"),
        )
        .child(
            Button::new()
                .primary()
                .width(Size::fill())
                .enabled(!busy)
                .on_press(confirm(false))
                .text("No, it didn't happen"),
        )
        .child(
            Button::new()
                .ghost()
                .width(Size::fill())
                .enabled(!busy)
                .on_press(move |_| mutation.mutate(BisectAction::Skip { cluster_id }))
                .text("I didn't get to test, ask me next time"),
        )
        .child(check_logs_button(cluster_id, busy, hidden));

    OverlayPopup::new()
        .child(
            rect()
                .width(Size::window_percent(100.))
                .height(Size::window_percent(100.))
                .center()
                .child(
                    rect()
                        .vertical()
                        .width(Size::px(440.))
                        .max_width(Size::window_percent(90.))
                        .spacing(14.)
                        .padding(Gaps::new_all(20.))
                        .corner_radius(CornerRadius::new_all(14.))
                        .background(CARD_BG)
                        .border(border_all_color(1., colors::component_border()))
                        .child(
                            rect()
                                .horizontal()
                                .cross_align(Alignment::Center)
                                .spacing(10.)
                                .maybe_child(crashed.then(|| {
                                    Icon::new(IconType::AlertTriangle)
                                        .size(18.)
                                        .color(colors::danger())
                                        .into_element()
                                }))
                                .child(
                                    label()
                                        .text(title)
                                        .font_size(16.)
                                        .font_weight(FontWeight::SEMI_BOLD)
                                        .color(colors::fg_primary()),
                                ),
                        )
                        .child(
                            label()
                                .text(body)
                                .font_size(12.)
                                .color(colors::fg_secondary()),
                        )
                        .child(content),
                ),
        )
        .into_element()
}

fn check_logs_button(cluster_id: i64, busy: bool, mut hidden: State<bool>) -> Element {
    Button::new()
        .ghost()
        .width(Size::fill())
        .enabled(!busy)
        .on_press(move |_| {
            hidden.set(true);
            if let Some(OpenGameOutput(mut wanted)) = try_consume_root_context::<OpenGameOutput>() {
                wanted.set(true);
            }
            spawn(async { invalidate_logs_queries().await });
            let _ = RouterContext::get().push(Route::ClusterLogs { cluster_id });
        })
        .child(Icon::new(IconType::Terminal).size(14.))
        .text("Check logs")
        .into_element()
}

#[derive(PartialEq)]
pub struct FindProblemModRow {
    pub cluster_id: i64,
}

impl Component for FindProblemModRow {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let content = use_cluster_content(cluster_id, ContentType::Mod);
        let running = use_bisect(cluster_id).is_some();
        let mutation = use_bisect_mutation();
        let game_active = use_game_snapshot().is_active(cluster_id);
        let mut confirm = use_state(|| false);
        let busy = crate::hooks::mutation_is_running(&mutation);

        // Counted the way the search picks its mods so the numbers agree
        let enabled_mods = cluster_content_items(&content)
            .iter()
            .filter(|info| info.enabled)
            .map(|info| info.hash.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len();

        let can_start = enabled_mods >= 2 && !game_active && !busy && !running;
        let tooltip = if running {
            "A search is already running, use the banner above to stop it"
        } else if game_active {
            "Close the game first"
        } else if enabled_mods < 2 {
            "Needs at least two enabled mods"
        } else {
            "Turn mods off in halves to find the one causing a crash or bug"
        };

        let launches = oneclient_core::bisect::launches_left(enabled_mods);
        let popup = confirm.read().then(|| {
            dialog(
                "Find the mod causing a problem?".to_string(),
                "Your mods will be put back the way they are now when you finish or stop. If the problem only happens sometimes, play long enough to be sure before answering."
                    .to_string(),
                Some(steps_list([
                    "We turn off about half of your mods.".to_string(),
                    "Launch the game and check if the problem happens.".to_string(),
                    "Close the game and tell us if it did.".to_string(),
                    format!("Repeat about {launches} times until we find the mod."),
                ])),
                move || confirm.set(false),
                [
                    Button::new()
                        .secondary()
                        .on_press(move |_| confirm.set(false))
                        .text("Cancel")
                        .into_element(),
                    Button::new()
                        .primary()
                        .on_press(move |_| {
                            confirm.set(false);
                            mutation.mutate(BisectAction::Start { cluster_id });
                        })
                        .child(Icon::new(IconType::SearchMd).size(14.))
                        .text("Start")
                        .into_element(),
                ],
            )
        });

        let button = rect()
            .child(
                Button::new()
                    .small()
                    .secondary()
                    .enabled(can_start)
                    .tooltip(tooltip)
                    .on_press(move |_| confirm.set(true))
                    .text(if running {
                        "Running..."
                    } else {
                        "Start the process"
                    }),
            )
            .maybe_child(popup);

        settings_row(
            IconType::SearchMd,
            "Mod Diagnostics",
            "Turn mods off in halves across a few launches to find the one \
             causing a crash or bug. Your mods are put back when it ends.",
            button,
        )
    }
}

fn steps_list<const N: usize>(steps: [String; N]) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(6.)
        .children(steps.into_iter().enumerate().map(|(index, step)| {
            rect()
                .horizontal()
                .width(Size::fill())
                .spacing(8.)
                .child(
                    label()
                        .text(format!("{}.", index + 1))
                        .width(Size::px(14.))
                        .font_size(12.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(step)
                        .width(Size::flex(1.))
                        .font_size(12.)
                        .color(colors::fg_primary()),
                )
                .into_element()
        }))
        .into_element()
}
