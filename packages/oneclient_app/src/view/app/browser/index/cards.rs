use super::*;

use std::collections::HashMap;

use freya::router::RouterContext;
use oneclient_content::packages::types::ProjectSummary;
use oneclient_content::packages::{ContentType, ProviderId};

use crate::components::{Button, Icon, IconType};
use crate::hooks::{
    content_type_for_slug, use_browser_compat, use_cluster, use_dispatch, use_installs_snapshot,
    use_package_versions_when, version_list,
};
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::border_all_color;

type InstalledMap = HashMap<(ProviderId, String), Installed>;

/// Diameter of the round install button
const INSTALL_BUTTON: f32 = 30.;

fn installed_for(installed: &InstalledMap, item: &ProjectSummary) -> Option<InstallSource> {
    installed
        .get(&(item.provider, item.id.clone()))
        .map(|installed| installed.source)
}

pub(super) fn grid_row(
    row: Vec<ProjectSummary>,
    cluster_id: i64,
    package_type: &str,
    installed: &InstalledMap,
) -> impl IntoElement {
    let package_type = package_type.to_string();
    let fill = GRID_COLUMNS - row.len();
    let installed: Vec<Option<InstallSource>> = row
        .iter()
        .map(|item| installed_for(installed, item))
        .collect();

    rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::px(CARD_H))
        .spacing(GRID_SPACING)
        .content(Content::Flex)
        .children(
            row.into_iter()
                .zip(installed)
                .map(move |(item, installed)| {
                    PackageCard::new(item, cluster_id, package_type.clone(), installed)
                        .into_element()
                }),
        )
        .maybe(fill > 0, move |mut el| {
            for _ in 0..fill {
                el = el.child(rect().width(Size::flex(1.0)));
            }
            el
        })
        .into_element()
}

fn open_package(cluster_id: i64, package_type: &str, provider: ProviderId, id: &str) {
    let _ = RouterContext::get().push(Route::BrowserPackage {
        cluster_id,
        package_type: package_type.to_string(),
        package_id: encode_package_id(provider, id),
    });
}

#[derive(PartialEq)]
struct PackageCard {
    item: ProjectSummary,
    cluster_id: i64,
    package_type: String,
    installed: Option<InstallSource>,
}

impl PackageCard {
    fn new(
        item: ProjectSummary,
        cluster_id: i64,
        package_type: String,
        installed: Option<InstallSource>,
    ) -> Self {
        Self {
            item,
            cluster_id,
            package_type,
            installed,
        }
    }
}

impl Component for PackageCard {
    fn render(&self) -> impl IntoElement {
        let id = self.item.id.clone();
        let provider = self.item.provider;
        let package_type = self.package_type.clone();
        let icon_url = self.item.icon_url.clone();
        let cluster_id = self.cluster_id;

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let focused = focus().is_focused();

        rect()
            .vertical()
            .width(Size::flex(1.0))
            .height(Size::px(CARD_H))
            .corner_radius(CornerRadius::new_all(10.))
            .background(CARD_BG)
            .border(border_all_color(
                1.,
                if focused {
                    colors::brand()
                } else {
                    colors::component_border()
                },
            ))
            .overflow(Overflow::Clip)
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .cursor(CursorIcon::Pointer)
            .on_all_press(move |_| open_package(cluster_id, &package_type, provider, &id))
            .child(
                rect()
                    .margin(Gaps::new_all(1.))
                    .corner_radius(CornerRadius::new(10., 10., 0., 0.))
                    .overflow(Overflow::Clip)
                    .child(PackageBanner::new(icon_url, BANNER_H))
                    // Top-left of the banner the first thing worth knowing about a result and the opposite corner is the install button's
                    .maybe_child(self.installed.map(|installed| {
                        rect()
                            .position(Position::new_absolute().top(8.).left(8.))
                            .layer(Layer::Relative(7))
                            .child(installed_badge_overlay(installed))
                            .into_element()
                    }))
                    .maybe_child(self.installed.is_none().then(|| {
                        rect()
                            .position(Position::new_absolute().top(8.).right(8.))
                            .layer(Layer::Relative(7))
                            .child(InstallButton::new(
                                &self.item,
                                self.cluster_id,
                                &self.package_type,
                                false,
                            ))
                            .into_element()
                    })),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .padding(Gaps::new_all(12.))
                    .main_align(Alignment::SpaceBetween)
                    .child(
                        rect()
                            .vertical()
                            .width(Size::fill())
                            .spacing(6.)
                            .child(
                                label()
                                    .text(self.item.name.clone())
                                    .font_size(16.)
                                    .font_weight(FontWeight::MEDIUM)
                                    .max_lines(1)
                                    .color(CARD_NAME),
                            )
                            .child(
                                rect()
                                    .horizontal()
                                    .cross_align(Alignment::Center)
                                    .spacing(4.)
                                    .child(
                                        label()
                                            .text(format!("by {}", self.item.author))
                                            .font_size(10.)
                                            .max_lines(1)
                                            .color(colors::fg_secondary()),
                                    )
                                    .child(Icon::new(self.item.provider).size(12.)),
                            )
                            .child(
                                label()
                                    .text(self.item.summary.clone())
                                    .font_size(11.)
                                    .max_lines(2)
                                    .width(Size::fill())
                                    .color(colors::fg_secondary()),
                            ),
                    )
                    .child(downloads_row(self.item.downloads)),
            )
    }
}

pub(super) fn list_row(
    item: ProjectSummary,
    cluster_id: i64,
    package_type: &str,
    installed: &InstalledMap,
) -> impl IntoElement {
    ListRow {
        installed: installed_for(installed, &item),
        item,
        cluster_id,
        package_type: package_type.to_string(),
    }
}

