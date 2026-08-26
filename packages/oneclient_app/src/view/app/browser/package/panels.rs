use super::*;

use oneclient_content::packages::ProviderId;
use oneclient_content::packages::types::{PackageBody, ProjectDetail, ReleaseType, VersionSummary};

use crate::Actions;
use crate::components::{Button, Icon, IconType, Segment, SegmentedControl};
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

pub(super) fn tabs(active_tab: State<usize>, has_gallery: bool) -> impl IntoElement {
    let mut control = SegmentedControl::new(active_tab)
        .height(36.)
        .segment(Segment::new(0usize).label("About"))
        .segment(Segment::new(1usize).label("Versions"));
    if has_gallery {
        control = control.segment(Segment::new(2usize).label("Gallery"));
    }
    control
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

#[allow(clippy::too_many_arguments)]
pub(super) fn versions_panel(
    versions: Vec<VersionSummary>,
    total_versions: usize,
    versions_page: State<usize>,
    provider: ProviderId,
    project_id: String,
    cluster_id: i64,
    dispatch: Actions,
    installed: Option<Installed>,
    installing: bool,
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
                let tag = installed
                    .as_ref()
                    .and_then(|installed| installed.find_version(&v.version_id))
                    .cloned();
                let duplicated = installed
                    .as_ref()
                    .is_some_and(|installed| installed.is_duplicated());
                version_row(
                    v,
                    provider,
                    project_id.clone(),
                    cluster_id,
                    dispatch.clone(),
                    tag,
                    duplicated,
                    installing,
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
                el.cursor(CursorIcon::Pointer)
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

#[allow(clippy::too_many_arguments)]
fn version_row(
    v: VersionSummary,
    provider: ProviderId,
    project_id: String,
    cluster_id: i64,
    dispatch: Actions,
    installed: Option<InstalledVersion>,
    // Saying which version is live only tells the user anything when there are several
    duplicated: bool,
    installing: bool,
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
        .maybe_child(
            installed
                .as_ref()
                .map(|installed| installed_badge(installed.source, 11.).into_element()),
        )
        .maybe_child(
            installed
                .as_ref()
                .filter(|_| duplicated)
                .map(|installed| activity_badge(installed.enabled).into_element()),
        )
        .child(version_button(
            installed,
            v.name,
            provider,
            project_id,
            version_id,
            cluster_id,
            dispatch,
            installing,
        ))
}

/// A bundle pin with nothing linked leaves nothing to press no artifact to remove and installing by hand would duplicate it
#[allow(clippy::too_many_arguments)]
fn version_button(
    installed: Option<InstalledVersion>,
    version_name: String,
    provider: ProviderId,
    project_id: String,
    version_id: String,
    cluster_id: i64,
    dispatch: Actions,
    busy: bool,
) -> impl IntoElement {
    let Some(installed) = installed else {
        return Button::new()
            .secondary()
            .small()
            .enabled(!busy)
            .on_press(move |_| {
                dispatch.install_package(
                    cluster_id,
                    provider,
                    project_id.clone(),
                    version_id.clone(),
                );
            })
            .text("Install");
    };

    let Some(hash) = installed.hash else {
        return Button::new().secondary().small().enabled(false).text("Install");
    };

    Button::new()
        .danger()
        .small()
        .enabled(!busy)
        .on_press(move |_| {
            dispatch.remove_package_version(
                cluster_id,
                provider,
                project_id.clone(),
                hash.clone(),
                version_name.clone(),
            );
        })
        .text("Remove")
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
