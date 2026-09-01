use std::collections::HashMap;
use std::path::{Path, PathBuf};

use freya::animation::{AnimNum, Ease, Function, OnCreation, use_animation};
use freya::prelude::*;
use freya::router::use_route;
use oneclient_core::clusters::Cluster;
use oneclient_content::packages::ContentType;
use oneclient_db::models::ClusterId;

use crate::Route;
use crate::components::{Button, Dropdown, Icon, IconType, OverlayPopup, ScrollArea};
use crate::hooks::{settled_or_loading, use_active_cluster_id, use_clusters, use_dispatch};
use crate::theme::colors;
use crate::ui::border_all_color;

const DIALOG_BG: Color = Color::from_rgb(26, 34, 41);
const DIALOG_W: f32 = 480.;
const FILE_ROW_H: f32 = 34.;
const FILE_LIST_MAX_ROWS: usize = 5;
const TYPE_DROPDOWN_W: f32 = 108.;

/// Worlds are directories and modpacks/datapacks have no view so both are excluded
const IMPORTABLE: [ContentType; 3] = [
    ContentType::Mod,
    ContentType::ResourcePack,
    ContentType::Shader,
];

const SHADER_HINTS: [&str; 5] = ["shader", "bsl", "seus", "complementary", "sildur"];

fn content_label(content_type: ContentType) -> &'static str {
    match content_type {
        ContentType::ResourcePack => "Textures",
        ContentType::Shader => "Shaders",
        _ => "Mods",
    }
}

fn extension(path: &Path) -> Option<String> {
    Some(path.extension()?.to_str()?.to_ascii_lowercase())
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Precedence `.jar` is always a mod then the current route then a name sniff
fn infer_content_type(path: &Path, route_type: Option<ContentType>) -> ContentType {
    if extension(path).as_deref() == Some("jar") {
        return ContentType::Mod;
    }
    if let Some(route_type) = route_type {
        return route_type;
    }
    let name = file_name(path).to_lowercase();
    if SHADER_HINTS.iter().any(|hint| name.contains(hint)) {
        ContentType::Shader
    } else {
        ContentType::ResourcePack
    }
}

fn route_target(route: &Route) -> Option<(ClusterId, ContentType)> {
    match route {
        Route::ClusterMods { cluster_id } => Some((*cluster_id, ContentType::Mod)),
        Route::ClusterShaders { cluster_id } => Some((*cluster_id, ContentType::Shader)),
        Route::ClusterTextures { cluster_id } => Some((*cluster_id, ContentType::ResourcePack)),
        _ => None,
    }
}

/// State lives in the shell (owner of the bubbling `on_file_drop`) so it never re-renders on hover
#[derive(PartialEq)]
pub struct FileDropOverlay {
    pub hovering: State<bool>,
    pub pending: State<Vec<PathBuf>>,
}

impl Component for FileDropOverlay {
    fn render(&self) -> impl IntoElement {
        let files = self.pending.read().clone();

        if !files.is_empty() {
            return DropPrompt {
                files,
                pending: self.pending,
            }
            .into_element();
        }

        if !*self.hovering.read() {
            return rect().into_element();
        }

        DropVeil.into_element()
    }
}

#[derive(PartialEq)]
struct DropVeil;

impl Component for DropVeil {
    fn render(&self) -> impl IntoElement {
        let intro = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            AnimNum::new(0., 1.)
                .time(170)
                .ease(Ease::Out)
                .function(Function::Cubic)
        });
        let progress = intro.read().value();

        rect()
            .layer(Layer::Overlay)
            .position(Position::new_global().top(0.).left(0.))
            .width(Size::window_percent(100.))
            .height(Size::window_percent(100.))
            // Purely decorative the drop must reach the shell handler beneath
            .interactive(false)
            .center()
            .opacity(progress)
            .background(Color::from_argb(190, 10, 14, 20))
            .child(
                rect()
                    .vertical()
                    .center()
                    .spacing(10.)
                    .padding(Gaps::new_symmetric(22., 28.))
                    .max_width(Size::px(360.))
                    .offset_y((1. - progress) * 12.)
                    .corner_radius(CornerRadius::new_all(16.))
                    .background(DIALOG_BG)
                    .border(border_all_color(1., colors::brand().with_a(120)))
                    .shadow(Shadow::from((
                        0.,
                        16.,
                        44.,
                        0.,
                        Color::from_argb(160, 0, 0, 0),
                    )))
                    .child(icon_badge(IconType::FolderDownload, 52., 22.))
                    .child(
                        label()
                            .text("Drop files to add them")
                            .font_size(15.)
                            .font_weight(FontWeight::SEMI_BOLD)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text("You'll pick where they go next.")
                            .font_size(12.5)
                            .color(colors::fg_secondary()),
                    ),
            )
    }
}

