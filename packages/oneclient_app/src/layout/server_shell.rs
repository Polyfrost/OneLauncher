use freya::prelude::*;
use freya::router::*;
use freya::text_edit::Clipboard;

use crate::components::{Button, Icon, IconType, TabBar, TabItem};
use crate::hooks::{
    settled_or_loading, use_dispatch, use_server_files, use_server_hub, use_servers,
};
use crate::i18n::{t, tf};
use crate::routes::Route;
use crate::servers::{self, ServerStatus};
use crate::theme::{colors, text_size};
use crate::view::app::servers::{IconSeeder, fmt_uptime, server_icon, status_line};

const SIDE_PADDING: f32 = 40.;
const ACTION_SIZE: f32 = 40.;
const ICON_SIZE: f32 = 20.;

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Overview,
    Console,
    Players,
    Worlds,
    Backups,
    Mods,
    Settings,
    Live,
}

impl Tab {
    const ALL: [Tab; 8] = [
        Tab::Overview,
        Tab::Console,
        Tab::Players,
        Tab::Worlds,
        Tab::Backups,
        Tab::Mods,
        Tab::Settings,
        Tab::Live,
    ];

    fn label(self) -> &'static str {
        match self {
            Tab::Overview => t("oneclient.servers.tab.overview"),
            Tab::Console => t("oneclient.servers.tab.console"),
            Tab::Players => t("oneclient.servers.tab.players"),
            Tab::Worlds => t("oneclient.servers.tab.worlds"),
            Tab::Backups => t("oneclient.servers.tab.backups"),
            Tab::Mods => t("oneclient.servers.tab.mods"),
            Tab::Settings => t("oneclient.servers.tab.settings"),
            Tab::Live => t("oneclient.servers.tab.live"),
        }
    }

    fn route(self, server_id: String) -> Route {
        match self {
            Tab::Overview => Route::ServerOverview { server_id },
            Tab::Console => Route::ServerConsole { server_id },
            Tab::Players => Route::ServerPlayers { server_id },
            Tab::Worlds => Route::ServerWorlds { server_id },
            Tab::Backups => Route::ServerBackups { server_id },
            Tab::Mods => Route::ServerMods { server_id },
            Tab::Settings => Route::ServerSettings { server_id },
            Tab::Live => Route::ServerLiveConsole { server_id },
        }
    }
}

fn route_tab(route: &Route) -> Option<(String, Tab)> {
    Some(match route {
        Route::ServerOverview { server_id } => (server_id.clone(), Tab::Overview),
        Route::ServerConsole { server_id } => (server_id.clone(), Tab::Console),
        Route::ServerPlayers { server_id } => (server_id.clone(), Tab::Players),
        Route::ServerWorlds { server_id } => (server_id.clone(), Tab::Worlds),
        Route::ServerBackups { server_id } => (server_id.clone(), Tab::Backups),
        Route::ServerMods { server_id } => (server_id.clone(), Tab::Mods),
        Route::ServerSettings { server_id } => (server_id.clone(), Tab::Settings),
        Route::ServerLiveConsole { server_id } => (server_id.clone(), Tab::Live),
        _ => return None,
    })
}

fn server_tab_index(route: &Route) -> Option<i32> {
    route_tab(route).and_then(|(_, tab)| Tab::ALL.iter().position(|t| *t == tab))
        .map(|i| i as i32)
}

pub fn server_id_of(route: &Route) -> Option<String> {
    route_tab(route).map(|(id, _)| id)
}

#[derive(PartialEq)]
pub struct ServerShell;

impl Component for ServerShell {
    fn render(&self) -> impl IntoElement {
        let route = use_route::<Route>();
        let (server_id, active) = route_tab(&route).unwrap_or((String::new(), Tab::Overview));
        let list = settled_or_loading(&use_servers()).unwrap_or_default();
        let files = settled_or_loading(&use_server_files(server_id.clone()));
        let hub = use_server_hub();
        let live = hub.get(&server_id);

        let seed_icon = list
            .iter()
            .find(|s| s.id == server_id)
            .filter(|_| files.as_ref().is_some_and(|f| f.icon.is_none()))
            .map(|server| IconSeeder { server: server.clone() }.into_element());

        let header = list.iter().find(|s| s.id == server_id).map(|server| {
            let port = files
                .as_ref()
                .map(|f| f.props.get_or("server-port", "25565"))
                .unwrap_or_else(|| "25565".into());
            let mut status = status_line(live.status, server);
            if let Some(started) = live
                .started
                .filter(|_| live.status == ServerStatus::Running)
            {
                status = format!(
                    "{status} \u{b7} {}",
                    tf("oneclient.servers.up", &[&fmt_uptime(started)])
                );
            }
            header(
                server_id.clone(),
                server.name.clone(),
                status,
                live.status,
                server_icon(server, files.as_ref().and_then(|f| f.icon.clone()), 56.),
                server.dir().ok(),
                port,
            )
        });

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .padding(Gaps::new(16., SIDE_PADDING, 16., SIDE_PADDING))
                    .spacing(16.)
                    .maybe_child(header)
                    .child(tabs(active, server_id)),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .overflow(Overflow::Clip)
                    .padding(Gaps::new(0., SIDE_PADDING, SIDE_PADDING, SIDE_PADDING))
                    .child(AnimatedRouter::<Route>::new(super::TabContentOutlet {
                        tab_index: server_tab_index,
                    })),
            )
            .maybe_child(seed_icon)
    }
}

