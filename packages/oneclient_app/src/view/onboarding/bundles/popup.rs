use super::*;

use std::collections::HashSet;

use freya::query::QueryStateData;
use oneclient_core::packages::{CachedPackageMeta, ProviderId};
use oneclient_core::{BundleArchive, BundleFileKind};

use crate::components::{
    Button, Icon, OverlayPopup, ScrollArea, TabBar, TabItem,
};
use crate::hooks::{
    package_meta_batch, use_cached_image, use_package_meta_batch,
};
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::utils::format_size;

pub(super) struct BundlePopup {
    pub(super) bundle: UnifiedBundle,
    pub(super) selected: State<HashSet<String>>,
    pub(super) open: State<Option<String>>,
}

impl PartialEq for BundlePopup {
    fn eq(&self, other: &Self) -> bool {
        self.bundle.display_name == other.bundle.display_name
            && self.bundle.members.len() == other.bundle.members.len()
    }
}

impl Component for BundlePopup {
    fn render(&self) -> impl IntoElement {
        let selected = self.selected;
        let mut open = self.open;
        let members = &self.bundle.members;

        let mut active = use_state(|| 0usize);
        let active_idx = (*active.read()).min(members.len().saturating_sub(1));
        let member = members.get(active_idx);

        let cluster_id = member.map(|m| m.cluster.id).unwrap_or(0);
        let bundle_name = member
            .map(|m| m.archive.manifest.name.clone())
            .unwrap_or_default();

        let (mr_ids, cf_ids) = member.map(|m| collect_ids(&m.archive)).unwrap_or_default();
        let mr_meta = package_meta_batch(&use_package_meta_batch(ProviderId::Modrinth, mr_ids));
        let cf_meta = package_meta_batch(&use_package_meta_batch(ProviderId::CurseForge, cf_ids));

        let selected_set = selected.read().clone();

        let tabs: Vec<TabItem> = members
            .iter()
            .enumerate()
            .map(|(i, m)| {
                TabItem::new(m.cluster.mc_version.clone(), i == active_idx)
                    .on_press(move |_| active.set(i))
            })
            .collect();

        let mut sections: Vec<Element> = Vec::new();
        let mut selected_count = 0usize;

        if let Some(member) = member {
            for ct in SECTION_ORDER {
                let rows: Vec<Element> = member
                    .archive
                    .manifest
                    .files
                    .iter()
                    .filter(|f| !f.hidden && f.content_type() == ct)
                    .map(|file| {
                        let provider = file_provider(file);
                        let package_id = file.kind.package_id();
                        let meta = match provider {
                            ProviderId::Modrinth => mr_meta.get(&package_id),
                            ProviderId::CurseForge => cf_meta.get(&package_id),
                            ProviderId::Local => None,
                        };
                        let name = meta
                            .map(|m| m.name.clone())
                            .filter(|n| !n.is_empty())
                            .unwrap_or_else(|| file.display_name());
                        let author = meta.map(|m| m.author.clone()).unwrap_or_default();
                        let icon_url = meta.and_then(|m: &CachedPackageMeta| m.icon_url.clone());
                        let key = pkg_key(cluster_id, &bundle_name, &package_id);
                        let enabled = selected_set.contains(&key);
                        if enabled {
                            selected_count += 1;
                        }
                        let toggle_key = key.clone();

                        rect()
                            .key(&package_id)
                            .width(Size::fill())
                            .child(OnboardingPackageRow {
                                provider,
                                name,
                                author,
                                icon_url,
                                size: file.size,
                                enabled,
                                on_toggle: (move |()| flip(selected, toggle_key.clone())).into(),
                            })
                            .into_element()
                    })
                    .collect();
                if !rows.is_empty() {
                    sections.push(section(section_label(ct), rows).into_element());
                }
            }
        }
        let sections_empty = sections.is_empty();

        let title = self.bundle.display_name.clone();

        OverlayPopup::new()
            .on_close(move |()| open.set(None))
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(640.))
                            .max_width(Size::window_percent(92.))
                            .height(Size::px(560.))
                            .max_height(Size::window_percent(88.))
                            .spacing(14.)
                            .padding(Gaps::new_all(20.))
                            .corner_radius(CornerRadius::new_all(16.))
                            .background(colors::page_elevated())
                            .border(border_all_color(1., colors::component_border()))
                            .content(Content::Flex)
                            .child(
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .cross_align(Alignment::Center)
                                    .spacing(6.)
                                    .child(
                                        label()
                                            .text("Content for ")
                                            .font_size(18.)
                                            .font_weight(FontWeight::SEMI_BOLD)
                                            .color(colors::fg_primary()),
                                    )
                                    .child(
                                        label()
                                            .text(title)
                                            .font_size(18.)
                                            .font_weight(FontWeight::SEMI_BOLD)
                                            .color(colors::brand()),
                                    ),
                            )
                            .maybe_child((members.len() > 1).then(|| {
                                TabBar::new()
                                    .tabs(tabs)
                                    .width(Size::fill())
                                    .height(Size::px(30.))
                                    .spacing(20.)
                                    .font_size(13.)
                                    .into_element()
                            }))
                            .child(
                                rect()
                                    .vertical()
                                    .width(Size::fill())
                                    .height(Size::flex(1.0))
                                    .content(Content::Flex)
                                    .child(
                                        ScrollArea::new()
                                            .width(Size::fill())
                                            .height(Size::flex(1.0))
                                            .spacing(16.)
                                            .children(sections),
                                    )
                                    .maybe_child(
                                        sections_empty
                                            .then(|| empty_hint("No content in this bundle.")),
                                    ),
                            )
                            .child(
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .cross_align(Alignment::Center)
                                    .spacing(16.)
                                    .content(Content::Flex)
                                    .child(
                                        label()
                                            .text(format!("Selected {selected_count}"))
                                            .width(Size::flex(1.0))
                                            .font_size(13.)
                                            .color(colors::fg_secondary()),
                                    )
                                    .child(
                                        Button::new()
                                            .primary()
                                            .on_press(move |_| open.set(None))
                                            .text("Confirm"),
                                    ),
                            ),
                    ),
            )
    }
}

