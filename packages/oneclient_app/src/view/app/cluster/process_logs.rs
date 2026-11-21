use std::path::PathBuf;
use std::sync::Arc;

use freya::prelude::*;
use oneclient_cluster::logs::parse_level;
use oneclient_core::{LogKind, LogLevel};

use crate::components::LogViewer;
use crate::hooks::{
    try_cluster_logs, use_cluster, use_cluster_logs, use_game_snapshot, use_log_action,
    use_upload_log,
};
use crate::i18n::t;
use crate::layout::cluster_content;
use crate::theme::{colors, text_size};
use crate::view::app::cluster::logs::{
    Confirm, LevelFilter, confirm_overlay, use_thread_dump, viewer_header,
};

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
        let search_id = use_a11y();
        let thread_dump = use_thread_dump(self.cluster_id);
        let mut dump_confirm = use_state(|| None::<Confirm>);
        let mut dump_path = use_state(PathBuf::new);

        let game = use_game_snapshot();
        let Some(_cluster) = use_cluster(self.cluster_id) else {
            return cluster_not_found();
        };

        let active = game.stage(self.cluster_id).is_some();
        let dump_button = game.is_running(self.cluster_id).then(|| {
            thread_dump.button(move |path, prompt| {
                dump_path.set(path);
                dump_confirm.set(Some(prompt));
            })
        });

        let output_path = try_cluster_logs(&logs_query)
            .unwrap_or_default()
            .into_iter()
            .find(|file| matches!(file.kind, LogKind::Game { .. }))
            .map(|file| file.path);

        let body = if active {
            let lines = game.logs_for(self.cluster_id);
            let has_log = !lines.is_empty();
            let visible = filter_lines(&lines, &search.read(), *level.read());

            LogViewer::new(t("oneclient.cluster.process_logs.title"), visible)
                .streaming(true)
                .header(viewer_header(
                    search,
                    level,
                    confirm,
                    has_log,
                    output_path.clone().filter(|_| has_log),
                    search_id,
                    dump_button,
                ))
                .into_element()
        } else {
            not_running()
        };

        cluster_content()
            .child(body)
            .maybe_child(
                (*confirm.read())
                    .zip(output_path)
                    .map(|(kind, path)| confirm_overlay(kind, confirm, action, upload, path)),
            )
            .maybe_child((*dump_confirm.read()).map(|kind| {
                confirm_overlay(kind, dump_confirm, action, upload, dump_path.read().clone())
            }))
            .into_element()
    }
}

fn filter_lines(
    lines: &Arc<Vec<Arc<str>>>,
    search: &str,
    level: LevelFilter,
) -> Arc<Vec<Arc<str>>> {
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
                .text(t("oneclient.cluster.process_logs.not_running"))
                .font_size(text_size(14.))
                .color(colors::fg_secondary()),
        )
        .into_element()
}
