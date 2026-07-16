use freya::prelude::*;
use freya::query::QueryStateData;

use oneclient_core::game::ServerStat;

use crate::components::{
    Button, Icon, IconType, OverlayPopup, PieChart, ScrollArea, ValueUnit, slice_color,
};
use crate::hooks::use_cached_image;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::utils::{format_duration_hm, plural};

use super::{card, card_header};

/// Number of servers charted; the rest are grouped into an "Other" slice.
const TOP_SERVERS: usize = 10;

pub(super) fn servers_section(servers: &[ServerStat]) -> Element {
    ServersSection {
        servers: servers.to_vec(),
    }
    .into_element()
}

#[derive(PartialEq)]
struct ServersSection {
    servers: Vec<ServerStat>,
}

impl Component for ServersSection {
    fn render(&self) -> impl IntoElement {
        let hovered = use_state(|| Option::<usize>::None);
        let mut show_all = use_state(|| false);

        let total_joins: i64 = self.servers.iter().map(|s| s.joins).sum();

        let mut sorted = self.servers.clone();
        sorted.sort_by_key(|s| std::cmp::Reverse(s.total_secs));

        let (top, rest) = sorted.split_at(sorted.len().min(TOP_SERVERS));
        let mut values: Vec<i64> = top.iter().map(|s| s.total_secs).collect();
        let mut labels: Vec<String> = top.iter().map(server_label).collect();
        if !rest.is_empty() {
            values.push(rest.iter().map(|s| s.total_secs).sum());
            labels.push(format!("{} more", rest.len()));
        }

        let details = (!self.servers.is_empty()).then(|| {
            Button::new()
                .ghost()
                .small()
                .on_press(move |_| show_all.set(true))
                .child(Icon::new(IconType::Eye).size(12.))
                .text("Details")
                .into_element()
        });

        card()
            .width(Size::fill())
            .spacing(14.)
            .child(card_header(
                "Servers",
                format!(
                    "{} server{} · {} join{}",
                    self.servers.len(),
                    plural(self.servers.len() as i64),
                    total_joins,
                    plural(total_joins),
                ),
                details,
            ))
            .child(
                rect()
                    .horizontal()
                    .content(Content::Flex)
                    .width(Size::fill())
                    .cross_align(Alignment::Center)
                    .spacing(20.)
                    .child(
                        PieChart::new(values.clone(), labels.clone())
                            .unit(ValueUnit::Duration)
                            .hovered(hovered),
                    )
                    .child(rect().width(Size::flex(1.0)).child(legend(
                        &values,
                        &labels,
                        *hovered.read(),
                    ))),
            )
            .maybe_child(show_all.read().then(|| {
                OverlayPopup::new()
                    .on_close(move |_| show_all.set(false))
                    .child(
                        rect()
                            .width(Size::window_percent(100.))
                            .height(Size::window_percent(100.))
                            .center()
                            .child(ServerDetailsPopup {
                                servers: sorted.clone(),
                                on_close: EventHandler::new(move |()| show_all.set(false)),
                            }),
                    )
                    .into_element()
            }))
    }
}

