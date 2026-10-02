use std::path::{Path, PathBuf};

use freya::prelude::*;
use freya::router::RouterContext;
use notify::{Event, EventKind};
use oneclient_content::modpacks::BlockedFile;

use crate::components::{Button, Icon, IconType, OverlayPopup};
use crate::hooks::{use_dispatch, use_folder_watch, use_notifications_snapshot};
use crate::notifications::BlockedDownloads;
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::border_all_color;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);
const DIALOG_W: f32 = 560.;
const DIALOG_PAD: f32 = 22.;
const LIST_MAX_H: f32 = 300.;
const ROW_H: f32 = 52.;

#[derive(PartialEq)]
pub struct BlockedDownloadsPopup;

impl Component for BlockedDownloadsPopup {
    fn render(&self) -> impl IntoElement {
        let snapshot = use_notifications_snapshot();
        let dispatch = use_dispatch();
        let router = RouterContext::get();

        use_side_effect_with_deps(&snapshot.open_cluster, move |requested| {
            if requested.is_none() {
                return;
            }
            if let Some(cluster_id) = dispatch.take_open_cluster() {
                let _ = router.push(Route::ClusterOverview { cluster_id });
            }
        });

        match snapshot.blocked_downloads {
            Some(blocked) => BlockedDownloadsDialog { blocked }.into_element(),
            None => rect().into_element(),
        }
    }
}

fn downloads_dir() -> Option<PathBuf> {
    directories::UserDirs::new().and_then(|dirs| dirs.download_dir().map(Path::to_path_buf))
}

fn touches_file(_root: &Path, event: &Event) -> bool {
    matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_))
}

type ExtraFolder = State<Option<(i64, PathBuf)>>;

fn bump(mut tick: State<u64>) {
    let next = *tick.peek() + 1;
    tick.set(next);
}

#[derive(PartialEq)]
struct ExtraFolderWatch {
    folder: PathBuf,
    tick: State<u64>,
    key: DiffKey,
}

impl KeyExt for ExtraFolderWatch {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for ExtraFolderWatch {
    fn render(&self) -> impl IntoElement {
        let tick = self.tick;
        use_folder_watch(Some(self.folder.clone()), false, touches_file, move || {
            bump(tick)
        });
        rect()
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

#[derive(PartialEq)]
struct BlockedDownloadsDialog {
    blocked: BlockedDownloads,
}

impl Component for BlockedDownloadsDialog {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let downloads = use_hook(downloads_dir);
        let tick = use_state(|| 0u64);
        let extra: ExtraFolder = use_state(|| None);

        use_folder_watch(downloads.clone(), false, touches_file, move || bump(tick));

        let cluster_id = self.blocked.cluster_id;
        let chosen = extra
            .read()
            .as_ref()
            .filter(|(owner, _)| *owner == cluster_id)
            .map(|(_, folder)| folder.clone());

        let deps = (
            *tick.read(),
            cluster_id,
            self.blocked.remaining(),
            chosen.clone(),
        );
        {
            let dispatch = dispatch.clone();
            let downloads = downloads.clone();
            use_side_effect_with_deps(&deps, move |(_, cluster_id, files, chosen)| {
                if files.is_empty() {
                    return;
                }
                let locations: Vec<PathBuf> =
                    downloads.iter().chain(chosen.iter()).cloned().collect();
                if !locations.is_empty() {
                    dispatch.scan_blocked_downloads(*cluster_id, locations, files.clone());
                }
            });
        }

        let close = dispatch.clone();
        OverlayPopup::new()
            .on_close(move |_| close.close_blocked_downloads())
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(dialog(
                        &self.blocked,
                        downloads.as_deref(),
                        chosen.as_deref(),
                        extra,
                        dispatch,
                    )),
            )
            .maybe_child(chosen.map(|folder| {
                let key = folder.to_string_lossy().into_owned();
                ExtraFolderWatch {
                    folder,
                    tick,
                    key: DiffKey::None,
                }
                .key(key)
                .into_element()
            }))
            .into_element()
    }
}

fn folder_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn watched_text(downloads: Option<&Path>, chosen: Option<&Path>) -> String {
    match (downloads, chosen) {
        (Some(_), Some(chosen)) => format!("Downloads and {}", chosen.display()),
        (Some(downloads), None) => downloads.display().to_string(),
        (None, Some(chosen)) => chosen.display().to_string(),
        (None, None) => "Nothing yet. Add a folder or use Browse.".to_string(),
    }
}

fn pick_extra_folder(cluster_id: i64, start: Option<PathBuf>, mut extra: ExtraFolder) {
    spawn(async move {
        let mut picker = rfd::AsyncFileDialog::new().set_title("Pick the folder you download into");
        if let Some(start) = start {
            picker = picker.set_directory(start);
        }
        if let Some(handle) = picker.pick_folder().await {
            extra.set(Some((cluster_id, handle.path().to_path_buf())));
        }
    });
}

fn folder_row(
    cluster_id: i64,
    downloads: Option<&Path>,
    chosen: Option<&Path>,
    extra: ExtraFolder,
) -> impl IntoElement {
    let start = chosen
        .map(Path::to_path_buf)
        .or_else(|| downloads.map(Path::to_path_buf));
    let has_extra = chosen.is_some();

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(6.)
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::Flex)
                .cross_align(Alignment::Center)
                .padding(Gaps::new_symmetric(8., 12.))
                .spacing(10.)
                .corner_radius(CornerRadius::new_all(10.))
                .background(colors::component_bg())
                .border(border_all_color(1., colors::component_border()))
                .child(
                    Icon::new(IconType::FolderCheck)
                        .size(16.)
                        .color(colors::fg_secondary()),
                )
                .child(
                    rect()
                        .vertical()
                        .width(Size::flex(1.))
                        .spacing(2.)
                        .child(
                            label()
                                .text("Watching")
                                .font_size(11.)
                                .color(colors::fg_secondary()),
                        )
                        .child(
                            label()
                                .text(watched_text(downloads, chosen))
                                .font_size(12.5)
                                .max_lines(2)
                                .color(colors::fg_primary()),
                        ),
                )
                .child(
                    Button::new()
                        .secondary()
                        .small()
                        .on_press(move |_| pick_extra_folder(cluster_id, start.clone(), extra))
                        .text(if has_extra {
                            "Change folder"
                        } else {
                            "Add folder"
                        }),
                )
                .maybe_child(has_extra.then(|| {
                    let mut extra = extra;
                    Button::new()
                        .ghost()
                        .small()
                        .on_press(move |_| extra.set(None))
                        .text("Remove")
                        .into_element()
                })),
        )
        .child(
            label()
                .text(
                    "Your browser saves to its own download folder. To save somewhere else, \
                     set it to ask where to save each file.",
                )
                .font_size(11.)
                .max_lines(2)
                .color(colors::fg_secondary()),
        )
}

