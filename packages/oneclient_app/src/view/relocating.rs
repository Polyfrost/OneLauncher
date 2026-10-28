use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::relocate::{RelocationOutcome, RelocationPlan};
use oneclient_core::storage::format_bytes;

use crate::Route;
use crate::components::{Button, Icon, IconType, progress_track};
use crate::hooks::{Actions, use_dispatch, use_relocation};
use crate::i18n::{t, tf};
use crate::theme::colors;
use crate::ui::{note, path_block};

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
        .child(heading(
            IconType::FolderDownload,
            t("oneclient.relocating.copying.title"),
            colors::fg_primary(),
        ))
        .child(path_block(t("oneclient.relocating.from"), &plan.from))
        .child(path_block(t("oneclient.relocating.to"), &plan.to))
        .child(progress_track(
            copied as f32 / total as f32 * 100.,
            6.,
            colors::brand(),
            colors::component_bg(),
        ))
        .child(
            label()
                .text(tf(
                    "oneclient.relocating.copying.progress",
                    &[&format_bytes(copied), &format_bytes(total)],
                ))
                .font_size(12.)
                .color(colors::fg_secondary()),
        )
        .child(note(
            t("oneclient.relocating.copying.note").to_string(),
            colors::brand(),
        ))
        .into_element()
}

fn moved(outcome: &RelocationOutcome, actions: Actions) -> Element {
    let mut body = tf(
        "oneclient.relocating.moved.body",
        &[&format_bytes(outcome.bytes), &outcome.to.display()],
    );

    if outcome.skipped_links > 0 {
        body.push(' ');
        body.push_str(&if outcome.skipped_links == 1 {
            tf(
                "oneclient.relocating.moved.skipped_links.one",
                &[&outcome.skipped_links],
            )
        } else {
            tf(
                "oneclient.relocating.moved.skipped_links.other",
                &[&outcome.skipped_links],
            )
        });
    }

    screen()
        .child(heading(
            IconType::FolderCheck,
            t("oneclient.relocating.moved.title"),
            colors::brand(),
        ))
        .child(note(body, colors::brand()))
        .child(
            label()
                .text(t("oneclient.relocating.moved.old_copy"))
                .font_size(12.)
                .max_lines(3)
                .color(colors::fg_secondary()),
        )
        .child(
            buttons().child(back_button(actions)).child(
                Button::new()
                    .primary()
                    .on_press(|_| crate::platform::quit())
                    .text(t("oneclient.relocating.moved.quit")),
            ),
        )
        .into_element()
}

fn failed(message: String, actions: Actions) -> Element {
    screen()
        .child(heading(
            IconType::AlertTriangle,
            t("oneclient.relocating.failed.title"),
            colors::danger(),
        ))
        .child(note(message, colors::danger()))
        .child(
            label()
                .text(t("oneclient.relocating.failed.body"))
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
        .text(t("oneclient.relocating.back_to_settings"))
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

fn buttons() -> Rect {
    rect()
        .horizontal()
        .width(Size::fill())
        .main_align(Alignment::End)
        .spacing(8.)
}
