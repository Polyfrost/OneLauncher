use freya::prelude::*;
use oneclient_core::storage::{ReclaimableEntry, StorageEntry, StorageReport, format_bytes};

use super::{section_header, settings_page};
use crate::components::{Button, Icon, IconType, open_folder_button};
use crate::hooks::{
    StorageAction, mutation_is_running, try_storage_report, use_storage_action, use_storage_report,
    use_storage_scan_progress,
};
use crate::state::StorageScanProgress;
use crate::theme::colors;

/// Rows narrower than this would render as a sliver so they get a floor
const MIN_BAR_FRACTION: f32 = 0.015;

#[derive(PartialEq)]
pub struct SettingsStorage;

impl Component for SettingsStorage {
    fn render(&self) -> impl IntoElement {
        // Every hook before any early return the report is absent on the first render and a later-only hook would change the hook order
        let report_query = use_storage_report();
        let scan = use_storage_scan_progress();

        let Some(report) = try_storage_report(&report_query) else {
            return settings_page()
                .child(scan_card(scan.as_ref()))
                .into_element();
        };

        let refresh = Button::new()
            .secondary()
            .small()
            .on_press(move |_| {
                report_query.invalidate();
            })
            .child(label().text("Refresh"));

        let mut page = settings_page().child(hero(&report, refresh.into_element()));

        if scan.is_some() {
            page = page.child(scan_card(scan.as_ref()));
        }

        page = page
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

// the first-load placeholder and the strip shown while a refresh rescans
fn scan_card(scan: Option<&StorageScanProgress>) -> impl IntoElement {
    let counting = scan.filter(|scan| scan.total > 0);
    let fraction = counting.map_or(0.0, |scan| scan.current as f32 / scan.total as f32);

    let mut header = rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .child(
            rect().width(Size::flex(1.0)).child(
                label()
                    .text(scan.map_or_else(
                        || "Measuring disk usage…".to_string(),
                        |scan| format!("{}…", scan.label),
                    ))
                    .font_size(14.)
                    .color(colors::fg_secondary()),
            ),
        );

    if let Some(scan) = counting {
        header = header.child(
            label()
                .text(format!("{} / {}", scan.current, scan.total))
                .font_size(12.)
                .color(colors::fg_secondary()),
        );
    }

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .padding(Gaps::new_symmetric(16., 16.))
        .corner_radius(CornerRadius::new_all(12.))
        .background(colors::page_elevated())
        .child(header)
        .child(proportion_bar(fraction, colors::brand()))
        .into_element()
}

/// A component rather than a function because it owns a hook each instance gets its own scope so a button tracks only its own progress
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
