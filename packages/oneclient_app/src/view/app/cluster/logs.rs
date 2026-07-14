use std::path::PathBuf;
use std::sync::Arc;

use freya::prelude::*;
use freya::query::MutationStateData;
use freya::text_edit::Clipboard;
use oneclient_core::{LogFileInfo, LogKind, LogLevel};

use crate::components::{
    Button, Icon, IconType, LogViewer, OverlayPopup, ScrollArea, Segment, SegmentedControl,
    TextInput, open_folder_button,
};
use crate::hooks::{
    LogAction, UploadLogKeys, UseLogAction, UseUploadLog, invalidate_logs_queries,
    try_cluster_logs, try_log_content, use_cluster_logs, use_dispatch, use_log_action,
    use_log_content, use_upload_log,
};
use crate::layout::cluster_content;
use crate::theme::colors;
use crate::ui;
use crate::utils::format_size;

use super::{cluster_not_found, load_cluster};
const CARD_BG: Color = Color::from_rgb(26, 34, 41);
const PICKER_ROW_H: f32 = 44.;
const PICKER_ROW_SPACING: f32 = 2.;
const PICKER_LIST_MAX_H: f32 = 300.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LevelFilter {
    All,
    Error,
    Warn,
    Info,
    Debug,
}

impl LevelFilter {
    fn to_level(self) -> Option<LogLevel> {
        match self {
            Self::All => None,
            Self::Error => Some(LogLevel::Error),
            Self::Warn => Some(LogLevel::Warn),
            Self::Info => Some(LogLevel::Info),
            Self::Debug => Some(LogLevel::Debug),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Error => "Errors",
            Self::Warn => "Warnings",
            Self::Info => "Info",
            Self::Debug => "Debug",
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Confirm {
    Clear,
    Upload,
}

fn kind_badge(kind: LogKind) -> &'static str {
    match kind {
        LogKind::Game { .. } => "Launcher",
        LogKind::Minecraft => "Minecraft",
        LogKind::CrashReport => "Crash report",
        LogKind::Other => "Other",
    }
}

#[derive(PartialEq)]
pub struct ClusterLogs {
    pub cluster_id: i64,
}

impl Component for ClusterLogs {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;

        let logs_query = use_cluster_logs(cluster_id);
        let upload = use_upload_log();
        let action = use_log_action();
        let dispatch = use_dispatch();
        let search = use_state(String::new);
        let level = use_state(|| LevelFilter::All);
        let confirm = use_state(|| None::<Confirm>);
        let mut selected = use_state(PathBuf::new);
        let mut handled_upload = use_state(|| None::<String>);

        use_side_effect(move || {
            let files = try_cluster_logs(&logs_query).unwrap_or_default();

            if files.is_empty() {
                if !selected.peek().as_os_str().is_empty() {
                    selected.set(PathBuf::new());
                }
                return;
            }

            let current = selected.peek().clone();
            let valid = !current.as_os_str().is_empty() && files.iter().any(|f| f.path == current);

            if !valid {
                let pick = files
                    .iter()
                    .find(|f| f.name == "latest.log")
                    .or_else(|| files.first());

                if let Some(file) = pick {
                    selected.set(file.path.clone());
                }
            }
        });

        use_side_effect(move || match &*upload.read().state() {
            MutationStateData::Settled {
                res: Ok(result), ..
            } => {
                if handled_upload.peek().as_deref() == Some(result.url.as_str()) {
                    return;
                }

                handled_upload.set(Some(result.url.clone()));
                let _ = Clipboard::set(result.url.clone());

                dispatch
                    .notify("Uploaded to mclo.gs")
                    .body(format!("{} (copied to clipboard)", result.url))
                    .info()
                    .icon(IconType::LinkExternal01)
                    .send();
            }
            MutationStateData::Settled { res: Err(err), .. } => {
                let msg = err.to_string();
                if handled_upload.peek().as_deref() == Some(msg.as_str()) {
                    return;
                }

                handled_upload.set(Some(msg.clone()));
                dispatch.notify("Upload failed").body(msg).error().send();
            }
            _ => {}
        });

        let selected_path = selected.read().clone();
        let search_query = {
            let value = search.read().clone();
            (!value.trim().is_empty()).then_some(value)
        };
        
        let content_query = use_log_content(
            selected_path.clone(),
            level.read().to_level(),
            search_query,
            None,
        );

        let Some(cluster) = load_cluster(cluster_id) else {
            return cluster_not_found();
        };
        let folder = cluster.game_dir().ok().map(|d| d.join("logs"));

        let files = try_cluster_logs(&logs_query).unwrap_or_default();
        let has_log = !files.is_empty();
        let selected_info = files.iter().find(|f| f.path == selected_path).cloned();

        let lines = try_log_content(&content_query).unwrap_or_default();
        let viewer_lines: Arc<Vec<Arc<str>>> = Arc::new(
            lines
                .iter()
                .map(|line| Arc::<str>::from(line.text.as_str()))
                .collect(),
        );

        let size_text = selected_info
            .as_ref()
            .map(|i| format!("{} on disk", format_size(i.size_bytes)))
            .unwrap_or_else(|| "No logs recorded yet".to_string());

        let body = rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .spacing(12.)
            .child(top_bar(
                files.clone(),
                selected_info.clone(),
                size_text,
                has_log,
                folder,
                selected,
                confirm,
            ))
            .child(
                rect().width(Size::fill()).height(Size::flex(1.0)).child(
                    LogViewer::new("", viewer_lines)
                        .header(viewer_header(search, level, confirm, has_log)),
                ),
            );

        cluster_content()
            .child(body.into_element())
            .maybe_child((*confirm.read()).map(move |kind| {
                confirm_overlay(kind, confirm, action, upload, selected_path.clone())
            }))
            .into_element()
    }
}

fn top_bar(
    files: Vec<LogFileInfo>,
    selected_info: Option<LogFileInfo>,
    size_text: String,
    has_log: bool,
    folder: Option<PathBuf>,
    selected: State<PathBuf>,
    mut confirm: State<Option<Confirm>>,
) -> impl IntoElement {
    let picker_label = selected_info
        .as_ref()
        .map(|i| i.name.clone())
        .unwrap_or_else(|| "No logs".to_string());

    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .content(Content::Flex)
        .spacing(10.)
        .child(
            LogPicker {
                label: picker_label,
                files,
                enabled: has_log,
                on_select: (move |path: PathBuf| {
                    let mut selected = selected;
                    selected.set(path);
                })
                .into(),
            }
            .into_element(),
        )
        .child(
            label()
                .text(size_text)
                .font_size(11.)
                .max_lines(1)
                .color(colors::fg_secondary()),
        )
        .child(rect().width(Size::flex(1.0)))
        .maybe_child(folder.map(open_folder_button))
        .child(
            Button::new()
                .secondary()
                .on_press(move |_| {
                    spawn(async move {
                        invalidate_logs_queries().await;
                    });
                })
                .child(Icon::new(IconType::RefreshCw01).size(15.))
                .text("Refresh"),
        )
        .child(
            Button::new()
                .danger()
                .enabled(has_log)
                .on_press(move |_| confirm.set(Some(Confirm::Clear)))
                .child(Icon::new(IconType::Trash01).size(15.))
                .text("Clear log"),
        )
        .into_element()
}

fn viewer_header(
    search: State<String>,
    level: State<LevelFilter>,
    mut confirm: State<Option<Confirm>>,
    has_log: bool,
) -> impl IntoElement {
    let search_input = {
        let base = TextInput::new(search).enabled(has_log).leading(
            Icon::new(IconType::SearchMd)
                .size(15.)
                .color(colors::fg_secondary())
                .into_element(),
        );
        if has_log {
            base.placeholder("Search this log...")
        } else {
            base
        }
    };

    let row = rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .content(Content::Flex)
        .spacing(10.)
        .padding(Gaps::new_symmetric(10., 14.))
        .child(rect().width(Size::flex(1.0)).child(search_input))
        .child(
            SegmentedControl::new(level)
                .no_tint()
                .disabled(!has_log)
                .segments(
                    [
                        LevelFilter::All,
                        LevelFilter::Error,
                        LevelFilter::Warn,
                        LevelFilter::Info,
                        LevelFilter::Debug,
                    ]
                    .into_iter()
                    .map(|filter| Segment::new(filter).label(filter.label())),
                )
                .into_element(),
        )
        .child(
            Button::new()
                .secondary()
                .enabled(has_log)
                .on_press(move |_| confirm.set(Some(Confirm::Upload)))
                .child(Icon::new(IconType::LinkExternal01).size(15.))
                .text("Upload to mclo.gs"),
        );

    rect()
        .vertical()
        .width(Size::fill())
        .child(row)
        .child(
            rect()
                .width(Size::fill())
                .height(Size::px(1.))
                .background(colors::component_border()),
        )
        .into_element()
}

#[derive(PartialEq)]
struct LogPicker {
    label: String,
    files: Vec<LogFileInfo>,
    enabled: bool,
    on_select: EventHandler<PathBuf>,
}

impl Component for LogPicker {
    fn render(&self) -> impl IntoElement {
        let mut open = use_state(|| false);
        let filter = use_state(String::new);

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);

