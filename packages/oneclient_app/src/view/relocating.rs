use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::relocate::{RelocationOutcome, RelocationPlan};
use oneclient_core::storage::format_bytes;

use crate::Route;
use crate::components::{Button, Icon, IconType};
use crate::hooks::{Actions, use_dispatch, use_relocation};
use crate::theme::colors;
use crate::ui::{note, path_block};
use crate::utils::plural;

#[derive(PartialEq)]
pub struct Relocating;

impl Component for Relocating {
    fn render(&self) -> impl IntoElement {
        let relocation = use_relocation();
        let actions = use_dispatch();

        let Some(plan) = relocation.plan.clone() else {
            // Landed here without a move in flight
            let _ = RouterContext::get().replace(Route::SettingsLauncher {});
            return rect().into_element();
        };

        let content = match &relocation.result {
            None => copying(&plan, relocation.copied, relocation.total),
            Some(Ok(outcome)) => moved(outcome, actions),
            Some(Err(message)) => failed(message.clone(), actions),
        };

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

fn copying(plan: &RelocationPlan, copied: u64, total: u64) -> Element {
    // The plan's estimate stands in until the copy has counted the tree itself
    let total = if total > 0 { total } else { plan.bytes }.max(1);

    screen()
        .child(heading(IconType::FolderDownload, "Moving your data folder", colors::fg_primary()))
        .child(path_block("From", &plan.from))
        .child(path_block("To", &plan.to))
        .child(progress_track(copied as f32 / total as f32 * 100.))
        .child(
            label()
                .text(format!(
                    "{} of {}",
                    format_bytes(copied),
                    format_bytes(total)
                ))
                .font_size(12.)
                .color(colors::fg_secondary()),
        )
        .child(note(
            "Leave OneClient open until this finishes. Nothing is removed from the old folder, \
             and the launcher stays closed to everything else while files are copied."
                .to_string(),
            colors::brand(),
        ))
        .into_element()
}

fn moved(outcome: &RelocationOutcome, actions: Actions) -> Element {
    let mut body = format!(
        "{} copied to {}. OneClient keeps running from the old folder until you restart it.",
        format_bytes(outcome.bytes),
        outcome.to.display()
    );

    if outcome.skipped_links > 0 {
        body.push_str(&format!(
            " {} shortcut{} pointing outside the folder were left behind.",
            outcome.skipped_links,
            plural(outcome.skipped_links as i64)
        ));
    }

    screen()
        .child(heading(IconType::FolderCheck, "Game data moved", colors::brand()))
        .child(note(body, colors::brand()))
        .child(
            label()
                .text(
                    "The old copy is still there. Once you have restarted, Launcher Settings can \
                     remove it.",
                )
                .font_size(12.)
                .max_lines(3)
                .color(colors::fg_secondary()),
        )
        .child(
            buttons()
                .child(back_button(actions))
                .child(
                    Button::new()
                        .primary()
                        .on_press(|_| {
                            let platform = Platform::get();
                            Platform::get().with_window(None, move |window| {
                                platform.close_window(window.id());
                            });
                        })
                        .text("Quit OneClient"),
                ),
        )
        .into_element()
}

fn failed(message: String, actions: Actions) -> Element {
    screen()
        .child(heading(IconType::AlertTriangle, "The move didn't finish", colors::danger()))
        .child(note(message, colors::danger()))
        .child(
            label()
                .text("Nothing was changed. OneClient is still running from its current folder.")
                .font_size(12.)
                .max_lines(2)
                .color(colors::fg_secondary()),
        )
        .child(buttons().child(back_button(actions)))
        .into_element()
}

fn back_button(actions: Actions) -> Button {
    Button::new()
        .secondary()
        .on_press(move |_| {
            actions.end_relocation();
            let _ = RouterContext::get().replace(Route::SettingsLauncher {});
        })
        .text("Back to settings")
}

fn screen() -> Rect {
    rect()
        .vertical()
        .width(Size::px(460.))
        .max_width(Size::window_percent(90.))
        .spacing(14.)
}

fn heading(icon: IconType, title: &'static str, tint: Color) -> impl IntoElement {
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(10.)
        .child(Icon::new(icon).size(20.).color(tint))
        .child(
            label()
                .text(title)
                .font_size(18.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        )
}

fn progress_track(pct: f32) -> impl IntoElement {
    rect()
        .width(Size::fill())
        .height(Size::px(6.))
        .corner_radius(CornerRadius::new_all(3.))
        .background(colors::component_bg())
        .child(
            rect()
                .width(Size::percent(pct.clamp(0.0, 100.0)))
                .height(Size::fill())
                .corner_radius(CornerRadius::new_all(3.))
                .background(colors::brand()),
        )
}

fn buttons() -> Rect {
    rect()
        .horizontal()
        .width(Size::fill())
        .main_align(Alignment::End)
        .spacing(8.)
}
