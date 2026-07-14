use freya::prelude::*;
use freya::router::RouterContext;

use crate::components::{Button, Icon, IconType, TextInput, toggle};
use crate::hooks::use_dispatch;
use crate::notifications::{ClusterUpdateSummary, NotificationAction, NotificationActionKind};
use crate::routes::Route;
use crate::theme::colors;

#[derive(PartialEq)]
pub struct Debug;

impl Component for Debug {
    fn render(&self) -> impl IntoElement {
        let log_debug_info = use_state(|| false);
        let show_dev_stuff = use_state(|| false);
        let seen_onboarding = use_state(|| true);
        let use_grid_on_mods_list = use_state(|| true);

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .padding(40.)
            .spacing(8.)
            .child(
                ScrollView::new()
                    .child(
                        rect()
                            .vertical()
                            .child(
                                label()
                                    .text("Debug")
                                    .font_size(32.)
                                    .font_weight(FontWeight::BOLD)
                                    .color(colors::fg_primary()),
                            )
                            .child(
                                label()
                                    .text("The end user won't really be looking at this page.")
                                    .font_size(13.)
                                    .color(colors::fg_secondary()),
                            ),
                    )
                    .child(divider())
                    .child(section(
                        "Settings",
                        vec![
                            toggle_row("Log Debug Info", log_debug_info),
                            toggle_row("Show Dev stuff", show_dev_stuff),
                            toggle_row("Seen Onboarding", seen_onboarding),
                            toggle_row("Use Grid On Mods List", use_grid_on_mods_list),
                        ],
                    ))
                    .child(divider())
                    .child(section(
                        "Toast Controller",
                        vec![ToastController.into_element()],
                    ))
                    .child(divider())
                    .child(section(
                        "Cluster Update Simulator",
                        vec![ClusterUpdateSimulator.into_element()],
                    ))
                    .child(divider())
                    .child(section(
                        "Other",
                        vec![action_row(vec![
                            ("Open Dev Tools", IconType::CodeSnippet02),
                            ("Open Onboarding", IconType::Rocket02),
                            ("Open Launcher Data", IconType::Folder),
                            ("Log Running Processes", IconType::Terminal),
                        ])],
                    )),
            )
    }
}

#[derive(PartialEq)]
struct ToastController;

impl Component for ToastController {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let title = use_state(|| "Downloading assets".to_string());
        let body = use_state(|| "PolyBlock by Polyfrost".to_string());
        let mut progress = use_state(|| 0u64);

        let info = dispatch.clone();
        let error = dispatch.clone();
        let prog = dispatch.clone();
        let reset = dispatch;

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(10.)
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .spacing(12.)
                    .child(
                        rect()
                            .width(Size::flex(1.0))
                            .child(TextInput::new(title).placeholder("Title")),
                    )
                    .child(
                        rect()
                            .width(Size::flex(1.0))
                            .child(TextInput::new(body).placeholder("Body")),
                    ),
            )
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .spacing(12.)
                    .child(
                        Button::new()
                            .primary()
                            .child(Icon::new(IconType::InfoCircle).size(16.))
                            .text("Send Info")
                            .on_press(move |_| {
                                info.notify(title.read().clone())
                                    .body(body.read().clone())
                                    .info()
                                    .send();
                            }),
                    )
                    .child(
                        Button::new()
                            .danger()
                            .child(Icon::new(IconType::AlertTriangle).size(16.))
                            .text("Send Error")
                            .on_press(move |_| {
                                error
                                    .notify(title.read().clone())
                                    .body(body.read().clone())
                                    .error()
                                    .send();
                            }),
                    )
                    .child(
                        Button::new()
                            .secondary()
                            .child(Icon::new(IconType::FolderDownload).size(16.))
                            .text("Progress +10")
                            .on_press(move |_| {
                                let next = (*progress.read() + 10).min(100);
                                progress.set(next);
                                prog.send_test_progress(next, 100);
                            }),
                    )
                    .child(
                        Button::new()
                            .secondary()
                            .text("Reset Progress")
                            .on_press(move |_| {
                                progress.set(0);
                                reset.send_test_progress(0, 100);
                            }),
                    ),
            )
            .into_element()
    }
}

#[derive(PartialEq)]
struct ClusterUpdateSimulator;