        let enabled = self.enabled;
        let label_text = self.label.clone();
        let files = self.files.clone();
        let on_select = self.on_select.clone();
        let is_open = open() && enabled;

        let query = filter.read().to_lowercase();
        let rows: Vec<Element> = files
            .into_iter()
            .filter(|f| {
                query.is_empty()
                    || f.name.to_lowercase().contains(&query)
                    || kind_badge(f.kind).to_lowercase().contains(&query)
            })
            .map(|file| {
                let on_select = on_select.clone();
                let path = file.path.clone();
                PickerRow {
                    file,
                    on_press: (move |_: Event<PressEventData>| {
                        on_select.call(path.clone());
                        open.set(false);
                    })
                    .into(),
                }
                .into_element()
            })
            .collect();
        let list_h = ((rows.len() as f32 * (PICKER_ROW_H + PICKER_ROW_SPACING))
            - PICKER_ROW_SPACING)
            .clamp(0., PICKER_LIST_MAX_H);

        rect()
            .width(Size::px(260.))
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::px(36.))
                    .horizontal()
                    .cross_align(Alignment::Center)
                    .content(Content::Flex)
                    .spacing(8.)
                    .padding(Gaps::new_symmetric(0., 12.))
                    .corner_radius(CornerRadius::new_all(9.))
                    .background(colors::component_bg())
                    .a11y_id(a11y_id)
                    .a11y_focusable(enabled)
                    .a11y_role(AccessibilityRole::Button)
                    .border(ui::border_all_color(
                        1.,
                        if enabled && focus().is_focused() {
                            colors::brand()
                        } else {
                            colors::component_border()
                        },
                    ))
                    .on_pointer_enter(move |_| {
                        if enabled {
                            Cursor::set(CursorIcon::Pointer)
                        }
                    })
                    .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
                    .on_all_press(move |e: Event<PressEventData>| {
                        if !enabled {
                            return;
                        }
                        e.stop_propagation();
                        open.toggle();
                    })
                    .child(
                        rect().width(Size::flex(1.0)).child(
                            label().text(label_text).font_size(13.).max_lines(1).color(
                                if enabled {
                                    colors::fg_primary()
                                } else {
                                    colors::fg_secondary().with_a(110)
                                },
                            ),
                        ),
                    )
                    .child(
                        Icon::new(IconType::ChevronDown)
                            .size(15.)
                            .color(colors::fg_secondary()),
                    ),
            )
            .maybe_child(is_open.then(|| {
                rect()
                    .height(Size::px(0.))
                    .width(Size::px(0.))
                    .layer(Layer::Overlay)
                    .child(
                        rect()
                            .position(Position::new_global().top(0.).left(0.))
                            .width(Size::window_percent(100.))
                            .height(Size::window_percent(100.))
                            .layer(Layer::RelativeOverlay(10))
                            .on_press(move |_| open.set(false)),
                    )
                    .child(
                        rect()
                            .margin(Gaps::new(4., 0., 0., 0.))
                            .width(Size::px(320.))
                            .max_height(Size::px(360.))
                            .layer(Layer::RelativeOverlay(12))
                            .vertical()
                            .spacing(4.)
                            .padding(6.)
                            .corner_radius(CornerRadius::new_all(10.))
                            .border(ui::border_all_color(1., colors::component_border()))
                            .background(colors::page_elevated())
                            .overflow(Overflow::Clip)
                            .child(
                                TextInput::new(filter)
                                    .auto_focus(true)
                                    .placeholder("Search logs...")
                                    .leading(
                                        Icon::new(IconType::SearchMd)
                                            .size(14.)
                                            .color(colors::fg_secondary())
                                            .into_element(),
                                    ),
                            )
                            .child(
                                ScrollArea::new()
                                    .height(Size::px(list_h))
                                    .spacing(PICKER_ROW_SPACING)
                                    .children(rows),
                            ),
                    )
            }))
    }
}