fn dialog(
    blocked: &BlockedDownloads,
    downloads: Option<&Path>,
    chosen: Option<&Path>,
    extra: ExtraFolder,
    dispatch: crate::Actions,
) -> impl IntoElement {
    let count = blocked.files.len();
    let added = blocked.added.len().min(count);
    let done = added == count;
    let plural = crate::utils::plural(count as i64);
    let where_to = match (downloads, chosen) {
        (_, Some(chosen)) => format!(
            "Download each one from its page and it will be picked up from Downloads or {} automatically.",
            folder_label(chosen)
        ),
        (Some(_), None) => {
            "Download each one from its page and it will be picked up from your Downloads folder automatically."
                .to_string()
        }
        (None, None) => {
            "Download each one from its page, then add the folder you saved them to or use Browse."
                .to_string()
        }
    };
    let subtitle = if done {
        format!(
            "All {count} file{plural} are in {}. It is ready to play.",
            blocked.cluster_name
        )
    } else {
        format!(
            "CurseForge does not let launchers download {count} file{plural} that {} needs. {where_to}",
            blocked.cluster_name
        )
    };
    let title = if done {
        "Manual downloads added".to_string()
    } else if added > 0 {
        format!("Some files need a manual download ({added} of {count} added)")
    } else {
        "Some files need a manual download".to_string()
    };

    let cluster_id = blocked.cluster_id;
    let files = blocked.remaining();
    let browse = dispatch.clone();
    let close = dispatch.clone();
    let browse_start = chosen
        .map(Path::to_path_buf)
        .or_else(|| downloads.map(Path::to_path_buf));

    rect()
        .vertical()
        .width(Size::px(DIALOG_W))
        .height(Size::Inner)
        .max_width(Size::window_percent(95.))
        .max_height(Size::window_percent(85.))
        .overflow(Overflow::Clip)
        .corner_radius(CornerRadius::new_all(16.))
        .background(CARD_BG)
        .border(border_all_color(1., colors::component_border()))
        .shadow(Shadow::from((
            0.,
            18.,
            52.,
            0.,
            Color::from_argb(150, 0, 0, 0),
        )))
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .height(Size::Inner)
                .padding(Gaps::new_all(DIALOG_PAD))
                .spacing(14.)
                .child(
                    rect()
                        .vertical()
                        .width(Size::fill())
                        .spacing(3.)
                        .child(
                            label()
                                .text(title)
                                .font_size(17.)
                                .font_weight(FontWeight::SEMI_BOLD)
                                .color(colors::fg_primary()),
                        )
                        .child(
                            label()
                                .text(subtitle)
                                .font_size(12.5)
                                .max_lines(4)
                                .color(colors::fg_secondary()),
                        ),
                )
                .maybe_child(
                    (!done)
                        .then(|| folder_row(cluster_id, downloads, chosen, extra).into_element()),
                )
                .child(file_list(blocked))
                .child(if done {
                    let close = close.clone();
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .main_align(Alignment::End)
                        .child(
                            Button::new()
                                .primary()
                                .on_press(move |_| close.close_blocked_downloads())
                                .child(Icon::new(IconType::Check).size(15.))
                                .text("Done"),
                        )
                        .into_element()
                } else {
                    footer(cluster_id, files, browse_start, browse, close).into_element()
                }),
        )
}