fn icon_badge(icon: IconType, size: f32, icon_size: f32) -> impl IntoElement {
    rect()
        .width(Size::px(size))
        .height(Size::px(size))
        .center()
        .corner_radius(CornerRadius::new_all(size / 2.))
        .background(colors::brand().with_a(46))
        .child(Icon::new(icon).size(icon_size).color(colors::brand()))
}

#[derive(PartialEq)]
struct DropPrompt {
    files: Vec<PathBuf>,
    pending: State<Vec<PathBuf>>,
}

impl Component for DropPrompt {
    fn render(&self) -> impl IntoElement {
        let route = use_route::<Route>();
        let active_cluster = use_active_cluster_id();
        let clusters_query = use_clusters();
        let dispatch = use_dispatch();

        let files = self.files.clone();
        let mut pending = self.pending;

        let target = route_target(&route);
        let initial_cluster = target.map(|(id, _)| id).or(*active_cluster.peek());
        let selected_cluster = use_state(move || initial_cluster);
        // Absent entries fall back to the inferred type covering drops landing mid-prompt
        let overrides = use_state(HashMap::<PathBuf, ContentType>::new);

        let clusters: Vec<Cluster> = settled_or_loading(&clusters_query).unwrap_or_default();

        let body = if clusters.is_empty() {
            no_clusters_body(pending)
        } else {
            prompt_body(
                &files,
                &clusters,
                target.map(|(_, ct)| ct),
                selected_cluster,
                overrides,
                dispatch,
                pending,
            )
        };

        OverlayPopup::new()
            .on_close(move |_| pending.set(Vec::new()))
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(DIALOG_W))
                            .max_width(Size::window_percent(94.))
                            .padding(Gaps::new_all(22.))
                            .spacing(16.)
                            .corner_radius(CornerRadius::new_all(16.))
                            .background(DIALOG_BG)
                            .border(border_all_color(1., colors::component_border()))
                            .shadow(Shadow::from((
                                0.,
                                18.,
                                52.,
                                0.,
                                Color::from_argb(150, 0, 0, 0),
                            )))
                            .child(body),
                    ),
            )
            .into_element()
    }
}

fn dialog_header(title: String, subtitle: String) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        // Without Content::Flex the flexed text column doesn't resolve and the
        // badge stops centring against it
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .child(icon_badge(IconType::FilePlus02, 38., 18.))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
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
                        .max_lines(2)
                        .color(colors::fg_secondary()),
                ),
        )
}

fn no_clusters_body(mut pending: State<Vec<PathBuf>>) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(16.)
        .child(dialog_header(
            "Nowhere to put these".to_string(),
            "Create a cluster first, then drop the files again.".to_string(),
        ))
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .main_align(Alignment::End)
                .child(
                    Button::new()
                        .primary()
                        .on_press(move |_| pending.set(Vec::new()))
                        .text("Close"),
                ),
        )
        .into_element()
}

