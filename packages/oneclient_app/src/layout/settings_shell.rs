use std::borrow::Cow;
use std::cell::RefCell;

use freya::animation::*;
use freya::prelude::*;
use freya::router::*;
use freya::text_edit::Clipboard;
use oneclient_common::search::{MatchScore, SearchQuery};
use sysinfo::CpuRefreshKind;
use sysinfo::MemoryRefreshKind;
use sysinfo::RefreshKind;
use sysinfo::System;

use crate::components::{Button, Icon, IconType, ScrollAreaCtx, TextInput, TooltipPlacement};
use crate::hooks::{latest_changelog_version, use_changelog, use_settings_snapshot};
use crate::i18n::{t, tf};
use crate::routes::Route;
use crate::theme::{colors, glass, rounded, text_size};

use super::workspace_dialog::{CONTENT_TOP, WorkspaceLayout, close_button, header_divider};
use super::workspace_rail;
use crate::use_dispatch;

const SEARCH_RESULTS_MAX: usize = 12;

thread_local! {
    static SETTINGS_SCROLL_CTX: RefCell<Option<ScrollAreaCtx>> = const { RefCell::new(None) };
    static SETTINGS_SCROLL_CONTROLLER: RefCell<Option<ScrollController>> = const { RefCell::new(None) };
    static PENDING_SETTING_FOCUS: RefCell<Option<&'static str>> = const { RefCell::new(None) };
}

pub(crate) fn set_pending_setting_focus(id: &'static str) {
    PENDING_SETTING_FOCUS.with(|cell| *cell.borrow_mut() = Some(id));

    spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        PENDING_SETTING_FOCUS.with(|cell| {
            if *cell.borrow() == Some(id) {
                *cell.borrow_mut() = None;
            }
        });
    });
}

#[derive(Clone)]
pub(crate) struct SearchItem {
    pub id: &'static str,
    pub icon: IconType,
    pub title: &'static str,
    pub description: &'static str,
    pub keywords: &'static [&'static str],
    pub route: Route,
}

inventory::collect!(SearchItem);

fn search_index() -> Vec<SearchItem> {
    let mut items: Vec<SearchItem> = inventory::iter::<SearchItem>
        .into_iter()
        .map(|item| SearchItem {
            title: t(item.title),
            description: t(item.description),
            ..item.clone()
        })
        .collect();
    items.sort_by_key(|item| item.id);
    items
}

#[derive(PartialEq, Clone, Copy)]
pub enum SettingsTab {
    MinecraftSettings,
    Accounts,
    LauncherSettings,
    Java,
    Storage,
    Appearance,
    Apis,
    Language,
    DeveloperOptions,
    Changelog,
}

impl SettingsTab {
    fn label(self) -> &'static str {
        match self {
            Self::MinecraftSettings => t("oneclient.layout.settings.tab.minecraft"),
            Self::Accounts => t("oneclient.layout.settings.tab.accounts"),
            Self::LauncherSettings => t("oneclient.layout.settings.tab.launcher"),
            Self::Java => "Java",
            Self::Storage => t("oneclient.layout.settings.tab.storage"),
            Self::Appearance => t("oneclient.layout.settings.tab.appearance"),
            Self::Apis => t("oneclient.layout.settings.tab.apis"),
            Self::Language => t("oneclient.layout.settings.tab.language"),
            Self::DeveloperOptions => t("oneclient.layout.settings.tab.developer"),
            Self::Changelog => t("oneclient.layout.settings.tab.changelog"),
        }
    }

    fn icon(self) -> IconType {
        match self {
            Self::MinecraftSettings => IconType::Sliders04,
            Self::Accounts => IconType::Users01,
            Self::LauncherSettings => IconType::Rocket02,
            Self::Java => IconType::CodeSnippet02,
            Self::Storage => IconType::Database01,
            Self::Appearance => IconType::Brush01,
            Self::Apis => IconType::Key01,
            Self::Language => IconType::Globe01,
            Self::DeveloperOptions => IconType::CodeSnippet02,
            Self::Changelog => IconType::RefreshCcw02,
        }
    }

    fn route(self) -> Option<Route> {
        match self {
            Self::MinecraftSettings => Some(Route::SettingsMinecraft {}),
            Self::Accounts => Some(Route::SettingsAccounts {}),
            Self::LauncherSettings => Some(Route::SettingsLauncher {}),
            Self::Java => Some(Route::SettingsJava {}),
            Self::Storage => Some(Route::SettingsStorage {}),
            Self::Appearance => Some(Route::SettingsAppearance {}),
            Self::Apis => Some(Route::SettingsApis {}),
            Self::Language => Some(Route::SettingsLanguage {}),
            Self::DeveloperOptions => Some(Route::SettingsDeveloper {}),
            Self::Changelog => Some(Route::SettingsChangelog {}),
        }
    }
}

