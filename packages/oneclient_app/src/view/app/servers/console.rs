use std::path::PathBuf;

use freya::prelude::*;
use oneclient_core::LogFileInfo;

use super::{card, card_head};
use crate::components::{Button, Icon, IconType, LogViewer, TextInput, auto_scroll_toggle};
use crate::hooks::{
    settled_or_loading, use_dispatch, use_log_action, use_server_console, use_server_hub,
    use_server_logs, use_upload_log,
};
use crate::i18n::t;
use crate::servers::{self, ServerStatus};
use crate::state::AppChannel;
use crate::theme::colors;
use crate::view::app::cluster::filter_lines;
use crate::view::app::cluster::logs::{
    Category, Confirm, LevelFilter, LogBrowser, confirm_overlay, viewer_header_inset,
};

fn log_file(server_id: &str) -> Option<PathBuf> {
    oneclient_core::servers::server_dir(server_id)
        .ok()
        .map(|dir| dir.join("logs").join("latest.log"))
}

/// Live output with the same filters as the cluster log viewer and a command box,
/// `height` fixes it for the overview card
#[derive(PartialEq)]
pub struct ConsolePanel {
    pub server_id: String,
    pub height: Option<f32>,
}

impl Component for ConsolePanel {
    fn render(&self) -> impl IntoElement {
        let hub = use_server_hub();
        let live = hub.get(&self.server_id);
        let search = use_state(String::new);
        let level = use_state(|| LevelFilter::All);
        let confirm = use_state(|| None::<Confirm>);
        let search_id = use_a11y();
        let auto_scroll = use_state(|| true);
        let upload = use_upload_log();
        let action = use_log_action();
        let mut command = use_state(String::new);
        let dispatch = use_dispatch();
        let running = live.status.is_live();

        let all = use_server_console(&self.server_id);
        let has_log = !all.is_empty();
        let lines = filter_lines(&all, &search.read(), *level.read());
        let log_path = has_log.then(|| log_file(&self.server_id)).flatten();

        let subtitle = match live.status {
            ServerStatus::Stopped => t("oneclient.servers.console.stopped"),
            ServerStatus::Installing => t("oneclient.servers.status.installing"),
            ServerStatus::Starting => t("oneclient.servers.status.starting"),
            ServerStatus::Running => t("oneclient.servers.console.running"),
            ServerStatus::Stopping => t("oneclient.servers.status.stopping"),
        };

        let clear_id = self.server_id.clone();
        let mut station = dispatch.station();
        let controls = rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(10.)
            .child(auto_scroll_toggle(auto_scroll))
            .child(
                Button::new()
                    .secondary()
                    .enabled(has_log)
                    .on_press(move |_| {
                        station
                            .write_channel(AppChannel::ServerConsole)
                            .servers
                            .clear_lines(&clear_id);
                    })
                    .child(Icon::new(IconType::Trash01).size(14.))
                    .text(t("oneclient.servers.console.clear")),
            )
            .into_element();

        let send_id = self.server_id.clone();
        let mut send = move |text: String| {
            if !text.trim().is_empty() && servers::send_command(&send_id, &text) {
                command.set(String::new());
            }
        };
        let mut send_again = send.clone();

        let input = rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(10.)
            .child(
                TextInput::new(command)
                    .width(Size::flex(1.0))
                    .enabled(running)
                    .font_family(crate::theme::mono_font())
                    .placeholder(if running {
                        t("oneclient.servers.console.placeholder")
                    } else {
                        t("oneclient.servers.console.placeholder_stopped")
                    })
                    .leading(
                        Icon::new(IconType::Terminal)
                            .size(16.)
                            .color(colors::fg_secondary())
                            .into_element(),
                    )
                    .on_submit(move |text: String| send(text)),
            )
            .child(
                Button::new()
                    .primary()
                    .enabled(running)
                    .on_press(move |_| send_again(command.peek().clone()))
                    .child(Icon::new(IconType::Send01).size(14.))
                    .text(t("oneclient.servers.console.send")),
            );

        card()
            .maybe(self.height.is_none(), |el| el.height(Size::fill()))
            .content(Content::Flex)
            .child(card_head(
                t("oneclient.servers.console.title"),
                subtitle,
                Some(controls),
            ))
            .child(
                rect()
                    .width(Size::fill())
                    .height(self.height.map_or_else(|| Size::flex(1.0), Size::px))
                    .child(
                        LogViewer::new("", lines)
                            .streaming(true)
                            .auto_scroll(auto_scroll)
                            .header(viewer_header_inset(
                                search,
                                level,
                                confirm,
                                has_log,
                                log_path.clone(),
                                search_id,
                                0.,
                            )),
                    ),
            )
            .child(input)
            .maybe_child(
                (*confirm.read())
                    .zip(log_path)
                    .map(|(kind, path)| confirm_overlay(kind, confirm, action, upload, path)),
            )
    }
}

#[derive(PartialEq)]
pub struct ServerLiveConsole {
    pub server_id: String,
}

impl Component for ServerLiveConsole {
    fn render(&self) -> impl IntoElement {
        ConsolePanel {
            server_id: self.server_id.clone(),
            height: None,
        }
    }
}

#[derive(PartialEq)]
pub struct ServerConsole {
    pub server_id: String,
}

impl Component for ServerConsole {
    fn render(&self) -> impl IntoElement {
        let logs = use_server_logs(self.server_id.clone());
        let confirm = use_state(|| None::<Confirm>);
        let category = use_state(|| Category::Logs);
        let mut selected = use_state(PathBuf::new);

        let visible = move |category: Category| -> Vec<LogFileInfo> {
            settled_or_loading(&logs)
                .unwrap_or_default()
                .into_iter()
                .filter(|file| category.accepts(file.kind))
                .collect()
        };

        use_side_effect(move || {
            let files = visible(*category.read());
            let current = selected.peek().clone();
            if files.iter().any(|f| f.path == current) {
                return;
            }
            let pick = files
                .iter()
                .find(|f| f.name == "latest.log")
                .or_else(|| files.first())
                .map(|f| f.path.clone())
                .unwrap_or_default();
            if pick != current {
                selected.set(pick);
            }
        });

        LogBrowser {
            files: visible(*category.read()),
            folder: log_file(&self.server_id).and_then(|p| p.parent().map(Into::into)),
            selected,
            category,
            confirm,
            extra: None,
        }
    }
}
