use std::cell::RefCell;
use std::rc::Rc;
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
use crate::view::app::cluster::logs::{Confirm, LevelFilter, confirm_overlay, viewer_controls};

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

        let mut search = use_state(String::new);
        let mut level = use_state(|| LevelFilter::All);
        let confirm = use_state(|| None::<Confirm>);
        let search_id = use_a11y();
        let filter = use_hook(|| Rc::new(RefCell::new(LiveLogFilter::new())));

        let game = use_game_snapshot();
        let Some(_cluster) = use_cluster(self.cluster_id) else {
            return cluster_not_found();
        };

        let active = game.stage(self.cluster_id).is_some();

        let output_path = try_cluster_logs(&logs_query)
            .unwrap_or_default()
            .into_iter()
            .find(|file| matches!(file.kind, LogKind::Game { .. }))
            .map(|file| file.path);

        let body = if active {
            let lines = game.logs_for(self.cluster_id);
            let has_log = !lines.is_empty();
            let (visible, numbers) = filter
                .borrow_mut()
                .update(&lines, &search.read(), *level.read());
            let on_jump = numbers.is_some().then(|| {
                EventHandler::from(move |()| {
                    search.set(String::new());
                    level.set(LevelFilter::All);
                })
            });

            LogViewer::new(t("oneclient.cluster.process_logs.title"), visible)
                .numbers(numbers)
                .on_jump(on_jump)
                .streaming(true)
                .actions(viewer_controls(
                    search,
                    level,
                    confirm,
                    has_log,
                    output_path.clone().filter(|_| has_log),
                    search_id,
                ))
                .into_element()
        } else {
            *filter.borrow_mut() = LiveLogFilter::new();
            not_running()
        };

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

struct LiveLogFilter {
    search: String,
    level: LevelFilter,
    processed: usize,
    last_line: Option<Arc<str>>,
    carried: LogLevel,
    visible: Arc<Vec<Arc<str>>>,
    numbers: Arc<Vec<usize>>,
}

impl LiveLogFilter {
    fn new() -> Self {
        Self {
            search: String::new(),
            level: LevelFilter::All,
            processed: 0,
            last_line: None,
            carried: LogLevel::Unknown,
            visible: Arc::default(),
            numbers: Arc::default(),
        }
    }

    fn update(
        &mut self,
        lines: &Arc<Vec<Arc<str>>>,
        search: &str,
        level: LevelFilter,
    ) -> (Arc<Vec<Arc<str>>>, Option<Arc<Vec<usize>>>) {
        let wanted = level.to_level();
        if search.trim().is_empty() && wanted.is_none() {
            if self.processed != 0 {
                *self = Self::new();
            }
            return (lines.clone(), None);
        }

        // A live log only appends. A new session replaces the buffer with fresh lines.
        let replaced = self.last_line.as_ref().is_some_and(|last| {
            !lines
                .get(self.processed - 1)
                .is_some_and(|line| Arc::ptr_eq(line, last))
        });
        if replaced || search != self.search || level != self.level {
            *self = Self::new();
            self.search = search.to_owned();
            self.level = level;
        }

        let needle = (!search.trim().is_empty()).then(|| search.to_lowercase());
        for (index, line) in lines.iter().enumerate().skip(self.processed) {
            if let Some(wanted) = wanted {
                self.carried = parse_level(line).unwrap_or(self.carried);
                if wanted != self.carried {
                    continue;
                }
            }
            if needle
                .as_ref()
                .is_some_and(|needle| !line.to_lowercase().contains(needle))
            {
                continue;
            }
            Arc::make_mut(&mut self.visible).push(line.clone());
            Arc::make_mut(&mut self.numbers).push(index + 1);
        }

        self.processed = lines.len();
        self.last_line = lines.last().cloned();
        (self.visible.clone(), Some(self.numbers.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appended_lines_inherit_the_previous_log_level() {
        let mut filter = LiveLogFilter::new();
        let mut lines = Arc::new(vec![Arc::from("[Render thread/WARN] warning")]);
        let (first, _) = filter.update(&lines, "", LevelFilter::Warn);
        Arc::make_mut(&mut lines).extend([Arc::from("continuation"), Arc::from("[INFO] ready")]);

        let (visible, numbers) = filter.update(&lines, "", LevelFilter::Warn);
        assert_eq!(first.len(), 1);
        assert_eq!(visible.len(), 2);
        assert_eq!(&*visible[1], "continuation");
        assert_eq!(*numbers.unwrap(), vec![1, 2]);
    }

    #[test]
    fn filtered_lines_keep_their_original_numbers() {
        let mut filter = LiveLogFilter::new();
        let lines = Arc::new(vec![Arc::from("[INFO] a"), Arc::from("[WARN] needle")]);
        let (_, numbers) = filter.update(&lines, "needle", LevelFilter::All);
        assert_eq!(*numbers.unwrap(), vec![2]);
    }

    #[test]
    fn a_new_log_or_filter_rebuilds_results() {
        let mut filter = LiveLogFilter::new();
        let first = Arc::new(vec![Arc::from("[WARN] First needle")]);
        assert_eq!(filter.update(&first, "NEEDLE", LevelFilter::All).0.len(), 1);

        let next = Arc::new(vec![Arc::from("[INFO] Next session")]);
        assert!(filter.update(&next, "NEEDLE", LevelFilter::All).0.is_empty());
        assert_eq!(filter.update(&next, "NEXT", LevelFilter::Info).0.len(), 1);
        assert!(Arc::ptr_eq(
            &next,
            &filter.update(&next, "", LevelFilter::All).0
        ));
    }
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
