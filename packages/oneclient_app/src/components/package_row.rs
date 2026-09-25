use freya::prelude::*;
use freya::query::UseMutation;
use freya::router::RouterContext;
use oneclient_content::packages::ProviderId;
use oneclient_core::SeenStatus;

use crate::components::{ContextMenu, Icon, IconType, toggle_controlled};
use crate::essential::EssentialPackage;
use crate::hooks::{
    ClusterAction, EssentialGuardKind, ClusterMutation, PendingEssential, loaded_image, use_cached_image,
    use_cluster_mutation, use_essential_guard,
};
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::{ImageFallbackExt, border_all_color};
use crate::utils::format_size;

pub(crate) const CARD_BG: Color = Color::from_rgb(26, 34, 41);
pub(crate) const CARD_NAME: Color = Color::from_rgb(213, 219, 255);
pub(crate) const CARD_H: f32 = 84.;
pub(crate) const CARD_GRID_H: f32 = 112.;
pub(crate) const GRID_GAP: f32 = 12.;
pub(crate) const GRID_MIN_W: f32 = 290.;

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
    pub version: Option<String>,
    pub description: String,
    pub icon_url: Option<String>,
    pub size: u64,
    pub categories: Vec<String>,
    pub enabled: bool,
    pub installed: bool,
    pub hash: Option<String>,
    pub manifest_default: bool,
    /// Private bundle dependency only set for bundle rows
    pub hidden: bool,
    /// Only set for browser-installed content bundle packages use the bundle update flow
    pub update_available: bool,
    /// Recency badge state cleared once the user views the list
    pub seen_status: SeenStatus,
    pub essential: Option<&'static EssentialPackage>,
}

impl PackageEntry {
    pub fn is_remote(&self) -> bool {
        self.provider != ProviderId::Local
    }

    pub fn in_bundle(&self) -> bool {
        self.bundle_name.is_some()
    }

    /// Bundle packages are never marked here so the two update flows cannot contradict
    pub fn is_outdated(&self) -> bool {
        self.update_available && self.is_remote() && !self.in_bundle()
    }

    pub fn recency_badge(&self) -> Option<Element> {
        match self.seen_status {
            SeenStatus::New => Some(new_badge()),
            SeenStatus::Updated => Some(updated_badge()),
            SeenStatus::Seen => None,
        }
    }
}

#[derive(PartialEq)]
pub struct PackageRow {
    item: PackageEntry,
    cluster_id: i64,
    package_type: &'static str,
    layout: CardLayout,
    on_context: EventHandler<(f32, f32)>,
    key: DiffKey,
}

impl PackageRow {
    pub fn new(item: PackageEntry, cluster_id: i64, package_type: &'static str) -> Self {
        Self {
            item,
            cluster_id,
            package_type,
            layout: CardLayout::List,
            on_context: (|_| {}).into(),
            key: DiffKey::None,
        }
    }

    pub fn layout(mut self, layout: CardLayout) -> Self {
        self.layout = layout;
        self
    }

    pub fn on_context(mut self, on_context: impl Into<EventHandler<(f32, f32)>>) -> Self {
        self.on_context = on_context.into();
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
        let guard = use_essential_guard();
        let hovered = use_state(|| false);

        let icon_size = match layout {
            CardLayout::List => 44.,
            CardLayout::Grid => 40.,
        };
        let icon_query = use_cached_image(item.icon_url.clone(), 256);
        let icon = package_icon(&item, &icon_query, icon_size);

        let on_toggle: EventHandler<()> = {
            let hash = item.hash.clone();
            let bundle_name = item.bundle_name.clone();
            let package_id = item.package_id.clone();
            let enabled_now = item.enabled;
            let manifest_default = item.manifest_default;
            let essential = item.essential;
            let mut guard = guard;
            (move |()| {
                let action = if let Some(h) = &hash {
                    ClusterAction::SetArtifactEnabled {
                        cluster_id,
                        hash: h.clone(),
                        enabled: !enabled_now,
                    }
                } else if let Some(bundle) = &bundle_name {
                    ClusterAction::SetBundlePackageEnabled {
                        cluster_id,
                        bundle_name: bundle.clone(),
                        package_id: package_id.clone(),
                        enabled: !enabled_now,
                        manifest_default,
                    }
                } else {
                    return;
                };

                match essential.filter(|_| enabled_now) {
                    Some(package) => guard.set(Some(PendingEssential {
                        package,
                        kind: EssentialGuardKind::Disable,
                        action,
                    })),
                    None => cluster.mutate(action),
                }
            })
            .into()
        };

        let on_context = has_menu(&item).then(|| self.on_context.clone());

        match layout {
            CardLayout::List => list_card(&item, package_type, cluster_id, icon, on_toggle, on_context),
            CardLayout::Grid => grid_card(
                &item,
                package_type,
                cluster_id,
                icon,
                on_toggle,
                true,
                on_context,
                hovered,
            ),
        }
    }
}