impl Component for ClusterUpdateSimulator {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let cluster_id = use_state(|| "1".to_string());
        let cluster_name = use_state(|| "PolyBlock".to_string());
        let updated = use_state(|| "Sodium 0.5 → 0.6, Iris 1.7 → 1.8".to_string());
        let added = use_state(|| "Lithium".to_string());
        let removed = use_state(|| "OptiFine".to_string());

        let simulate = dispatch;

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(10.)
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .spacing(12.)
                    .child(
                        rect()
                            .width(Size::px(120.))
                            .child(TextInput::new(cluster_id).placeholder("Cluster ID")),
                    )
                    .child(
                        rect()
                            .width(Size::flex(1.0))
                            .child(TextInput::new(cluster_name).placeholder("Cluster name")),
                    ),
            )
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .spacing(12.)
                    .child(
                        rect().width(Size::flex(1.0)).child(
                            TextInput::new(updated).placeholder("Updated (comma separated)"),
                        ),
                    )
                    .child(
                        rect()
                            .width(Size::flex(1.0))
                            .child(TextInput::new(added).placeholder("Added (comma separated)")),
                    )
                    .child(
                        rect().width(Size::flex(1.0)).child(
                            TextInput::new(removed).placeholder("Removed (comma separated)"),
                        ),
                    ),
            )
            .child(
                Button::new()
                    .primary()
                    .child(Icon::new(IconType::DownloadCloud02).size(16.))
                    .text("Simulate Cluster Update")
                    .on_press(move |_| {
                        let name = cluster_name.read().clone();
                        let summary = ClusterUpdateSummary {
                            cluster_id: cluster_id.read().trim().parse().unwrap_or(1),
                            cluster_name: name.clone(),
                            updated: split_csv(&updated.read()),
                            added: split_csv(&added.read()),
                            removed: split_csv(&removed.read()),
                        };
                        let total = summary.total();
                        simulate
                            .notify("Cluster updated")
                            .body(format!(
                                "{total} package{} changed in {name}",
                                if total == 1 { "" } else { "s" }
                            ))
                            .icon(IconType::DownloadCloud02)
                            .action(NotificationAction {
                                label: "View changes".to_string(),
                                kind: NotificationActionKind::OpenClusterUpdate(summary),
                            })
                            .send();
                    }),
            )
            .into_element()
    }
}

fn split_csv(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn divider() -> impl IntoElement {
    rect()
        .width(Size::fill())
        .height(Size::px(1.))
        .margin(Gaps::new_symmetric(8., 0.))
        .corner_radius(CornerRadius::new_all(1.))
        .background(colors::component_border())
        .into_element()
}

fn section(title: &'static str, rows: Vec<Element>) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .child(
            label()
                .text(title)
                .font_size(22.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        )
        .children(rows)
        .into_element()
}

// fn field_row(label_text: &'static str, value: &'static str) -> impl IntoElement {
//     rect()
//         .horizontal()
//         .width(Size::fill())
//         .cross_align(Alignment::Center)
//         .spacing(12.)
//         .child(
//             label()
//                 .text(label_text)
//                 .width(Size::px(80.))
//                 .font_size(13.)
//                 .color(colors::fg_secondary()),
//         )
//         .child(
//             rect()
//                 .width(Size::flex(1.0))
//                 .padding(Gaps::new_symmetric(7., 12.))
//                 .corner_radius(CornerRadius::new_all(8.))
//                 .background(colors::component_bg())
//                 .border(border_all_color(1., colors::component_border()))
//                 .child(
//                     label()
//                         .text(value)
//                         .font_size(12.)
//                         .color(colors::fg_primary()),
//                 ),
//         )
//         .into_element()
// }

fn toggle_row(title: &'static str, on: State<bool>) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .main_align(Alignment::SpaceBetween)
        .padding(Gaps::new_symmetric(10., 14.))
        .corner_radius(CornerRadius::new_all(10.))
        .background(colors::page_elevated())
        .child(
            label()
                .text(title)
                .font_size(14.)
                .color(colors::fg_primary()),
        )
        .child(toggle(on))
        .into_element()
}

fn action_row(buttons: Vec<(&'static str, IconType)>) -> Element {
    let mut row = rect().horizontal().width(Size::fill()).spacing(12.);

    for (text, icon) in buttons {
        let mut button = Button::new()
            .secondary()
            .child(Icon::new(icon).size(16.))
            .text(text);

        if text == "Open Onboarding" {
            button = button.on_press(|_| {
                let _ = RouterContext::get().replace(Route::OnboardingWelcome {});
            });
        }

        row = row.child(button);
    }

    row.into_element()
}