struct SettingsGroup {
    label: &'static str,
    tabs: &'static [SettingsTab],
}

fn groups() -> [SettingsGroup; 3] {
    [
        SettingsGroup {
            label: t("oneclient.layout.settings.group.launcher"),
            tabs: &[
                SettingsTab::LauncherSettings,
                SettingsTab::Java,
                SettingsTab::Storage,
                SettingsTab::Appearance,
                SettingsTab::Apis,
                SettingsTab::Language,
            ],
        },
        SettingsGroup {
            label: t("oneclient.layout.settings.group.game"),
            tabs: &[SettingsTab::MinecraftSettings, SettingsTab::Accounts],
        },
        SettingsGroup {
            label: t("oneclient.layout.settings.group.about"),
            tabs: &[SettingsTab::DeveloperOptions, SettingsTab::Changelog],
        },
    ]
}

#[derive(PartialEq)]
pub struct SettingsShell;

fn route_tab(route: &Route) -> SettingsTab {
    match route {
        Route::SettingsMinecraft {} => SettingsTab::MinecraftSettings,
        Route::SettingsAccounts {} => SettingsTab::Accounts,
        Route::SettingsLauncher {} => SettingsTab::LauncherSettings,
        Route::SettingsJava {} => SettingsTab::Java,
        Route::SettingsStorage {} => SettingsTab::Storage,
        Route::SettingsApis {} => SettingsTab::Apis,
        Route::SettingsLanguage {} => SettingsTab::Language,
        Route::SettingsDeveloper {} => SettingsTab::DeveloperOptions,
        Route::SettingsChangelog {} => SettingsTab::Changelog,
        _ => SettingsTab::Appearance,
    }
}