pub fn has_menu(item: &PackageEntry) -> bool {
    item.is_remote() || item.hash.is_some()
}

pub fn package_context_menu(
    x: f32,
    y: f32,
    item: &PackageEntry,
    cluster_id: i64,
    package_type: &'static str,
    cluster: UseMutation<ClusterMutation>,
) -> ContextMenu {
    let mut menu = ContextMenu::new(x, y).title(item.name.clone());

    if item.is_remote() {
        let provider = item.provider;
        let package_id = item.package_id.clone();
        let package_type = package_type.to_string();
        menu = menu.action(IconType::LinkExternal01, "View in browser", move |()| {
            let _ = RouterContext::get().push(Route::BrowserPackage {
                cluster_id,
                package_type: package_type.clone(),
                package_id: format!("{}:{}", provider as u8, package_id),
            });
        });
    }

    if let Some(hash) = item.hash.clone() {
        menu = menu.action(IconType::Folder, "View in folder", move |()| {
            reveal_in_store(hash.clone());
        });
    }

    if item.installed && !item.in_bundle() {
        let hash = item.hash.clone();
        menu = menu
            .separator()
            .danger_action(IconType::Trash01, "Delete", move |()| {
                if let Some(hash) = &hash {
                    cluster.mutate(ClusterAction::RemoveArtifact {
                        cluster_id,
                        hash: hash.clone(),
                    });
                }
            });
    }

    menu
}

fn on_secondary(handler: Option<EventHandler<(f32, f32)>>) -> impl FnMut(Event<PressEventData>) {
    move |e: Event<PressEventData>| {
        if let (Some(handler), PressEventData::Mouse(m)) = (handler.as_ref(), e.data()) {
            e.stop_propagation();
            handler.call((m.global_location.x as f32, m.global_location.y as f32));
        }
    }
}

fn list_card(
    item: &PackageEntry,
    package_type: &'static str,
    cluster_id: i64,
    icon: impl IntoElement,
    on_toggle: EventHandler<()>,
    on_context: Option<EventHandler<(f32, f32)>>,
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
        .on_secondary_down(on_secondary(on_context.clone()))
        .child(package_info(item, package_type, cluster_id, icon))
        .child(meta_size(item.size))
        .child(toggle_controlled(item.enabled, on_toggle))
        .maybe_child(on_context.map(kebab_button))
        .into_element()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn grid_card(
    item: &PackageEntry,
    package_type: &'static str,
    cluster_id: i64,
    icon: impl IntoElement,
    on_toggle: EventHandler<()>,
    navigable: bool, // if true, it takes the user to the mod page
    on_context: Option<EventHandler<(f32, f32)>>,
    mut hovered: State<bool>,
) -> Element {
    let enabled = item.enabled;
    let title = item.name.clone();
    let hovering = *hovered.read();

    let bg = match (enabled, hovering) {
		(_, true) => colors::component_bg_hover(),
        (true, false) => colors::component_bg(),
		(false, false) => colors::component_bg_disabled(),
    };

	let alpha = if enabled {
		255u8
	} else {
		115u8
	};

    let border = if !hovering {
        colors::component_border()
    } else {
        colors::component_border_hover()
    };

    let header = rect()
        .horizontal()
        .width(Size::fill())
        // .cross_align(Alignment::Center)
        .spacing(11.)
        .content(Content::Flex)
        .child(
			rect()
				.opacity(alpha as f32 / 255.)
				.child(icon)
		)
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(3.)
                .child(
                    label()
                        .text(title)
                        .font_size(14.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis)
                        .width(Size::fill())
                        .color(if enabled {
                            Color::WHITE
                        } else {
                            CARD_NAME.with_a(alpha)
                        }),
                )
                .child(grid_meta(item, package_type, cluster_id, navigable, alpha)),
        )
        .maybe_child(on_context.clone().map(kebab_button))
        .into_element();

    let description = (!item.description.is_empty()).then(|| {
        label()
            .text(item.description.clone())
            .font_size(11.)
            .line_height(1.45)
            .max_lines(2)
            .text_overflow(TextOverflow::Ellipsis)
            .width(Size::fill())
            .color(colors::fg_primary().with_a(scale_a(alpha, 0.72)))
            .into_element()
    });

    let floating = (item.is_outdated() || item.recency_badge().is_some()).then(|| {
        rect()
            .horizontal()
            .position(Position::new_absolute().bottom(10.).right(10.))
            .cross_align(Alignment::Center)
            .spacing(4.)
            .maybe_child(item.is_outdated().then(outdated_badge))
            .maybe_child(item.recency_badge())
            .into_element()
    });

    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .spacing(9.)
        .padding(Gaps::new_all(14.))
        .corner_radius(CornerRadius::new_all(6.))
        .background(bg.with_a(alpha))
        .border(border_all_color(1., border))
        .overflow(Overflow::Clip)
        .content(Content::Flex)
        .cursor(CursorIcon::Pointer)
        .on_pointer_enter(move |_| hovered.set(true))
        .on_pointer_leave(move |_| hovered.set(false))
        .on_secondary_down(on_secondary(on_context))
        .on_press(move |_| on_toggle.call(()))
        .child(header)
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .height(Size::flex(1.0))
                .maybe_child(description),
        )
        .maybe_child(floating)
        .into_element()
}