fn footer(
    cluster_id: i64,
    files: Vec<BlockedFile>,
    browse_start: Option<PathBuf>,
    browse: crate::Actions,
    close: crate::Actions,
) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .main_align(Alignment::End)
        .spacing(8.)
        .child(
            Button::new()
                .ghost()
                .on_press(move |_| close.close_blocked_downloads())
                .text("Skip for now"),
        )
        .child(
            Button::new()
                .primary()
                .on_press(move |_| {
                    let dispatch = browse.clone();
                    let files = files.clone();
                    let start = browse_start.clone();
                    spawn(async move {
                        let mut picker =
                            rfd::AsyncFileDialog::new().set_title("Pick the downloaded files");
                        if let Some(start) = start {
                            picker = picker.set_directory(start);
                        }
                        if let Some(handles) = picker.pick_files().await {
                            let picked = handles
                                .iter()
                                .map(|handle| handle.path().to_path_buf())
                                .collect();
                            dispatch.scan_blocked_downloads(cluster_id, picked, files);
                        }
                    });
                })
                .child(Icon::new(IconType::Folder).size(15.))
                .text("Browse"),
        )
}

fn file_list(blocked: &BlockedDownloads) -> impl IntoElement {
    ScrollView::new()
        .width(Size::fill())
        .height(Size::Inner)
        .max_height(Size::px(LIST_MAX_H))
        .child(
            rect().vertical().width(Size::fill()).spacing(6.).children(
                blocked
                    .files
                    .iter()
                    .map(|file| file_row(file, blocked.is_added(file)).into_element()),
            ),
        )
}

fn file_row(file: &BlockedFile, added: bool) -> impl IntoElement {
    let page = file.page_url.clone();
    let (background, border) = if added {
        (colors::success().with_a(28), colors::success().with_a(120))
    } else {
        (colors::component_bg(), colors::component_border())
    };

    let trailing = if added {
        rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(6.)
            .padding(Gaps::new_symmetric(0., 4.))
            .child(
                Icon::new(IconType::CheckCircle)
                    .size(14.)
                    .color(colors::success()),
            )
            .child(
                label()
                    .text("Added")
                    .font_size(12.)
                    .font_weight(FontWeight::SEMI_BOLD)
                    .color(colors::success()),
            )
            .into_element()
    } else {
        Button::new()
            .secondary()
            .small()
            .enabled(page.is_some())
            .on_press(move |_| {
                if let Some(url) = &page {
                    crate::platform::open_url(url);
                }
            })
            .child(Icon::new(IconType::LinkExternal01).size(13.))
            .text("Open page")
            .into_element()
    };

    rect()
        .key(file.sha1.clone())
        .horizontal()
        .width(Size::fill())
        .height(Size::px(ROW_H))
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .padding(Gaps::new_symmetric(0., 12.))
        .spacing(10.)
        .corner_radius(CornerRadius::new_all(10.))
        .background(background)
        .border(border_all_color(1., border))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.))
                .spacing(2.)
                .child(
                    label()
                        .text(file.project_name.clone())
                        .font_size(13.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .max_lines(1)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(file.file_name.clone())
                        .font_size(11.)
                        .max_lines(1)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(trailing)
}