impl Component for SettingsShell {
    fn render(&self) -> impl IntoElement {
        let route = use_route::<Route>();
        let active = route_tab(&route);
        let layout = WorkspaceLayout::current();

        let changelog_unread = has_unread_changelog();

        let search = use_state(String::new);
        let query = SearchQuery::new(&search.read());
        let searching = !query.is_empty();
        let scroll = use_scroll_controller(ScrollConfig::default);
        SETTINGS_SCROLL_CONTROLLER.with(|cell| *cell.borrow_mut() = Some(scroll));

        let animate = crate::motion::use_animations_enabled();
        let anim = use_animation(|_| {
            AnimNum::new(0., 1.)
                .time(220)
                .ease(Ease::Out)
                .function(Function::Cubic)
        });
        let mut last_route = use_state(|| route.clone());
        if *last_route.peek() != route {
            last_route.set(route.clone());
            if animate {
                anim.run(AnimDirection::Forward);
            }
        }
        // The dialog moves as one piece on open. Only subsequent category changes move the content.
        let p = if animate && *anim.has_run_yet().read() {
            anim.get().value()
        } else {
            1.
        };

        let children: Vec<Element> = if !searching {
            const SHIFT_PX: f32 = 16.;
            vec![
                rect()
                    .width(Size::fill())
                    .offset_y((1.0 - p) * SHIFT_PX)
                    .opacity(p)
                    .child(Outlet::<Route>::new())
                    .into_element(),
            ]
        } else {
            search_results(&query, search)
        };

        let title = if searching {
            t("oneclient.layout.settings.search.results")
        } else {
            active.label()
        };
        rect()
            .vertical()
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .child(
                layout
                    .header()
                    .spacing(layout.padding)
                    .child(
                        rect()
                            .width(Size::px(layout.rail_width))
                            .child(search_box(search)),
                    )
                    .child(
                        label()
                            .text(title)
                            .font_size(text_size(18.))
                            .font_weight(FontWeight::SEMI_BOLD)
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .width(Size::flex(1.))
                            .color(colors::fg_primary()),
                    )
                    .child(close_button()),
            )
            .child(header_divider())
            .child(
                rect()
                    .horizontal()
                    .content(Content::Flex)
                    .width(Size::fill())
                    .height(Size::flex(1.))
                    .padding(Gaps::new(0., 0., 0., layout.padding))
                    .child(sidebar(active, changelog_unread, layout))
                    .child(
                        rect()
                            .width(Size::flex(1.))
                            .height(Size::fill())
                            .padding(Gaps::new(0., 0., layout.padding, 0.))
                            .child(
                                crate::view::app::settings::settings_pane()
                                    .padding(Gaps::new(CONTENT_TOP, layout.padding, 0., 0.))
                                    .spacing(6.)
                                    .reset_key(u64::from(searching))
                                    .scroll_controller(scroll)
                                    .on_ctx(|ctx| {
                                        SETTINGS_SCROLL_CTX
                                            .with(|cell| *cell.borrow_mut() = Some(ctx));
                                    })
                                    .children(children),
                            ),
                    ),
            )
    }
}

/// The same height and text size as the inputs on the settings rows
const SEARCH_HEIGHT: f32 = 34.;

fn search_box(mut search: State<String>) -> impl IntoElement {
    let has_query = !search.read().trim().is_empty();
    let base = TextInput::new(search)
        .placeholder(t("oneclient.layout.settings.search.placeholder"))
        .width(Size::fill())
        .height(Size::px(SEARCH_HEIGHT))
        .leading(
            Icon::new(IconType::SearchMd)
                .size(16.)
                .color(colors::fg_secondary())
                .into_element(),
        );

    if !has_query {
        return base;
    }

    base.trailing(
        rect()
            .padding(Gaps::new_symmetric(2., 2.))
            .corner_radius(rounded(6.))
            .background(Color::TRANSPARENT)
            .cursor(CursorIcon::Pointer)
            .on_press(move |_| search.set(String::new()))
            .child(
                Icon::new(IconType::XClose)
                    .size(16.)
                    .color(colors::fg_secondary()),
            )
            .into_element(),
    )
}

/// A title hit outranks a keyword or description hit however well those scored
fn settings_score(item: &SearchItem, query: &SearchQuery) -> Option<(u8, MatchScore)> {
    if let Some(score) = query.score(item.title) {
        return Some((2, score));
    }
    if let Some(score) = query.best_score(item.keywords.iter().copied()) {
        return Some((1, score));
    }
    query.score(item.description).map(|score| (0, score))
}

