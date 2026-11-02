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

use crate::components::{
    Icon, IconType, ScrollArea, ScrollAreaCtx, TextInput, TooltipPlacement, use_tooltip_anchor,
};
use crate::hooks::{latest_changelog_version, use_changelog, use_settings_snapshot};
use crate::i18n::{t, tf};
use crate::routes::Route;
use crate::theme::colors;
use crate::use_dispatch;

const SIDEBAR_WIDTH_PX: f32 = 225.;
const ITEM_HEIGHT_PX: f32 = 33.;
const SEARCH_WIDTH_PX: f32 = 256.;
const SEARCH_MIN_WIDTH_PX: f32 = 120.;
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
struct SearchItem {
    id: &'static str,
    icon: IconType,
    title: &'static str,
    description: &'static str,
    keywords: &'static [&'static str],
    route: Route,
}

// TODO Replace this with inventory crate or something
fn search_index() -> Vec<SearchItem> {
    vec![
        SearchItem {
            id: "launcher.discord_rpc",
            icon: IconType::Link03,
            title: t("oneclient.settings.launcher.discord_rpc.title"),
            description: t("oneclient.settings.launcher.discord_rpc.description"),
            keywords: &["discord", "rpc", "rich presence"],
            route: Route::SettingsLauncher {},
        },
        SearchItem {
            id: "launcher.folder",
            icon: IconType::Folder,
            title: t("oneclient.settings.launcher.data_folder.title"),
            description: t("oneclient.layout.search.launcher.folder.description"),
            keywords: &[
                "data dir",
                "directory",
                "folder",
                "open",
                "move",
                "change",
                "relocate",
                "location",
                "drive",
                "disk",
            ],
            route: Route::SettingsLauncher {},
        },
        SearchItem {
            id: "appearance.accent_color",
            icon: IconType::PaintPour,
            title: t("oneclient.settings.appearance.accent_color.title"),
            description: t("oneclient.layout.search.appearance.accent_color.description"),
            keywords: &["theme", "accent", "color", "brand"],
            route: Route::SettingsAppearance {},
        },
        SearchItem {
            id: "appearance.custom_theme",
            icon: IconType::Colors,
            title: t("oneclient.settings.appearance.custom_theme.title"),
            description: t("oneclient.layout.search.appearance.custom_theme.description"),
            keywords: &["theme", "custom", "import", "export"],
            route: Route::SettingsAppearance {},
        },
        SearchItem {
            id: "appearance.parallax_background",
            icon: IconType::Eye,
            title: t("oneclient.settings.appearance.parallax_background.title"),
            description: t("oneclient.layout.search.appearance.parallax_background.description"),
            keywords: &["background", "parallax", "motion"],
            route: Route::SettingsAppearance {},
        },
        SearchItem {
            id: "appearance.animations",
            icon: IconType::Play,
            title: t("oneclient.settings.appearance.animations.title"),
            description: t("oneclient.layout.search.appearance.animations.description"),
            keywords: &["reduce motion", "animations", "effects"],
            route: Route::SettingsAppearance {},
        },
        SearchItem {
            id: "apis.modrinth_key",
            icon: IconType::Key01,
            title: t("oneclient.layout.search.apis.modrinth_key.title"),
            description: t("oneclient.settings.apis.modrinth_key.description"),
            keywords: &["modrinth", "token", "api key", "pat"],
            route: Route::SettingsApis {},
        },
        SearchItem {
            id: "apis.curseforge_key",
            icon: IconType::Key01,
            title: t("oneclient.layout.search.apis.curseforge_key.title"),
            description: t("oneclient.settings.apis.curseforge_key.description"),
            keywords: &["curseforge", "api key"],
            route: Route::SettingsApis {},
        },
        SearchItem {
            id: "apis.custom_endpoint",
            icon: IconType::Globe01,
            title: t("oneclient.settings.apis.custom_endpoint.title"),
            description: t("oneclient.settings.apis.custom_endpoint.description"),
            keywords: &["endpoint", "api", "backend", "url"],
            route: Route::SettingsApis {},
        },
        SearchItem {
            id: "minecraft.force_fullscreen",
            icon: IconType::Maximize01,
            title: t("oneclient.settings.minecraft.force_fullscreen.title"),
            description: t("oneclient.settings.minecraft.force_fullscreen.description"),
            keywords: &["fullscreen", "window"],
            route: Route::SettingsMinecraft {},
        },
        SearchItem {
            id: "minecraft.resolution",
            icon: IconType::LayoutTop,
            title: t("oneclient.settings.minecraft.resolution.title"),
            description: t("oneclient.settings.minecraft.resolution.description"),
            keywords: &["width", "height", "window size", "resolution"],
            route: Route::SettingsMinecraft {},
        },
        SearchItem {
            id: "minecraft.memory",
            icon: IconType::Database01,
            title: t("oneclient.settings.minecraft.memory.title"),
            description: t("oneclient.layout.search.minecraft.memory.description"),
            keywords: &["ram", "memory", "heap", "xmx"],
            route: Route::SettingsMinecraft {},
        },
        SearchItem {
            id: "minecraft.pre_launch",
            icon: IconType::FilePlus02,
            title: t("oneclient.settings.minecraft.pre_launch.title"),
            description: t("oneclient.settings.minecraft.pre_launch.description"),
            keywords: &["hook", "command", "pre launch", "prelaunch"],
            route: Route::SettingsMinecraft {},
        },
        SearchItem {
            id: "minecraft.wrapper",
            icon: IconType::ParagraphWrap,
            title: t("oneclient.settings.minecraft.wrapper.title"),
            description: t("oneclient.settings.minecraft.wrapper.description"),
            keywords: &["hook", "command", "wrapper", "gamescope"],
            route: Route::SettingsMinecraft {},
        },
        SearchItem {
            id: "minecraft.post_exit",
            icon: IconType::FileX02,
            title: t("oneclient.settings.minecraft.post_exit.title"),
            description: t("oneclient.settings.minecraft.post_exit.description"),
            keywords: &["hook", "command", "post exit", "postexit"],
            route: Route::SettingsMinecraft {},
        },
        // Java routing only the page is not built from settings_row_focus yet
        SearchItem {
            id: "java.install",
            icon: IconType::Download01,
            title: t("oneclient.layout.search.java.install.title"),
            description: t("oneclient.layout.search.java.install.description"),
            keywords: &["java", "runtime", "temurin", "zulu", "install manager"],
            route: Route::SettingsJava {},
        },
        SearchItem {
            id: "java.add_folder",
            icon: IconType::Folder,
            title: t("oneclient.layout.search.java.add_folder.title"),
            description: t("oneclient.layout.search.java.add_folder.description"),
            keywords: &["java", "runtime", "custom", "folder", "path"],
            route: Route::SettingsJava {},
        },
        SearchItem {
            id: "storage.usage",
            icon: IconType::Database01,
            title: t("oneclient.layout.search.storage.usage.title"),
            description: t("oneclient.layout.search.storage.usage.description"),
            keywords: &["storage", "disk", "space", "size", "usage", "cache"],
            route: Route::SettingsStorage {},
        },
        SearchItem {
            id: "storage.cleanup",
            icon: IconType::Trash01,
            title: t("oneclient.layout.search.storage.cleanup.title"),
            description: t("oneclient.layout.search.storage.cleanup.description"),
            keywords: &[
                "storage", "clean", "cleanup", "free", "space", "cache", "unused", "orphan",
                "leftover",
            ],
            route: Route::SettingsStorage {},
        },
        SearchItem {
            id: "language.select",
            icon: IconType::Globe01,
            title: t("oneclient.layout.settings.tab.language"),
            description: t("oneclient.layout.search.language.select.description"),
            keywords: &[
                "language", "locale", "english", "spanish", "french", "german", "russian",
                "japanese", "chinese",
            ],
            route: Route::SettingsLanguage {},
        },
        SearchItem {
            id: "developer.compat_only",
            icon: IconType::SearchMd,
            title: t("oneclient.settings.developer.compat_only.title"),
            description: t("oneclient.layout.search.developer.compat_only.description"),
            keywords: &["browser", "compat", "filter"],
            route: Route::SettingsDeveloper {},
        },
        SearchItem {
            id: "developer.log_debug",
            icon: IconType::Terminal,
            title: t("oneclient.settings.developer.log_debug.title"),
            description: t("oneclient.layout.search.developer.log_debug.description"),
            keywords: &["debug", "logs", "logging"],
            route: Route::SettingsDeveloper {},
        },
        SearchItem {
            id: "developer.show_dev",
            icon: IconType::CodeSnippet02,
            title: t("oneclient.layout.search.developer.show_dev.title"),
            description: t("oneclient.layout.search.developer.show_dev.description"),
            keywords: &["devtools", "developer", "debug"],
            route: Route::SettingsDeveloper {},
        },
        SearchItem {
            id: "developer.debug_page",
            icon: IconType::CodeSnippet02,
            title: t("oneclient.settings.developer.debug_page.title"),
            description: t("oneclient.layout.search.developer.debug_page.description"),
            keywords: &["debug", "page"],
            route: Route::SettingsDeveloper {},
        },
        SearchItem {
            id: "nav.accounts",
            icon: IconType::Users01,
            title: t("oneclient.layout.settings.tab.accounts"),
            description: t("oneclient.layout.search.nav.accounts.description"),
            keywords: &["account", "login", "microsoft", "msa"],
            route: Route::SettingsAccounts {},
        },
        SearchItem {
            id: "nav.changelog",
            icon: IconType::RefreshCcw02,
            title: t("oneclient.layout.settings.tab.changelog"),
            description: t("oneclient.layout.search.nav.changelog.description"),
            keywords: &["changelog", "release notes", "updates"],
            route: Route::SettingsChangelog {},
        },
    ]
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

        let changelog_unread = has_unread_changelog();

        let search = use_state(String::new);
        let header_width = use_state(|| 0f32);
        let query = SearchQuery::new(&search.read());
        let scroll = use_scroll_controller(ScrollConfig::default);
        SETTINGS_SCROLL_CONTROLLER.with(|cell| *cell.borrow_mut() = Some(scroll));

        let anim = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            AnimNum::new(0., 1.)
                .time(340)
                .ease(Ease::Out)
                .function(Function::Cubic)
        });
        let mut last_route = use_state(|| route.clone());
        if *last_route.peek() != route {
            last_route.set(route.clone());
            anim.run(AnimDirection::Forward);
        }
        let p = anim.get().value();

        let children: Vec<Element> = if query.is_empty() {
            const SHIFT_PX: f32 = 46.;
            vec![
                rect()
                    .width(Size::fill())
                    .offset_x(-(1.0 - p) * SHIFT_PX)
                    .opacity(p)
                    .child(Outlet::<Route>::new())
                    .into_element(),
            ]
        } else {
            search_results(&query, search)
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .padding(Gaps::new(0., 40., 40., 32.))
            .spacing(40.)
            .child(sidebar(active, changelog_unread))
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .height(Size::fill())
                    .overflow(Overflow::Clip)
                    .spacing(24.)
                    .child(content_header(
                        active.label().to_string(),
                        search,
                        header_width,
                    ))
                    .child(
                        ScrollArea::new()
                            .width(Size::fill())
                            .height(Size::flex(1.0))
                            .show_scrollbar(true)
                            .scrollbar_gutter(true)
                            .spacing(4.)
                            .scroll_controller(scroll)
                            .on_ctx(|ctx| {
                                SETTINGS_SCROLL_CTX.with(|cell| *cell.borrow_mut() = Some(ctx));
                            })
                            .children(children),
                    ),
            )
    }
}