fn legend(values: &[i64], labels: &[String], active: Option<usize>) -> Element {
    let total: i64 = values.iter().sum();
    let mut list = rect().vertical().width(Size::fill()).spacing(4.);

    for (i, (value, name)) in values.iter().zip(labels).enumerate() {
        let is_active = Some(i) == active;
        let pct = if total > 0 {
            (*value as f32 / total as f32) * 100.
        } else {
            0.
        };
        let dim = active.is_some() && !is_active;

        list = list.child(
            rect()
                .horizontal()
                .content(Content::Flex)
                .width(Size::fill())
                .cross_align(Alignment::Center)
                .spacing(8.)
                .padding(Gaps::new_symmetric(3., 6.))
                .corner_radius(CornerRadius::new_all(6.))
                .background(if is_active {
                    colors::component_bg()
                } else {
                    Color::TRANSPARENT
                })
                .child(
                    rect()
                        .width(Size::px(8.))
                        .height(Size::px(8.))
                        .corner_radius(CornerRadius::new_all(2.))
                        .background(slice_color(i).with_a(if dim { 120 } else { 255 })),
                )
                .child(
                    label()
                        .text(name.clone())
                        .font_size(11.)
                        .max_lines(1)
                        .width(Size::flex(1.0))
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(format!("{pct:.0}%"))
                        .font_size(11.)
                        .color(colors::fg_secondary()),
                )
                .child(
                    label()
                        .text(format_duration_hm(*value))
                        .font_size(11.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                ),
        );
    }

    list.into_element()
}

/// A Component (not a plain builder) because it mounts a `ScrollArea`, whose
/// hooks run in the caller's render scope — mounting it conditionally from
/// `ServersSection` would change that scope's hook count and panic Freya.
#[derive(PartialEq)]
struct ServerDetailsPopup {
    servers: Vec<ServerStat>,
    on_close: EventHandler<()>,
}

impl Component for ServerDetailsPopup {
    fn render(&self) -> impl IntoElement {
        let servers = &self.servers;
        let on_close = self.on_close.clone();
        let max_secs = servers
            .iter()
            .map(|s| s.total_secs)
            .max()
            .unwrap_or(0)
            .max(1);

        let mut list = rect().vertical().width(Size::fill()).spacing(4.);
        for server in servers {
            list = list.child(ServerRow {
                server: server.clone(),
                max_secs,
            });
        }

        rect()
            .vertical()
            .width(Size::px(520.))
            .max_height(Size::window_percent(70.))
            .padding(20.)
            .spacing(14.)
            .corner_radius(CornerRadius::new_all(14.))
            .background(colors::page_elevated())
            .border(border_all_color(1., colors::component_border()))
            .child(
                rect()
                    .horizontal()
                    .content(Content::Flex)
                    .width(Size::fill())
                    .cross_align(Alignment::Center)
                    .spacing(12.)
                    .child(
                        rect()
                            .vertical()
                            .width(Size::flex(1.0))
                            .spacing(2.)
                            .child(
                                label()
                                    .text("All servers")
                                    .font_size(16.)
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .color(colors::fg_primary()),
                            )
                            .child(
                                label()
                                    .text(format!(
                                        "{} server{} by playtime",
                                        servers.len(),
                                        plural(servers.len() as i64)
                                    ))
                                    .font_size(12.)
                                    .color(colors::fg_secondary()),
                            ),
                    )
                    .child(
                        Button::new()
                            .ghost()
                            .small()
                            .on_press(move |_| on_close.call(()))
                            .child(Icon::new(IconType::XClose).size(12.)),
                    ),
            )
            .child(
                ScrollArea::new()
                    .width(Size::fill())
                    .height(Size::px(380.))
                    .child(list),
            )
    }
}

/// IP addresses stay masked in the always-visible chart legend; the details
/// popup reveals them on hover only.
fn server_label(s: &ServerStat) -> String {
    if s.is_ip {
        return "Direct IP".to_string();
    }
    display_address(s)
}

#[derive(PartialEq)]
struct ServerRow {
    server: ServerStat,
    max_secs: i64,
}

impl Component for ServerRow {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let is_hovered = *hovered.read();

        let s = &self.server;
        let reveal = is_hovered || !s.is_ip;
        let address = if reveal {
            display_address(s)
        } else {
            mask(&display_address(s))
        };

        let frac = (s.total_secs as f32 / self.max_secs as f32).clamp(0., 1.);

        let bar = rect()
            .width(Size::fill())
            .height(Size::px(4.))
            .corner_radius(CornerRadius::new_all(2.))
            .background(colors::component_bg())
            .child(
                rect()
                    .width(Size::percent(frac * 100.))
                    .height(Size::fill())
                    .corner_radius(CornerRadius::new_all(2.))
                    .background(colors::brand().with_a(if reveal { 255 } else { 150 })),
            );

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(6.)
            .padding(Gaps::new_symmetric(8., 10.))
            .corner_radius(CornerRadius::new_all(8.))
            .background(if is_hovered {
                colors::component_bg_hover()
            } else {
                Color::TRANSPARENT
            })
            .on_pointer_enter(move |_| {
                *hovered.write() = true;
                Cursor::set(CursorIcon::Pointer);
            })
            .on_pointer_leave(move |_| {
                *hovered.write() = false;
                Cursor::set(CursorIcon::default());
            })
            .child(
                rect()
                    .horizontal()
                    .content(Content::Flex)
                    .width(Size::fill())
                    .cross_align(Alignment::Center)
                    .spacing(10.)
                    .child(ServerIcon {
                        url: server_icon_url(s),
                        is_ip: s.is_ip,
                        size: 28.,
                    })
                    .child(
                        label()
                            .text(address)
                            .font_size(13.)
                            .font_weight(FontWeight::MEDIUM)
                            .color(colors::fg_primary())
                            .width(Size::flex(1.0)),
                    )
                    .child(
                        label()
                            .text(format!("{}×", s.joins))
                            .font_size(12.)
                            .color(colors::fg_secondary()),
                    )
                    .child(
                        label()
                            .text(format_duration_hm(s.total_secs))
                            .font_size(12.)
                            .font_weight(FontWeight::SEMI_BOLD)
                            .color(colors::fg_primary()),
                    ),
            )
            .child(bar)
    }
}

const SERVER_ICON_EDGE: u32 = 64;

fn server_icon_url(s: &ServerStat) -> Option<String> {
    if s.is_ip {
        return None;
    }

    Some(format!("https://api.mcsrvstat.us/icon/{}", s.address))
}

#[derive(PartialEq)]
struct ServerIcon {
    url: Option<String>,
    is_ip: bool,
    size: f32,
}

impl Component for ServerIcon {
    fn render(&self) -> impl IntoElement {
        let size = self.size;
        let query = use_cached_image(self.url.clone(), SERVER_ICON_EDGE);
        let reader = query.read();
        let loaded = match (&self.url, &*reader.state()) {
            (Some(url), QueryStateData::Settled { res: Ok(bytes), .. })
            | (
                Some(url),
                QueryStateData::Loading {
                    res: Some(Ok(bytes)),
                },
            ) => Some((url.clone(), bytes.clone())),
            _ => None,
        };

        match loaded {
            Some((url, bytes)) => ImageViewer::new((url, bytes))
                .width(Size::px(size))
                .height(Size::px(size))
                .aspect_ratio(AspectRatio::Min)
                .corner_radius(CornerRadius::new_all(6.))
                .into_element(),
            None => server_icon_placeholder(self.is_ip, size),
        }
    }
}

fn server_icon_placeholder(is_ip: bool, size: f32) -> Element {
    let icon = if is_ip {
        IconType::Eye
    } else {
        IconType::Globe01
    };
    rect()
        .center()
        .width(Size::px(size))
        .height(Size::px(size))
        .corner_radius(CornerRadius::new_all(6.))
        .background(colors::component_bg())
        .child(
            Icon::new(icon)
                .size(size * 0.5)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn display_address(s: &ServerStat) -> String {
    match s.port {
        Some(port) if port != 25565 => format!("{}:{}", s.address, port),
        _ => s.address.clone(),
    }
}

fn mask(address: &str) -> String {
    let (host, tail) = match address.split_once(':') {
        Some((h, p)) => (h, format!(":{p}")),
        None => (address, String::new()),
    };
    let dots = "•".repeat(host.chars().count().clamp(6, 18));
    format!("{dots}{tail}")
}
