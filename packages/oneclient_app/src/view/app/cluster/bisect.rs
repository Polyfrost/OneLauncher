use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_common::domain::ContentType;
use oneclient_core::{
    BisectAnswer, BisectChoice, BisectExit, BisectPlanKind, BisectRole, BisectStage, BisectStatus,
    ConflictReport,
};

use crate::components::{Button, Icon, IconType, OverlayPopup, ScrollArea, progress_track};
use freya::query::UseMutation;

use crate::Route;
use crate::hooks::{
    BisectAction, BisectMutation, bisect_report, cluster_content_items, invalidate_logs_queries,
    use_bisect, use_bisect_mutation, use_cluster_content, use_dispatch, use_game_snapshot,
};
use crate::i18n::{t, tf};
use crate::theme::{colors, rounded, text_size};
use crate::ui::border_all_color;
use crate::view::app::settings::settings_row;

use super::OpenGameOutput;
use super::folder_list::dialog;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Panel {
    None,
    Manage,
    History,
    Results,
}

#[derive(PartialEq)]
pub struct BisectBanner {
    pub cluster_id: i64,
    pub question_hidden: State<bool>,
}

fn banner_button(text: impl Into<String>, primary: bool) -> Button {
    let button = Button::new()
        .height(Size::px(34.))
        .font_size(text_size(12.))
        .text(text.into());
    if primary { button.primary() } else { button.secondary() }
}

fn link_button(text: impl Into<String>, icon: IconType) -> Button {
    Button::new()
        .ghost()
        .small()
        .child(Icon::new(icon).size(12.))
        .text(text.into())
}

fn note(text: String, color: Color) -> Element {
    label()
        .text(text)
        .font_size(text_size(12.))
        .font_weight(FontWeight::MEDIUM)
        .color(color)
        .into_element()
}

impl Component for BisectBanner {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let status = use_bisect(cluster_id);
        let mutation = use_bisect_mutation();
        let game_active = use_game_snapshot().is_active(cluster_id);
        let dispatch = use_dispatch();
        let mut panel = use_state(|| Panel::None);

        let Some(status) = status else {
            return rect().into_element();
        };

        let busy = crate::hooks::mutation_is_running(&mutation);
        let idle = !busy && !game_active;
        let (title, body) = banner_text(&status, game_active);

        let mut notes: Vec<Element> = Vec::new();
        if !game_active {
            if let Some(retry) = status.retry_note.clone() {
                notes.push(note(retry, colors::brand()));
            }
            if let Some(problem) = status.resolution_problem.clone() {
                notes.push(note(
                    tf("oneclient.cluster.bisect.banner.resolution", &[&problem]),
                    colors::danger(),
                ));
            }
            if !status.unresolvable.is_empty() {
                notes.push(note(
                    tf(
                        "oneclient.cluster.bisect.banner.unresolvable",
                        &[&status.unresolvable.len()],
                    ),
                    colors::danger(),
                ));
            }
            if !status.pending.is_empty() {
                notes.push(note(
                    tf(
                        "oneclient.cluster.bisect.banner.pending",
                        &[&status.pending.join(", ")],
                    ),
                    colors::fg_secondary(),
                ));
            }
            if !status.removed.is_empty() {
                notes.push(note(
                    tf(
                        "oneclient.cluster.bisect.banner.removed",
                        &[&status.removed.join(", ")],
                    ),
                    colors::fg_secondary(),
                ));
            }
        }