fn content_header(title: String, search: State<String>, mut width: State<f32>) -> impl IntoElement {
    let search_width = search_width_for(*width.read());

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .on_sized(move |event: Event<SizedEventData>| {
            let next = event.data().area.width();
            if (*width.peek() - next).abs() > 0.5 {
                width.set(next);
            }
        })
        .child(
            label()
                .text(title)
                .font_size(30.)
                .font_weight(FontWeight::SEMI_BOLD)
                .max_lines(1)
                .width(Size::flex(1.0))
                .color(colors::fg_primary()),
        )
        .child(search_box(search, search_width))
}

fn search_width_for(header_width: f32) -> f32 {
    if header_width <= 0. {
        return SEARCH_WIDTH_PX;
    }
    (header_width * 0.45).clamp(SEARCH_MIN_WIDTH_PX, SEARCH_WIDTH_PX)
}

fn search_box(mut search: State<String>, width: f32) -> impl IntoElement {
    let has_query = !search.read().trim().is_empty();
    let base = TextInput::new(search)
        .placeholder(t("oneclient.layout.settings.search.placeholder"))
        .width(Size::px(width))
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
            .corner_radius(CornerRadius::new_all(6.))
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
                .corner_radius(CornerRadius::new_all(12.))
                .background(colors::page_elevated())
                .child(
                    label()
                        .text(t("oneclient.layout.settings.search.empty"))
                        .font_size(12.)
                        .color(colors::fg_secondary()),
                )
                .into_element(),
        ];
    }

    let mut out: Vec<Element> = Vec::with_capacity(matches.len() + 1);
    out.push(
        rect()
            .padding(Gaps::new(16., 0., 8., 2.))
            .child(
                label()
                    .text(t("oneclient.layout.settings.search.results"))
                    .font_size(11.)
                    .font_weight(FontWeight::MEDIUM)
                    .color(colors::fg_secondary()),
            )
            .into_element(),
    );

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
                .padding(Gaps::new_symmetric(12., 16.))
                .corner_radius(CornerRadius::new_all(12.))
                .background(colors::page_elevated())
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
                                .font_size(14.)
                                .font_weight(FontWeight::MEDIUM)
                                .color(colors::fg_primary()),
                        )
                        .child(
                            label()
                                .text(desc)
                                .font_size(11.)
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

fn sidebar(active: SettingsTab, changelog_unread: bool) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::px(SIDEBAR_WIDTH_PX))
        .min_width(Size::px(SIDEBAR_WIDTH_PX))
        .height(Size::fill())
        .overflow(Overflow::Clip)
        .padding(Gaps::new(16., 0., 0., 0.))
        .content(Content::Flex)
        .child(
            ScrollArea::new()
                .width(Size::fill())
                .height(Size::flex(1.0))
                .spacing(16.)
                .children(groups().into_iter().map(|group| {
                    rect()
                        .vertical()
                        .width(Size::fill())
                        .spacing(6.)
                        .child(
                            rect().padding(Gaps::new(0., 0., 2., 6.)).child(
                                label()
                                    .text(group.label)
                                    .font_size(11.)
                                    .font_weight(FontWeight::MEDIUM)
                                    .color(colors::fg_secondary()),
                            ),
                        )
                        .children(group.tabs.iter().map(|tab| {
                            SidebarItem {
                                tab: *tab,
                                active: *tab == active,
                                has_dot: matches!(tab, SettingsTab::Changelog) && changelog_unread,
                            }
                            .into_element()
                        }))
                        .into_element()
                })),
        )
        .child(SidebarInfo::new())
}

