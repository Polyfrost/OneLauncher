use freya::prelude::*;

use super::console::ConsolePanel;
use super::{card, card_head, fmt_uptime, head, list_preview, player_head};
use crate::components::{Button, Icon, IconType, ScrollArea, copy_button};
use crate::hooks::{
    ServerFiles, settled_or_loading, use_dispatch, use_server, use_server_files, use_server_hub,
    use_server_mods, use_server_run,
};
use crate::i18n::{t, tf};
use crate::servers::{self, ServerLive, ServerStatus};
use crate::theme::{colors, rounded, text_size};
use crate::ui::{border_all_color, centered_note};
use oneclient_core::servers::LocalServer;

const PORT_GUIDE: &str =
    "https://minecraft.wiki/w/Tutorial:Setting_up_a_Java_Edition_server#Port_forwarding";
const SIDE_COLUMN: f32 = 360.;

#[derive(PartialEq)]
pub struct ServerOverview {
    pub server_id: String,
}

impl Component for ServerOverview {
    fn render(&self) -> impl IntoElement {
        let server = use_server(&self.server_id);
        let files =
            settled_or_loading(&use_server_files(self.server_id.clone())).unwrap_or_default();
        let world_size = settled_or_loading(&use_server_run(self.server_id.clone()))
            .map_or(0, |run| run.world_size);
        let mod_count = settled_or_loading(&use_server_mods(self.server_id.clone()))
            .map_or(0, |mods| mods.len());
        let live = use_server_hub().get(&self.server_id);
        let Some(server) = server else {
            return centered_note(t("oneclient.servers.not_found"));
        };

        let max_players = files.props.get_or("max-players", "20");
        let port = files.props.get_or("server-port", "25565");

        let preview = card()
            .child(card_head(
                t("oneclient.servers.preview.title"),
                tf("oneclient.servers.preview.subtitle", &[&server.name]),
                Some(
                    Button::new()
                        .secondary()
                        .on_press({
                            let server_id = self.server_id.clone();
                            move |_| super::settings::open_at_motd(server_id.clone())
                        })
                        .child(Icon::new(IconType::Pencil01).size(14.))
                        .text(t("oneclient.servers.preview.edit"))
                        .into_element(),
                ),
            ))
            .child(list_preview(
                &server,
                files.icon.clone(),
                &files.props.get_or("motd", "A Minecraft Server"),
                format!("{}/{max_players}", live.players.len()),
            ));

        ScrollArea::new()
            .width(Size::fill())
            .height(Size::fill())
            .scrollbar_gutter(true)
            .spacing(16.)
            .child(
                rect()
                    .vertical()
                    .spacing(4.)
                    .padding(Gaps::new(0., 0., 4., 0.))
                    .child(
                        label()
                            .text(t("oneclient.servers.overview.title"))
                            .font_size(text_size(20.))
                            .font_weight(FontWeight::SEMI_BOLD)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text(tf("oneclient.servers.overview.subtitle", &[&server.name]))
                            .font_size(text_size(12.))
                            .color(colors::fg_secondary()),
                    ),
            )
            .child(stats(&live, &files, world_size, &max_players))
            .child(preview)
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .content(Content::Flex)
                    .spacing(16.)
                    .child(
                        rect()
                            .vertical()
                            .width(Size::flex(1.0))
                            .spacing(16.)
                            .child(ConsolePanel {
                                server_id: self.server_id.clone(),
                                height: Some(340.),
                            })
                            .child(join_card(&port)),
                    )
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(SIDE_COLUMN))
                            .spacing(16.)
                            .child(players_card(&live, &files))
                            .child(requirements_card(
                                &server,
                                &files,
                                mod_count,
                                InternetCheck {
                                    port: port.trim().parse().unwrap_or(25565),
                                    running: live.status == ServerStatus::Running,
                                },
                            ))
                            .child(backups_card(&server, self.server_id.clone())),
                    ),
            )
            .into_element()
    }
}