fn scale_a(alpha: u8, factor: f32) -> u8 {
    (alpha as f32 * factor) as u8
}

fn grid_meta(
    item: &PackageEntry,
    package_type: &'static str,
    cluster_id: i64,
    navigable: bool,
    alpha: u8,
) -> Element {
    let muted = CARD_NAME.with_a(scale_a(alpha, 0.5));

    let source = if item.is_remote() && navigable {
        SourceLink {
            provider: item.provider,
            package_id: item.package_id.clone(),
            package_type,
            cluster_id,
            alpha,
        }
        .into_element()
    } else if item.is_remote() {
        meta_text(item.provider.to_string(), muted)
    } else {
        meta_text("Local file".to_string(), muted)
    };

    let mut parts: Vec<Element> = Vec::new();
    if !item.author.is_empty() {
        parts.push(meta_text(format!("by {}", item.author), muted));
    }
    if let Some(version) = &item.version {
        parts.push(meta_text(version.clone(), muted));
    }
    parts.push(source);

    let mut row = rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(5.)
        .overflow(Overflow::Clip);

    for (i, part) in parts.into_iter().enumerate() {
        if i > 0 {
            row = row.child(meta_text("\u{b7}".to_string(), muted));
        }
        row = row.child(part);
    }

    row.into_element()
}

#[derive(PartialEq)]
struct SourceLink {
    provider: ProviderId,
    package_id: String,
    package_type: &'static str,
    cluster_id: i64,
    alpha: u8,
}

impl Component for SourceLink {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let mut pressed = use_state(|| false);

        let color = if *pressed.read() {
            colors::fg_primary_pressed()
        } else if *hovered.read() {
            colors::fg_primary_hover()
        } else {
            CARD_NAME.with_a(scale_a(self.alpha, 0.68))
        };

        let provider = self.provider;
        let package_id = self.package_id.clone();
        let package_type = self.package_type.to_string();
        let cluster_id = self.cluster_id;

        rect()
            .cursor(CursorIcon::Pointer)
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_leave(move |_| {
                hovered.set(false);
                pressed.set(false);
            })
            .on_pointer_down(move |_| pressed.set(true))
            .on_press(move |e: Event<PressEventData>| {
                e.stop_propagation();
                pressed.set(false);
                let _ = RouterContext::get().push(Route::BrowserPackage {
                    cluster_id,
                    package_type: package_type.clone(),
                    package_id: format!("{}:{}", provider as u8, package_id),
                });
            })
            .child(meta_text(provider.to_string(), color))
    }
}

fn meta_text(text: String, color: Color) -> Element {
    label()
        .text(text)
        .font_size(11.)
        .max_lines(1)
        .color(color)
        .into_element()
}

fn kebab_button(on_context: EventHandler<(f32, f32)>) -> Element {
    KebabButton { on_context }.into_element()
}

#[derive(PartialEq)]
struct KebabButton {
    on_context: EventHandler<(f32, f32)>,
}

impl Component for KebabButton {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let on_context = self.on_context.clone();

        rect()
            .center()
            .width(Size::px(26.))
            .height(Size::px(26.))
            .corner_radius(CornerRadius::new_all(6.))
            .background(if *hovered.read() {
                colors::ghost_overlay_hover()
            } else {
                Color::TRANSPARENT
            })
            .cursor(CursorIcon::Pointer)
            .a11y_role(AccessibilityRole::Button)
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_leave(move |_| hovered.set(false))
            .on_press(move |e: Event<PressEventData>| {
                e.stop_propagation();
                if let PressEventData::Mouse(m) = e.data() {
                    on_context.call((m.global_location.x as f32, m.global_location.y as f32));
                }
            })
            .child(
                Icon::new(IconType::DotsVertical)
                    .size(16.)
                    .color(if *hovered.read() {
                        colors::fg_primary()
                    } else {
                        colors::fg_secondary()
                    }),
            )
    }
}

