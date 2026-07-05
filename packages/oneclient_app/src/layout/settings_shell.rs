use std::borrow::Cow;
use std::cell::RefCell;

use freya::animation::*;
use freya::prelude::*;
use freya::router::*;
use freya::text_edit::Clipboard;
use sysinfo::CpuRefreshKind;
use sysinfo::MemoryRefreshKind;
use sysinfo::RefreshKind;
use sysinfo::System;

use crate::components::{Icon, IconType, ScrollArea, ScrollAreaCtx, TextInput};
use crate::routes::Route;
use crate::theme::colors;
use crate::use_dispatch;

const SIDEBAR_WIDTH_PX: f32 = 225.;
const ITEM_HEIGHT_PX: f32 = 33.;
const SEARCH_WIDTH_PX: f32 = 256.;
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

// TODO: Replace this with inventory crate or something
const SEARCH_INDEX: &[SearchItem] = &[
    // Launcher
    SearchItem {
        id: "launcher.discord_rpc",
        icon: IconType::Link03,
        title: "Discord RPC",
        description: "Enable Discord Rich Presence.",
        keywords: &["discord", "rpc", "rich presence"],
        route: Route::SettingsLauncher {},
    },
    SearchItem {
        id: "launcher.folder",
        icon: IconType::Folder,
        title: "Launcher Folder",
        description: "Open the launcher data directory.",
        keywords: &["data dir", "directory", "folder", "open"],
        route: Route::SettingsLauncher {},
    },
    // Appearance
    SearchItem {
        id: "appearance.accent_color",
        icon: IconType::PaintPour,
        title: "Accent color",
        description: "The main color used across the launcher.",
        keywords: &["theme", "accent", "color", "brand"],
        route: Route::SettingsAppearance {},
    },
    SearchItem {
        id: "appearance.custom_theme",
        icon: IconType::Colors,
        title: "Custom theme",
        description: "Create, edit, and import launcher themes.",
        keywords: &["theme", "custom", "import", "export"],
        route: Route::SettingsAppearance {},
    },
    SearchItem {
        id: "appearance.parallax_background",
        icon: IconType::Eye,
        title: "Parallax background",
        description: "Make the home screen background drift with your cursor.",
        keywords: &["background", "parallax", "motion"],
        route: Route::SettingsAppearance {},
    },
    SearchItem {
        id: "appearance.animations",
        icon: IconType::Play,
        title: "Animations",
        description: "Toggle all animations in the launcher.",
        keywords: &["reduce motion", "animations", "effects"],
        route: Route::SettingsAppearance {},
    },
    // APIs
    SearchItem {
        id: "apis.modrinth_key",
        icon: IconType::Key01,
        title: "Modrinth API key",
        description: "Personal access token used for Modrinth requests.",
        keywords: &["modrinth", "token", "api key", "pat"],
        route: Route::SettingsApis {},
    },
    SearchItem {
        id: "apis.curseforge_key",
        icon: IconType::Key01,
        title: "CurseForge API key",
        description: "API key used for CurseForge requests.",
        keywords: &["curseforge", "api key"],
        route: Route::SettingsApis {},
    },
    SearchItem {
        id: "apis.custom_endpoint",
        icon: IconType::Globe01,
        title: "Custom API Endpoint",
        description: "Override the default OneClient backend endpoint.",
        keywords: &["endpoint", "api", "backend", "url"],
        route: Route::SettingsApis {},
    },
    // Minecraft settings
    SearchItem {
        id: "minecraft.force_fullscreen",
        icon: IconType::Maximize01,
        title: "Force Fullscreen",
        description: "Force Minecraft to start in fullscreen mode.",
        keywords: &["fullscreen", "window"],
        route: Route::SettingsMinecraft {},
    },
    SearchItem {
        id: "minecraft.resolution",
        icon: IconType::LayoutTop,
        title: "Resolution",
        description: "The game window resolution in pixels.",
        keywords: &["width", "height", "window size", "resolution"],
        route: Route::SettingsMinecraft {},
    },
    SearchItem {
        id: "minecraft.memory",
        icon: IconType::Database01,
        title: "Memory",
        description: "The amount of memory allocated for the game.",
        keywords: &["ram", "memory", "heap", "xmx"],
        route: Route::SettingsMinecraft {},
    },
    SearchItem {
        id: "minecraft.pre_launch",
        icon: IconType::FilePlus02,
        title: "Pre-Launch Command",
        description: "Command to run before launching the game.",
        keywords: &["hook", "command", "pre launch", "prelaunch"],
        route: Route::SettingsMinecraft {},
    },
    SearchItem {
        id: "minecraft.wrapper",
        icon: IconType::ParagraphWrap,
        title: "Wrapper Command",
        description: "Command to run when launching the game.",
        keywords: &["hook", "command", "wrapper", "gamescope"],
        route: Route::SettingsMinecraft {},
    },
    SearchItem {
        id: "minecraft.post_exit",
        icon: IconType::FileX02,
        title: "Post-Exit Command",
        description: "Command to run after exiting the game.",
        keywords: &["hook", "command", "post exit", "postexit"],
        route: Route::SettingsMinecraft {},
    },
    // Java (routing only, page isn't built from settings_row_focus yet)
    SearchItem {
        id: "java.install",
        icon: IconType::Download01,
        title: "Install Java",
        description: "Install a Java runtime from a provider.",
        keywords: &["java", "runtime", "temurin", "zulu", "install manager"],
        route: Route::SettingsJava {},
    },
    SearchItem {
        id: "java.add_folder",
        icon: IconType::Folder,
        title: "Add Java from folder",
        description: "Add an existing Java installation.",
        keywords: &["java", "runtime", "custom", "folder", "path"],
        route: Route::SettingsJava {},
    },
    // Language
    SearchItem {
        id: "language.select",
        icon: IconType::Globe01,
        title: "Language",
        description: "Choose the launcher language.",
        keywords: &["language", "locale", "english", "spanish", "french", "german", "russian", "japanese", "chinese"],
        route: Route::SettingsLanguage {},
    },
    // Developer
    SearchItem {
        id: "developer.compat_only",
        icon: IconType::SearchMd,
        title: "Compatible content only",
        description: "Filter the content browser to the active cluster.",
        keywords: &["browser", "compat", "filter"],
        route: Route::SettingsDeveloper {},
    },
    SearchItem {
        id: "developer.log_debug",
        icon: IconType::Terminal,
        title: "Log Debug Info",
        description: "Enable extra debug logging (requires restart).",
        keywords: &["debug", "logs", "logging"],
        route: Route::SettingsDeveloper {},
    },
    SearchItem {
        id: "developer.show_dev",
        icon: IconType::CodeSnippet02,
        title: "Show Dev stuff",
        description: "Enable dev tools and show the debug page.",
        keywords: &["devtools", "developer", "debug"],
        route: Route::SettingsDeveloper {},
    },
    SearchItem {
        id: "developer.debug_page",
        icon: IconType::CodeSnippet02,
        title: "Debug Page",
        description: "Open the debug page.",
        keywords: &["debug", "page"],
        route: Route::SettingsDeveloper {},
    },
    // Navigation-level items
    SearchItem {
        id: "nav.accounts",
        icon: IconType::Users01,
        title: "Accounts",
        description: "Manage Minecraft / Microsoft accounts.",
        keywords: &["account", "login", "microsoft", "msa"],
        route: Route::Accounts {},
    },
    SearchItem {
        id: "nav.changelog",
        icon: IconType::RefreshCcw02,
        title: "Changelog",
        description: "View what's new in OneClient.",
        keywords: &["changelog", "release notes", "updates"],
        route: Route::SettingsChangelog {},
    },
];

