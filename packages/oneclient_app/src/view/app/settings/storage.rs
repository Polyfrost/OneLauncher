use freya::prelude::*;
use oneclient_core::relocate::{RelocationOutcome, RelocationPlan};
use oneclient_core::storage::{ReclaimableEntry, StorageEntry, StorageReport, format_bytes};

use super::{section_header, settings_page};
use crate::components::{Button, Icon, IconType, OverlayPopup, open_folder_button};
use crate::hooks::{
    DiscardLeftoversKeys, StorageAction, mutation_error, mutation_is_running, mutation_ok,
    try_leftovers, try_storage_report, use_discard_leftovers, use_leftovers, use_relocate,
    use_storage_action, use_storage_report,
};
use crate::theme::colors;
use crate::ui::border_all_color;

/// Rows narrower than this would render as a sliver so they get a floor
const MIN_BAR_FRACTION: f32 = 0.015;

#[derive(PartialEq)]
pub struct SettingsStorage;

impl Component for SettingsStorage {
    fn render(&self) -> impl IntoElement {
        // Every hook before any early return the report is absent on the first render and a later-only hook would change the hook order
        let report_query = use_storage_report();

        let Some(report) = try_storage_report(&report_query) else {
            return settings_page()
                .child(hero_placeholder())
                .into_element();
        };

        let refresh = Button::new()
            .secondary()
            .small()
            .on_press(move |_| {
                report_query.invalidate();
            })
            .child(label().text("Refresh"));

        let mut page = settings_page()
            .child(hero(&report, refresh.into_element()))
            .child(section_header("LOCATION"))
            .child(LocationSection.into_element())
            .child(section_header("FREE UP SPACE"))
            .child(
                ReclaimRow {
                    icon: IconType::FileX02,
                    title: "Unreferenced cache files",
                    description: unused_cache_description(&report.unreferenced_cache),
                    action: StorageAction::CleanUnreferencedCache,
                    empty: report.unreferenced_cache.is_empty(),
                }
                .into_element(),
            )
            .child(
                ReclaimRow {
                    icon: IconType::Trash01,
                    title: "Leftover cluster content",
                    description: legacy_content_description(&report.legacy_cluster_content),
                    action: StorageAction::CleanLegacyClusterContent,
                    empty: report.legacy_cluster_content.is_empty(),
                }
                .into_element(),
            );

        page = page.child(section_header("WHAT'S USING SPACE"));
        page = match largest(&report.categories) {
            Some(largest) => {
                let mut list = page;
                for entry in report.categories.iter().filter(|e| e.bytes > 0) {
                    list = list.child(usage_row(entry, largest, colors::brand()));
                }
                list
            }
            None => page.child(empty_note("Nothing stored yet.")),
        };

        page = page.child(section_header("CLUSTERS"));
        page = match largest(&report.clusters) {
            Some(largest) => {
                let mut list = page;
                for entry in report.clusters.iter().filter(|e| e.bytes > 0) {
                    list = list.child(usage_row(entry, largest, colors::fg_secondary()));
                }
                list
            }
            None => page.child(empty_note("No cluster is using any space.")),
        };

        page.child(footnote()).into_element()
    }
}

fn hero(report: &StorageReport, refresh: Element) -> impl IntoElement {
    let reclaimable = report.unreferenced_cache.bytes + report.legacy_cluster_content.bytes;

    let subtitle = if reclaimable > 0 {
        format!("{} can be freed", format_bytes(reclaimable))
    } else {
        "Nothing to free up".to_string()
    };

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(16.)
        .padding(Gaps::new_symmetric(20., 16.))
        .corner_radius(CornerRadius::new_all(12.))
        .background(colors::page_elevated())
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(2.)
                .child(
                    label()
                        .text(format_bytes(report.total_bytes))
                        .font_size(28.)
                        .font_weight(FontWeight::BOLD)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(subtitle)
                        .font_size(12.)
                        .color(if reclaimable > 0 {
                            colors::brand()
                        } else {
                            colors::fg_secondary()
                        }),
                ),
        )
        .child(refresh)
        .into_element()
}

#[derive(PartialEq)]
struct LocationSection;

