use freya::prelude::*;
use freya::query::QueryStateData;

use oneclient_core::game::ServerStat;

use crate::components::{Icon, IconType};
use crate::hooks::use_cached_image;
use crate::theme::colors;
use crate::utils::{format_duration_hm, plural};

use super::{card, card_header};

pub(super) fn servers_section(servers: &[ServerStat]) -> Element {
    let total_joins: i64 = servers.iter().map(|s| s.joins).sum();
    let max_secs = servers.iter().map(|s| s.total_secs).max().unwrap_or(0).max(1);

    let mut list = rect().vertical().width(Size::fill()).spacing(4.);
    for server in servers.iter().take(10) {
        list = list.child(ServerRow {
            server: server.clone(),
            max_secs,
        });
    }

    card()
        .width(Size::fill())
        .spacing(14.)
        .child(card_header(
            "Servers",
            format!(
                "{} server{} · {} join{}",
                servers.len(),
                plural(servers.len() as i64),
                total_joins,
                plural(total_joins),
            ),
            None,
        ))
        .child(list)
        .into_element()
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
            | (Some(url), QueryStateData::Loading { res: Some(Ok(bytes)) }) => {
                Some((url.clone(), bytes.clone()))
            }
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
        .child(Icon::new(icon).size(size * 0.5).color(colors::fg_secondary()))
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