#[derive(PartialEq, Clone, Copy)]
pub enum SettingsTab {
    MinecraftSettings,
    Accounts,
    LauncherSettings,
    Java,
    Appearance,
    Apis,
    Language,
    DeveloperOptions,
    Changelog,
}

impl SettingsTab {
    fn label(self) -> &'static str {
        match self {
            Self::MinecraftSettings => "Minecraft settings",
            Self::Accounts => "Accounts",
            Self::LauncherSettings => "Launcher settings",
            Self::Java => "Java",
            Self::Appearance => "Appearance",
            Self::Apis => "APIs",
            Self::Language => "Language",
            Self::DeveloperOptions => "Developer options",
            Self::Changelog => "Changelog",
        }
    }

    fn icon(self) -> IconType {
        match self {
            Self::MinecraftSettings => IconType::Sliders04,
            Self::Accounts => IconType::Users01,
            Self::LauncherSettings => IconType::Rocket02,
            Self::Java => IconType::CodeSnippet02,
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
            Self::Accounts => Some(Route::Accounts {}),
            Self::LauncherSettings => Some(Route::SettingsLauncher {}),
            Self::Java => Some(Route::SettingsJava {}),
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

const GROUPS: &[SettingsGroup] = &[
    SettingsGroup {
        label: "LAUNCHER SETTINGS",
        tabs: &[
            SettingsTab::LauncherSettings,
            SettingsTab::Java,
            SettingsTab::Appearance,
            SettingsTab::Apis,
            SettingsTab::Language,
        ],
    },
    SettingsGroup {
        label: "GAME SETTINGS",
        tabs: &[SettingsTab::MinecraftSettings, SettingsTab::Accounts],
    },
    SettingsGroup {
        label: "ABOUT",
        tabs: &[
            SettingsTab::DeveloperOptions,
            SettingsTab::Changelog,
        ],
    },
];

#[derive(PartialEq)]
pub struct SettingsShell;

fn route_tab(route: &Route) -> SettingsTab {
    match route {
        Route::SettingsMinecraft {} => SettingsTab::MinecraftSettings,
        Route::SettingsLauncher {} => SettingsTab::LauncherSettings,
        Route::SettingsJava {} => SettingsTab::Java,
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

        let search = use_state(String::new);
        let query = search.read().trim().to_string();
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
            search_results(query, search)
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .padding(Gaps::new(0., 40., 40., 32.))
            .spacing(40.)
            .child(sidebar(active))
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .height(Size::fill())
                    .overflow(Overflow::Clip)
                    .spacing(24.)
                    .child(content_header(active.label().to_string(), search))
                    .child(
                        ScrollArea::new()
                            .width(Size::fill())
                            .height(Size::flex(1.0))
                            .show_scrollbar(true)
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

fn content_header(title: String, search: State<String>) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .main_align(Alignment::SpaceBetween)
        .child(
            label()
                .text(title)
                .font_size(30.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        )
        .child(search_box(search))
}

fn search_box(mut search: State<String>) -> impl IntoElement {
    let has_query = !search.read().trim().is_empty();
    let base = TextInput::new(search)
        .placeholder("Search settings...")
        .width(Size::px(SEARCH_WIDTH_PX))
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
            .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
            .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
            .on_press(move |_| search.set(String::new()))
            .child(
                Icon::new(IconType::XClose)
                    .size(16.)
                    .color(colors::fg_secondary()),
            )
            .into_element(),
    )
}

fn search_results(query: String, mut search: State<String>) -> Vec<Element> {
    let q = query.to_lowercase();
    let mut matches: Vec<SearchItem> = SEARCH_INDEX
        .iter()
        .filter(|&item| {
            item.title.to_lowercase().contains(&q)
                || item.description.to_lowercase().contains(&q)
                || item.keywords.iter().any(|k| k.to_lowercase().contains(&q))
        })
        .cloned()
        .collect();

    matches.sort_by_key(|m| m.title.len());
    matches.truncate(SEARCH_RESULTS_MAX);

    if matches.is_empty() {
        return vec![rect()
            .width(Size::fill())
            .padding(Gaps::new_symmetric(10., 14.))
            .corner_radius(CornerRadius::new_all(12.))
            .background(colors::page_elevated())
            .child(
                label()
                    .text("No settings matched your search.")
                    .font_size(12.)
                    .color(colors::fg_secondary()),
            )
            .into_element()];
    }

    let mut out: Vec<Element> = Vec::with_capacity(matches.len() + 1);
    out.push(
        rect()
            .padding(Gaps::new(16., 0., 8., 2.))
            .child(
                label()
                    .text("SEARCH RESULTS")
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
                .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
                .on_press(move |_| {
                    set_pending_setting_focus(id);
                    search.set(String::new());
                    let _ = RouterContext::get().push(route.clone());
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

fn sidebar(active: SettingsTab) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::px(SIDEBAR_WIDTH_PX))
        .min_width(Size::px(SIDEBAR_WIDTH_PX))
        .height(Size::fill())
        .overflow(Overflow::Clip)
        .padding(Gaps::new(16., 0., 0., 0.))
        .content(Content::Flex)
        .child(
            rect()
                .width(Size::fill())
                .height(Size::flex(1.0))
                .spacing(16.)
                .children(GROUPS.iter().map(|group| {
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
            Cow::Owned(format!("RAM Total of {} MB", self.ram_mb)),
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
                    .notify("Copy failed")
                    .body("Could not copy system information to the clipboard.")
                    .error()
                    .send();
            } else {
                dispatch
                    .notify("Copied to clipboard")
                    .body("System information copied to your clipboard.")
                    .info()
                    .icon(IconType::ClipboardCheck)
                    .send();
            }
        };

        use_drop(|| {
            Cursor::set(CursorIcon::default());
        });

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(4.)
            .font_size(12.)
            .color(colors::fg_secondary())
            .children(items.into_iter().map(|item| {
                label()
                    .text(item)
                    .into_element()
            }))
            .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
            .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
            .on_press(copy_to_clipboard)
    }
}

#[derive(PartialEq)]
struct SidebarItem {
    tab: SettingsTab,
    active: bool,
}

impl Component for SidebarItem {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);

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

        let has_dot = matches!(tab, SettingsTab::Changelog);

        rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::px(ITEM_HEIGHT_PX))
            .cross_align(Alignment::Center)
            .spacing(10.)
            .padding(Gaps::new_symmetric(0., 12.))
            .corner_radius(CornerRadius::new_all(7.))
            .background(background)
            .on_pointer_enter(move |_| {
                *hovering.write() = true;
                Cursor::set(CursorIcon::Pointer);
            })
            .on_pointer_leave(move |_| {
                *hovering.write() = false;
                Cursor::set(CursorIcon::default());
            })
            .map(route, |el, route| {
                el.on_press(move |_| {
                    let _ = RouterContext::get().push(route.clone());
                })
            })
            .child(Icon::new(tab.icon()).size(18.).color(colors::fg_primary()))
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