impl Component for LocationSection {
    fn render(&self) -> impl IntoElement {
        let relocate = use_relocate();
        let discard = use_discard_leftovers();
        let leftovers_query = use_leftovers();

        let mut pending = use_state(|| None::<RelocationPlan>);
        let mut error = use_state(|| None::<String>);
        let mut checking = use_state(|| false);

        let Ok(dir) = oneclient_common::paths::data_dir() else {
            return rect().into_element();
        };

        let moving = mutation_is_running(&relocate);
        let moved = mutation_ok(&relocate);
        let busy = moving || moved.is_some() || *checking.read();

        let browse = move |_| {
            if *checking.peek() {
                return;
            }

            spawn(async move {
                let mut dialog = rfd::AsyncFileDialog::new()
                    .set_title("Choose where OneClient should store game data");

                if let Some(start) = oneclient_common::paths::picker_start_dir() {
                    dialog = dialog.set_directory(start);
                }

                let Some(handle) = dialog.pick_folder().await else {
                    return;
                };

                checking.set(true);
                error.set(None);

                match crate::launcher::state() {
                    Ok(state) => {
                        match oneclient_core::relocate::plan(&state, handle.path()).await {
                            Ok(planned) => pending.set(Some(planned)),
                            Err(message) => error.set(Some(message)),
                        }
                    }
                    Err(err) => error.set(Some(err.to_string())),
                }

                checking.set(false);
            });
        };

        let mut section = rect()
            .vertical()
            .width(Size::fill())
            .spacing(10.)
            .child(
                row(IconType::Folder)
                    .child(
                        rect()
                            .vertical()
                            .width(Size::flex(1.0))
                            .spacing(2.)
                            .child(row_title("Game data folder"))
                            .child(row_detail(dir.display().to_string())),
                    )
                    .child(open_folder_button(dir.to_path_buf()))
                    .child(
                        Button::new()
                            .secondary()
                            .small()
                            .disabled(busy)
                            .on_press(browse)
                            .text(match (moving, moved.is_some()) {
                                (true, _) => "Moving…",
                                (_, true) => "Moved",
                                _ => "Change…",
                            }),
                    ),
            );

        if let Some(outcome) = &moved {
            section = section.child(moved_note(outcome));
        }

        if let Some(message) = mutation_error(&relocate) {
            section = section.child(note(message, colors::danger()));
        }

        if let Some(message) = error.read().clone() {
            section = section.child(note(message, colors::danger()));
        }

        if let Some(left) = try_leftovers(&leftovers_query) {
            let clearing = mutation_is_running(&discard);

            section = section.child(
                row(IconType::Database01)
                    .child(
                        rect()
                            .vertical()
                            .width(Size::flex(1.0))
                            .spacing(2.)
                            .child(row_title("Old location"))
                            .child(row_detail(format!(
                                "{} still sitting in {}. Nothing uses it any more.",
                                format_bytes(left.bytes),
                                left.path.display()
                            ))),
                    )
                    .child(open_folder_button(left.path.clone()))
                    .child(
                        Button::new()
                            .danger()
                            .small()
                            .disabled(clearing)
                            .on_press(move |_| {
                                discard.mutate(DiscardLeftoversKeys);
                            })
                            .text(if clearing { "Removing…" } else { "Remove" }),
                    ),
            );
        }

        if let Some(message) = mutation_error(&discard) {
            section = section.child(note(message, colors::danger()));
        }

        if let Some(planned) = pending.read().clone() {
            section = section.child(confirm_move(planned, pending, relocate));
        }

        section.into_element()
    }
}

fn confirm_move(
    planned: RelocationPlan,
    mut pending: State<Option<RelocationPlan>>,
    relocate: crate::hooks::UseRelocate,
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
        .child(path_block("From", &planned.from.display().to_string()))
        .child(path_block("To", &planned.to.display().to_string()))
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
                     you remove it, and OneClient has to restart before it uses the new folder.",
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
                            relocate.mutate(start.clone());
                            pending.set(None);
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

fn moved_note(outcome: &RelocationOutcome) -> Element {
    let mut body = format!(
        "{} copied to {}. Restart OneClient to start using it, until then it keeps running from \
         the folder above.",
        format_bytes(outcome.bytes),
        outcome.to.display()
    );

    if outcome.skipped_links > 0 {
        body.push_str(&format!(
            " {} shortcut{} pointing outside the folder were left behind.",
            outcome.skipped_links,
            plural(outcome.skipped_links)
        ));
    }

    note(body, colors::brand())
}

fn note(message: String, accent: Color) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .spacing(10.)
        .padding(Gaps::new_symmetric(12., 14.))
        .corner_radius(CornerRadius::new_all(10.))
        .background(accent.with_a(30))
        .child(
            label()
                .text(message)
                .font_size(12.)
                .max_lines(5)
                .width(Size::flex(1.0))
                .color(colors::fg_primary()),
        )
        .into_element()
}

fn path_block(caption: &'static str, path: &str) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(4.)
        .child(
            label()
                .text(caption)
                .font_size(11.)
                .color(colors::fg_secondary()),
        )
        .child(
            rect()
                .width(Size::fill())
                .padding(Gaps::new_all(10.))
                .corner_radius(CornerRadius::new_all(8.))
                .background(colors::component_bg())
                .border(border_all_color(1., colors::component_border()))
                .child(
                    label()
                        .text(path.to_string())
                        .font_size(12.)
                        .max_lines(3)
                        .width(Size::fill())
                        .color(colors::fg_primary()),
                ),
        )
        .into_element()
}