#[derive(PartialEq)]
struct SidebarInfo {
    kernel_version: String,
    ram_mb: u64,
    cpu_kind: String,
}

impl SidebarInfo {
    pub fn new() -> Self {
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
            kernel_version,
            ram_mb,
            cpu_kind: cpu_kind.to_string(),
        }
    }
}

impl Component for SidebarInfo {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let tooltip = use_tooltip_anchor(
            Some(t("oneclient.layout.settings.info.copy_tooltip").into()),
            TooltipPlacement::Top,
        );

        let items: [Cow<'static, str>; 4] = [
            Cow::Borrowed(concat!(
                "OneClient v",
                env!("CARGO_PKG_VERSION"),
                cfg_select! {
                    debug_assertions => " (debug)",
                    _ => " (release)"
                }
            )),
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

        tooltip.attach(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(4.)
                .font_size(12.)
                .padding(Gaps::new(15., 0., 0., 0.))
                .color(colors::fg_secondary())
                .children(
                    items
                        .into_iter()
                        .map(|item| label().text(item).into_element()),
                )
                .cursor(CursorIcon::Pointer)
                .on_press(copy_to_clipboard),
        )
    }
}

#[derive(PartialEq)]
struct SidebarItem {
    tab: SettingsTab,
    active: bool,
    has_dot: bool,
}