#[derive(PartialEq)]
struct ListRow {
    item: ProjectSummary,
    cluster_id: i64,
    package_type: String,
    installed: Option<InstallSource>,
}

impl Component for ListRow {
    fn render(&self) -> impl IntoElement {
        let item = self.item.clone();
        let cluster_id = self.cluster_id;
        let id = item.id.clone();
        let provider = item.provider;
        let package_type = self.package_type.clone();

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let focused = focus().is_focused();

        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(LIST_ROW_H))
            .cross_align(Alignment::Center)
            .spacing(12.)
            .padding(Gaps::new_all(16.))
            .corner_radius(CornerRadius::new_all(10.))
            .background(CARD_BG)
            .border(border_all_color(
                1.,
                if focused {
                    colors::brand()
                } else {
                    colors::component_border()
                },
            ))
            .overflow(Overflow::Clip)
            .content(Content::Flex)
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .cursor(CursorIcon::Pointer)
            .on_all_press(move |_| open_package(cluster_id, &package_type, provider, &id))
            .child(Thumbnail::new(item.icon_url.clone(), 48.).radius(8.))
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(3.)
                    .child(
                        rect()
                            .horizontal()
                            .cross_align(Alignment::Center)
                            .spacing(6.)
                            .child(
                                label()
                                    .text(item.name.clone())
                                    .font_size(15.)
                                    .font_weight(FontWeight::MEDIUM)
                                    .max_lines(1)
                                    .color(CARD_NAME),
                            )
                            .child(
                                label()
                                    .text(format!("by {}", item.author))
                                    .font_size(10.)
                                    .max_lines(1)
                                    .color(colors::fg_secondary()),
                            )
                            .child(Icon::new(item.provider).size(12.))
                            // Beside the attribution rather than far right where it drifted from the package and crowded the install button
                            .maybe_child(
                                self.installed.map(|installed| {
                                    installed_badge(installed, 10.).into_element()
                                }),
                            ),
                    )
                    .child(
                        label()
                            .text(item.summary.clone())
                            .font_size(11.)
                            .max_lines(2)
                            .width(Size::fill())
                            .color(colors::fg_secondary()),
                    ),
            )
            .child(downloads_row(item.downloads))
            // Kept once installed rather than dropped a row that loses its button leaves its neighbours misaligned
            .child(InstallButton::new(
                &self.item,
                self.cluster_id,
                &self.package_type,
                self.installed.is_some(),
            ))
    }
}

/// Same version pick and dispatch as the install button on the package page
#[derive(PartialEq)]
struct InstallButton {
    provider: ProviderId,
    project_id: String,
    cluster_id: i64,
    content_type: ContentType,
    /// Cluster already has this one callers that hide the button entirely pass `false`
    installed: bool,
}

impl InstallButton {
    fn new(item: &ProjectSummary, cluster_id: i64, package_type: &str, installed: bool) -> Self {
        Self {
            provider: item.provider,
            project_id: item.id.clone(),
            cluster_id,
            content_type: content_type_for_slug(package_type),
            installed,
        }
    }
}

impl Component for InstallButton {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let provider = self.provider;
        let project_id = self.project_id.clone();

        let dispatch = use_dispatch();
        let compat = *use_browser_compat().read();
        let cluster = use_cluster(cluster_id);

        // The same narrowing the package page does so both agree on what "latest" installs
        let (game_version, loader) = match (compat, &cluster) {
            (true, Some(c)) => (
                Some(c.mc_version.clone()),
                (self.content_type == ContentType::Mod).then_some(c.mc_loader),
            ),
            _ => (None, None),
        };

        let installed = self.installed;

        let versions = version_list(&use_package_versions_when(
            !installed,
            provider,
            project_id.clone(),
            game_version,
            loader,
            0,
        ));
        let latest = versions.first().map(|v| v.version_id.clone());

        // Nothing to start twice while an install is running or before versions arrive
        let installing = use_installs_snapshot().is_installing(cluster_id, provider, &project_id);

        rect()
            // The card behind is one big press target stop here so the package page doesn't open on top of the install
            .on_all_press(|e: Event<PressEventData>| e.stop_propagation())
            .child(
                Button::new()
                    .primary()
                    .width(Size::px(INSTALL_BUTTON))
                    .height(Size::px(INSTALL_BUTTON))
                    .padding(Gaps::new_all(0.))
                    .corner_radius(CornerRadius::new_all(INSTALL_BUTTON / 2.))
                    .alt(if installed {
                        "Already installed"
                    } else if installing {
                        "Installing"
                    } else {
                        "Install"
                    })
                    .enabled(!installed && latest.is_some() && !installing)
                    .on_press(move |_| {
                        if let Some(version_id) = latest.clone() {
                            dispatch.install_package(
                                cluster_id,
                                provider,
                                project_id.clone(),
                                version_id,
                            );
                        }
                    })
                    .child(
                        Icon::new(if installing {
                            IconType::Loading02
                        } else {
                            IconType::Download01
                        })
                        .size(15.)
                        .color(colors::fg_primary()),
                    ),
            )
    }
}

fn downloads_row(downloads: u64) -> impl IntoElement {
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(4.)
        .child(
            Icon::new(IconType::Download01)
                .size(12.)
                .color(colors::fg_secondary()),
        )
        .child(
            label()
                .text(abbreviate_number(downloads))
                .font_size(11.)
                .color(colors::fg_secondary()),
        )
}

pub(super) fn empty_state() -> impl IntoElement {
    rect()
        .width(Size::fill())
        .height(Size::fill())
        .center()
        .child(
            label()
                .text("No results. Try a different search, category or provider.")
                .font_size(14.)
                .color(colors::fg_secondary()),
        )
}
