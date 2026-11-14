use std::time::Instant;

use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::servers::LocalServer;

use super::create::CreateServerModal;
use super::{DASH, fmt_bytes, fmt_uptime, loader_label, server_icon, status_label, version_art};
use crate::Route;
use crate::components::{ART_PREVIEW_EDGE, Avatar, Button, Icon, IconType, ScrollArea};
use crate::hooks::{
    settled_or_loading, use_dispatch, use_server_files, use_server_hub, use_servers,
};
use crate::i18n::t;
use crate::servers::{self, ServerLive, ServerStatus};
use crate::theme::{colors, rounded, text_size};
use crate::ui::{border_all_color, centered_note, fixed_grid, grid_columns_for_width};

const CARD_W: f32 = 340.;
const CARD_H: f32 = 384.;
const GAP: f32 = 16.;
const BANNER_H: f32 = 104.;
const ICON: f32 = 60.;
const ICON_RING: f32 = 3.;
const HEADS: usize = 4;
const MOTD_FONT: f32 = 20.;

#[derive(PartialEq)]
pub struct Servers;

impl Component for Servers {
    fn render(&self) -> impl IntoElement {
        let mut creating = use_state(|| false);
        let list = settled_or_loading(&use_servers()).unwrap_or_default();
        let hub = use_server_hub();
        let mut width = use_state(|| 0f32);

        let cards: Vec<Element> = list
            .iter()
            .map(|server| {
                ServerCard {
                    server: server.clone(),
                    live: CardLive::from(&hub.get(&server.id)),
                }
                .into_element()
            })
            .collect();
        let columns = grid_columns_for_width(*width.read(), CARD_W, GAP);

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .padding(Gaps::new(12., 40., 28., 40.))
            .spacing(18.)
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .content(Content::Flex)
                    .cross_align(Alignment::End)
                    .spacing(16.)
                    .child(
                        rect()
                            .vertical()
                            .width(Size::flex(1.0))
                            .spacing(6.)
                            .child(
                                label()
                                    .text(t("oneclient.servers.page.title"))
                                    .font_size(text_size(32.))
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .color(colors::fg_strong()),
                            )
                            .child(
                                label()
                                    .text(t("oneclient.servers.page.description"))
                                    .font_size(text_size(13.))
                                    .color(colors::fg_secondary()),
                            ),
                    )
                    .child(
                        Button::new()
                            .primary()
                            .on_press(move |_| creating.set(true))
                            .child(Icon::new(IconType::Plus).size(16.))
                            .text(t("oneclient.servers.page.new")),
                    ),
            )
            .child(if cards.is_empty() {
                centered_note(t("oneclient.servers.page.empty"))
            } else {
                rect()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .on_sized(move |event: Event<SizedEventData>| {
                        let w = event.data().area.width();
                        if (w - *width.peek()).abs() > 0.5 {
                            *width.write() = w;
                        }
                    })
                    .child(
                        ScrollArea::new()
                            .width(Size::fill())
                            .height(Size::fill())
                            .scrollbar_gutter(true)
                            .children([fixed_grid(cards, columns, CARD_H, GAP)]),
                    )
                    .into_element()
            })
            .maybe_child(
                creating
                    .read()
                    .then(|| CreateServerModal::new(move |()| creating.set(false)).into_element()),
            )
    }
}

#[derive(Clone, PartialEq)]
struct CardLive {
    status: ServerStatus,
    players: Vec<(String, String)>,
    started: Option<Instant>,
    memory_bytes: Option<u64>,
}

impl From<&ServerLive> for CardLive {
    fn from(live: &ServerLive) -> Self {
        Self {
            status: live.status,
            players: live
                .players
                .iter()
                .map(|name| (name.clone(), live.uuids.get(name).cloned().unwrap_or_default()))
                .collect(),
            started: live.started,
            memory_bytes: live.memory_bytes,
        }
    }
}

fn status_color(status: ServerStatus) -> Color {
    match status {
        ServerStatus::Running => colors::success(),
        ServerStatus::Starting | ServerStatus::Stopping => colors::code_warn(),
        ServerStatus::Installing => colors::code_info(),
        ServerStatus::Stopped => colors::fg_secondary(),
    }
}

#[derive(PartialEq)]
struct ServerCard {
    server: LocalServer,
    live: CardLive,
}