fn tile(
    name: &'static str,
    value: String,
    suffix: Option<String>,
    accent: bool,
    extra: Option<Element>,
) -> Element {
    rect()
        .vertical()
        .width(Size::flex(1.0))
        .spacing(10.)
        .padding(Gaps::new_symmetric(20., 22.))
        .corner_radius(rounded(14.))
        .background(if accent {
            colors::brand().with_a(28)
        } else {
            colors::page_elevated()
        })
        .border(border_all_color(
            1.,
            if accent {
                colors::brand()
            } else {
                colors::component_border()
            },
        ))
        .child(
            label()
                .text(name)
                .font_size(text_size(13.))
                .color(colors::fg_secondary()),
        )
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::Flex)
                .cross_align(Alignment::End)
                .spacing(6.)
                .child(
                    label()
                        .text(value)
                        .font_size(text_size(26.))
                        .font_weight(FontWeight::BOLD)
                        .color(colors::fg_primary()),
                )
                .maybe_child(suffix.map(|s| {
                    label()
                        .text(s)
                        .font_size(text_size(12.))
                        .color(colors::fg_secondary())
                        .into_element()
                }))
                .maybe(extra.is_some(), |el| {
                    el.child(rect().width(Size::flex(1.0)))
                })
                .maybe_child(extra),
        )
        .into_element()
}

fn row(children: Vec<Element>) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .spacing(16.)
        .children(children)
        .into_element()
}

fn stats(live: &ServerLive, files: &ServerFiles, world_size: u64, max_players: &str) -> Element {
    let running = live.status == ServerStatus::Running;
    let last_backup = files.backups.first().map_or_else(
        || super::none(),
        |b| b.created.format("%H:%M").to_string(),
    );

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(16.)
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::Flex)
                .spacing(16.)
                .child(rect().width(Size::flex(2.0)).child(tile(
                    t("oneclient.servers.stats.players"),
                    live.players.len().to_string(),
                    Some(tf("oneclient.servers.stats.slots", &[&max_players])),
                    true,
                    (running && !live.players.is_empty()).then(|| {
                        rect()
                            .horizontal()
                            .spacing(4.)
                            .children(live.players.iter().map(|p| head(live, p, 24.)))
                            .into_element()
                    }),
                )))
                .child(tile(
                    t("oneclient.servers.stats.uptime"),
                    live.started
                        .filter(|_| running)
                        .map_or_else(|| crate::utils::format_duration(0), fmt_uptime),
                    None,
                    false,
                    None,
                )),
        )
        .child(row(vec![
            tile(
                t("oneclient.servers.stats.memory"),
                live.memory_bytes
                    .map_or_else(|| super::size_label(0), super::size_label),
                None,
                false,
                None,
            ),
            tile(
                t("oneclient.servers.stats.cpu"),
                live.cpu_percent
                    .map_or_else(|| "0%".to_string(), |c| format!("{c:.0}%")),
                None,
                false,
                None,
            ),
            tile(
                t("oneclient.servers.stats.world"),
                super::size_label(world_size),
                None,
                false,
                None,
            ),
            tile(
                t("oneclient.servers.stats.backup"),
                last_backup,
                None,
                false,
                None,
            ),
        ]))
        .into_element()
}

#[derive(PartialEq)]
struct AddressRow {
    icon: IconType,
    address: String,
    caption: &'static str,
    spoiler: bool,
}

fn masked(address: &str) -> String {
    let (host, port) = address.rsplit_once(':').unwrap_or((address, ""));
    let host: String = host
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { '\u{2022}' } else { c })
        .collect();
    if port.is_empty() {
        host
    } else {
        format!("{host}:{port}")
    }
}

