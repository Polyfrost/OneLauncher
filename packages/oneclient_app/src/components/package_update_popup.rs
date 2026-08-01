//! The launch-time "these packages are out of date" modal.
//!
//! Only ever shows packages the user installed from the browser; bundle content
//! has its own flow and is never listed here.
//!
//! Every row is answered on its own — one Update button, one Skip button — so a
//! user who wants a single mod moved forward is not made to take the rest with
//! it. Answering removes the row; the modal closes when the last one is gone.

use std::collections::HashMap;

use freya::prelude::*;
use oneclient_content::packages::{CachedPackageMeta, ProviderId};
use oneclient_core::BrowserPackageUpdate;

use crate::components::{Button, Icon, IconType, OverlayPopup, ScrollArea};
use crate::hooks::{
    package_meta_batch, use_dispatch, use_notifications_snapshot, use_package_meta_batch,
};
use crate::notifications::PackageUpdateGroup;
use crate::theme::colors;
use crate::ui::border_all_color;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);
const DIALOG_W: f32 = 460.;
const DIALOG_H: f32 = 420.;

type MetaMap = HashMap<(ProviderId, String), CachedPackageMeta>;

#[derive(PartialEq)]
pub struct PackageUpdatePopup;

impl Component for PackageUpdatePopup {
    fn render(&self) -> impl IntoElement {
        let snapshot = use_notifications_snapshot();
        let dispatch = use_dispatch();

        let groups = snapshot.package_updates.clone();

        // Hooks run unconditionally, so the meta lookup is built from a possibly
        // empty id list and happens before the early return below.
        let mut meta = MetaMap::new();
        for provider in ProviderId::REMOTE_PROVIDERS.iter().copied() {
            let ids: Vec<String> = groups
                .iter()
                .flatten()
                .flat_map(|group| &group.packages)
                .filter(|update| update.provider == provider)
                .map(|update| update.project_id.clone())
                .collect();
            let query = use_package_meta_batch(provider, ids);
            for (project_id, cached) in package_meta_batch(&query) {
                meta.insert((provider, project_id), cached);
            }
        }

        let Some(groups) = groups.filter(|groups| !groups.is_empty()) else {
            return rect().into_element();
        };

        let close = dispatch.clone();

        OverlayPopup::new()
            .on_close(move |_| close.close_package_updates())
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(dialog(&groups, &meta, dispatch)),
            )
            .into_element()
    }
}

fn resolve_name(update: &BrowserPackageUpdate, meta: &MetaMap) -> String {
    meta.get(&(update.provider, update.project_id.clone()))
        .map(|cached| cached.name.clone())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| update.display_name.clone())
}

fn dialog(
    groups: &[PackageUpdateGroup],
    meta: &MetaMap,
    dispatch: crate::Actions,
) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::px(DIALOG_W))
        .height(Size::px(DIALOG_H))
        .max_width(Size::window_percent(95.))
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
        .child(content(groups, meta, dispatch))
}

fn content(
    groups: &[PackageUpdateGroup],
    meta: &MetaMap,
    dispatch: crate::Actions,
) -> impl IntoElement {
    let dismiss = dispatch.clone();
    let update_all = dispatch.clone();
    let total: usize = groups.iter().map(|group| group.packages.len()).sum();

    let subtitle = match groups {
        [only] => format!(
            "{total} package{} in {} can be updated.",
            if total == 1 { "" } else { "s" },
            only.cluster_name
        ),
        _ => format!(
            "{total} package{} across {} clusters can be updated.",
            if total == 1 { "" } else { "s" },
            groups.len()
        ),
    };

    let all: Vec<BrowserPackageUpdate> = groups
        .iter()
        .flat_map(|group| group.packages.iter().cloned())
        .collect();

    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .content(Content::Flex)
        .padding(Gaps::new_all(22.))
        .spacing(14.)
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(3.)
                .child(
                    label()
                        .text("Updates available")
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
        .child(update_list(groups, meta, groups.len() > 1, &dispatch))
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .cross_align(Alignment::Center)
                .main_align(Alignment::SpaceBetween)
                .spacing(8.)
                .child(
                    Button::new()
                        .ghost()
                        .on_press(move |_| dismiss.close_package_updates())
                        .text("Not now"),
                )
                .child(
                    Button::new()
                        .primary()
                        .on_press(move |_| {
                            for update in &all {
                                update_all.apply_package_update(update.clone());
                            }
                        })
                        .child(Icon::new(IconType::DownloadCloud02).size(15.))
                        .text("Update all"),
                ),
        )
}

