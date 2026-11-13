use std::path::PathBuf;
use std::sync::Arc;

use freya::prelude::*;

use super::{card, card_head};
use crate::components::{Button, Icon, IconType, LogViewer, TextInput, auto_scroll_toggle};
use crate::hooks::{
    settled_or_loading, use_dispatch, use_log_action, use_server_files, use_server_hub,
    use_upload_log,
};
use crate::i18n::t;
use crate::servers::{self, ServerStatus};
use crate::state::AppChannel;
use crate::theme::colors;
use crate::ui::centered_note;
use crate::view::app::cluster::filter_lines;
use crate::view::app::cluster::logs::{
    Confirm, LevelFilter, confirm_overlay, viewer_header, viewer_header_inset,
};

fn latest_log(server_id: &str) -> Option<PathBuf> {
    oneclient_core::servers::server_dir(server_id)
        .ok()
        .map(|dir| dir.join("logs").join("latest.log"))
        .filter(|path| path.exists())
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

        let has_log = !live.lines.is_empty();
        let lines = filter_lines(&live.lines, &search.read(), *level.read());
        let log_path = latest_log(&self.server_id);

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
                            .write_channel(AppChannel::Servers)
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

/// The last run's log file, readable while the server is stopped
#[derive(PartialEq)]
pub struct ServerConsole {
    pub server_id: String,
}

impl Component for ServerConsole {
    fn render(&self) -> impl IntoElement {
        let files = settled_or_loading(&use_server_files(self.server_id.clone()));
        let search = use_state(String::new);
        let level = use_state(|| LevelFilter::All);
        let confirm = use_state(|| None::<Confirm>);
        let search_id = use_a11y();
        let upload = use_upload_log();
        let action = use_log_action();

        let Some(files) = files else {
            return centered_note(t("oneclient.servers.loading"));
        };
        if files.latest_log.is_empty() {
            return centered_note(t("oneclient.servers.console.no_log"));
        }
        let all: Arc<Vec<Arc<str>>> = Arc::new(
            files
                .latest_log
                .iter()
                .map(|l| Arc::from(l.as_str()))
                .collect(),
        );
        let lines = filter_lines(&all, &search.read(), *level.read());
        let log_path = latest_log(&self.server_id);

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .child(LogViewer::new("latest.log", lines).header(viewer_header(
                search,
                level,
                confirm,
                true,
                log_path.clone(),
                search_id,
            )))
            .maybe_child(
                (*confirm.read())
                    .zip(log_path)
                    .map(|(kind, path)| confirm_overlay(kind, confirm, action, upload, path)),
            )
            .into_element()
    }
}
