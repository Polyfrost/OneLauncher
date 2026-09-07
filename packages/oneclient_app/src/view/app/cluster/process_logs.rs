use std::sync::Arc;

use freya::prelude::*;
use oneclient_cluster::logs::parse_level;
use oneclient_core::{LogKind, LogLevel};

use crate::components::LogViewer;
use crate::components::upload_mclogs::use_mclogs_feedback;
use crate::hooks::{
    try_cluster_logs, use_cluster, use_cluster_logs, use_game_snapshot, use_log_action,
    use_upload_log,
};
use crate::layout::cluster_content;
use crate::theme::colors;
use crate::view::app::cluster::logs::{Confirm, LevelFilter, confirm_overlay, viewer_header};

use super::cluster_not_found;

#[derive(PartialEq)]
pub struct ProcessLogs {
    pub cluster_id: i64,
}

impl Component for ProcessLogs {
    fn render(&self) -> impl IntoElement {
        let logs_query = use_cluster_logs(self.cluster_id);
        let upload = use_upload_log();
        let action = use_log_action();

        let search = use_state(String::new);
        let level = use_state(|| LevelFilter::All);
        let confirm = use_state(|| None::<Confirm>);

        use_mclogs_feedback(upload);

        let game = use_game_snapshot();
        let Some(_cluster) = use_cluster(self.cluster_id) else {
            return cluster_not_found();
        };

        let active = game.stage(self.cluster_id).is_some();

        let body = if active {
            let lines = game.logs_for(self.cluster_id);
            let has_log = !lines.is_empty();
            let visible = filter_lines(&lines, &search.read(), *level.read());

            LogViewer::new("Game output", visible)
                .streaming(true)
                .header(viewer_header(search, level, confirm, has_log))
                .into_element()
        } else {
            not_running()
        };

        let output_path = try_cluster_logs(&logs_query)
            .unwrap_or_default()
            .into_iter()
            .find(|file| matches!(file.kind, LogKind::Game { .. }))
            .map(|file| file.path);

        cluster_content()
            .child(body)
            .maybe_child(
                (*confirm.read())
                    .zip(output_path)
                    .map(|(kind, path)| confirm_overlay(kind, confirm, action, upload, path)),
            )
            .into_element()
    }
}

fn filter_lines(lines: &Arc<Vec<Arc<str>>>, search: &str, level: LevelFilter) -> Arc<Vec<Arc<str>>> {
    let needle = (!search.trim().is_empty()).then(|| search.to_lowercase());
    let wanted = level.to_level();

    if needle.is_none() && wanted.is_none() {
        return lines.clone();
    }

    let mut carried = LogLevel::Unknown;
    let mut out = Vec::new();

    for line in lines.iter() {
        carried = parse_level(line).unwrap_or(carried);

        if wanted.is_some_and(|wanted| wanted != carried) {
            continue;
        }
        if needle
            .as_ref()
            .is_some_and(|needle| !line.to_lowercase().contains(needle))
        {
            continue;
        }

        out.push(line.clone());
    }

    Arc::new(out)
}

fn not_running() -> Element {
    rect()
        .width(Size::fill())
        .height(Size::fill())
        .center()
        .child(
            label()
                .text("This cluster is not running.")
                .font_size(14.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}
