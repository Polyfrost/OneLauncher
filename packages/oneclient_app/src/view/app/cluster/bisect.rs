use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_common::domain::ContentType;
use oneclient_core::{BisectExit, BisectRole, BisectStatus};

use crate::components::{Button, Icon, IconType, OverlayPopup, ScrollArea, progress_track};
use freya::query::UseMutation;

use crate::Route;
use crate::hooks::{
    BisectAction, BisectMutation, cluster_content_items, invalidate_logs_queries, use_bisect,
    use_bisect_mutation, use_cluster_content, use_game_snapshot,
};
use crate::i18n::{t, tf};
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
                    .text(t("oneclient.cluster.bisect.banner.answer"))
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
                    .text(t("oneclient.cluster.bisect.banner.try_again"))
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
                    .tooltip(t("oneclient.cluster.bisect.banner.restart.tooltip"))
                    .on_press(move |_| mutation.mutate(BisectAction::Restart { cluster_id }))
                    .text(t("oneclient.cluster.bisect.banner.restart"))
                    .into_element(),
            );
        }
        if status.confirmed && !status.finishing && !culprits.is_empty() {
            let disable: Vec<String> = culprits.iter().map(|(hash, _)| hash.clone()).collect();
            let label = if culprits.len() == 1 {
                t("oneclient.cluster.bisect.banner.disable.one")
            } else {
                t("oneclient.cluster.bisect.banner.disable.other")
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
                    .tooltip(t("oneclient.cluster.bisect.banner.undo.tooltip"))
                    .on_press(move |_| mutation.mutate(BisectAction::Undo { cluster_id }))
                    .child(Icon::new(IconType::ClockRewind).size(14.))
                    .text(t("oneclient.cluster.bisect.banner.undo"))
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
                        t("oneclient.cluster.bisect.banner.restore_all")
                    } else {
                        t("oneclient.cluster.bisect.banner.stop")
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
            t("oneclient.cluster.bisect.banner.restoring.title").to_string(),
            t("oneclient.cluster.bisect.banner.restoring.body").to_string(),
        );
    }

    let names: Vec<&str> = status.suspects().map(|m| m.name.as_str()).collect();
    let culprits: Vec<&str> = status.culprits().iter().map(|m| m.name.as_str()).collect();
    let together = culprits.join(" + ");

    if status.finished {
        let playing = t("oneclient.cluster.bisect.banner.finished.playing");
        return match culprits.as_slice() {
            [] => (
                t("oneclient.cluster.bisect.banner.empty.title").to_string(),
                t("oneclient.cluster.bisect.banner.empty.body").to_string(),
            ),
            _ if status.confirmed => (
                tf("oneclient.cluster.bisect.banner.found.title", &[&together]),
                if culprits.len() == 1 {
                    t("oneclient.cluster.bisect.banner.found.body.one").to_string()
                } else {
                    t("oneclient.cluster.bisect.banner.found.body.other").to_string()
                },
            ),
            _ if game_active => (
                tf("oneclient.cluster.bisect.banner.checking", &[&together]),
                playing.to_string(),
            ),
            [only] => (
                tf("oneclient.cluster.bisect.banner.probably.title", &[only]),
                t("oneclient.cluster.bisect.banner.probably.body").to_string(),
            ),
            many => (
                tf(
                    "oneclient.cluster.bisect.banner.narrowed.title",
                    &[&many.len()],
                ),
                tf(
                    "oneclient.cluster.bisect.banner.narrowed.body",
                    &[&many.join(", ")],
                ),
            ),
        };
    }

    let title = tf(
        "oneclient.cluster.bisect.banner.running.title",
        &[&(status.round + 1)],
    );
    let left = status.launches_left();
    let body = if game_active {
        t("oneclient.cluster.bisect.banner.running.playing").to_string()
    } else {
        let partner = if culprits.is_empty() {
            String::new()
        } else {
            format!(
                "{} ",
                tf(
                    "oneclient.cluster.bisect.banner.running.partner",
                    &[&together]
                )
            )
        };
        let (testing, total) = (status.testing_count(), names.len());
        let progress = if left == 1 {
            tf(
                "oneclient.cluster.bisect.banner.running.body.one",
                &[&testing, &total, &left],
            )
        } else {
            tf(
                "oneclient.cluster.bisect.banner.running.body.other",
                &[&testing, &total, &left],
            )
        };
        format!("{partner}{progress}")
    };
    (title, body)
}