fn row(icon: IconType) -> Rect {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(16.)
        .padding(Gaps::new_symmetric(12., 16.))
        .corner_radius(CornerRadius::new_all(12.))
        .background(colors::page_elevated())
        .child(Icon::new(icon))
}

fn row_title(text: &'static str) -> impl IntoElement {
    label()
        .text(text)
        .font_size(16.)
        .font_weight(FontWeight::MEDIUM)
        .color(colors::fg_primary())
}

fn row_detail(text: String) -> impl IntoElement {
    label()
        .text(text)
        .font_size(12.)
        .max_lines(2)
        .color(colors::fg_secondary())
}

fn hero_placeholder() -> impl IntoElement {
    rect()
        .width(Size::fill())
        .padding(Gaps::new_symmetric(20., 16.))
        .corner_radius(CornerRadius::new_all(12.))
        .background(colors::page_elevated())
        .child(
            label()
                .text("Measuring disk usage…")
                .font_size(16.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

#[derive(PartialEq)]
struct ReclaimRow {
    icon: IconType,
    title: &'static str,
    description: String,
    action: StorageAction,
    empty: bool,
}

impl Component for ReclaimRow {
    fn render(&self) -> impl IntoElement {
        let action = use_storage_action();
        let running = mutation_is_running(&action);

        let kind = self.action;
        let button = Button::new()
            .secondary()
            .small()
            .disabled(self.empty || running)
            .on_press(move |_| {
                action.mutate(kind);
            })
            .child(label().text(if running { "Cleaning…" } else { "Clean up" }));

        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(16.)
            .padding(Gaps::new_symmetric(12., 16.))
            .corner_radius(CornerRadius::new_all(12.))
            .background(colors::page_elevated())
            .maybe(self.empty, |el| el.opacity(0.55))
            .child(Icon::new(self.icon))
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(2.)
                    .child(
                        label()
                            .text(self.title)
                            .font_size(16.)
                            .font_weight(FontWeight::MEDIUM)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text(self.description.clone())
                            .font_size(12.)
                            .color(colors::fg_secondary()),
                    ),
            )
            .child(button)
            .into_element()
    }
}

fn usage_row(entry: &StorageEntry, largest: u64, bar: Color) -> impl IntoElement {
    let fraction = if largest == 0 {
        0.0
    } else {
        (entry.bytes as f32 / largest as f32).max(MIN_BAR_FRACTION)
    };

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(16.)
        .padding(Gaps::new_symmetric(12., 16.))
        .corner_radius(CornerRadius::new_all(12.))
        .background(colors::page_elevated())
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(6.)
                .child(
                    label()
                        .text(entry.label.clone())
                        .font_size(14.)
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_primary()),
                )
                .child(proportion_bar(fraction, bar)),
        )
        .child(
            label()
                .text(format_bytes(entry.bytes))
                .font_size(13.)
                .color(colors::fg_secondary()),
        )
        .child(open_folder_button(entry.path.clone()))
        .into_element()
}

fn proportion_bar(fraction: f32, fill: Color) -> impl IntoElement {
    rect()
        .width(Size::fill())
        .height(Size::px(4.))
        .corner_radius(CornerRadius::new_all(2.))
        .background(colors::component_bg())
        .child(
            rect()
                .width(Size::percent(fraction.clamp(0.0, 1.0) * 100.))
                .height(Size::fill())
                .corner_radius(CornerRadius::new_all(2.))
                .background(fill),
        )
        .into_element()
}

fn largest(entries: &[StorageEntry]) -> Option<u64> {
    entries.iter().map(|e| e.bytes).max().filter(|max| *max > 0)
}

fn unused_cache_description(entry: &ReclaimableEntry) -> String {
    if entry.is_empty() {
        return "Nothing here — every cached file belongs to an installed package.".to_string();
    }

    format!(
        "{} across {} file{} that no installed package points at.",
        format_bytes(entry.bytes),
        entry.files,
        plural(entry.files)
    )
}

fn legacy_content_description(entry: &ReclaimableEntry) -> String {
    if entry.is_empty() {
        return "Nothing here — no cluster folder is holding old content.".to_string();
    }

    format!(
        "{} across {} file{} left in cluster folders by an older version. Already ignored when \
         you play; this just reclaims the space.",
        format_bytes(entry.bytes),
        entry.files,
        plural(entry.files)
    )
}

fn empty_note(text: &'static str) -> impl IntoElement {
    rect()
        .width(Size::fill())
        .padding(Gaps::new_symmetric(14., 16.))
        .corner_radius(CornerRadius::new_all(12.))
        .background(colors::page_elevated())
        .child(
            label()
                .text(text)
                .font_size(13.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn footnote() -> impl IntoElement {
    rect()
        .padding(Gaps::new(16., 4., 8., 4.))
        .child(
            label()
                .text(
                    "Packages live in a shared cache and are copied into the game folder only \
                     while you play. Removing a package frees its cache entry automatically.",
                )
                .font_size(12.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}
