use freya::prelude::*;
use oneclient_core::storage::{ReclaimableEntry, StorageEntry, StorageReport, format_bytes};

use super::{PAGE_GAP, first_section_header, section_header, settings_page, settings_row_surface};
use crate::Route;
use crate::components::{Button, Icon, IconType, open_folder_button};
use crate::hooks::{
    StorageAction, mutation_is_running, try_storage_report, use_storage_action, use_storage_report,
    use_storage_scan_progress,
};
use crate::i18n::{t, tf};
use crate::layout::SearchItem;
use crate::state::StorageScanProgress;
use crate::theme::{colors, rounded, text_size};

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
            .child(label().text(t("oneclient.common.refresh")));

        settings_page()
            .child(super::focus_anchor(
                "storage.usage",
                hero(&report, refresh.into_element(), scan.as_ref()),
            ))
            .child(super::focus_anchor(
                "storage.cleanup",
                first_section_header(t("oneclient.settings.storage.section.free_up_space")),
            ))
            .child(
                ReclaimRow {
                    icon: IconType::FileX02,
                    title: t("oneclient.settings.storage.unused_cache.title"),
                    description: unused_cache_description(&report.unreferenced_cache),
                    action: StorageAction::CleanUnreferencedCache,
                    empty: report.unreferenced_cache.is_empty(),
                }
                .into_element(),
            )
            .child(
                ReclaimRow {
                    icon: IconType::Trash01,
                    title: t("oneclient.settings.storage.legacy.title"),
                    description: legacy_content_description(
                        &report.legacy_cluster_content,
                        &report.unused_natives,
                    ),
                    action: StorageAction::CleanLegacyClusterContent,
                    empty: report.legacy_cluster_content.is_empty()
                        && report.unused_natives.is_empty(),
                }
                .into_element(),
            )
            .child(section_header(t(
                "oneclient.settings.storage.section.usage",
            )))
            .child(usage_list(
                &report.categories,
                colors::brand(),
                t("oneclient.settings.storage.usage_empty"),
            ))
            .child(section_header(t(
                "oneclient.settings.storage.section.clusters",
            )))
            .child(usage_list(
                &report.clusters,
                colors::fg_secondary(),
                t("oneclient.settings.storage.clusters_empty"),
            ))
            .child(footnote())
            .into_element()
    }
}

fn hero(
    report: &StorageReport,
    refresh: Element,
    scan: Option<&StorageScanProgress>,
) -> impl IntoElement {
    let reclaimable = report.unreferenced_cache.bytes
        + report.legacy_cluster_content.bytes
        + report.unused_natives.bytes;

    // Progress replaces the subtitle and runs along the card's foot, so a refresh moves nothing
    let subtitle = match scan {
        Some(scan) if scan.total > 0 => {
            format!("{}… {} / {}", scan.label, scan.current, scan.total)
        }
        Some(scan) => format!("{}…", scan.label),
        None if reclaimable > 0 => tf(
            "oneclient.settings.storage.reclaimable",
            &[&format_bytes(reclaimable)],
        ),
        None => t("oneclient.settings.storage.nothing_to_free").to_string(),
    };

    let summary = rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(16.)
        .padding(Gaps::new_symmetric(20., 16.))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(2.)
                .child(
                    label()
                        .text(format_bytes(report.total_bytes))
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis)
                        .font_size(text_size(28.))
                        .font_weight(FontWeight::BOLD)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(subtitle)
                        .font_size(text_size(12.))
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis)
                        .color(if scan.is_none() && reclaimable > 0 {
                            colors::brand()
                        } else {
                            colors::fg_secondary()
                        }),
                ),
        )
        .child(refresh);

    rect()
        .width(Size::fill())
        .corner_radius(rounded(12.))
        .background(colors::card())
        .child(summary)
        .maybe_child(scan.map(|scan| {
            let fraction = if scan.total > 0 {
                scan.current as f32 / scan.total as f32
            } else {
                0.
            };
            rect()
                .position(Position::new_absolute().left(16.).right(16.).bottom(8.))
                .child(proportion_bar(fraction, colors::brand()))
        }))
        .into_element()
}