impl Component for AddressRow {
    fn render(&self) -> impl IntoElement {
        let mut revealed = use_state(|| !self.spoiler);
        let mut hovering = use_state(|| false);
        let dispatch = use_dispatch();
        let hidden = !*revealed.read();

        let address = label()
            .text(if hidden {
                masked(&self.address)
            } else {
                self.address.clone()
            })
            .font_family(crate::theme::mono_font())
            .font_size(text_size(15.))
            .max_lines(1)
            .color(if hidden {
                colors::fg_secondary()
            } else {
                colors::fg_primary()
            });

        let address = rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(8.)
            .padding(Gaps::new_symmetric(2., 8.))
            .margin(Gaps::new_symmetric(0., -8.))
            .corner_radius(rounded(6.))
            .child(address)
            .maybe(hidden, |el| {
                el.background(if *hovering.read() {
                    colors::ghost_overlay()
                } else {
                    Color::TRANSPARENT
                })
                .cursor(CursorIcon::Pointer)
                .a11y_role(AccessibilityRole::Button)
                .a11y_alt(t("oneclient.servers.join.show"))
                .on_pointer_enter(move |_| hovering.set(true))
                .on_pointer_leave(move |_| hovering.set(false))
                .on_press(move |_| {
                    hovering.set(false);
                    revealed.set(true);
                })
                .child(
                    label()
                        .text(t("oneclient.servers.join.reveal"))
                        .font_size(text_size(11.))
                        .font_weight(FontWeight::MEDIUM)
                        .color(if *hovering.read() {
                            colors::fg_primary()
                        } else {
                            colors::fg_secondary()
                        }),
                )
            });

        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(14.)
            .padding(Gaps::new_symmetric(12., 16.))
            .corner_radius(rounded(10.))
            .background(colors::component_bg())
            .child(Icon::new(self.icon).size(20.).color(colors::fg_secondary()))
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(2.)
                    .child(address)
                    .child(
                        label()
                            .text(self.caption)
                            .font_size(text_size(12.))
                            .color(colors::fg_secondary()),
                    ),
            )
            .maybe_child(self.spoiler.then(|| {
                Button::new()
                    .ghost()
                    .icon()
                    .tooltip(if hidden {
                        t("oneclient.servers.join.show")
                    } else {
                        t("oneclient.servers.join.hide")
                    })
                    .on_press(move |_| revealed.set(hidden))
                    .child(
                        Icon::new(IconType::Eye)
                            .size(16.)
                            .color(if hidden {
                                colors::fg_secondary()
                            } else {
                                colors::fg_primary()
                            }),
                    )
                    .into_element()
            }))
            .child(copy_button(
                self.address.clone(),
                t("oneclient.servers.copied"),
                dispatch,
            ))
    }
}

fn join_card(port: &str) -> Element {
    let lan = oneclient_core::servers::lan_ip().map(|ip| format!("{ip}:{port}"));
    card()
        .child(card_head(
            t("oneclient.servers.join.title"),
            t("oneclient.servers.join.subtitle"),
            None,
        ))
        .child(AddressRow {
            icon: IconType::LayoutTop,
            address: format!("localhost:{port}"),
            caption: t("oneclient.servers.join.this_pc"),
            spoiler: false,
        })
        .maybe_child(lan.map(|lan| {
            AddressRow {
                icon: IconType::Globe01,
                address: lan,
                caption: t("oneclient.servers.join.lan"),
                spoiler: true,
            }
            .into_element()
        }))
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(10.)
                .padding(Gaps::new_symmetric(14., 16.))
                .corner_radius(rounded(10.))
                .background(colors::brand().with_a(24))
                .border(border_all_color(1., colors::brand().with_a(120)))
                .child(
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .content(Content::Flex)
                        .spacing(10.)
                        .child(
                            Icon::new(IconType::InfoCircle)
                                .size(16.)
                                .color(colors::brand()),
                        )
                        .child(
                            label()
                                .text(tf("oneclient.servers.join.internet", &[&port]))
                                .width(Size::flex(1.0))
                                .font_size(text_size(13.))
                                .line_height(1.4)
                                .color(colors::fg_primary()),
                        ),
                )
                .child(
                    Button::new()
                        .secondary()
                        .small()
                        .on_press(|_| crate::platform::open_url(PORT_GUIDE))
                        .text(t("oneclient.servers.join.guide")),
                ),
        )
        .into_element()
}