        let mut question_hidden = self.question_hidden;
        let mut buttons: Vec<Element> = Vec::new();
        if status.finishing {
            buttons.push(
                banner_button(t("oneclient.cluster.bisect.banner.try_again"), true)
                    .enabled(idle)
                    .on_press(move |_| {
                        mutation.mutate(BisectAction::Finish {
                            cluster_id,
                            disable: Vec::new(),
                        })
                    })
                    .into_element(),
            );
        } else {
            if *question_hidden.read() && status.awaiting_answer() && !game_active {
                buttons.push(
                    banner_button(t("oneclient.cluster.bisect.banner.answer"), true)
                        .on_press(move |_| question_hidden.set(false))
                        .into_element(),
                );
            }
            match status.stage {
                BisectStage::Complete => {
                    let culprits: Vec<String> =
                        status.all_found().map(|m| m.hash.clone()).collect();
                    if !culprits.is_empty() {
                        let label = if culprits.len() == 1 {
                            t("oneclient.cluster.bisect.banner.disable.one")
                        } else {
                            t("oneclient.cluster.bisect.banner.disable.other")
                        };
                        buttons.push(
                            banner_button(label, true)
                                .enabled(idle)
                                .on_press(move |_| {
                                    mutation.mutate(BisectAction::Finish {
                                        cluster_id,
                                        disable: culprits.clone(),
                                    })
                                })
                                .into_element(),
                        );
                    }
                    if status.can_continue {
                        buttons.push(
                            banner_button(t("oneclient.cluster.bisect.banner.continue"), false)
                                .enabled(idle)
                                .tooltip(t("oneclient.cluster.bisect.banner.continue.tooltip"))
                                .on_press(move |_| {
                                    mutation.mutate(BisectAction::Continue { cluster_id })
                                })
                                .into_element(),
                        );
                    }
                }
                BisectStage::Halted => {
                    if status.can_look_for_hidden {
                        buttons.push(
                            banner_button(t("oneclient.cluster.bisect.halted.look"), true)
                                .enabled(idle)
                                .tooltip(t("oneclient.cluster.bisect.halted.look.tooltip"))
                                .on_press(move |_| {
                                    mutation.mutate(BisectAction::FindHidden { cluster_id })
                                })
                                .into_element(),
                        );
                    }
                    buttons.push(
                        banner_button(t("oneclient.cluster.bisect.banner.reset"), false)
                            .enabled(idle)
                            .tooltip(t("oneclient.cluster.bisect.banner.reset.tooltip"))
                            .on_press(move |_| mutation.mutate(BisectAction::Reset { cluster_id }))
                            .into_element(),
                    );
                }
                BisectStage::Testing | BisectStage::Verifying => {}
            }
            if status.can_undo {
                buttons.push(
                    banner_button(t("oneclient.cluster.bisect.banner.undo"), false)
                        .enabled(idle)
                        .tooltip(t("oneclient.cluster.bisect.banner.undo.tooltip"))
                        .on_press(move |_| mutation.mutate(BisectAction::Undo { cluster_id }))
                        .child(Icon::new(IconType::ClockRewind).size(14.))
                        .into_element(),
                );
            }
            buttons.push(
                banner_button(
                    if status.finished() {
                        t("oneclient.cluster.bisect.banner.restore_all")
                    } else {
                        t("oneclient.cluster.bisect.banner.stop")
                    },
                    false,
                )
                .enabled(idle)
                .on_press(move |_| {
                    mutation.mutate(BisectAction::Finish {
                        cluster_id,
                        disable: Vec::new(),
                    })
                })
                .into_element(),
            );
        }

        let links = (!status.finishing).then(|| {
            rect()
                .horizontal()
                .spacing(4.)
                .child(
                    link_button(t("oneclient.cluster.bisect.manage.open"), IconType::Settings01)
                        .enabled(idle)
                        .on_press(move |_| panel.set(Panel::Manage)),
                )
                .child(
                    link_button(t("oneclient.cluster.bisect.results.open"), IconType::CheckCircle)
                        .on_press(move |_| panel.set(Panel::Results)),
                )
                .child(
                    link_button(t("oneclient.cluster.bisect.history.open"), IconType::ClockRewind)
                        .on_press(move |_| panel.set(Panel::History)),
                )
                .child(
                    link_button(t("oneclient.cluster.bisect.report.copy"), IconType::Copy01).on_press(
                        move |_| {
                            let dispatch = dispatch.clone();
                            spawn(async move {
                                let copied = match bisect_report(cluster_id).await {
                                    Ok(report) => freya::text_edit::Clipboard::set(report)
                                        .map_err(|err| format!("{err:?}")),
                                    Err(err) => Err(err),
                                };
                                match copied {
                                    Ok(()) => dispatch
                                        .notify(t("oneclient.cluster.bisect.report.copied"))
                                        .body(t("oneclient.cluster.bisect.report.copied.body"))
                                        .toast_only()
                                        .send(),
                                    Err(err) => dispatch
                                        .notify(t("oneclient.components.button.copy_failed.title"))
                                        .body(err)
                                        .error()
                                        .toast_only()
                                        .send(),
                                }
                            });
                        },
                    ),
                )
                .into_element()
        });

