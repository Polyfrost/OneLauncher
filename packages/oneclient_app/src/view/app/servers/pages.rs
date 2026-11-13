use std::path::PathBuf;

use freya::prelude::*;
use oneclient_core::servers as core;

use super::settings::{ConfirmRow, whitelist_row};
use super::{DASH, card, card_head, fmt_bytes};
use crate::components::{Button, Icon, IconType, ScrollArea, open_folder_button};
use crate::hooks::{settled_or_loading, use_server, use_server_files, use_server_hub};
use crate::i18n::{t, tf};
use crate::servers::{self, ServerStatus};
use crate::theme::{colors, rounded, text_size};
use crate::ui::centered_note;
use crate::view::app::settings::settings_row;

fn page(children: Vec<Element>) -> Element {
    ScrollArea::new()
        .width(Size::fill())
        .height(Size::fill())
        .scrollbar_gutter(true)
        .spacing(10.)
        .padding(Gaps::new(0., 0., 20., 0.))
        .children(children)
        .into_element()
}

fn file_row(icon: IconType, name: String, meta: String, actions: Element) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(14.)
        .padding(Gaps::new_symmetric(10., 16.))
        .corner_radius(rounded(10.))
        .background(colors::page_elevated())
        .child(Icon::new(icon).size(18.).color(colors::fg_secondary()))
        .child(
            label()
                .text(name)
                .width(Size::flex(1.0))
                .max_lines(1)
                .font_size(text_size(14.))
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text(meta)
                .font_size(text_size(12.))
                .color(colors::fg_secondary()),
        )
        .child(actions)
        .into_element()
}

#[derive(PartialEq)]
pub struct ServerPlayers {
    pub server_id: String,
}

impl Component for ServerPlayers {
    fn render(&self) -> impl IntoElement {
        let files = settled_or_loading(&use_server_files(self.server_id.clone()));
        let live = use_server_hub().get(&self.server_id);
        let Some(files) = files else {
            return centered_note(t("oneclient.servers.loading"));
        };

        let online: Element = if live.status != ServerStatus::Running {
            label()
                .text(t("oneclient.servers.players.offline"))
                .font_size(text_size(13.))
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
                .children(live.players.iter().map(|name| {
                    let (id, kick) = (self.server_id.clone(), name.clone());
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .content(Content::Flex)
                        .cross_align(Alignment::Center)
                        .child(
                            label()
                                .text(name.clone())
                                .width(Size::flex(1.0))
                                .font_size(text_size(14.))
                                .color(colors::fg_primary()),
                        )
                        .child(
                            Button::new()
                                .secondary()
                                .small()
                                .on_press(move |_| {
                                    servers::send_command(&id, &format!("kick {kick}"));
                                })
                                .text(t("oneclient.servers.players.kick")),
                        )
                        .into_element()
                }))
                .into_element()
        };

        page(vec![
            card()
                .child(card_head(
                    t("oneclient.servers.players.online"),
                    tf(
                        "oneclient.servers.stats.slots",
                        &[&files.props.get_or("max-players", "20")],
                    ),
                    None,
                ))
                .child(online)
                .into_element(),
            whitelist_row(&self.server_id, &files, false),
            whitelist_row(&self.server_id, &files, true),
        ])
    }
}

#[derive(PartialEq)]
pub struct ServerWorlds {
    pub server_id: String,
}

impl Component for ServerWorlds {
    fn render(&self) -> impl IntoElement {
        let server = use_server(&self.server_id);
        let files = settled_or_loading(&use_server_files(self.server_id.clone()));
        let (Some(server), Some(files)) = (server, files) else {
            return centered_note(t("oneclient.servers.loading"));
        };
        let folder = server.dir().ok().map(|d| d.join(&files.world));
        let size = if files.world_size == 0 {
            DASH.to_string()
        } else {
            fmt_bytes(files.world_size)
        };

        page(vec![
            settings_row(
                IconType::Globe01,
                t("oneclient.servers.worlds.current"),
                tf("oneclient.servers.worlds.meta", &[&files.world, &size]),
                folder.map_or_else(
                    || rect().into_element(),
                    |f| open_folder_button(f).into_element(),
                ),
            )
            .into_element(),
            ConfirmRow {
                server,
                delete: false,
            }
            .into_element(),
        ])
    }
}

