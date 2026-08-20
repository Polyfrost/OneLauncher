use std::collections::{HashMap, HashSet};

use freya::prelude::*;
use oneclient_content::packages::{CachedPackageMeta, ProviderId};

use crate::components::{
    Button, CARD_GRID_H, GRID_GAP, GRID_MIN_W, Icon, IconType, OverlayPopup, PackageEntry,
    grid_card, package_icon,
};
use crate::hooks::{
    package_meta_batch, use_cached_image, use_dispatch, use_notifications_snapshot,
    use_package_meta_batch,
};
use crate::notifications::{ClusterUpdateItem, OptionalModRef, OptionalModsGroup};
use crate::theme::colors;
use crate::ui::border_all_color;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);
const DIALOG_W: f32 = 720.;
const DIALOG_PAD: f32 = 22.;
const LIST_MAX_H: f32 = 306.;
const ICON_SIZE: f32 = 52.;

type MetaMap = HashMap<(ProviderId, String), CachedPackageMeta>;

type Picked = HashSet<(i64, String)>;

#[derive(PartialEq)]
pub struct OptionalModsPopup;

impl Component for OptionalModsPopup {
    fn render(&self) -> impl IntoElement {
        let snapshot = use_notifications_snapshot();
        let dispatch = use_dispatch();
        let picked = use_state(Picked::new);

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
                    .child(dialog(&groups, &meta, picked, dispatch)),
            )
            .into_element()
    }
}

fn package_key(item: &ClusterUpdateItem) -> String {
    item.offer
        .as_ref()
        .map(|(_, package_id)| package_id.clone())
        .unwrap_or_else(|| item.fallback.clone())
}

fn entry_from_item(
    item: &ClusterUpdateItem,
    meta: &MetaMap,
    package_id: String,
    selected: bool,
) -> PackageEntry {
    let cached = item
        .project_id
        .as_ref()
        .and_then(|project_id| meta.get(&(item.provider, project_id.clone())));

    PackageEntry {
        package_id,
        bundle_name: item.offer.as_ref().map(|(bundle, _)| bundle.clone()),
        provider: item.provider,
        name: cached
            .map(|cached| cached.name.clone())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| item.fallback.clone()),
        file_name: item.fallback.clone(),
        author: cached
            .map(|cached| cached.author.clone())
            .unwrap_or_default(),
        description: cached
            .map(|cached| cached.summary.clone())
            .unwrap_or_default(),
        icon_url: cached.and_then(|cached| cached.icon_url.clone()),
        size: 0,
        categories: Vec::new(),
        enabled: selected,
        installed: false,
        hash: None,
        manifest_default: false,
        hidden: false,
        update_available: false,
    }
}

fn dialog(
    groups: &[OptionalModsGroup],
    meta: &MetaMap,
    picked: State<Picked>,
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
        .child(content(groups, meta, picked, dispatch))
}

fn content(
    groups: &[OptionalModsGroup],
    meta: &MetaMap,
    picked: State<Picked>,
    dispatch: crate::Actions,
) -> impl IntoElement {
    let total: usize = groups.iter().map(|group| group.mods.len()).sum();
    let offers: Vec<(i64, OptionalModRef)> =
        groups.iter().flat_map(|group| group.offers()).collect();

    let chosen = picked.read().clone();
    let selected =
        |cluster_id: i64, package_id: &str| chosen.contains(&(cluster_id, package_id.to_string()));
    let (enable, skip): (Vec<_>, Vec<_>) = offers
        .iter()
        .cloned()
        .partition(|(cluster_id, (_, package_id))| selected(*cluster_id, package_id));

    let plural = if total == 1 { "" } else { "s" };
    let subtitle = match groups {
        [only] => format!(
            "{} includes {total} optional mod{plural} that {} off by default. Pick the {} to turn on.",
            only.cluster_name,
            if total == 1 { "is" } else { "are" },
            if total == 1 { "one" } else { "ones" }
        ),
        _ => format!(
            "Your bundles include {total} optional mod{plural} across {} clusters that are off by default. Pick the ones to turn on.",
            groups.len()
        ),
    };

    let cancel = dispatch.clone();
    let declined = offers.clone();

    let enable_text = if enable.is_empty() {
        "Enable".to_string()
    } else {
        format!("Enable {}", enable.len())
    };

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
        .child(mod_list(groups, meta, picked))
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
                    let apply = dispatch.clone();
                    Button::new()
                        .primary()
                        .disabled(enable.is_empty())
                        .on_press(move |_| {
                            apply.record_skipped_optional_mods(skip.clone());
                            apply.enable_optional_mods(enable.clone());
                        })
                        .child(Icon::new(IconType::Check).size(15.))
                        .text(enable_text.clone())
                        .into_element()
                })),
        )
}

