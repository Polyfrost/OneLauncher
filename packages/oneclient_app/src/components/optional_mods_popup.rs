use std::collections::HashMap;

use freya::prelude::*;
use oneclient_content::packages::{CachedPackageMeta, ProviderId};
use oneclient_db::models::OptionalModStatus;

use crate::components::{Button, Icon, IconType, OverlayPopup};
use crate::hooks::{
    package_meta_batch, use_dispatch, use_notifications_snapshot, use_package_meta_batch,
};
use crate::notifications::{ClusterUpdateItem, OptionalModRef, OptionalModsGroup};
use crate::theme::colors;
use crate::ui::border_all_color;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);
const DIALOG_W: f32 = 440.;
const LIST_MAX_H: f32 = 260.;

type MetaMap = HashMap<(ProviderId, String), CachedPackageMeta>;

#[derive(PartialEq)]
pub struct OptionalModsPopup;

impl Component for OptionalModsPopup {
    fn render(&self) -> impl IntoElement {
        let snapshot = use_notifications_snapshot();
        let dispatch = use_dispatch();

        let groups = snapshot.optional_mods.clone();

        let mut meta = MetaMap::new();
        for provider in ProviderId::REMOTE_PROVIDERS.iter().copied() {
            let ids: Vec<String> = groups
                .iter()
                .flatten()
                .flat_map(|group| &group.mods)
                .filter(|item| item.provider == provider)
                .filter_map(|item| item.project_id.clone())
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
        let dismissed: Vec<(i64, OptionalModRef)> =
            groups.iter().flat_map(|group| group.offers()).collect();

        OverlayPopup::new()
            .on_close(move |_| close.decline_optional_mods(dismissed.clone()))
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

fn resolve_name(item: &ClusterUpdateItem, meta: &MetaMap) -> String {
    item.project_id
        .as_ref()
        .and_then(|project_id| meta.get(&(item.provider, project_id.clone())))
        .map(|cached| cached.name.clone())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| item.fallback.clone())
}

fn dialog(
    groups: &[OptionalModsGroup],
    meta: &MetaMap,
    dispatch: crate::Actions,
) -> impl IntoElement {
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
        .child(content(groups, meta, dispatch))
}

fn content(
    groups: &[OptionalModsGroup],
    meta: &MetaMap,
    dispatch: crate::Actions,
) -> impl IntoElement {
    let total: usize = groups.iter().map(|group| group.mods.len()).sum();
    let offers: Vec<(i64, OptionalModRef)> =
        groups.iter().flat_map(|group| group.offers()).collect();

    let plural = if total == 1 { "" } else { "s" };
    let subtitle = match groups {
        [only] => format!(
            "{} includes {total} optional mod{plural} that {} off by default. Turn {} on?",
            only.cluster_name,
            if total == 1 { "is" } else { "are" },
            if total == 1 { "it" } else { "them" }
        ),
        _ => format!(
            "Your bundles include {total} optional mod{plural} across {} clusters that are off by default. Turn them on?",
            groups.len()
        ),
    };

    let cancel = dispatch.clone();
    let declined = offers.clone();

    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::Inner)
        .padding(Gaps::new_all(22.))
        .spacing(14.)
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(3.)
                .child(
                    label()
                        .text("Optional mods available")
                        .font_size(17.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(subtitle)
                        .font_size(12.5)
                        .max_lines(3)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(mod_list(groups, meta))
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
                        .on_press(move |_| cancel.decline_optional_mods(declined.clone()))
                        .text("Cancel"),
                )
                .maybe_child((!offers.is_empty()).then(|| {
                    let enable = dispatch.clone();
                    let mods = offers.clone();
                    Button::new()
                        .primary()
                        .on_press(move |_| enable.enable_optional_mods(mods.clone()))
                        .child(Icon::new(IconType::Check).size(15.))
                        .text("Enable")
                        .into_element()
                })),
        )
}

fn mod_list(groups: &[OptionalModsGroup], meta: &MetaMap) -> impl IntoElement {
    let show_headers = groups.len() > 1;

    let mut list = rect().vertical().width(Size::fill()).spacing(4.);

    for (index, group) in groups.iter().enumerate() {
        if show_headers {
            list = list.child(cluster_header(&group.cluster_name, index == 0));
        }

        for item in &group.mods {
            list = list.child(mod_row(resolve_name(item, meta), item.status));
        }
    }

    ScrollView::new()
        .width(Size::fill())
        .height(Size::Inner)
        .max_height(Size::px(LIST_MAX_H))
        .child(list)
}

/// `Skipped` never reaches this prompt today, so every rendered badge reads
/// "New" the arm is kept because the distinction is wanted later
fn mod_row(name: String, status: Option<OptionalModStatus>) -> impl IntoElement {
    let accent = match status {
        Some(OptionalModStatus::Skipped) => colors::fg_primary_disabled(),
        _ => colors::brand(),
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
        .child(Icon::new(IconType::Plus).size(14.).color(accent))
        .child(
            label()
                .text(name)
                .font_size(12.5)
                .font_weight(FontWeight::MEDIUM)
                .max_lines(1)
                .width(Size::flex(1.0))
                .color(colors::fg_primary()),
        )
        .maybe_child(status.map(status_badge))
}

fn status_badge(status: OptionalModStatus) -> impl IntoElement {
    let (text, foreground) = match status {
        OptionalModStatus::New => ("New", colors::brand()),
        OptionalModStatus::Skipped => ("Skipped", colors::fg_secondary()),
    };

    rect()
        .padding(Gaps::new_symmetric(2., 7.))
        .corner_radius(CornerRadius::new_all(6.))
        .background(colors::component_border())
        .child(
            label()
                .text(text)
                .font_size(10.5)
                .font_weight(FontWeight::SEMI_BOLD)
                .max_lines(1)
                .color(foreground),
        )
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
