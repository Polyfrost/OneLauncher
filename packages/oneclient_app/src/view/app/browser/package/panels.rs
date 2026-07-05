use super::*;

use oneclient_core::packages::types::{
    GalleryImage, PackageBody, ProjectDetail, ReleaseType, VersionSummary,
};
use oneclient_core::packages::ProviderId;

use crate::BridgeDispatch;
use crate::components::{Button, Icon, IconType};
use crate::hooks::VERSIONS_PAGE_SIZE;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::utils::format_size;


pub(super) fn loading_body() -> impl IntoElement {
    rect()
        .width(Size::fill())
        .center()
        .padding(Gaps::new_all(32.))
        .child(
            label()
                .text("Loading package...")
                .font_size(14.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

pub(super) fn tabs(current: usize, active_tab: State<usize>, has_gallery: bool) -> impl IntoElement {
    let mut labels: Vec<&str> = vec!["About", "Versions"];
    if has_gallery {
        labels.push("Gallery");
    }
    rect()
        .horizontal()
        .spacing(6.)
        .padding(Gaps::new_all(4.))
        .corner_radius(CornerRadius::new_all(9.))
        .background(colors::component_bg())
        .children(labels.into_iter().enumerate().map(move |(i, t)| {
            let selected = i == current;
            let mut active_tab = active_tab;
            rect()
                .center()
                .padding(Gaps::new_symmetric(6., 16.))
                .corner_radius(CornerRadius::new_all(7.))
                .background(if selected {
                    colors::component_bg_pressed()
                } else {
                    Color::TRANSPARENT
                })
                .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
                .on_press(move |_| *active_tab.write() = i)
                .child(label().text(t).font_size(13.).color(if selected {
                    colors::fg_primary()
                } else {
                    colors::fg_secondary()
                }))
                .into_element()
        }))
        .into_element()
}

pub(super) fn about_panel(project: &ProjectDetail) -> impl IntoElement {
    let body = match &project.body {
        PackageBody::Raw(md) => md.clone(),
        PackageBody::Url(url) => format!("{}\n\n[View online]({url})", project.summary),
    };
    MarkdownPanel { body }.into_element()
}

#[derive(PartialEq)]
struct MarkdownPanel {
    body: String,
}

impl Component for MarkdownPanel {
    fn render(&self) -> impl IntoElement {
        rect()
            .width(Size::fill())
            .padding(Gaps::new_all(20.))
            .corner_radius(CornerRadius::new_all(12.))
            .background(PANEL_BG)
            .border(border_all_color(1., colors::component_border()))
            .child(
                MarkdownViewer::new(self.body.clone())
                    .width(Size::fill())
                    .color(colors::fg_primary())
                    .color_link(colors::code_info())
                    .background_code(colors::component_bg())
                    .color_code(colors::fg_primary())
                    .background_blockquote(colors::component_bg())
                    .border_blockquote(colors::brand())
                    .background_divider(colors::component_border())
                    .heading_h1(26.)
                    .heading_h2(22.)
                    .heading_h3(18.)
                    .heading_h4(16.)
                    .heading_h5(14.)
                    .heading_h6(13.)
                    .paragraph_size(13.)
                    .code_font_size(12.),
            )
    }
}

pub(super) fn gallery_panel(images: Vec<GalleryImage>) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(12.)
        .children(images.into_iter().map(|img| {
            let key = img.url.clone();
            GalleryTile {
                image: img,
                key: DiffKey::None,
            }
            .key(key)
            .into_element()
        }))
        .into_element()
}

#[derive(PartialEq)]
struct GalleryTile {
    image: GalleryImage,
    key: DiffKey,
}

impl KeyExt for GalleryTile {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for GalleryTile {
    fn render(&self) -> impl IntoElement {
        rect()
            .vertical()
            .width(Size::fill())
            .corner_radius(CornerRadius::new_all(10.))
            .overflow(Overflow::Clip)
            .background(PANEL_BG)
            .border(border_all_color(1., colors::component_border()))
            .child(PackageBanner::new(Some(self.image.url.clone()), 240.))
            .maybe(self.image.title.is_some(), |el| {
                el.child(
                    rect()
                        .width(Size::fill())
                        .padding(Gaps::new_all(10.))
                        .child(
                            label()
                                .text(self.image.title.clone().unwrap_or_default())
                                .font_size(12.)
                                .max_lines(2)
                                .color(colors::fg_secondary()),
                        ),
                )
            })
    }
}

pub(super) fn versions_panel(
    versions: Vec<VersionSummary>,
    total_versions: usize,
    versions_page: State<usize>,
    provider: ProviderId,
    project_id: String,
    cluster_id: i64,
    dispatch: BridgeDispatch,
) -> impl IntoElement {
    let current = *versions_page.read();
    let total_pages = total_versions.div_ceil(VERSIONS_PAGE_SIZE).max(1);

    let rows: Element = if versions.is_empty() {
        rect()
            .width(Size::fill())
            .center()
            .padding(Gaps::new_all(32.))
            .child(
                label()
                    .text("No matching versions.")
                    .font_size(13.)
                    .color(colors::fg_secondary()),
            )
            .into_element()
    } else {
        rect()
            .vertical()
            .width(Size::fill())
            .spacing(8.)
            .children(versions.into_iter().map(move |v| {
                version_row(
                    v,
                    provider,
                    project_id.clone(),
                    cluster_id,
                    dispatch.clone(),
                )
                .into_element()
            }))
            .into_element()
    };

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(12.)
        .child(rows)
        .maybe(total_versions > 0, |el| {
            el.child(version_pager(current, total_pages, versions_page))
        })
        .into_element()
}

fn version_pager(current: usize, total_pages: usize, page: State<usize>) -> impl IntoElement {
    let nav = move |target: usize, enabled: bool, icon: IconType| {
        let mut page = page;
        rect()
            .center()
            .width(Size::px(32.))
            .height(Size::px(32.))
            .corner_radius(CornerRadius::new_all(8.))
            .background(colors::component_bg())
            .border(border_all_color(1., colors::component_border()))
            .maybe(enabled, |el| {
                el.on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                    .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
                    .on_press(move |_| page.set(target))
            })
            .child(Icon::new(icon).size(14.).color(if enabled {
                colors::fg_primary()
            } else {
                colors::fg_secondary().with_a(90)
            }))
            .into_element()
    };

    rect()
        .horizontal()
        .width(Size::fill())
        .main_align(Alignment::Center)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .child(nav(
            current.saturating_sub(1),
            current > 0,
            IconType::ArrowLeft,
        ))
        .child(
            label()
                .text(format!("Page {} / {}", current + 1, total_pages))
                .font_size(12.)
                .color(colors::fg_secondary()),
        )
        .child(nav(
            current + 1,
            current + 1 < total_pages,
            IconType::ArrowRight,
        ))
        .into_element()
}

fn version_row(
    v: VersionSummary,
    provider: ProviderId,
    project_id: String,
    cluster_id: i64,
    dispatch: BridgeDispatch,
) -> impl IntoElement {
    let version_id = v.version_id.clone();
    let mut chips: Vec<String> = v.loaders.iter().map(|l| l.to_string()).collect();
    chips.extend(v.game_versions.iter().cloned());
    let stats = {
        let mut parts = vec![format!("{} downloads", abbreviate_number(v.downloads))];
        if v.file_size > 0 {
            parts.push(format_size(v.file_size));
        }
        parts.push(v.published.format("%Y-%m-%d").to_string());
        parts.join("  ·  ")
    };
    let has_chips = !chips.is_empty();

    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(12.)
        .content(Content::Flex)
        .padding(Gaps::new_all(12.))
        .corner_radius(CornerRadius::new_all(10.))
        .background(PANEL_BG)
        .border(border_all_color(1., colors::component_border()))
        .child(release_badge(v.release_type))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(6.)
                .child(
                    label()
                        .text(v.name.clone())
                        .font_size(14.)
                        .font_weight(FontWeight::MEDIUM)
                        .max_lines(1)
                        .color(colors::fg_primary()),
                )
                .maybe(has_chips, |el| el.child(pill_flow(&chips, 8, 10)))
                .child(
                    label()
                        .text(stats)
                        .font_size(11.)
                        .max_lines(1)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(
            Button::new()
                .secondary()
                .small()
                .on_press(move |_| {
                    dispatch.install_package(
                        cluster_id,
                        provider,
                        project_id.clone(),
                        version_id.clone(),
                    );
                })
                .text("Install"),
        )
}

fn release_badge(release_type: ReleaseType) -> impl IntoElement {
    let (text, color) = match release_type {
        ReleaseType::Release => ("R", colors::code_info()),
        ReleaseType::Beta => ("B", colors::code_warn()),
        ReleaseType::Alpha => ("A", colors::code_error()),
    };
    rect()
        .center()
        .width(Size::px(28.))
        .height(Size::px(28.))
        .corner_radius(CornerRadius::new_all(7.))
        .background(color.with_a(40))
        .child(
            label()
                .text(text)
                .font_size(13.)
                .font_weight(FontWeight::BOLD)
                .color(color),
        )
}