fn players_card(live: &ServerLive, files: &ServerFiles) -> Element {
    let running = live.status == ServerStatus::Running;
    let whitelist_on = files.props.get("white-list").is_some_and(|v| v == "true");
    let body: Element = if !running {
        label()
            .text(t("oneclient.servers.players.offline"))
            .width(Size::fill())
            .text_align(TextAlign::Center)
            .font_size(text_size(13.))
            .line_height(1.4)
            .color(colors::fg_secondary())
            .into_element()
    } else if live.players.is_empty() {
        label()
            .text(t("oneclient.servers.players.none"))
            .font_size(text_size(13.))
            .color(colors::fg_secondary())
            .into_element()
    } else {
        rect()
            .vertical()
            .spacing(6.)
            .children(live.players.iter().map(|p| player_head(live, p, 22.)))
            .into_element()
    };

    card()
        .child(card_head(
            t("oneclient.servers.tab.players"),
            "",
            Some(
                label()
                    .text(if running {
                        t("oneclient.servers.status.running")
                    } else {
                        t("oneclient.servers.players.offline_tag")
                    })
                    .font_size(text_size(12.))
                    .color(colors::fg_secondary())
                    .into_element(),
            ),
        ))
        .child(body)
        .child(
            label()
                .text(if whitelist_on {
                    tf(
                        "oneclient.servers.players.whitelist_on",
                        &[&files.whitelist.len()],
                    )
                } else {
                    t("oneclient.servers.players.whitelist_off").to_string()
                })
                .font_size(text_size(12.))
                .color(colors::brand()),
        )
        .into_element()
}

fn fact(name: &'static str, value: String, warn: bool) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .padding(Gaps::new_symmetric(11., 0.))
        .border(
            Border::new()
                .fill(colors::component_border())
                .width(BorderWidth {
                    top: 1.,
                    right: 0.,
                    bottom: 0.,
                    left: 0.,
                }),
        )
        .child(
            label()
                .text(name)
                .width(Size::flex(1.0))
                .font_size(text_size(13.))
                .color(colors::fg_secondary()),
        )
        .maybe_child(warn.then(|| {
            Icon::new(IconType::AlertTriangle)
                .size(14.)
                .color(colors::code_warn())
                .into_element()
        }))
        .child(
            label()
                .text(value)
                .font_size(text_size(13.))
                .font_weight(FontWeight::SEMI_BOLD)
                .color(if warn {
                    colors::code_warn()
                } else {
                    colors::fg_primary()
                }),
        )
        .into_element()
}

#[derive(Clone, PartialEq)]
enum Reach {
    Unchecked,
    Checking,
    Done(oneclient_core::servers::Reachability),
    Failed,
}

#[derive(PartialEq)]
struct InternetCheck {
    port: u16,
    running: bool,
}

