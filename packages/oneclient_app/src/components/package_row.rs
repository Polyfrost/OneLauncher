use freya::prelude::*;
use freya::query::QueryStateData;
use freya::router::RouterContext;
use oneclient_core::packages::ProviderId;

use crate::components::{Icon, IconType, toggle_controlled};
use crate::hooks::{ClusterAction, use_cached_image, use_cluster_mutation};
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::utils::format_size;

pub(crate) const CARD_BG: Color = Color::from_rgb(26, 34, 41);
pub(crate) const CARD_NAME: Color = Color::from_rgb(213, 219, 255);
pub(crate) const CARD_H: f32 = 84.;

#[derive(Clone, Copy, PartialEq)]
pub enum CardLayout {
    List,
    Grid,
}

impl From<oneclient_core::settings::ViewLayout> for CardLayout {
    fn from(layout: oneclient_core::settings::ViewLayout) -> Self {
        match layout {
            oneclient_core::settings::ViewLayout::Grid => CardLayout::Grid,
            oneclient_core::settings::ViewLayout::List => CardLayout::List,
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct PackageEntry {
    pub package_id: String,
    pub bundle_name: Option<String>,
    pub provider: ProviderId,
    pub name: String,
    pub file_name: String,
    pub author: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub size: u64,
    pub categories: Vec<String>,
    pub enabled: bool,
    pub installed: bool,
    pub hash: Option<String>,
}

impl PackageEntry {
    pub fn is_remote(&self) -> bool {
        self.provider != ProviderId::Local
    }

    pub fn in_bundle(&self) -> bool {
        self.bundle_name.is_some()
    }
}

#[derive(PartialEq)]
pub struct PackageRow {
    item: PackageEntry,
    cluster_id: i64,
    package_type: &'static str,
    layout: CardLayout,
    key: DiffKey,
}

impl PackageRow {
    pub fn new(item: PackageEntry, cluster_id: i64, package_type: &'static str) -> Self {
        Self {
            item,
            cluster_id,
            package_type,
            layout: CardLayout::List,
            key: DiffKey::None,
        }
    }

    pub fn layout(mut self, layout: CardLayout) -> Self {
        self.layout = layout;
        self
    }
}

impl KeyExt for PackageRow {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for PackageRow {
    fn render(&self) -> impl IntoElement {
        let item = self.item.clone();
        let cluster_id = self.cluster_id;
        let package_type = self.package_type;
        let layout = self.layout;
        let cluster = use_cluster_mutation();
        let remove_hover = use_state(|| false);

        let icon_size = match layout {
            CardLayout::List => 44.,
            CardLayout::Grid => 52.,
        };
        let icon_query = use_cached_image(item.icon_url.clone(), 256);
        let icon = if item.is_remote() {
            remote_icon(&item.icon_url, &icon_query, icon_size)
        } else {
            local_icon(icon_size)
        };

        let on_toggle: EventHandler<()> = {
            let hash = item.hash.clone();
            let bundle_name = item.bundle_name.clone();
            let package_id = item.package_id.clone();
            let enabled_now = item.enabled;
            (move |()| {
                if let Some(h) = &hash {
                    cluster.mutate(ClusterAction::ToggleArtifact {
                        cluster_id,
                        hash: h.clone(),
                    });
                } else if let Some(bundle) = &bundle_name {
                    cluster.mutate(ClusterAction::SetBundlePackageEnabled {
                        cluster_id,
                        bundle_name: bundle.clone(),
                        package_id: package_id.clone(),
                        enabled: !enabled_now,
                    });
                }
            })
            .into()
        };

        match layout {
            CardLayout::List => {
                let removable = !item.in_bundle();
                let can_remove = removable && item.installed;
                let rm_hash = item.hash.clone();
                let on_remove = move || {
                    if let Some(h) = &rm_hash {
                        cluster.mutate(ClusterAction::RemoveArtifact {
                            cluster_id,
                            hash: h.clone(),
                        });
                    }
                };
                list_card(
                    &item,
                    package_type,
                    cluster_id,
                    icon,
                    on_toggle,
                    can_remove,
                    on_remove,
                    remove_hover,
                )
            }
            CardLayout::Grid => grid_card(&item, package_type, cluster_id, icon, on_toggle),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn list_card(
    item: &PackageEntry,
    package_type: &'static str,
    cluster_id: i64,
    icon: impl IntoElement,
    on_toggle: EventHandler<()>,
    can_remove: bool,
    on_remove: impl FnMut() + 'static,
    hovering: State<bool>,
) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::px(CARD_H))
        .cross_align(Alignment::Center)
        .spacing(12.)
        .padding(Gaps::new_all(10.))
        .corner_radius(CornerRadius::new_all(8.))
        .background(CARD_BG)
        .content(Content::Flex)
        .child(package_info(item, package_type, cluster_id, icon))
        .child(meta_size(item.size))
        .child(toggle_controlled(item.enabled, on_toggle))
        .child(remove_button(can_remove, on_remove, hovering))
        .into_element()
}

fn grid_card(
    item: &PackageEntry,
    package_type: &'static str,
    cluster_id: i64,
    icon: impl IntoElement,
    on_toggle: EventHandler<()>,
) -> Element {
    let remote = item.is_remote();
    let enabled = item.enabled;
    let title = if remote {
        item.name.clone()
    } else {
        item.file_name.clone()
    };

    let (bg, border, content_alpha) = if enabled {
        (colors::brand().with_a(38), colors::brand(), 255)
    } else {
        (CARD_BG, colors::component_border(), 140)
    };

    let badge = if remote {
        let provider = item.provider;
        let package_id = item.package_id.clone();
        let package_type_owned = package_type.to_string();
        rect()
            .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
            .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
            .on_press(move |e: Event<PressEventData>| {
                e.stop_propagation();
                let _ = RouterContext::get().push(Route::BrowserPackage {
                    cluster_id,
                    package_type: package_type_owned.clone(),
                    package_id: format!("{}:{}", provider as u8, package_id),
                });
            })
            .child(provider_badge(item.provider))
            .into_element()
    } else {
        local_badge()
    };

    let header = rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(10.)
        .content(Content::Flex)
        .child(icon)
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(3.)
                .child(
                    label()
                        .text(title)
                        .font_size(14.)
                        .font_weight(FontWeight::MEDIUM)
                        .max_lines(1)
                        .width(Size::fill())
                        .color(CARD_NAME.with_a(content_alpha)),
                )
                .maybe(!item.author.is_empty(), |el| {
                    el.child(
                        label()
                            .text(format!("by {}", item.author))
                            .font_size(10.)
                            .max_lines(1)
                            .width(Size::fill())
                            .color(colors::fg_secondary().with_a(content_alpha)),
                    )
                })
                .child(badge),
        )
        .into_element();

    let description = if item.description.is_empty() {
        None
    } else {
        Some(
            label()
                .text(item.description.clone())
                .font_size(11.)
                .max_lines(3)
                .width(Size::fill())
                .color(colors::fg_secondary().with_a(content_alpha))
                .into_element(),
        )
    };

    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .spacing(8.)
        .padding(Gaps::new_all(12.))
        .corner_radius(CornerRadius::new_all(8.))
        .background(bg)
        .border(border_all_color(1.5, border))
        .content(Content::Flex)
        .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
        .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
        .on_press(move |_| on_toggle.call(()))
        .child(header)
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .height(Size::flex(1.0))
                .maybe_child(description),
        )
        .into_element()
}

fn package_info(
    item: &PackageEntry,
    package_type: &'static str,
    cluster_id: i64,
    icon: impl IntoElement,
) -> impl IntoElement {
    let remote = item.is_remote();
    let provider = item.provider;
    let package_id = item.package_id.clone();
    let package_type = package_type.to_string();
    let title = if remote {
        item.name.clone()
    } else {
        item.file_name.clone()
    };

    rect()
        .horizontal()
        .width(Size::flex(1.0))
        .cross_align(Alignment::Center)
        .spacing(12.)
        .content(Content::Flex)
        .maybe(remote, |el| {
            el.on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
                .on_press(move |_| {
                    let _ = RouterContext::get().push(Route::BrowserPackage {
                        cluster_id,
                        package_type: package_type.clone(),
                        package_id: format!("{}:{}", provider as u8, package_id),
                    });
                })
        })
        .child(icon)
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(3.)
                .child(
                    rect()
                        .horizontal()
                        .cross_align(Alignment::Center)
                        .spacing(8.)
                        .child(
                            label()
                                .text(title)
                                .font_size(15.)
                                .font_weight(FontWeight::MEDIUM)
                                .max_lines(1)
                                .color(CARD_NAME),
                        )
                        .child(if remote {
                            provider_badge(item.provider)
                        } else {
                            local_badge()
                        }),
                )
                .maybe(!item.author.is_empty(), |el| {
                    el.child(
                        label()
                            .text(format!("by {}", item.author))
                            .font_size(10.)
                            .color(colors::fg_secondary()),
                    )
                })
                .maybe(!item.description.is_empty(), |el| {
                    el.child(
                        label()
                            .text(item.description.clone())
                            .font_size(11.)
                            .max_lines(2)
                            .width(Size::fill())
                            .color(colors::fg_secondary()),
                    )
                }),
        )
        .into_element()
}

fn remote_icon(
    icon_url: &Option<String>,
    icon_query: &freya::query::UseQuery<crate::hooks::CachedImageQuery>,
    size: f32,
) -> Element {
    let reader = icon_query.read();
    let loaded = match (icon_url, &*reader.state()) {
        (Some(url), QueryStateData::Settled { res: Ok(bytes), .. })
        | (Some(url), QueryStateData::Loading { res: Some(Ok(bytes)) }) => {
            Some((url.clone(), bytes.clone()))
        }
        _ => None,
    };

    match loaded {
        Some((url, bytes)) => ImageViewer::new((url, bytes))
            .width(Size::px(size))
            .height(Size::px(size))
            .aspect_ratio(AspectRatio::Min)
            .corner_radius(CornerRadius::new_all(8.))
            .into_element(),

        None => icon_box(IconType::DotsGrid, size),
    }
}

fn local_icon(size: f32) -> Element {
    icon_box(IconType::HelpCircle, size)
}

fn icon_box(icon: IconType, size: f32) -> Element {
    rect()
        .center()
        .width(Size::px(size))
        .height(Size::px(size))
        .corner_radius(CornerRadius::new_all(8.))
        .background(colors::component_bg())
        .child(Icon::new(icon).size(size * 0.4).color(colors::fg_secondary()))
        .into_element()
}

fn meta_size(size: u64) -> impl IntoElement {
    rect()
        .maybe_child((size > 0).then(|| {
            label()
                .text(format_size(size))
                .font_size(11.)
                .color(colors::fg_secondary())
        }))
        .into_element()
}

fn provider_badge(provider: ProviderId) -> Element {
    badge(Icon::new(provider).size(12.).into_element(), provider.to_string())
}

fn local_badge() -> Element {
    badge(
        Icon::new(IconType::File02)
            .size(12.)
            .color(colors::fg_secondary())
            .into_element(),
        "Local file".to_string(),
    )
}

fn badge(icon: impl IntoElement, text: String) -> Element {
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(4.)
        .padding(Gaps::new_symmetric(2., 8.))
        .corner_radius(CornerRadius::new_all(999.))
        .border(border_all_color(1., colors::component_border()))
        .background(colors::component_bg())
        .child(icon)
        .child(
            label()
                .text(text)
                .font_size(10.)
                .font_weight(FontWeight::MEDIUM)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn remove_button(
    enabled: bool,
    mut on_remove: impl FnMut() + 'static,
    mut hovering: State<bool>,
) -> impl IntoElement {
    let hot = enabled && *hovering.read();
    let color = if !enabled {
        colors::fg_secondary().with_a(90)
    } else if hot {
        colors::danger()
    } else {
        colors::fg_secondary()
    };

    rect()
        .center()
        .width(Size::px(30.))
        .height(Size::px(30.))
        .corner_radius(CornerRadius::new_all(7.))
        .background(if hot {
            colors::danger().with_a(30)
        } else {
            Color::TRANSPARENT
        })
        .maybe(enabled, |el| {
            el.on_pointer_enter(move |_| {
                hovering.set(true);
                Cursor::set(CursorIcon::Pointer);
            })
            .on_pointer_leave(move |_| {
                hovering.set(false);
                Cursor::set(CursorIcon::default());
            })
            .on_press(move |_| on_remove())
        })
        .child(Icon::new(IconType::Trash01).size(14.).color(color))
        .into_element()
}