fn grid_columns() -> usize {
    let content_w = DIALOG_W - DIALOG_PAD * 2.;
    (((content_w + GRID_GAP) / (GRID_MIN_W + GRID_GAP)).floor() as usize).clamp(1, 3)
}

fn mod_list(
    groups: &[OptionalModsGroup],
    meta: &MetaMap,
    picked: State<Picked>,
) -> impl IntoElement {
    let show_headers = groups.len() > 1;
    let cols = grid_columns();

    let mut list = rect().vertical().width(Size::fill()).spacing(GRID_GAP);

    for (group_index, group) in groups.iter().enumerate() {
        if show_headers {
            list = list.child(cluster_header(&group.cluster_name, group_index == 0));
        }

        for (row_index, row_items) in group.mods.chunks(cols).enumerate() {
            list = list.child(grid_row(
                group.cluster_id,
                row_items,
                meta,
                picked,
                cols,
                format!("{group_index}-{row_index}"),
            ));
        }
    }

    ScrollView::new()
        .width(Size::fill())
        .height(Size::Inner)
        .max_height(Size::px(LIST_MAX_H))
        .child(list)
}

fn grid_row(
    cluster_id: i64,
    items: &[ClusterUpdateItem],
    meta: &MetaMap,
    picked: State<Picked>,
    cols: usize,
    key: String,
) -> impl IntoElement {
    let mut row = rect()
        .key(key)
        .horizontal()
        .width(Size::fill())
        .height(Size::px(CARD_GRID_H))
        .spacing(GRID_GAP)
        .content(Content::Flex);

    for slot in 0..cols {
        let cell = rect().width(Size::flex(1.0)).height(Size::px(CARD_GRID_H));
        row = row.child(match items.get(slot) {
            Some(item) => {
                let package_id = package_key(item);
                cell.key(package_id.clone())
                    .child(mod_card(cluster_id, item, meta, picked, package_id))
            }
            None => cell,
        });
    }

    row
}

fn mod_card(
    cluster_id: i64,
    item: &ClusterUpdateItem,
    meta: &MetaMap,
    mut picked: State<Picked>,
    package_id: String,
) -> impl IntoElement {
    let key = (cluster_id, package_id.clone());
    let selected = picked.read().contains(&key);

    let on_toggle: EventHandler<()> = (move |()| {
        let mut next = picked.read().clone();
        if !next.remove(&key) {
            next.insert(key.clone());
        }
        picked.set(next);
    })
    .into();

    OptionalModCard {
        entry: entry_from_item(item, meta, package_id, selected),
        cluster_id,
        on_toggle,
    }
}

#[derive(PartialEq)]
struct OptionalModCard {
    entry: PackageEntry,
    cluster_id: i64,
    on_toggle: EventHandler<()>,
}

impl Component for OptionalModCard {
    fn render(&self) -> impl IntoElement {
        let icon_query = use_cached_image(self.entry.icon_url.clone(), 256);
        let icon = package_icon(&self.entry, &icon_query, ICON_SIZE);

        grid_card(
            &self.entry,
            "mod",
            self.cluster_id,
            icon,
            self.on_toggle.clone(),
            false,
        )
    }
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
