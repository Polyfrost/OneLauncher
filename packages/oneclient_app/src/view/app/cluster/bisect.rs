use freya::prelude::*;
use oneclient_core::{BisectExit, BisectStatus};

use crate::components::{Button, CARD_BG, Icon, IconType, OverlayPopup, progress_track};
use crate::hooks::{BisectAction, use_bisect, use_bisect_mutation, use_game_snapshot};
use crate::theme::colors;
use crate::ui::border_all_color;

use super::folder_list::dialog;

#[derive(PartialEq)]
pub struct BisectBanner {
    pub cluster_id: i64,
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
            .suspects()
            .map(|m| (m.hash.clone(), m.name.clone()))
            .collect();

        let (title, body) = banner_text(&status, game_active);
        let note = status.retry_note.clone().filter(|_| !game_active);

        let mut buttons: Vec<Element> = Vec::new();
        if status.finished && !culprits.is_empty() {
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
        if status.round > 0 {
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

        let cleared = total - suspects.min(total);
        let pct = if status.finished {
            100.
        } else {
            cleared as f32 / total as f32 * 100.
        };

        let accent = if status.finished {
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
                        Icon::new(if status.finished {
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
    let names: Vec<&str> = status.suspects().map(|m| m.name.as_str()).collect();

    if status.finished {
        return match names.as_slice() {
            [] => (
                "Nothing left to test".to_string(),
                "Every mod in the search was removed. Restore your mods to finish.".to_string(),
            ),
            [only] => (
                format!("Found it: {only}"),
                "This mod is the most likely cause. Right now only it and the mods it needs are on, so you can launch once more to double-check.".to_string(),
            ),
            many => (
                format!("Narrowed it down to {} mods", many.len()),
                format!(
                    "{} need each other, so they can't be tested separately.",
                    many.join(", ")
                ),
            ),
        };
    }

    let title = format!("Finding the problem mod · launch {}", status.round + 1);
    let left = status.launches_left();
    let plural = if left == 1 { "launch" } else { "launches" };
    let body = if game_active {
        "Play until the problem shows up, or until you're sure it won't. Close the game when you're done.".to_string()
    } else {
        format!(
            "{} of {} suspects are on. Launch the game and check if the problem happens, about {left} {plural} to go.",
            status.testing_count(),
            names.len(),
        )
    };
    (title, body)
}

#[derive(PartialEq)]
pub struct BisectQuestion {
    pub cluster_id: i64,
}

impl Component for BisectQuestion {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let status = use_bisect(cluster_id);
        let mutation = use_bisect_mutation();
        let game_active = use_game_snapshot().is_active(cluster_id);
        let mut picking = use_state(|| false);
        let mut picked = use_state(Vec::<String>::new);

        let Some(status) = status.filter(|s| s.awaiting_answer() && !game_active) else {
            return rect().into_element();
        };
        let busy = crate::hooks::mutation_is_running(&mutation);

        let crashed = status.pending_exit == BisectExit::Crashed;
        let choosing = *picking.read();
        let title = if choosing {
            "Which mod should stay on?"
        } else if crashed {
            "The game crashed. Is this the problem you're looking for?"
        } else {
            "Did the problem happen?"
        };
        let body = if choosing {
            "Pick the mods the crash says are missing. These are usually libraries, like a config or API mod. They will stay on whenever the mods being tested are on.".to_string()
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
                .into_element()
        };

        OverlayPopup::new()
            .on_close(move |_| {
                reset();
                mutation.mutate(BisectAction::Skip { cluster_id })
            })
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

#[derive(PartialEq)]
pub struct StartBisectButton {
    pub cluster_id: i64,
    pub enabled_mods: usize,
}

impl Component for StartBisectButton {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let enabled_mods = self.enabled_mods;
        let mutation = use_bisect_mutation();
        let game_active = use_game_snapshot().is_active(cluster_id);
        let mut confirm = use_state(|| false);
        let busy = crate::hooks::mutation_is_running(&mutation);

        let can_start = enabled_mods >= 2 && !game_active && !busy;
        let tooltip = if game_active {
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
                format!(
                    "OneClient will turn off about half of your {enabled_mods} enabled mods. Launch the game, check whether the problem still happens, and answer one question when you close it. After about {launches} launches it will point to the mod. When you finish or stop, your mods are put back exactly as they are now."
                ),
                None,
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

        rect()
            .child(
                Button::new()
                    .secondary()
                    .height(Size::px(34.))
                    .font_size(12.)
                    .enabled(can_start)
                    .tooltip(tooltip)
                    .on_press(move |_| confirm.set(true))
                    .child(Icon::new(IconType::SearchMd).size(15.))
                    .text("Find problem mod"),
            )
            .maybe_child(popup)
    }
}