impl Component for SidebarItem {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);

        let tab = self.tab;
        let active = self.active;
        let route = tab.route();

        let background = if active {
            colors::page_elevated()
        } else if *hovering.read() {
            colors::ghost_overlay()
        } else {
            Color::TRANSPARENT
        };

        let has_dot = self.has_dot;

        let mut el = rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::px(ITEM_HEIGHT_PX))
            .cross_align(Alignment::Center)
            .spacing(10.)
            .padding(Gaps::new_symmetric(0., 12.))
            .corner_radius(CornerRadius::new_all(7.))
            .background(background)
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .cursor(CursorIcon::Pointer)
            .on_pointer_enter(move |_| *hovering.write() = true)
            .on_pointer_leave(move |_| *hovering.write() = false)
            .map(route, |el, route| {
                el.on_press(move |_| {
                    let _ = RouterContext::get().replace(route.clone());
                })
            });

        if focus().is_focused() {
            el = el.border(
                Border::new()
                    .fill(colors::brand())
                    .width(1.)
                    .alignment(BorderAlignment::Inner),
            );
        }

        el.child(Icon::new(tab.icon()).size(18.).color(colors::fg_primary()))
            .child(
                rect()
                    .horizontal()
                    .width(Size::flex(1.0))
                    .cross_align(Alignment::Center)
                    .spacing(6.)
                    .child(
                        label()
                            .text(tab.label())
                            .font_size(14.)
                            .color(colors::fg_primary()),
                    )
                    .maybe_child(has_dot.then(notification_dot)),
            )
    }
}

fn notification_dot() -> impl IntoElement {
    rect()
        .width(Size::px(4.))
        .height(Size::px(4.))
        .corner_radius(CornerRadius::new_all(2.))
        .background(colors::brand())
}