struct PickerRow {
    file: LogFileInfo,
    on_press: EventHandler<Event<PressEventData>>,
}

impl PartialEq for PickerRow {
    fn eq(&self, other: &Self) -> bool {
        self.file.path == other.file.path
    }
}

impl Component for PickerRow {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);
        let file = self.file.clone();

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);

        rect()
            .width(Size::fill())
            .horizontal()
            .cross_align(Alignment::Center)
            .content(Content::Flex)
            .spacing(8.)
            .padding(Gaps::new_symmetric(6., 8.))
            .corner_radius(CornerRadius::new_all(7.))
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .background(if hovering() || focus().is_focused() {
                colors::ghost_overlay_hover()
            } else {
                Color::TRANSPARENT
            })
            .on_pointer_enter(move |_| {
                hovering.set(true);
                Cursor::set(CursorIcon::Pointer);
            })
            .on_pointer_leave(move |_| {
                hovering.set(false);
                Cursor::set(CursorIcon::default());
            })
            .on_all_press(self.on_press.clone())
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(1.)
                    .child(
                        label()
                            .text(file.name.clone())
                            .font_size(13.)
                            .max_lines(1)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text(format!(
                                "{}  •  {}",
                                kind_badge(file.kind),
                                format_size(file.size_bytes)
                            ))
                            .font_size(10.)
                            .color(colors::fg_secondary()),
                    ),
            )
            .into_element()
    }
}