fn header(
    id: String,
    name: String,
    status: String,
    state: ServerStatus,
    icon: Element,
    folder: Option<std::path::PathBuf>,
    port: String,
) -> Element {
    let dispatch = use_dispatch();
    let station = dispatch.station();
    let live = state.is_live();
    let (start_label, enabled) = match state {
        ServerStatus::Stopped => (t("oneclient.servers.start"), true),
        ServerStatus::Installing => (t("oneclient.servers.status.installing"), false),
        ServerStatus::Starting => (t("oneclient.servers.status.starting"), false),
        ServerStatus::Running => (t("oneclient.servers.status.running"), false),
        ServerStatus::Stopping => (t("oneclient.servers.status.stopping"), false),
    };
    let (start_id, stop_id, restart_id) = (id.clone(), id.clone(), id);
    let address = format!(
        "{}:{port}",
        oneclient_core::servers::lan_ip()
            .map_or_else(|| "localhost".to_string(), |ip| ip.to_string())
    );

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(14.)
        .child(icon)
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(4.)
                .child(
                    label()
                        .text(name)
                        .font_size(text_size(28.))
                        .font_weight(FontWeight::BOLD)
                        .max_lines(1)
                        .color(colors::fg_primary()),
                )
                .child(
                    rect()
                        .horizontal()
                        .cross_align(Alignment::Center)
                        .spacing(6.)
                        .child(
                            rect()
                                .width(Size::px(7.))
                                .height(Size::px(7.))
                                .corner_radius(crate::theme::rounded(99.))
                                .background(if state == ServerStatus::Running {
                                    colors::success()
                                } else {
                                    colors::fg_secondary()
                                }),
                        )
                        .child(
                            label()
                                .text(status)
                                .font_size(text_size(12.))
                                .font_weight(FontWeight::MEDIUM)
                                .color(colors::fg_secondary()),
                        ),
                ),
        )
        .child(
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(10.)
                .maybe_child(folder.map(|folder| {
                    crate::components::sized_open_folder_button(folder, ICON_SIZE)
                        .width(Size::px(ACTION_SIZE))
                        .height(Size::px(ACTION_SIZE))
                }))
                .child(
                    Button::new()
                        .secondary()
                        .icon()
                        .width(Size::px(ACTION_SIZE))
                        .height(Size::px(ACTION_SIZE))
                        .tooltip(t("oneclient.servers.copy_address"))
                        .on_press({
                            let dispatch = dispatch.clone();
                            move |_| {
                                if Clipboard::set(address.clone()).is_ok() {
                                    dispatch
                                        .notify(t("oneclient.servers.copied"))
                                        .body(address.clone())
                                        .toast_only()
                                        .send();
                                }
                            }
                        })
                        .child(Icon::new(IconType::Share01).size(ICON_SIZE)),
                )
                .child(
                    Button::new()
                        .secondary()
                        .icon()
                        .width(Size::px(ACTION_SIZE))
                        .height(Size::px(ACTION_SIZE))
                        .tooltip(t("oneclient.servers.restart"))
                        .enabled(state != ServerStatus::Installing)
                        .on_press(move |_| servers::restart(station, restart_id.clone()))
                        .child(Icon::new(IconType::RefreshCw01).size(ICON_SIZE)),
                )
                .maybe(live, |el| {
                    el.child(
                        Button::new()
                            .danger()
                            .icon()
                            .width(Size::px(ACTION_SIZE))
                            .height(Size::px(ACTION_SIZE))
                            .tooltip(t("oneclient.servers.stop"))
                            .on_press(move |_| servers::stop(station, stop_id.clone()))
                            .child(Icon::new(IconType::Square).size(ICON_SIZE)),
                    )
                })
                .child(
                    Button::new()
                        .primary()
                        .large()
                        .width(Size::px(180.))
                        .height(Size::px(ACTION_SIZE))
                        .enabled(enabled)
                        .on_press(move |_| {
                            if enabled {
                                servers::start(station, start_id.clone());
                            }
                        })
                        .text(start_label),
                ),
        )
        .into_element()
}

fn tabs(active: Tab, server_id: String) -> impl IntoElement {
    let items = Tab::ALL.into_iter().map(|tab| {
        let route = tab.route(server_id.clone());
        let mut item = TabItem::new(tab.label(), tab == active).on_press(move |_| {
            let _ = RouterContext::get().push(route.clone());
        });
        if tab == Tab::Live {
            item = item.accent(colors::brand());
        }
        item
    });

    rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::px(38.))
        .cross_align(Alignment::Center)
        .border(
            Border::new()
                .fill(colors::component_border())
                .width(BorderWidth {
                    top: 0.,
                    right: 0.,
                    bottom: 1.,
                    left: 0.,
                }),
        )
        .child(
            TabBar::new()
                .width(Size::auto())
                .height(Size::fill())
                .spacing(22.)
                .font_size(text_size(13.))
                .tabs(items),
        )
}