impl Component for InternetCheck {
    fn render(&self) -> impl IntoElement {
        let mut reach = use_state(|| Reach::Unchecked);
        let (port, running) = (self.port, self.running);
        let current = reach.read().clone();
        let (value, warn) = match &current {
            Reach::Unchecked => (t("oneclient.servers.reach.unchecked").to_string(), true),
            Reach::Checking => (t("oneclient.servers.reach.checking").to_string(), false),
            Reach::Done(found) if found.online => {
                (tf("oneclient.servers.reach.open", &[&found.address]), false)
            }
            Reach::Done(_) => (tf("oneclient.servers.reach.closed", &[&port]), true),
            Reach::Failed => (t("oneclient.servers.reach.failed").to_string(), true),
        };
        rect()
            .vertical()
            .width(Size::fill())
            .spacing(6.)
            .child(fact(t("oneclient.servers.requirements.internet"), value, warn))
            .child(
                Button::new()
                    .secondary()
                    .small()
                    .enabled(running && current != Reach::Checking)
                    .on_press(move |_| {
                        reach.set(Reach::Checking);
                        spawn(async move {
                            let result = crate::launcher::off_ui(async move {
                                let state = crate::launcher::state()?;
                                oneclient_core::servers::check_reachable(&state, port).await
                            })
                            .await;
                            reach.set(match result {
                                Ok(found) => Reach::Done(found),
                                Err(err) => {
                                    tracing::warn!(%err, "could not check if the server is reachable");
                                    Reach::Failed
                                }
                            });
                        });
                    })
                    .text(if running {
                        t("oneclient.servers.reach.check")
                    } else {
                        t("oneclient.servers.reach.start_first")
                    }),
            )
    }
}

fn requirements_card(
    server: &LocalServer,
    files: &ServerFiles,
    mod_count: usize,
    internet: InternetCheck,
) -> Element {
    let total_gb = oneclient_common::memory::total_ram_mb() / 1024;
    let java = server.java_major.map_or_else(
        super::none,
        |major| {
            if server.java_path.is_some() {
                major.to_string()
            } else {
                tf("oneclient.servers.requirements.bundled", &[&major])
            }
        },
    );
    let loader = if server.takes_content() {
        tf(
            if server.plugins() {
                "oneclient.servers.requirements.plugins"
            } else {
                "oneclient.servers.requirements.mods"
            },
            &[&server.software_name(), &mod_count],
        )
    } else {
        server.software_name()
    };

    card()
        .spacing(0.)
        .child(rect().padding(Gaps::new(0., 0., 10., 0.)).child(card_head(
            t("oneclient.servers.requirements.title"),
            "",
            None,
        )))
        .child(fact(t("oneclient.servers.facts.java"), java, false))
        .child(fact(
            t("oneclient.servers.requirements.eula"),
            if files.eula {
                t("oneclient.servers.requirements.accepted")
            } else {
                t("oneclient.servers.requirements.not_accepted")
            }
            .to_string(),
            !files.eula,
        ))
        .child(fact(t("oneclient.servers.facts.loader"), loader, false))
        .child(fact(
            t("oneclient.servers.facts.memory"),
            tf(
                "oneclient.servers.requirements.memory",
                &[&(server.memory_mb / 1024).max(1), &total_gb],
            ),
            false,
        ))
        .child(internet)
        .into_element()
}

fn backups_card(server: &LocalServer, id: String) -> Element {
    let schedule = if server.backup_hours == 0 {
        t("oneclient.servers.backups.off").to_string()
    } else {
        tf(
            "oneclient.servers.backups.schedule",
            &[&server.backup_hours, &server.backups_kept],
        )
    };
    card()
        .child(card_head(
            t("oneclient.servers.tab.backups"),
            schedule,
            None,
        ))
        .child(
            Button::new()
                .primary()
                .width(Size::fill())
                .on_press(move |_| servers::backup(id.clone(), false))
                .child(Icon::new(IconType::Plus).size(14.))
                .text(t("oneclient.servers.backups.now")),
        )
        .into_element()
}

#[cfg(test)]
mod tests {
    use super::masked;

    #[test]
    fn masks_the_host_and_keeps_the_port() {
        assert_eq!(
            masked("192.168.1.20:25565"),
            "\u{2022}\u{2022}\u{2022}.\u{2022}\u{2022}\u{2022}.\u{2022}.\u{2022}\u{2022}:25565"
        );
        assert_eq!(masked("10.0.0.1"), "\u{2022}\u{2022}.\u{2022}.\u{2022}.\u{2022}");
    }
}