fn search_results(query: &SearchQuery, mut search: State<String>) -> Vec<Element> {
    let mut scored: Vec<(u8, MatchScore, SearchItem)> = search_index()
        .iter()
        .filter_map(|item| settings_score(item, query).map(|(f, s)| (f, s, item.clone())))
        .collect();

    // Shortest title last it is the entry closest to being just the query
    scored.sort_by_key(|(field, score, item)| {
        (
            std::cmp::Reverse(*field),
            std::cmp::Reverse(*score),
            item.title.len(),
        )
    });
    let mut matches: Vec<SearchItem> = scored.into_iter().map(|(_, _, item)| item).collect();
    matches.truncate(SEARCH_RESULTS_MAX);

    if matches.is_empty() {
        return vec![
            rect()
                .width(Size::fill())
                .padding(Gaps::new_symmetric(10., 14.))
                .corner_radius(rounded(12.))
                .background(colors::card())
                .child(
                    label()
                        .text(t("oneclient.layout.settings.search.empty"))
                        .font_size(text_size(12.))
                        .color(colors::fg_secondary()),
                )
                .into_element(),
        ];
    }

    let mut out = Vec::with_capacity(matches.len());

    for item in matches {
        let route = item.route;
        let id = item.id;
        let title = item.title;
        let desc = item.description;
        out.push(
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::Flex)
                .cross_align(Alignment::Center)
                .spacing(12.)
                .min_height(Size::px(64.))
                .padding(Gaps::new_symmetric(14., 16.))
                .corner_radius(rounded(12.))
                .background(colors::card())
                .cursor(CursorIcon::Pointer)
                .on_press(move |_| {
                    set_pending_setting_focus(id);
                    search.set(String::new());
                    let _ = RouterContext::get().replace(route.clone());
                })
                .child(Icon::new(item.icon))
                .child(
                    rect()
                        .vertical()
                        .width(Size::flex(1.0))
                        .spacing(2.)
                        .child(
                            label()
                                .text(title)
                                .font_size(text_size(14.))
                                .font_weight(FontWeight::MEDIUM)
                                .color(colors::fg_primary()),
                        )
                        .child(
                            label()
                                .text(desc)
                                .font_size(text_size(11.))
                                .max_lines(2)
                                .color(colors::fg_secondary()),
                        ),
                )
                .child(
                    Icon::new(IconType::ChevronRight)
                        .size(18.)
                        .color(colors::fg_secondary()),
                )
                .into_element(),
        );
    }
    out
}

/// False while the changelog is loading or failed rather than guessing
fn has_unread_changelog() -> bool {
    let settings = use_settings_snapshot().settings;
    let query = use_changelog();

    match latest_changelog_version(&query) {
        Some(latest) => settings.seen_changelog_version.as_deref() != Some(latest.as_str()),
        None => false,
    }
}

fn sidebar(
    active: SettingsTab,
    changelog_unread: bool,
    layout: WorkspaceLayout,
) -> impl IntoElement {
    let groups = groups();
    let tabs: Vec<_> = groups
        .iter()
        .flat_map(|group| group.tabs.iter().copied())
        .collect();
    let track = glass::use_vertical_pill_track(tabs.len());
    let lens = colors::active_theme().liquid_glass.then_some(track);
    let selected = tabs.iter().position(|tab| *tab == active);
    let mut available = use_state(|| 0f32);
    let sections: Vec<_> = groups.iter().map(|group| group.tabs.len()).collect();
    let measured = *available.read();
    let viewport = if measured > 0. {
        Size::px(workspace_rail::viewport_height(
            measured,
            &sections,
            layout.row_height,
            layout.row_gap,
        ))
    } else {
        Size::fill()
    };

    let nav = rect()
        .vertical()
        .width(Size::fill())
        .maybe_child(lens.map(|track| track.indicator(selected)))
        .children(groups.into_iter().scan(0, move |index, group| {
            let rows = group.tabs.iter().filter_map(|tab| {
                let route = tab.route()?;
                let tab_row = workspace_rail::Tab {
                    label: tab.label(),
                    icon: tab.icon(),
                    route,
                    active: *tab == active,
                    has_dot: *tab == SettingsTab::Changelog && changelog_unread,
                    height: layout.row_height,
                    lens: lens.map(|track| (track, *index)),
                };
                *index += 1;
                Some(tab_row.into_element())
            });
            Some(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .child(workspace_rail::heading(group.label))
                    .child(
                        rect()
                            .vertical()
                            .width(Size::fill())
                            .spacing(layout.row_gap)
                            .children(rows),
                    )
                    .into_element(),
            )
        }));

    workspace_rail::column(layout)
        // The footer's ghost button carries its own inset, so the rail ends half a gutter lower
        .padding(Gaps::new(0., 0., layout.padding / 2., 0.))
        .map(lens, |el, track| {
            track.events(el, move |index| {
                if Some(index) != selected
                    && let Some(route) = tabs[index].route()
                {
                    workspace_rail::select(&route);
                }
            })
        })
        .child(
            rect()
                .width(Size::fill())
                .height(Size::flex(1.))
                .on_sized(move |event: Event<SizedEventData>| {
                    let height = event.area.height();
                    if (*available.peek() - height).abs() > 0.5 {
                        available.set(height);
                    }
                })
                .child(
                    workspace_rail::scroll(lens)
                        .height(viewport)
                        .child(workspace_rail::navigation(layout).child(nav)),
                ),
        )
        .child(
            workspace_rail::footer(layout)
                .child(workspace_rail::divider())
                .child(SidebarInfo::new(layout.row_height)),
        )
}

