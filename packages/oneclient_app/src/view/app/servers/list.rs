use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::servers::LocalServer;

use super::create::CreateServerModal;
use super::{server_icon, status_line};
use crate::Route;
use crate::components::{Button, Icon, IconType, ScrollArea};
use crate::hooks::{
    settled_or_loading, use_dispatch, use_server_files, use_server_hub, use_servers,
};
use crate::i18n::t;
use crate::servers::{self, ServerStatus};
use crate::theme::{colors, rounded, text_size};
use crate::ui::{border_all_color, centered_note};

#[derive(PartialEq)]
pub struct Servers;

impl Component for Servers {
    fn render(&self) -> impl IntoElement {
        let mut creating = use_state(|| false);
        let list = settled_or_loading(&use_servers()).unwrap_or_default();
        let hub = use_server_hub();

        let rows: Vec<Element> = list
            .iter()
            .map(|server| {
                ServerRow {
                    server: server.clone(),
                    status: hub.status(&server.id),
                }
                .into_element()
            })
            .collect();

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
            .child(if rows.is_empty() {
                centered_note(t("oneclient.servers.page.empty"))
            } else {
                ScrollArea::new()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .scrollbar_gutter(true)
                    .spacing(8.)
                    .children(rows)
                    .into_element()
            })
            .maybe_child(
                creating
                    .read()
                    .then(|| CreateServerModal::new(move |()| creating.set(false)).into_element()),
            )
    }
}

#[derive(PartialEq)]
struct ServerRow {
    server: LocalServer,
    status: ServerStatus,
}

impl Component for ServerRow {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);
        let station = use_dispatch().station();
        let files = settled_or_loading(&use_server_files(self.server.id.clone()));
        let id = self.server.id.clone();
        let open_id = id.clone();
        let status = self.status;

        let action = match status {
            ServerStatus::Stopped => Button::new()
                .secondary()
                .on_press(move |_| servers::start(station, id.clone()))
                .child(Icon::new(IconType::Play).size(14.))
                .text(t("oneclient.servers.start")),
            ServerStatus::Starting | ServerStatus::Running => Button::new()
                .danger()
                .on_press(move |_| servers::stop(station, id.clone()))
                .child(Icon::new(IconType::Square).size(14.))
                .text(t("oneclient.servers.stop")),
            _ => Button::new()
                .secondary()
                .enabled(false)
                .text(super::status_label(status)),
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(14.)
            .padding(Gaps::new_symmetric(12., 16.))
            .corner_radius(rounded(12.))
            .background(if *hovering.read() {
                colors::component_bg_hover()
            } else {
                colors::page_elevated()
            })
            .border(border_all_color(1., colors::component_border()))
            .cursor(CursorIcon::Pointer)
            .on_pointer_enter(move |_| hovering.set(true))
            .on_pointer_leave(move |_| hovering.set(false))
            .on_press(move |_| {
                let _ = RouterContext::get().push(Route::ServerOverview {
                    server_id: open_id.clone(),
                });
            })
            .child(server_icon(&self.server, files.and_then(|f| f.icon), 44.))
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(3.)
                    .child(
                        label()
                            .text(self.server.name.clone())
                            .font_size(text_size(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .max_lines(1)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text(status_line(status, &self.server))
                            .font_size(text_size(12.))
                            .color(colors::fg_secondary()),
                    ),
            )
            .child(action)
    }
}