fn prompt_body(
    files: &[PathBuf],
    clusters: &[Cluster],
    route_type: Option<ContentType>,
    selected_cluster: State<Option<ClusterId>>,
    overrides: State<HashMap<PathBuf, ContentType>>,
    dispatch: crate::Actions,
    mut pending: State<Vec<PathBuf>>,
) -> Element {
    // The remembered cluster may have been deleted while the prompt was open
    let cluster_id = selected_cluster
        .read()
        .filter(|id| clusters.iter().any(|c| c.id == *id))
        .unwrap_or(clusters[0].id);
    let cluster_idx = clusters
        .iter()
        .position(|c| c.id == cluster_id)
        .unwrap_or_default();

    let resolved: Vec<(PathBuf, ContentType)> = files
        .iter()
        .map(|path| {
            let content_type = overrides
                .read()
                .get(path)
                .copied()
                .unwrap_or_else(|| infer_content_type(path, route_type));
            (path.clone(), content_type)
        })
        .collect();

    let import_list = resolved.clone();
    let import = move |_| {
        dispatch.import_local_files(cluster_id, import_list.clone());
        pending.set(Vec::new());
    };

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(16.)
        .child(dialog_header(
            format!(
                "Add {} file{}",
                files.len(),
                if files.len() == 1 { "" } else { "s" }
            ),
            "Pick a cluster, then check what each one gets installed as.".to_string(),
        ))
        .child(cluster_field(clusters, cluster_idx, selected_cluster))
        .child(file_field(&resolved, overrides))
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .cross_align(Alignment::Center)
                .main_align(Alignment::End)
                .spacing(8.)
                .child(
                    Button::new()
                        .ghost()
                        .on_press(move |_| pending.set(Vec::new()))
                        .text("Cancel"),
                )
                .child(
                    Button::new()
                        .primary()
                        .on_press(import)
                        .child(Icon::new(IconType::Plus).size(15.))
                        .text(match files.len() {
                            1 => "Add file".to_string(),
                            n => format!("Add {n} files"),
                        }),
                ),
        )
        .into_element()
}

fn field(title: &'static str, control: impl IntoElement) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(7.)
        .child(
            label()
                .text(title)
                .font_size(11.5)
                .font_weight(FontWeight::MEDIUM)
                .color(colors::fg_secondary()),
        )
        .child(control)
}

fn cluster_field(
    clusters: &[Cluster],
    selected_idx: usize,
    mut selected: State<Option<ClusterId>>,
) -> impl IntoElement {
    let labels: Vec<String> = clusters
        .iter()
        .map(|c| format!("{} · {}", c.name, c.mc_version))
        .collect();
    let ids: Vec<ClusterId> = clusters.iter().map(|c| c.id).collect();
    let current = labels[selected_idx].clone();

    field(
        "Destination cluster",
        Dropdown::new(current, labels)
            .width(Size::fill())
            .height(Size::px(32.))
            .on_select(move |idx: usize| {
                if let Some(id) = ids.get(idx) {
                    selected.set(Some(*id));
                }
            }),
    )
}

fn file_field(
    resolved: &[(PathBuf, ContentType)],
    overrides: State<HashMap<PathBuf, ContentType>>,
) -> impl IntoElement {
    let visible = resolved.len().clamp(1, FILE_LIST_MAX_ROWS);
    let rows = resolved
        .iter()
        .map(|(path, content_type)| file_row(path, *content_type, overrides).into_element())
        .collect::<Vec<_>>();

    field(
        "Files",
        rect()
            .width(Size::fill())
            .padding(Gaps::new_all(6.))
            .corner_radius(CornerRadius::new_all(10.))
            .background(colors::page())
            .border(border_all_color(1., colors::component_border()))
            .child(
                ScrollArea::new()
                    .width(Size::fill())
                    .height(Size::px(visible as f32 * FILE_ROW_H))
                    .children(rows),
            ),
    )
}

fn file_row(
    path: &Path,
    content_type: ContentType,
    mut overrides: State<HashMap<PathBuf, ContentType>>,
) -> impl IntoElement {
    let key = path.to_path_buf();
    let options: Vec<String> = IMPORTABLE
        .iter()
        .map(|ct| content_label(*ct).to_string())
        .collect();

    rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::px(FILE_ROW_H))
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(8.)
        .padding(Gaps::new_symmetric(0., 4.))
        .child(
            Icon::new(IconType::File02)
                .size(13.)
                .color(colors::fg_secondary()),
        )
        .child(
            label()
                .text(file_name(path))
                .font_size(12.5)
                .max_lines(1)
                .width(Size::flex(1.0))
                .color(colors::fg_primary()),
        )
        .child(
            Dropdown::new(content_label(content_type), options)
                .width(Size::px(TYPE_DROPDOWN_W))
                .height(Size::px(24.))
                .on_select(move |idx: usize| {
                    if let Some(content_type) = IMPORTABLE.get(idx) {
                        overrides.write().insert(key.clone(), *content_type);
                    }
                }),
        )
}