#[derive(PartialEq)]
struct SidebarInfo {
    height: f32,
    kernel_version: String,
    ram_mb: u64,
    cpu_kind: String,
}

impl SidebarInfo {
    fn new(height: f32) -> Self {
        let system = System::new_with_specifics(
            RefreshKind::nothing()
                .with_memory(MemoryRefreshKind::nothing().with_ram())
                .with_cpu(CpuRefreshKind::nothing()),
        );

        let kernel_version = System::kernel_long_version();
        let ram_mb = system.total_memory() / 1024 / 1024;
        let cpu_kind = system
            .cpus()
            .first()
            .map(|cpu| cpu.brand())
            .unwrap_or_default();

        Self {
            height,
            kernel_version,
            ram_mb,
            cpu_kind: cpu_kind.to_string(),
        }
    }
}

impl Component for SidebarInfo {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let copy_label = t("oneclient.layout.settings.info.copy_tooltip");
        let items: [Cow<'static, str>; 4] = [
            Cow::Borrowed(oneclient_common::constants::LAUNCHER_LABEL),
            Cow::Owned(self.kernel_version.clone()),
            Cow::Owned(self.cpu_kind.clone()),
            Cow::Owned(tf("oneclient.layout.settings.info.ram", &[&self.ram_mb])),
        ];

        let text = items
            .iter()
            .map(|s| s.as_ref())
            .collect::<Vec<_>>()
            .join("\n");

        let copy_to_clipboard = move |_| {
            if let Err(err) = Clipboard::set(text.clone()) {
                tracing::warn!("clipboard copy failed: {err:?}");
                dispatch
                    .notify(t("oneclient.layout.settings.info.copy_failed.title"))
                    .body(t("oneclient.layout.settings.info.copy_failed.body"))
                    .error()
                    .toast_only()
                    .send();
            } else {
                dispatch
                    .notify(t("oneclient.layout.settings.info.copied.title"))
                    .body(t("oneclient.layout.settings.info.copied.body"))
                    .info()
                    .icon(IconType::ClipboardCheck)
                    .toast_only()
                    .send();
            }
        };

        Button::new()
            .small()
            .ghost()
            .height(Size::px(self.height))
            .width(Size::fill())
            .foreground(crate::view::app::settings::readable_secondary())
            .alt(copy_label)
            .tooltip(copy_label)
            .tooltip_placement(TooltipPlacement::Top)
            .child(
                label()
                    .text(oneclient_common::constants::LAUNCHER_LABEL)
                    .width(Size::flex(1.))
                    .font_size(text_size(11.))
                    .max_lines(1)
                    .text_overflow(TextOverflow::Ellipsis),
            )
            .child(Icon::new(IconType::Copy01).size(13.))
            .on_press(copy_to_clipboard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_entries_have_unique_ids_and_known_keys() {
        let mut ids = std::collections::HashSet::new();
        for item in inventory::iter::<SearchItem> {
            assert!(ids.insert(item.id), "duplicate search id {}", item.id);
            for key in [item.title, item.description] {
                assert!(
                    crate::i18n::EN_US.contains_key(key),
                    "{} uses missing key {key}",
                    item.id
                );
            }
        }
        assert!(!ids.is_empty());
    }
}