fn reveal_in_store(hash: String) {
    spawn(async move {
        let Ok(state) = crate::launcher::state() else {
            return;
        };
        let row = match oneclient_db::dao::artifact::get_artifact_by_hash(&state.services.db, &hash)
            .await
        {
            Ok(Some(row)) => row,
            Ok(None) => return,
            Err(err) => {
                tracing::warn!(%err, "failed to look up artifact for reveal");
                return;
            }
        };
        let Ok(path) = oneclient_content::packages::store::artifact_absolute_path(&row.path) else {
            return;
        };
        if let Some(dir) = path.parent() {
            crate::platform::open_path(&dir.to_string_lossy());
        }
    });
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
    let title = item.name.clone();

    rect()
        .horizontal()
        .width(Size::flex(1.0))
        .cross_align(Alignment::Center)
        .spacing(12.)
        .content(Content::Flex)
        .maybe(remote, |el| {
            el.cursor(CursorIcon::Pointer).on_press(move |_| {
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
                        .width(Size::fill())
                        .cross_align(Alignment::Center)
                        .spacing(8.)
                        .child(
                            label()
                                .text(title)
                                .font_size(15.)
                                .font_weight(FontWeight::MEDIUM)
                                .max_lines(1)
                                .text_overflow(TextOverflow::Ellipsis)
                                .max_width(Size::percent(60.))
                                .color(CARD_NAME),
                        )
                        .child(if remote {
                            provider_badge(item.provider)
                        } else {
                            local_badge()
                        })
                        .maybe_child(item.is_outdated().then(outdated_badge))
                        .maybe_child(item.recency_badge()),
                )
                .maybe(!item.author.is_empty(), |el| {
                    el.child(
                        label()
                            .text(format!("by {}", item.author))
                            .font_size(10.)
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .width(Size::fill())
                            .color(colors::fg_secondary()),
                    )
                })
                .maybe(!item.description.is_empty(), |el| {
                    el.child(
                        label()
                            .text(item.description.clone())
                            .font_size(11.)
                            .max_lines(2)
                            .text_overflow(TextOverflow::Ellipsis)
                            .width(Size::fill())
                            .color(colors::fg_secondary()),
                    )
                }),
        )
        .into_element()
}

pub(crate) fn package_icon(
    item: &PackageEntry,
    icon_query: &freya::query::UseQuery<crate::hooks::CachedImageQuery>,
    size: f32,
) -> Element {
    let icon_url = &item.icon_url;
    let loaded = loaded_image(icon_url.as_deref(), icon_query);

    match loaded {
        Some((url, bytes)) => ImageViewer::new((url, bytes))
            .width(Size::px(size))
            .height(Size::px(size))
            .aspect_ratio(AspectRatio::Min)
            .corner_radius(CornerRadius::new_all(8.))
            .fallback(icon_box(IconType::DotsGrid, size))
            .into_element(),

        None if icon_url.is_some() => icon_box(IconType::DotsGrid, size),
        None => icon_box(IconType::HelpCircle, size),
    }
}

fn icon_box(icon: IconType, size: f32) -> Element {
    rect()
        .center()
        .width(Size::px(size))
        .height(Size::px(size))
        .corner_radius(CornerRadius::new_all(8.))
        .background(colors::component_bg())
        .child(
            Icon::new(icon)
                .size(size * 0.4)
                .color(colors::fg_secondary()),
        )
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

pub fn provider_badge(provider: ProviderId) -> Element {
    badge(
        Icon::new(provider).size(12.).into_element(),
        provider.to_string(),
    )
}

fn outdated_badge() -> Element {
    accent_badge(
        Icon::new(IconType::RefreshCw01)
            .size(12.)
            .color(colors::brand())
            .into_element(),
        "Update available".to_string(),
        colors::brand(),
    )
}

fn new_badge() -> Element {
    accent_badge(
        Icon::new(IconType::Plus)
            .size(12.)
            .color(colors::success())
            .into_element(),
        "New".to_string(),
        colors::success(),
    )
}

fn updated_badge() -> Element {
    accent_badge(
        Icon::new(IconType::RefreshCcw02)
            .size(12.)
            .color(colors::success())
            .into_element(),
        "Updated".to_string(),
        colors::success(),
    )
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
    accent_badge(icon, text, colors::fg_secondary())
}

fn accent_badge(icon: impl IntoElement, text: String, accent: Color) -> Element {
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
                .color(accent),
        )
        .into_element()
}