impl Component for ServerCard {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);
        let files = settled_or_loading(&use_server_files(self.server.id.clone()));
        let open_id = self.server.id.clone();
        let live = &self.live;
        let running = live.status == ServerStatus::Running;

        let props = files.as_ref().map(|f| f.props.clone()).unwrap_or_default();
        let motd = props.get_or("motd", "A Minecraft Server");
        let port = props.get_or("server-port", "25565");
        let max_players = props.get_or("max-players", "20");
        let world_size = files
            .as_ref()
            .map(|f| f.world_size)
            .filter(|size| *size > 0)
            .map_or_else(|| DASH.to_string(), fmt_bytes);
        let allocated = fmt_bytes(u64::from(self.server.memory_mb) * 1024 * 1024);
        let memory = match live.memory_bytes.filter(|_| live.status.is_live()) {
            Some(used) => fmt_bytes(used),
            None => allocated,
        };

        let mut status_text = status_label(live.status).to_string();
        if let Some(started) = live.started.filter(|_| running) {
            status_text = format!("{status_text} \u{b7} {}", fmt_uptime(started));
        }

        let hovered = *hovering.read();
        let border = if running {
            colors::success().with_a(150)
        } else if hovered {
            colors::component_border_hover()
        } else {
            colors::component_border()
        };

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .corner_radius(rounded(14.))
            .overflow(Overflow::Clip)
            .background(if hovered {
                colors::component_bg_hover()
            } else {
                colors::page_elevated()
            })
            .border(border_all_color(if running { 1.5 } else { 1. }, border))
            .cursor(CursorIcon::Pointer)
            .on_pointer_enter(move |_| hovering.set(true))
            .on_pointer_leave(move |_| hovering.set(false))
            .on_press(move |_| {
                let _ = RouterContext::get().push(Route::ServerOverview {
                    server_id: open_id.clone(),
                });
            })
            .child(banner(&self.server, live.status, status_text, port))
            .child(
                rect()
                    .position(
                        Position::new_absolute()
                            .top(BANNER_H - ICON / 2. - ICON_RING)
                            .left(18. - ICON_RING),
                    )
                    .layer(Layer::Relative(5))
                    .padding(Gaps::new_all(ICON_RING))
                    .corner_radius(rounded((ICON + ICON_RING * 2.) / 6.))
                    .background(colors::page_elevated())
                    .child(server_icon(&self.server, files.and_then(|f| f.icon), ICON)),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .content(Content::Flex)
                    .padding(Gaps::new(ICON / 2. + 8., 18., 16., 18.))
                    .spacing(12.)
                    .child(
                        rect()
                            .vertical()
                            .width(Size::fill())
                            .spacing(2.)
                            .child(
                                label()
                                    .text(self.server.name.clone())
                                    .font_size(text_size(18.))
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .max_lines(1)
                                    .text_overflow(TextOverflow::Ellipsis)
                                    .color(colors::fg_strong()),
                            )
                            .child(
                                label()
                                    .text(loader_label(&self.server))
                                    .font_size(text_size(12.))
                                    .max_lines(1)
                                    .color(colors::fg_secondary()),
                            ),
                    )
                    .child(motd_plate(&motd))
                    .child(stats_strip(live, &max_players, memory, world_size))
                    .child(actions(&self.server.id, live.status)),
            )
    }

    fn render_key(&self) -> DiffKey {
        let mut hasher = std::hash::DefaultHasher::new();
        std::hash::Hash::hash(&self.server.id, &mut hasher);
        DiffKey::U64(std::hash::Hasher::finish(&hasher))
    }
}

fn banner(server: &LocalServer, status: ServerStatus, status_text: String, port: String) -> Element {
    let color = status_color(status);
    rect()
        .width(Size::fill())
        .height(Size::px(BANNER_H))
        .child(
            rect()
                .width(Size::fill())
                .height(Size::fill())
                .position(Position::new_absolute())
                .child(version_art(server).max_edge(ART_PREVIEW_EDGE)),
        )
        .child(
            rect()
                .width(Size::fill())
                .height(Size::fill())
                .position(Position::new_absolute())
                .layer(Layer::Relative(3))
                .background(
                    LinearGradient::new()
                        .angle(0.)
                        .stop((colors::page_elevated().with_a(0), 30.))
                        .stop((colors::page_elevated().with_a(250), 100.)),
                ),
        )
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .position(Position::new_absolute().top(12.).left(12.))
                .layer(Layer::Relative(4))
                .padding(Gaps::new(0., 24., 0., 0.))
                .content(Content::Flex)
                .cross_align(Alignment::Center)
                .child(
                    media_chip()
                        .spacing(6.)
                        .child(
                            rect()
                                .width(Size::px(7.))
                                .height(Size::px(7.))
                                .corner_radius(rounded(4.))
                                .background(color),
                        )
                        .child(
                            label()
                                .text(status_text)
                                .font_size(text_size(11.))
                                .font_weight(FontWeight::MEDIUM)
                                .color(colors::on_media()),
                        ),
                )
                .child(rect().width(Size::flex(1.0)))
                .child(
                    media_chip().child(
                        label()
                            .text(format!(":{port}"))
                            .font_size(text_size(11.))
                            .font_family(crate::theme::mono_font())
                            .color(colors::on_media()),
                    ),
                ),
        )
        .into_element()
}