fn scan_card(scan: Option<&StorageScanProgress>) -> impl IntoElement {
    rect()
        .width(Size::fill())
        .padding(Gaps::new_all(16.))
        .corner_radius(rounded(12.))
        .background(colors::card())
        .child(scan_details(scan))
        .into_element()
}

fn scan_details(scan: Option<&StorageScanProgress>) -> impl IntoElement {
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
                        || t("oneclient.settings.storage.measuring").to_string(),
                        |scan| format!("{}…", scan.label),
                    ))
                    .width(Size::fill())
                    .font_size(text_size(14.))
                    .max_lines(1)
                    .text_overflow(TextOverflow::Ellipsis)
                    .color(colors::fg_secondary()),
            ),
        );

    if let Some(scan) = counting {
        header = header.child(
            label()
                .text(format!("{} / {}", scan.current, scan.total))
                .font_size(text_size(12.))
                .color(colors::fg_secondary()),
        );
    }

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(6.)
        .child(header)
        .child(proportion_bar(fraction, colors::brand()))
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
            .child(label().text(if running {
                t("oneclient.settings.storage.cleaning")
            } else {
                t("oneclient.settings.storage.clean_up")
            }));

        settings_row_surface(false)
            .child(Icon::new(self.icon))
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(2.)
                    .child(
                        label()
                            .text(self.title)
                            .font_size(text_size(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text(self.description.clone())
                            .font_size(text_size(12.))
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

    settings_row_surface(false)
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(6.)
                .child(
                    label()
                        .text(entry.label.clone())
                        .font_size(text_size(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_primary()),
                )
                .child(proportion_bar(fraction, bar)),
        )
        .child(
            label()
                .text(format_bytes(entry.bytes))
                .font_size(text_size(13.))
                .color(colors::fg_secondary()),
        )
        .child(open_folder_button(entry.path.clone()))
        .into_element()
}

fn proportion_bar(fraction: f32, fill: Color) -> impl IntoElement {
    rect()
        .width(Size::fill())
        .height(Size::px(4.))
        .corner_radius(rounded(2.))
        .background(colors::component_bg())
        .child(
            rect()
                .width(Size::percent(fraction.clamp(0.0, 1.0) * 100.))
                .height(Size::fill())
                .corner_radius(rounded(2.))
                .background(fill),
        )
        .into_element()
}

/// One row per entry that holds anything, bars scaled against the biggest of them
fn usage_list(entries: &[StorageEntry], bar: Color, empty: &'static str) -> Element {
    let Some(largest) = largest(entries) else {
        return empty_note(empty).into_element();
    };

    let mut list = rect().vertical().width(Size::fill()).spacing(PAGE_GAP);
    for entry in entries.iter().filter(|entry| entry.bytes > 0) {
        list = list.child(usage_row(entry, largest, bar));
    }

    list.into_element()
}

fn largest(entries: &[StorageEntry]) -> Option<u64> {
    entries.iter().map(|e| e.bytes).max().filter(|max| *max > 0)
}

fn unused_cache_description(entry: &ReclaimableEntry) -> String {
    if entry.is_empty() {
        return t("oneclient.settings.storage.unused_cache.empty").to_string();
    }

    if entry.files == 1 {
        tf(
            "oneclient.settings.storage.unused_cache.description.one",
            &[&format_bytes(entry.bytes), &entry.files],
        )
    } else {
        tf(
            "oneclient.settings.storage.unused_cache.description.other",
            &[&format_bytes(entry.bytes), &entry.files],
        )
    }
}

fn legacy_content_description(content: &ReclaimableEntry, natives: &ReclaimableEntry) -> String {
    if content.is_empty() && natives.is_empty() {
        return t("oneclient.settings.storage.legacy.empty").to_string();
    }

    let mut parts = Vec::new();

    if !content.is_empty() {
        parts.push(if content.files == 1 {
            tf(
                "oneclient.settings.storage.legacy.content.one",
                &[&format_bytes(content.bytes), &content.files],
            )
        } else {
            tf(
                "oneclient.settings.storage.legacy.content.other",
                &[&format_bytes(content.bytes), &content.files],
            )
        });
    }

    if !natives.is_empty() {
        parts.push(if natives.files == 1 {
            tf(
                "oneclient.settings.storage.legacy.natives.one",
                &[&format_bytes(natives.bytes), &natives.files],
            )
        } else {
            tf(
                "oneclient.settings.storage.legacy.natives.other",
                &[&format_bytes(natives.bytes), &natives.files],
            )
        });
    }

    // Natives are not inert deleting them costs a re-download on the next launch
    // so promising the space is free would be a lie
    let tail = if natives.is_empty() {
        t("oneclient.settings.storage.legacy.tail_content")
    } else {
        t("oneclient.settings.storage.legacy.tail_natives")
    };

    tf(
        "oneclient.settings.storage.legacy.summary",
        &[
            &parts.join(t("oneclient.settings.storage.legacy.joiner")),
            &tail,
        ],
    )
}

fn empty_note(text: &'static str) -> impl IntoElement {
    rect()
        .width(Size::fill())
        .padding(Gaps::new_symmetric(14., 16.))
        .corner_radius(rounded(12.))
        .background(colors::card())
        .child(
            label()
                .text(text)
                .font_size(text_size(13.))
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn footnote() -> impl IntoElement {
    rect()
        .padding(Gaps::new(16., 4., 8., 4.))
        .child(
            label()
                .text(t("oneclient.settings.storage.footnote"))
                .font_size(text_size(12.))
                .color(colors::fg_secondary()),
        )
        .into_element()
}

inventory::submit! {
    SearchItem {
        id: "storage.usage",
        icon: IconType::Database01,
        title: "oneclient.layout.search.storage.usage.title",
        description: "oneclient.layout.search.storage.usage.description",
        keywords: &["storage", "disk", "space", "size", "usage", "cache"],
        route: Route::SettingsStorage {},
    }
}

inventory::submit! {
    SearchItem {
        id: "storage.cleanup",
        icon: IconType::Trash01,
        title: "oneclient.layout.search.storage.cleanup.title",
        description: "oneclient.layout.search.storage.cleanup.description",
        keywords: &[
            "storage", "clean", "cleanup", "free", "space", "cache", "unused", "orphan",
            "leftover",
        ],
        route: Route::SettingsStorage {},
    }
}

#[cfg(test)]
mod hero_tests {
    use super::*;

    #[test]
    fn a_refresh_keeps_the_cards_height() {
        use freya_testing::TestingRunner;
        use std::time::Duration;

        fn app() -> impl IntoElement {
            let scan = use_consume::<State<bool>>();
            let progress = StorageScanProgress {
                label: "Measuring".into(),
                current: 1,
                total: 4,
            };
            let report = StorageReport {
                total_bytes: 44_000_000,
                categories: Vec::new(),
                clusters: Vec::new(),
                unreferenced_cache: Default::default(),
                legacy_cluster_content: Default::default(),
                unused_natives: Default::default(),
            };
            let refresh = rect().width(Size::px(60.)).height(Size::px(32.));
            rect()
                .width(Size::px(400.))
                .child(hero(
                    &report,
                    refresh.into_element(),
                    scan().then_some(&progress),
                ))
                .child(
                    rect()
                        .width(Size::px(10.))
                        .height(Size::px(10.))
                        .a11y_alt("below"),
                )
        }
        let (mut runner, mut scan) = TestingRunner::new(
            app,
            (400., 300.).into(),
            |runner| runner.provide_root_context(|| State::create(false)),
            1.,
        );
        let below = |runner: &TestingRunner| {
            runner
                .find(|node, element| {
                    (element.accessibility().builder.label() == Some("below"))
                        .then(|| node.layout().area.min_y())
                })
                .unwrap()
        };
        runner.poll_n(Duration::ZERO, 3);
        let idle = below(&runner);
        runner.run_in(|| scan.set(true));
        runner.poll_n(Duration::ZERO, 3);
        assert_eq!(below(&runner), idle);
        // A slot kept for the progress used to leave a gap here
        assert!(idle < 110., "{idle}");
    }
}