fn collect_ids(archive: &BundleArchive) -> (Vec<String>, Vec<String>) {
    let mut mr = Vec::new();
    let mut cf = Vec::new();
    for file in &archive.manifest.files {
        if file.hidden {
            continue;
        }
        if let BundleFileKind::Managed {
            provider,
            project_id,
            ..
        } = &file.kind
        {
            match provider {
                ProviderId::Modrinth => mr.push(project_id.clone()),
                ProviderId::CurseForge => cf.push(project_id.clone()),
                ProviderId::Local => {}
            }
        }
    }
    (mr, cf)
}

fn section(title: &str, rows: Vec<Element>) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(8.)
        .child(
            label()
                .text(title.to_string())
                .font_size(13.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_secondary()),
        )
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(8.)
                .children(rows),
        )
        .into_element()
}

#[derive(PartialEq)]
struct OnboardingPackageRow {
    provider: ProviderId,
    name: String,
    author: String,
    icon_url: Option<String>,
    size: u64,
    enabled: bool,
    on_toggle: EventHandler<()>,
}

impl Component for OnboardingPackageRow {
    fn render(&self) -> impl IntoElement {
        let icon_query = use_cached_image(self.icon_url.clone(), 128);
        let loaded = {
            let reader = icon_query.read();
            match (&self.icon_url, &*reader.state()) {
                (Some(url), QueryStateData::Settled { res: Ok(bytes), .. })
                | (
                    Some(url),
                    QueryStateData::Loading {
                        res: Some(Ok(bytes)),
                    },
                ) => Some((url.clone(), bytes.clone())),
                _ => None,
            }
        };

        let icon = match loaded {
            Some((url, bytes)) => ImageViewer::new((url, bytes))
                .width(Size::px(44.))
                .height(Size::px(44.))
                .aspect_ratio(AspectRatio::Min)
                .corner_radius(CornerRadius::new_all(8.))
                .into_element(),

            None => icon_box(self.provider).into_element(),
        };

        let (bg, border) = if self.enabled {
            (colors::brand().with_a(38), colors::brand())
        } else {
            (CARD_BG, colors::component_border())
        };
        let alpha = if self.enabled { 255 } else { 150 };

        let subtitle = if self.author.is_empty() {
            self.provider.to_string()
        } else {
            format!("by {}", self.author)
        };

        let on_toggle = self.on_toggle.clone();
        let size = self.size;

        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(64.))
            .cross_align(Alignment::Center)
            .spacing(12.)
            .padding(Gaps::new_all(10.))
            .corner_radius(CornerRadius::new_all(8.))
            .background(bg)
            .border(border_all_color(1.5, border))
            .content(Content::Flex)
            .a11y_role(AccessibilityRole::Button)
            .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
            .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
            .on_press(move |_| on_toggle.call(()))
            .child(icon)
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(3.)
                    .child(
                        label()
                            .text(self.name.clone())
                            .font_size(15.)
                            .font_weight(FontWeight::MEDIUM)
                            .max_lines(1)
                            .color(CARD_NAME.with_a(alpha)),
                    )
                    .child(
                        label()
                            .text(subtitle)
                            .font_size(10.)
                            .max_lines(1)
                            .color(colors::fg_secondary().with_a(alpha)),
                    ),
            )
            .maybe_child((size > 0).then(|| {
                label()
                    .text(format_size(size))
                    .font_size(11.)
                    .color(colors::fg_secondary())
                    .into_element()
            }))
    }
}

fn icon_box(provider: ProviderId) -> impl IntoElement {
    rect()
        .center()
        .width(Size::px(44.))
        .height(Size::px(44.))
        .corner_radius(CornerRadius::new_all(8.))
        .background(colors::component_bg())
        .child(Icon::new(provider).size(20.).color(colors::fg_secondary()))
        .into_element()
}

pub(super) fn empty_hint(text: &str) -> impl IntoElement {
    rect()
        .width(Size::fill())
        .padding(Gaps::new_all(24.))
        .center()
        .child(
            label()
                .text(text.to_string())
                .font_size(13.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}