fn media_chip() -> Rect {
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .padding(Gaps::new_symmetric(5., 9.))
        .corner_radius(rounded(8.))
        .background(Color::from_af32rgb(0.66, 11, 16, 19))
}

fn motd_plate(motd: &str) -> Element {
    rect()
        .width(Size::fill())
        .height(Size::flex(1.0))
        .main_align(Alignment::Center)
        .padding(Gaps::new_symmetric(8., 10.))
        .corner_radius(rounded(8.))
        .background(Color::from_rgb(10, 12, 14))
        .font_family(crate::theme::MINECRAFT_FONT)
        .font_size(MOTD_FONT)
        .child(super::motd::motd_text(
            motd,
            Color::from_rgb(128, 128, 128),
            0.9,
            None,
        ))
        .into_element()
}

fn stat(name: &'static str, value: String, extra: Option<Element>) -> Element {
    rect()
        .vertical()
        .width(Size::flex(1.0))
        .spacing(4.)
        .child(
            label()
                .text(name)
                .font_size(text_size(11.))
                .color(colors::fg_secondary()),
        )
        .child(
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(8.)
                .child(
                    label()
                        .text(value)
                        .font_size(text_size(15.))
                        .font_weight(FontWeight::SEMI_BOLD)
                        .max_lines(1)
                        .color(colors::fg_primary()),
                )
                .maybe_child(extra),
        )
        .into_element()
}

fn stats_strip(live: &CardLive, max_players: &str, memory: String, world: String) -> Element {
    let online = live.status == ServerStatus::Running;
    let players = if online {
        format!("{}/{max_players}", live.players.len())
    } else {
        format!("{DASH}/{max_players}")
    };
    let heads = (online && !live.players.is_empty()).then(|| {
        let extra = live.players.len().saturating_sub(HEADS);
        rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(2.)
            .children(live.players.iter().take(HEADS).map(|(_, uuid)| {
                Avatar::new(uuid.clone()).size(18.).radius(4.).into_element()
            }))
            .maybe(extra > 0, |el| {
                el.child(
                    label()
                        .text(format!("+{extra}"))
                        .font_size(text_size(11.))
                        .color(colors::fg_secondary()),
                )
            })
            .into_element()
    });

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(12.)
        .child(
            rect()
                .width(Size::fill())
                .height(Size::px(1.))
                .background(colors::component_border()),
        )
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::Flex)
                .spacing(12.)
                .child(stat(t("oneclient.servers.tab.players"), players, heads))
                .child(stat(t("oneclient.servers.facts.memory"), memory, None))
                .child(stat(t("oneclient.servers.facts.world"), world, None)),
        )
        .into_element()
}

fn actions(id: &str, status: ServerStatus) -> Element {
    let station = use_dispatch().station();
    let (run_id, console_id) = (id.to_string(), id.to_string());

    let main = match status {
        ServerStatus::Stopped => Button::new()
            .primary()
            .width(Size::flex(1.0))
            .on_press(move |_| servers::start(station, run_id.clone()))
            .child(Icon::new(IconType::Play).size(14.))
            .text(t("oneclient.servers.start")),
        ServerStatus::Starting | ServerStatus::Running => Button::new()
            .danger()
            .width(Size::flex(1.0))
            .on_press(move |_| servers::stop(station, run_id.clone()))
            .child(Icon::new(IconType::Square).size(14.))
            .text(t("oneclient.servers.stop")),
        ServerStatus::Installing | ServerStatus::Stopping => Button::new()
            .secondary()
            .width(Size::flex(1.0))
            .enabled(false)
            .child(Icon::new(IconType::Loading02).size(14.))
            .text(status_label(status)),
    };

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .spacing(8.)
        .on_press(|e: Event<PressEventData>| e.stop_propagation())
        .child(main)
        .child(
            Button::new()
                .secondary()
                .icon()
                .tooltip(t("oneclient.servers.tab.console"))
                .on_press(move |_| {
                    let _ = RouterContext::get().push(Route::ServerConsole {
                        server_id: console_id.clone(),
                    });
                })
                .child(Icon::new(IconType::Terminal).size(16.)),
        )
        .into_element()
}