fn changed_text(status: &BisectStatus) -> String {
    let mut parts = Vec::new();
    if !status.removed.is_empty() {
        parts.push(tf(
            "oneclient.cluster.bisect.changed.removed",
            &[&status.removed.join(", ")],
        ));
    }
    if !status.added.is_empty() {
        parts.push(tf(
            "oneclient.cluster.bisect.changed.added",
            &[&status.added.join(", ")],
        ));
    }
    tf(
        "oneclient.cluster.bisect.changed.body",
        &[&parts.join("; ")],
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
            t("oneclient.cluster.bisect.question.pick.title")
        } else if suggestion.is_some() {
            t("oneclient.cluster.bisect.question.suggestion.title")
        } else if crashed {
            t("oneclient.cluster.bisect.question.crashed.title")
        } else {
            t("oneclient.cluster.bisect.question.title")
        };
        let body = if choosing {
            t("oneclient.cluster.bisect.question.pick.body").to_string()
        } else if let Some(suggestion) = &suggestion {
            let who = if suggestion.needed_by.is_empty() {
                t("oneclient.cluster.bisect.question.suggestion.someone").to_string()
            } else {
                suggestion.needed_by.join(", ")
            };
            tf(
                "oneclient.cluster.bisect.question.suggestion.body",
                &[&suggestion.missing.join(", "), &who],
            )
        } else {
            tf(
                "oneclient.cluster.bisect.question.body",
                &[&status.testing_count()],
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
                    t("oneclient.cluster.bisect.question.unsure.none")
                } else {
                    t("oneclient.cluster.bisect.question.unsure.tooltip")
                })
                .on_press(move |_| {
                    mutation.mutate(BisectAction::KeepOn {
                        cluster_id,
                        hashes: cleared.clone(),
                    });
                    reset();
                })
                .text(t("oneclient.cluster.bisect.question.unsure"));
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(12.)
                .child(
                    ScrollArea::new()
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
                                .text(t("oneclient.common.back")),
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
                                .text(tf(
                                    "oneclient.cluster.bisect.question.keep",
                                    &[&chosen.len()],
                                )),
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
                        .text(t("oneclient.cluster.bisect.question.suggestion.accept")),
                )
                .child(
                    Button::new()
                        .secondary()
                        .width(Size::fill())
                        .enabled(!busy)
                        .on_press(move |_| {
                            mutation.mutate(BisectAction::DeclineSuggestion { cluster_id })
                        })
                        .text(t("oneclient.cluster.bisect.question.suggestion.decline")),
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
                        .text(t("oneclient.cluster.bisect.question.broken")),
                )
                .child(
                    Button::new()
                        .primary()
                        .width(Size::fill())
                        .enabled(!busy)
                        .on_press(answer(false))
                        .text(t("oneclient.cluster.bisect.question.works")),
                )
                .child(
                    Button::new()
                        .secondary()
                        .width(Size::fill())
                        .enabled(!busy)
                        .on_press(move |_| picking.set(true))
                        .text(t("oneclient.cluster.bisect.question.missing")),
                )
                .child(
                    Button::new()
                        .ghost()
                        .width(Size::fill())
                        .enabled(!busy)
                        .on_press(move |_| mutation.mutate(BisectAction::Skip { cluster_id }))
                        .text(t("oneclient.cluster.bisect.question.skip")),
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
                            .background(colors::card())
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
        t("oneclient.cluster.bisect.question.crashed.title")
    } else {
        t("oneclient.cluster.bisect.question.title")
    };
    let joined = names.join(", ");
    let body = if names.len() == 1 {
        tf("oneclient.cluster.bisect.confirm.body.one", &[&joined])
    } else {
        tf("oneclient.cluster.bisect.confirm.body.other", &[&joined])
    };
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
                .text(t("oneclient.cluster.bisect.confirm.yes")),
        )
        .child(
            Button::new()
                .primary()
                .width(Size::fill())
                .enabled(!busy)
                .on_press(confirm(false))
                .text(t("oneclient.cluster.bisect.confirm.no")),
        )
        .child(
            Button::new()
                .ghost()
                .width(Size::fill())
                .enabled(!busy)
                .on_press(move |_| mutation.mutate(BisectAction::Skip { cluster_id }))
                .text(t("oneclient.cluster.bisect.question.skip")),
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
                        .background(colors::card())
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
        .text(t("oneclient.cluster.bisect.check_logs"))
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
            t("oneclient.cluster.bisect.row.tooltip.running")
        } else if game_active {
            t("oneclient.cluster.bisect.row.tooltip.game_active")
        } else if enabled_mods < 2 {
            t("oneclient.cluster.bisect.row.tooltip.too_few")
        } else {
            t("oneclient.cluster.bisect.row.tooltip.ready")
        };

        let launches = oneclient_core::bisect::launches_left(enabled_mods);
        let popup = confirm.read().then(|| {
            dialog(
                t("oneclient.cluster.bisect.start.title").to_string(),
                t("oneclient.cluster.bisect.start.body").to_string(),
                Some(steps_list([
                    t("oneclient.cluster.bisect.start.step1").to_string(),
                    t("oneclient.cluster.bisect.start.step2").to_string(),
                    t("oneclient.cluster.bisect.start.step3").to_string(),
                    tf("oneclient.cluster.bisect.start.step4", &[&launches]),
                ])),
                move || confirm.set(false),
                [
                    Button::new()
                        .secondary()
                        .on_press(move |_| confirm.set(false))
                        .text(t("oneclient.common.cancel"))
                        .into_element(),
                    Button::new()
                        .primary()
                        .on_press(move |_| {
                            confirm.set(false);
                            mutation.mutate(BisectAction::Start { cluster_id });
                        })
                        .child(Icon::new(IconType::SearchMd).size(14.))
                        .text(t("oneclient.cluster.bisect.start.confirm"))
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
                        t("oneclient.cluster.bisect.row.running")
                    } else {
                        t("oneclient.cluster.bisect.row.start")
                    }),
            )
            .maybe_child(popup);

        settings_row(
            IconType::SearchMd,
            t("oneclient.cluster.bisect.row.title"),
            t("oneclient.cluster.bisect.row.description"),
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