#[derive(PartialEq)]
pub struct ServerBackups {
    pub server_id: String,
}

impl Component for ServerBackups {
    fn render(&self) -> impl IntoElement {
        let server = use_server(&self.server_id);
        let files = settled_or_loading(&use_server_files(self.server_id.clone()));
        let running = use_server_hub().status(&self.server_id).is_live();
        let mut armed = use_state(|| None::<PathBuf>);
        let (Some(server), Some(files)) = (server, files) else {
            return centered_note(t("oneclient.servers.loading"));
        };
        let id = self.server_id.clone();

        let mut children = vec![
            card()
                .child(card_head(
                    t("oneclient.servers.tab.backups"),
                    if server.backup_hours == 0 {
                        t("oneclient.servers.backups.off").to_string()
                    } else {
                        tf(
                            "oneclient.servers.backups.schedule",
                            &[&server.backup_hours, &server.backups_kept],
                        )
                    },
                    Some(
                        rect()
                            .horizontal()
                            .spacing(10.)
                            .maybe_child(
                                server
                                    .dir()
                                    .ok()
                                    .map(|d| open_folder_button(d.join("backups")).into_element()),
                            )
                            .child(
                                Button::new()
                                    .primary()
                                    .on_press({
                                        let id = id.clone();
                                        move |_| servers::backup(id.clone(), false)
                                    })
                                    .child(Icon::new(IconType::Plus).size(14.))
                                    .text(t("oneclient.servers.backups.now")),
                            )
                            .into_element(),
                    ),
                ))
                .into_element(),
        ];
        if files.backups.is_empty() {
            children.push(centered_note(t("oneclient.servers.backups.empty")));
        }
        for backup in &files.backups {
            let path = backup.path.clone();
            let confirming = armed.read().as_ref() == Some(&path);
            let (restore_path, delete_path, id) = (path.clone(), path.clone(), id.clone());
            let actions = rect()
                .horizontal()
                .spacing(8.)
                .child(
                    Button::new()
                        .small()
                        .enabled(!running)
                        .map(
                            Some(confirming),
                            |b, c| if c { b.danger() } else { b.secondary() },
                        )
                        .on_press(move |_| {
                            if armed.peek().as_ref() != Some(&restore_path) {
                                armed.set(Some(restore_path.clone()));
                                return;
                            }
                            armed.set(None);
                            let (id, backup) = (id.clone(), restore_path.clone());
                            servers::run(
                                t("oneclient.servers.backups.restore_failed"),
                                async move {
                                    core::restore_backup(&core::server_dir(&id)?, &backup).await
                                },
                            );
                        })
                        .text(if confirming {
                            t("oneclient.servers.settings.confirm")
                        } else {
                            t("oneclient.servers.backups.restore")
                        }),
                )
                .child(
                    Button::new()
                        .ghost()
                        .icon()
                        .small()
                        .alt(t("oneclient.servers.backups.delete"))
                        .on_press(move |_| {
                            let path = delete_path.clone();
                            servers::run(
                                t("oneclient.servers.backups.delete_failed"),
                                async move {
                                    tokio::fs::remove_file(path).await?;
                                    Ok(())
                                },
                            );
                        })
                        .child(Icon::new(IconType::Trash01).size(14.)),
                )
                .into_element();
            children.push(file_row(
                IconType::ClockRewind,
                backup.created.format("%Y-%m-%d %H:%M").to_string(),
                fmt_bytes(backup.size),
                actions,
            ));
        }
        page(children)
    }
}