        let halted = (status.stage == BisectStage::Halted && !game_active)
            .then(|| halted_details(&status, idle, mutation));

        let pct = if status.finished() {
            100.
        } else {
            let estimate = status.estimate.max(1) as f32;
            (status.tests_this_round as f32 / estimate * 100.).min(95.)
        };

        let accent = if status.finishing || status.stage == BisectStage::Halted {
            colors::danger()
        } else if status.finished() {
            colors::success()
        } else {
            colors::brand()
        };

        let popup = match *panel.read() {
            Panel::None => None,
            Panel::Manage => Some(manage_panel(&status, busy || game_active, mutation, panel)),
            Panel::History => Some(history_panel(&status, panel)),
            Panel::Results => Some(results_panel(&status, panel)),
        };

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(10.)
            .padding(Gaps::new_all(14.))
            .corner_radius(rounded(10.))
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
                        Icon::new(if status.finishing || status.stage == BisectStage::Halted {
                            IconType::AlertTriangle
                        } else if status.finished() {
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
                                    .font_size(text_size(14.))
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .color(colors::fg_primary()),
                            )
                            .child(
                                label()
                                    .text(body)
                                    .font_size(text_size(12.))
                                    .color(colors::fg_secondary()),
                            )
                            .children(notes),
                    )
                    .child(
                        rect()
                            .horizontal()
                            .cross_align(Alignment::Center)
                            .spacing(8.)
                            .children(buttons),
                    ),
            )
            .maybe_child(halted)
            .child(progress_track(pct, 4., accent, colors::component_bg()))
            .maybe_child(links)
            .maybe_child(popup)
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

    let culprits: Vec<&str> = status.culprits().iter().map(|m| m.name.as_str()).collect();
    let together = culprits.join(" + ");
    let round = if status.round > 1 {
        format!(
            "{} · ",
            tf("oneclient.cluster.bisect.banner.round", &[&status.round])
        )
    } else {
        String::new()
    };

    match status.stage {
        BisectStage::Complete => {
            if culprits.is_empty() {
                (
                    t("oneclient.cluster.bisect.banner.none.title").to_string(),
                    t("oneclient.cluster.bisect.banner.none.body").to_string(),
                )
            } else {
                (
                    tf("oneclient.cluster.bisect.banner.found.title", &[&together]),
                    if culprits.len() == 1 {
                        t("oneclient.cluster.bisect.banner.found.body.one").to_string()
                    } else {
                        t("oneclient.cluster.bisect.banner.found.body.other").to_string()
                    },
                )
            }
        }
        BisectStage::Halted => (
            t("oneclient.cluster.bisect.halted.title").to_string(),
            match &status.groups {
                Some((a, b)) if b.is_empty() && !a.is_empty() => {
                    tf("oneclient.cluster.bisect.halted.single.body", &[&a.join(", ")])
                }
                _ => t("oneclient.cluster.bisect.halted.body").to_string(),
            },
        ),
        BisectStage::Verifying => (
            format!(
                "{round}{}",
                tf("oneclient.cluster.bisect.banner.verifying.title", &[&together])
            ),
            if game_active {
                t("oneclient.cluster.bisect.banner.running.playing").to_string()
            } else {
                t("oneclient.cluster.bisect.banner.verifying.body").to_string()
            },
        ),
        BisectStage::Testing => {
            let title = format!(
                "{round}{}",
                tf(
                    "oneclient.cluster.bisect.banner.running.title",
                    &[&(status.tests_this_round + 1), &status.estimate.max(1)],
                )
            );
            let body = if game_active {
                t("oneclient.cluster.bisect.banner.running.playing").to_string()
            } else {
                let found = if culprits.is_empty() {
                    String::new()
                } else {
                    format!(
                        "{} ",
                        tf("oneclient.cluster.bisect.banner.running.partner", &[&together])
                    )
                };
                let part = |role, one, other| {
                    let n = status.count(role);
                    tf(if n == 1 { one } else { other }, &[&n])
                };
                let testing = part(
                    BisectRole::Testing,
                    "oneclient.cluster.bisect.banner.running.testing.one",
                    "oneclient.cluster.bisect.banner.running.testing.other",
                );
                let cleared = part(
                    BisectRole::Cleared,
                    "oneclient.cluster.bisect.banner.running.cleared.one",
                    "oneclient.cluster.bisect.banner.running.cleared.other",
                );
                let waiting = part(
                    BisectRole::Waiting,
                    "oneclient.cluster.bisect.banner.running.off.one",
                    "oneclient.cluster.bisect.banner.running.off.other",
                );
                format!(
                    "{found}{}",
                    tf(
                        "oneclient.cluster.bisect.banner.running.body",
                        &[&testing, &cleared, &waiting],
                    )
                )
            };
            (title, body)
        }
    }
}