fn confirm_overlay(
    kind: Confirm,
    mut confirm: State<Option<Confirm>>,
    action: UseLogAction,
    upload: UseUploadLog,
    path: PathBuf,
) -> impl IntoElement {
    let (title, body, confirm_label, danger) = match kind {
        Confirm::Clear => (
            "Delete this log?",
            "This log file will be permanently removed from disk.".to_string(),
            "Delete",
            true,
        ),
        Confirm::Upload => (
            "Upload to mclo.gs?",
            "This log will be uploaded to mclo.gs and hosted publicly. Anyone with the link can read it. Don't upload logs containing sensitive information.".to_string(),
            "Upload",
            false,
        ),
    };

    let run = move |_| {
        match kind {
            Confirm::Clear => action.mutate(LogAction::Delete { path: path.clone() }),
            Confirm::Upload => upload.mutate(UploadLogKeys { path: path.clone() }),
        }
        confirm.set(None);
    };

    OverlayPopup::new()
        .on_close(move |_| confirm.set(None))
        .child(
            rect()
                .width(Size::window_percent(100.))
                .height(Size::window_percent(100.))
                .center()
                .child(
                    rect()
                        .vertical()
                        .width(Size::px(440.))
                        .max_width(Size::window_percent(90.))
                        .spacing(14.)
                        .padding(Gaps::new_all(20.))
                        .corner_radius(CornerRadius::new_all(14.))
                        .background(CARD_BG)
                        .border(ui::border_all_color(1., colors::component_border()))
                        .child(
                            rect()
                                .horizontal()
                                .cross_align(Alignment::Center)
                                .spacing(10.)
                                .child(Icon::new(IconType::AlertTriangle).size(20.).color(
                                    if danger {
                                        colors::code_error()
                                    } else {
                                        colors::code_warn()
                                    },
                                ))
                                .child(
                                    label()
                                        .text(title)
                                        .font_size(16.)
                                        .font_weight(FontWeight::SEMI_BOLD)
                                        .color(colors::fg_primary()),
                                ),
                        )
                        .child(
                            label()
                                .text(body)
                                .font_size(12.)
                                .max_lines(6)
                                .width(Size::fill())
                                .color(colors::fg_secondary()),
                        )
                        .child(
                            rect()
                                .horizontal()
                                .width(Size::fill())
                                .main_align(Alignment::End)
                                .spacing(8.)
                                .child(
                                    Button::new()
                                        .secondary()
                                        .on_press(move |_| confirm.set(None))
                                        .text("Cancel"),
                                )
                                .child(
                                    if danger {
                                        Button::new().danger()
                                    } else {
                                        Button::new().primary()
                                    }
                                    .on_press(run)
                                    .text(confirm_label),
                                ),
                        ),
                ),
        )
        .into_element()
}