fn update_list(
    groups: &[PackageUpdateGroup],
    meta: &MetaMap,
    show_headers: bool,
    dispatch: &crate::Actions,
) -> impl IntoElement {
    let mut scroll = ScrollArea::new()
        .width(Size::fill())
        .height(Size::flex(1.0))
        .spacing(4.);

    for (index, group) in groups.iter().enumerate() {
        if show_headers {
            scroll = scroll.child(cluster_header(&group.cluster_name, index == 0));
        }

        for update in &group.packages {
            let key = format!("{}:{}", group.cluster_id, update.hash);
            scroll = scroll.child(
                UpdateRow::new(resolve_name(update, meta), update.clone(), dispatch.clone())
                    .key(key)
                    .into_element(),
            );
        }
    }

    scroll.into_element()
}

fn cluster_header(name: &str, first: bool) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .padding(Gaps::new_symmetric(6., 8.))
        .margin(Gaps::new(if first { 0. } else { 8. }, 0., 3., 0.))
        .corner_radius(CornerRadius::new_all(8.))
        .background(colors::component_bg())
        .child(
            label()
                .text(name.to_string())
                .font_size(12.)
                .font_weight(FontWeight::SEMI_BOLD)
                .max_lines(1)
                .width(Size::fill())
                .color(colors::fg_primary()),
        )
}

struct UpdateRow {
    name: String,
    update: BrowserPackageUpdate,
    dispatch: crate::Actions,
    key: DiffKey,
}

impl UpdateRow {
    fn new(name: String, update: BrowserPackageUpdate, dispatch: crate::Actions) -> Self {
        Self {
            name,
            update,
            dispatch,
            key: DiffKey::None,
        }
    }
}

impl PartialEq for UpdateRow {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.update == other.update
    }
}

impl KeyExt for UpdateRow {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for UpdateRow {
    fn render(&self) -> impl IntoElement {
        let update = self.update.clone();
        let cluster_id = update.cluster_id;
        let skip_hash = update.hash.clone();

        let apply_dispatch = self.dispatch.clone();
        let skip_dispatch = self.dispatch.clone();

        // "1.0.2 to 1.1.0" when both are known; a provider that records neither
        // still gets a usable row rather than an empty line.
        let versions = match (
            update.installed_version_name.is_empty(),
            update.latest_version_name.is_empty(),
        ) {
            (false, false) => format!(
                "{} to {}",
                update.installed_version_name, update.latest_version_name
            ),
            (true, false) => format!("New version {}", update.latest_version_name),
            _ => "A newer version is available".to_string(),
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .cross_align(Alignment::Center)
            .spacing(10.)
            .padding(Gaps::new_symmetric(8., 10.))
            .corner_radius(CornerRadius::new_all(8.))
            .background(colors::component_bg())
            .content(Content::Flex)
            .child(
                Icon::new(IconType::RefreshCw01)
                    .size(14.)
                    .color(colors::brand()),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(2.)
                    .child(
                        label()
                            .text(self.name.clone())
                            .font_size(12.5)
                            .font_weight(FontWeight::MEDIUM)
                            .max_lines(1)
                            .width(Size::fill())
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text(versions)
                            .font_size(11.)
                            .max_lines(1)
                            .width(Size::fill())
                            .color(colors::fg_secondary()),
                    ),
            )
            .child(
                Button::new()
                    .small()
                    .ghost()
                    .on_press(move |_| {
                        skip_dispatch.skip_package_update(cluster_id, skip_hash.clone())
                    })
                    .text("Skip"),
            )
            .child(
                Button::new()
                    .small()
                    .primary()
                    .on_press(move |_| apply_dispatch.apply_package_update(update.clone()))
                    .text("Update"),
            )
    }
}