fn names_box(title: String, names: &[String]) -> Element {
    rect()
        .vertical()
        .width(Size::flex(1.))
        .spacing(4.)
        .padding(Gaps::new_all(10.))
        .corner_radius(rounded(8.))
        .background(colors::component_bg())
        .child(
            label()
                .text(title)
                .font_size(text_size(12.))
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text(names.join(", "))
                .font_size(text_size(11.))
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn halted_details(status: &BisectStatus, idle: bool, mutation: UseMutation<BisectMutation>) -> Element {
    let cluster_id = status.cluster_id;
    let mut section = rect().vertical().width(Size::fill()).spacing(8.);
    if let Some((a, b)) = status.groups.as_ref().filter(|(_, b)| !b.is_empty()) {
        section = section.child(
            rect()
                .horizontal()
                .width(Size::fill())
                .spacing(8.)
                .content(Content::Flex)
                .child(names_box(t("oneclient.cluster.bisect.halted.group_a").to_string(), a))
                .child(names_box(t("oneclient.cluster.bisect.halted.group_b").to_string(), b)),
        );
    }
    if !status.hidden.is_empty() {
        let lines: Vec<Element> = status
            .hidden
            .iter()
            .map(|dep| {
                label()
                    .text(tf(
                        "oneclient.cluster.bisect.halted.hidden.line",
                        &[&dep.from, &dep.to, &dep.classes],
                    ))
                    .font_size(text_size(11.))
                    .color(colors::fg_secondary())
                    .into_element()
            })
            .collect();
        section = section
            .child(
                label()
                    .text(t("oneclient.cluster.bisect.halted.hidden.title"))
                    .font_size(text_size(12.))
                    .font_weight(FontWeight::SEMI_BOLD)
                    .color(colors::fg_primary()),
            )
            .child(
                ScrollArea::new()
                    .width(Size::fill())
                    .height(Size::px(110.))
                    .child(rect().vertical().width(Size::fill()).spacing(2.).children(lines)),
            )
            .child(
                rect()
                    .horizontal()
                    .spacing(8.)
                    .child(
                        Button::new()
                            .primary()
                            .small()
                            .enabled(idle)
                            .on_press(move |_| {
                                mutation.mutate(BisectAction::UseHidden {
                                    cluster_id,
                                    accept: true,
                                })
                            })
                            .text(t("oneclient.cluster.bisect.halted.hidden.apply")),
                    )
                    .child(
                        Button::new()
                            .secondary()
                            .small()
                            .enabled(idle)
                            .on_press(move |_| {
                                mutation.mutate(BisectAction::UseHidden {
                                    cluster_id,
                                    accept: false,
                                })
                            })
                            .text(t("oneclient.cluster.bisect.halted.hidden.dismiss")),
                    ),
            );
    }
    section.into_element()
}

fn panel_frame(
    title: String,
    body: Option<String>,
    top: Option<Element>,
    content: ScrollArea,
    footer: Vec<Element>,
    mut panel: State<Panel>,
) -> Element {
    OverlayPopup::new()
        .on_close(move |_| panel.set(Panel::None))
        .child(
            rect()
                .width(Size::window_percent(100.))
                .height(Size::window_percent(100.))
                .center()
                .child(
                    rect()
                        .vertical()
                        .width(Size::px(560.))
                        .max_width(Size::window_percent(92.))
                        .spacing(12.)
                        .padding(Gaps::new_all(20.))
                        .corner_radius(rounded(14.))
                        .background(colors::card())
                        .border(border_all_color(1., colors::component_border()))
                        .child(
                            label()
                                .text(title)
                                .font_size(text_size(16.))
                                .font_weight(FontWeight::SEMI_BOLD)
                                .color(colors::fg_primary()),
                        )
                        .maybe_child(body.map(|body| {
                            label()
                                .text(body)
                                .font_size(text_size(12.))
                                .color(colors::fg_secondary())
                                .into_element()
                        }))
                        .maybe_child(top)
                        .child(content.width(Size::fill()).height(Size::window_percent(55.)))
                        .child(
                            rect()
                                .horizontal()
                                .width(Size::fill())
                                .main_align(Alignment::End)
                                .spacing(8.)
                                .children(footer)
                                .child(
                                    Button::new()
                                        .secondary()
                                        .on_press(move |_| panel.set(Panel::None))
                                        .text(t("oneclient.common.close")),
                                ),
                        ),
                ),
        )
        .into_element()
}

fn choice_label(choice: BisectChoice) -> String {
    match choice {
        BisectChoice::Search => t("oneclient.cluster.bisect.manage.search"),
        BisectChoice::Required => t("oneclient.cluster.bisect.manage.required"),
        BisectChoice::KeptOff => t("oneclient.cluster.bisect.manage.kept_off"),
        BisectChoice::Omitted => t("oneclient.cluster.bisect.manage.omitted"),
    }
    .to_string()
}

/// Fits the name line, the role line and the choice buttons
const MANAGE_ROW_H: f32 = 44.;

fn manage_panel(
    status: &BisectStatus,
    busy: bool,
    mutation: UseMutation<BisectMutation>,
    panel: State<Panel>,
) -> Element {
    let cluster_id = status.cluster_id;
    let mut rows: Vec<Element> = Vec::new();

    for broken in &status.unresolvable {
        let (disable_hash, ignore_hash) = (broken.hash.clone(), broken.hash.clone());
        let blocks = if broken.blocks.is_empty() {
            String::new()
        } else {
            format!(
                " {}",
                tf(
                    "oneclient.cluster.bisect.manage.unresolvable.blocks",
                    &[&broken.blocks.join(", ")]
                )
            )
        };
        rows.push(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(6.)
                .padding(Gaps::new_all(10.))
                .corner_radius(rounded(8.))
                .background(colors::danger().with_a(24))
                .child(
                    label()
                        .text(tf(
                            "oneclient.cluster.bisect.manage.unresolvable.line",
                            &[&broken.name, &broken.missing.join(", ")],
                        ) + &blocks)
                        .font_size(text_size(12.))
                        .color(colors::fg_primary()),
                )
                .child(
                    rect()
                        .horizontal()
                        .spacing(6.)
                        .child(
                            Button::new()
                                .secondary()
                                .small()
                                .enabled(!busy)
                                .tooltip(t("oneclient.cluster.bisect.manage.unresolvable.disable.tooltip"))
                                .on_press(move |_| {
                                    mutation.mutate(BisectAction::Settle {
                                        cluster_id,
                                        hash: disable_hash.clone(),
                                        ignore: false,
                                    })
                                })
                                .text(t("oneclient.cluster.bisect.manage.unresolvable.disable")),
                        )
                        .child(
                            Button::new()
                                .ghost()
                                .small()
                                .enabled(!busy)
                                .tooltip(t("oneclient.cluster.bisect.manage.unresolvable.ignore.tooltip"))
                                .on_press(move |_| {
                                    mutation.mutate(BisectAction::Settle {
                                        cluster_id,
                                        hash: ignore_hash.clone(),
                                        ignore: true,
                                    })
                                })
                                .text(t("oneclient.cluster.bisect.manage.unresolvable.ignore")),
                        ),
                )
                .into_element(),
        );
    }

    // Packs can hold hundreds of mods, so only the rows in view are built
    let mods = std::rc::Rc::new(status.mods.clone());
    let count = mods.len();
    let render = move |i: usize| {
        let m = &mods[i];
        let locked = matches!(m.role, BisectRole::Problem | BisectRole::Found);
        let choices = [
            BisectChoice::Search,
            BisectChoice::Required,
            BisectChoice::Omitted,
            BisectChoice::KeptOff,
        ]
        .into_iter()
        .map(|choice| {
            let hash = m.hash.clone();
            let selected = m.choice == choice;
            let button = Button::new()
                .small()
                .enabled(!busy && !locked && !selected)
                .on_press(move |_| {
                    mutation.mutate(BisectAction::Choose {
                        cluster_id,
                        hashes: vec![hash.clone()],
                        choice,
                    })
                })
                .text(choice_label(choice));
            if selected { button.primary() } else { button.ghost() }.into_element()
        });
        rect()
                .horizontal()
                .width(Size::fill())
                .height(Size::fill())
                .cross_align(Alignment::Center)
                .spacing(8.)
                .padding(Gaps::new_symmetric(6., 10.))
                .corner_radius(rounded(8.))
                .background(colors::component_bg())
                .content(Content::Flex)
                .child(
                    rect()
                        .vertical()
                        .width(Size::flex(1.))
                        .child(
                            label()
                                .text(m.name.clone())
                                .font_size(text_size(12.))
                                .max_lines(1)
                                .text_overflow(TextOverflow::Ellipsis)
                                .color(colors::fg_primary()),
                        )
                        .child(
                            label()
                                .text(m.role.label())
                                .font_size(text_size(10.))
                                .color(colors::fg_secondary()),
                        ),
                )
                .child(rect().horizontal().spacing(2.).children(choices))
                .into_element()
    };

    // The few mods that can't load wrap to any height, so they sit above the list
    let top = (!rows.is_empty()).then(|| {
        ScrollArea::new()
            .width(Size::fill())
            .max_height(Size::px(160.))
            .child(rect().vertical().width(Size::fill()).spacing(6.).children(rows))
            .into_element()
    });

    panel_frame(
        t("oneclient.cluster.bisect.manage.title").to_string(),
        Some(t("oneclient.cluster.bisect.manage.body").to_string()),
        top,
        ScrollArea::new().lazy(count, MANAGE_ROW_H, 6., render),
        Vec::new(),
        panel,
    )
}

fn history_panel(status: &BisectStatus, panel: State<Panel>) -> Element {
    let rows: Vec<Element> = status
        .history
        .iter()
        .enumerate()
        .rev()
        .map(|(index, entry)| {
            let (answer, color) = match entry.answer {
                BisectAnswer::Broken => (t("oneclient.cluster.bisect.history.broken"), colors::danger()),
                BisectAnswer::Works => (t("oneclient.cluster.bisect.history.works"), colors::success()),
                BisectAnswer::Unsure => (t("oneclient.cluster.bisect.history.unsure"), colors::fg_secondary()),
            };
            let kind = match entry.kind {
                BisectPlanKind::Verification => t("oneclient.cluster.bisect.history.verification"),
                BisectPlanKind::Complement => t("oneclient.cluster.bisect.history.complement"),
                BisectPlanKind::NewBisection | BisectPlanKind::Continuation => {
                    t("oneclient.cluster.bisect.history.half")
                }
            };
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(4.)
                .padding(Gaps::new_all(10.))
                .corner_radius(rounded(8.))
                .background(colors::component_bg())
                .child(
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .spacing(8.)
                        .child(
                            label()
                                .text(tf(
                                    "oneclient.cluster.bisect.history.entry",
                                    &[&(index + 1), &entry.round, &entry.iteration, &kind],
                                ))
                                .font_size(text_size(12.))
                                .font_weight(FontWeight::SEMI_BOLD)
                                .color(colors::fg_primary()),
                        )
                        .child(
                            label()
                                .text(answer)
                                .font_size(text_size(12.))
                                .font_weight(FontWeight::SEMI_BOLD)
                                .color(color),
                        ),
                )
                .child(
                    label()
                        .text(tf(
                            "oneclient.cluster.bisect.history.tested",
                            &[&entry.enabled, &entry.tested.join(", ")],
                        ))
                        .font_size(text_size(11.))
                        .color(colors::fg_secondary()),
                )
                .into_element()
        })
        .collect();

    let content = if rows.is_empty() {
        label()
            .text(t("oneclient.cluster.bisect.history.empty"))
            .font_size(text_size(12.))
            .color(colors::fg_secondary())
            .into_element()
    } else {
        rect()
            .vertical()
            .width(Size::fill())
            .spacing(6.)
            .children(rows)
            .into_element()
    };
    panel_frame(
        t("oneclient.cluster.bisect.history.title").to_string(),
        None,
        None,
        ScrollArea::new().child(content),
        Vec::new(),
        panel,
    )
}

fn conflict_block(title: String, report: &ConflictReport, incomplete: bool) -> Element {
    let mut block = rect()
        .vertical()
        .width(Size::fill())
        .spacing(6.)
        .padding(Gaps::new_all(12.))
        .corner_radius(rounded(8.))
        .background(colors::component_bg())
        .child(
            label()
                .text(title)
                .font_size(text_size(13.))
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        );
    if report.mods.len() > 1 {
        block = block.child(
            label()
                .text(t("oneclient.cluster.bisect.results.any_one"))
                .font_size(text_size(11.))
                .color(colors::fg_secondary()),
        );
    }
    for m in &report.mods {
        block = block.child(
            label()
                .text(format!("• {}", m.name))
                .font_size(text_size(12.))
                .color(colors::danger()),
        );
        if !m.also.is_empty() {
            block = block.child(
                label()
                    .text(tf("oneclient.cluster.bisect.results.also", &[&m.also.join(", ")]))
                    .font_size(text_size(11.))
                    .color(colors::fg_secondary()),
            );
        }
        if !m.maybe.is_empty() {
            block = block.child(
                label()
                    .text(tf("oneclient.cluster.bisect.results.maybe", &[&m.maybe.join(", ")]))
                    .font_size(text_size(11.))
                    .color(colors::fg_secondary()),
            );
        }
    }
    if !report.if_all_also.is_empty() {
        block = block.child(
            label()
                .text(tf(
                    "oneclient.cluster.bisect.results.if_all",
                    &[&report.if_all_also.join(", ")],
                ))
                .font_size(text_size(11.))
                .color(colors::fg_secondary()),
        );
    }
    if !report.if_all_maybe.is_empty() {
        block = block.child(
            label()
                .text(tf(
                    "oneclient.cluster.bisect.results.if_all_maybe",
                    &[&report.if_all_maybe.join(", ")],
                ))
                .font_size(text_size(11.))
                .color(colors::fg_secondary()),
        );
    }
    if incomplete {
        block = block.child(
            label()
                .text(t("oneclient.cluster.bisect.results.more"))
                .font_size(text_size(11.))
                .font_weight(FontWeight::MEDIUM)
                .color(colors::brand()),
        );
    }
    block.into_element()
}

fn results_panel(status: &BisectStatus, panel: State<Panel>) -> Element {
    let mut blocks: Vec<Element> = Vec::new();
    for (index, report) in status.earlier.iter().enumerate() {
        blocks.push(conflict_block(
            tf("oneclient.cluster.bisect.results.earlier", &[&(index + 1)]),
            report,
            false,
        ));
    }
    if !status.current.mods.is_empty() || status.earlier.is_empty() {
        let title = if status.current.mods.is_empty() {
            t("oneclient.cluster.bisect.results.nothing_yet").to_string()
        } else {
            t("oneclient.cluster.bisect.results.current").to_string()
        };
        blocks.push(conflict_block(title, &status.current, !status.finished()));
    }
    if !status.unresolvable.is_empty() {
        let names: Vec<String> = status
            .unresolvable
            .iter()
            .map(|m| {
                tf(
                    "oneclient.cluster.bisect.manage.unresolvable.line",
                    &[&m.name, &m.missing.join(", ")],
                )
            })
            .collect();
        blocks.push(names_box(
            t("oneclient.cluster.bisect.results.unresolvable").to_string(),
            &names,
        ));
    }
    let cleared: Vec<String> = status
        .mods
        .iter()
        .filter(|m| m.role == BisectRole::Cleared)
        .map(|m| m.name.clone())
        .collect();
    if !cleared.is_empty() {
        blocks.push(names_box(
            t("oneclient.cluster.bisect.results.cleared").to_string(),
            &cleared,
        ));
    }

    panel_frame(
        t("oneclient.cluster.bisect.results.title").to_string(),
        None,
        None,
        ScrollArea::new().child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(8.)
                .children(blocks),
        ),
        Vec::new(),
        panel,
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
        let verifying = status.stage == BisectStage::Verifying;
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
        } else if verifying {
            let names: Vec<&str> = status.culprits().iter().map(|m| m.name.as_str()).collect();
            tf(
                "oneclient.cluster.bisect.question.verify.body",
                &[&names.join(", ")],
            )
        } else {
            tf(
                "oneclient.cluster.bisect.question.body",
                &[&status.mods.iter().filter(|m| m.on).count()],
            )
        };

        let mut reset = move || {
            picking.set(false);
            picked.set(Vec::new());
        };

        let content = if choosing {
            let chosen = picked.read().clone();
            let off: Vec<(String, String)> = status
                .off_this_round()
                .map(|m| (m.hash.clone(), m.name.clone()))
                .collect();

            let rows = off.into_iter().map(|(hash, name)| {
                let selected = chosen.contains(&hash);
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .cross_align(Alignment::Center)
                    .spacing(10.)
                    .padding(Gaps::new_symmetric(8., 10.))
                    .corner_radius(rounded(8.))
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
                            .font_size(text_size(12.))
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
            let answer = move |answer: BisectAnswer| {
                move |_: Event<PressEventData>| {
                    mutation.mutate(BisectAction::Answer { cluster_id, answer })
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
                        .on_press(answer(BisectAnswer::Broken))
                        .text(t("oneclient.cluster.bisect.question.broken")),
                )
                .child(
                    Button::new()
                        .primary()
                        .width(Size::fill())
                        .enabled(!busy)
                        .on_press(answer(BisectAnswer::Works))
                        .text(t("oneclient.cluster.bisect.question.works")),
                )
                .child(
                    Button::new()
                        .secondary()
                        .width(Size::fill())
                        .enabled(!busy)
                        .tooltip(t("oneclient.cluster.bisect.question.unsure.tooltip"))
                        .on_press(answer(BisectAnswer::Unsure))
                        .text(t("oneclient.cluster.bisect.question.unsure")),
                )
                .maybe_child((!verifying).then(|| {
                    Button::new()
                        .secondary()
                        .width(Size::fill())
                        .enabled(!busy)
                        .on_press(move |_| picking.set(true))
                        .text(t("oneclient.cluster.bisect.question.missing"))
                        .into_element()
                }))
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
                            .corner_radius(rounded(14.))
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
                                            .font_size(text_size(16.))
                                            .font_weight(FontWeight::SEMI_BOLD)
                                            .color(colors::fg_primary()),
                                    ),
                            )
                            .child(
                                label()
                                    .text(body)
                                    .font_size(text_size(12.))
                                    .color(colors::fg_secondary()),
                            )
                            .child(content),
                    ),
            )
            .into_element()
    }
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

        let launches = oneclient_core::bisect::estimate_launches(enabled_mods);
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
                        .font_size(text_size(12.))
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(step)
                        .width(Size::flex(1.))
                        .font_size(text_size(12.))
                        .color(colors::fg_primary()),
                )
                .into_element()
        }))
        .into_element()
}
