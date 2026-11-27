use std::collections::BTreeMap;

use freya::prelude::*;
use freya::router::RouterContext;
use freya::text_edit::Clipboard;
use oneclient_content::packages::{ContentType, ProviderId};
use oneclient_core::{BisectRole, SeenStatus};

use crate::components::{
    ContextMenu, Icon, IconType, TooltipPlacement, toggle_controlled, use_tooltip_anchor,
};
use crate::disable::{self, BundlePackage, DisableRequest, DisableRoot};
use crate::essential::EssentialPackage;
use freya::query::UseMutation;

use crate::hooks::{
    Actions, ClusterAction, ClusterMutation, EssentialGuardKind, Guarded, PendingEssential,
    disable_warnings, loaded_image, use_cached_image, use_cluster_mutation, use_disable_confirm,
    use_disable_warnings, use_dispatch, use_essential_guard,
};
use crate::i18n::{t, tf};
use crate::routes::Route;
use crate::theme::{colors, rounded, text_size};
use crate::ui::{ImageFallbackExt, border_all_color};
use crate::utils::format_size;

/// List row height on every instance content page (mods, packs, worlds, screenshots)
pub(crate) const CARD_H: f32 = 64.;
/// Padding of every content row and card
pub(crate) const ROW_PAD: f32 = 12.;
/// Icon edge in content rows and cards
pub(crate) const ROW_ICON: f32 = 40.;
pub(crate) const ROW_RADIUS: f32 = 12.;
/// Gap between list rows
pub(crate) const ROW_GAP: f32 = 8.;
/// Grid card height when a description has room for two lines
pub(crate) const CARD_GRID_H: f32 = 112.;
/// Grid card height when no card in its row has a description
pub(crate) const CARD_GRID_MIN_H: f32 = 88.;
pub(crate) const GRID_GAP: f32 = 12.;
pub(crate) const GRID_MIN_W: f32 = 290.;
/// Narrowest grid column on the instance content pages
pub(crate) const CONTENT_GRID_MIN_W: f32 = 280.;
const GRID_CARD_PADDING: f32 = ROW_PAD;
const BADGE_STRIP_H: f32 = 20.;
const BADGE_STRIP_LIFT: f32 = 2.;
/// Tint and outline of a selected card
const SELECTION_LAYER: i16 = 16;
/// The grid card's floating badges straddle its bottom edge, so they paint over
/// the selection outline and cover it the way they cover the card's own border
const BADGE_STRIP_LAYER: i16 = SELECTION_LAYER + 1;
/// Catches clicks while selecting, above the badges and everything nested in them,
/// so clicking a badge still selects the card instead of toggling it
const SELECTION_INPUT_LAYER: i16 = BADGE_STRIP_LAYER + 16;

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

/// Grid cards in a row share the height required by the longest content.
pub(crate) fn content_card_height(any_description: bool) -> f32 {
    if any_description {
        CARD_GRID_H
    } else {
        CARD_GRID_MIN_H
    }
}

/// How a grid card reads its on/off state
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum CardTone {
    /// Content without an on/off state (worlds, data packs)
    Neutral,
    Enabled,
    Disabled,
}

/// Content alpha of a switched-off card, as upstream's Mods grid draws it
const DISABLED_ALPHA: u8 = 115;

/// The surface of a list row on every instance content page
pub(crate) fn row_surface(mut hovered: State<bool>) -> Rect {
    rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::px(CARD_H))
        .cross_align(Alignment::Center)
        .spacing(12.)
        .padding(Gaps::new_all(ROW_PAD))
        .corner_radius(rounded(ROW_RADIUS))
        .background(colors::card())
        .border(
            border_all_color(
                1.,
                if hovered() {
                    colors::component_border_hover()
                } else {
                    colors::component_border()
                },
            )
            .alignment(BorderAlignment::Inner),
        )
        .on_pointer_enter(move |_| hovered.set_if_modified(true))
        .on_pointer_leave(move |_| hovered.set_if_modified(false))
        .content(Content::Flex)
}

/// The surface of a grid card on every instance content page
///
/// Enabled content is brand tinted and switched-off content is dimmed, the way upstream's Mods grid
/// draws it, so every content type reads its state the same way.
pub(crate) fn card_surface(mut hovered: State<bool>, tone: CardTone) -> Rect {
    let hovering = hovered();
    // Hover keeps the card's own tint and only lifts it, so a blue (enabled) or dimmed (disabled)
    // card never flips to a flat grey under the pointer
    let background = match (tone, hovering) {
        (CardTone::Enabled, false) => colors::brand().with_a(38),
        (CardTone::Enabled, true) => colors::brand().with_a(58),
        (CardTone::Disabled, false) => colors::component_bg_disabled().with_a(DISABLED_ALPHA),
        (CardTone::Disabled, true) => colors::component_bg_disabled().with_a(DISABLED_ALPHA + 40),
        (CardTone::Neutral, _) => colors::card(),
    };
    let border = match (tone, hovering) {
        (CardTone::Enabled, _) => colors::brand(),
        (_, true) => colors::component_border_hover(),
        _ => colors::component_border(),
    };

    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .spacing(8.)
        .padding(Gaps::new_all(GRID_CARD_PADDING))
        .corner_radius(rounded(ROW_RADIUS))
        .background(background)
        .border(border_all_color(1., border).alignment(BorderAlignment::Inner))
        .content(Content::Flex)
        .on_pointer_enter(move |_| hovered.set_if_modified(true))
        .on_pointer_leave(move |_| hovered.set_if_modified(false))
}

/// Title line of a content row or card
fn row_title_label(text: String, color: Color) -> Label {
    label()
        .text(text)
        .font_size(text_size(14.))
        .font_weight(FontWeight::SEMI_BOLD)
        .max_lines(1)
        .text_overflow(TextOverflow::Ellipsis)
        .color(color)
}

/// One secondary line under a row title: meta and description share it
pub(crate) fn row_line(text: String, color: Color) -> Element {
    label()
        .text(text)
        .font_size(text_size(12.))
        .max_lines(1)
        .text_overflow(TextOverflow::Ellipsis)
        .width(Size::fill())
        .color(color)
        .into_element()
}

/// The body text of a grid card; one line on short cards, two on tall ones
pub(crate) fn card_description(text: String, color: Color) -> Element {
    card_description_label(text, color, 2).into_element()
}

const DESCRIPTION_LINE_HEIGHT: f32 = 1.4;

fn card_description_label(text: String, color: Color, lines: usize) -> Label {
    label()
        .text(text)
        .font_size(text_size(12.))
        .line_height(DESCRIPTION_LINE_HEIGHT)
        .max_lines(lines)
        .text_overflow(TextOverflow::Ellipsis)
        .width(Size::fill())
        .color(color)
}

/// Joins the non-empty parts of a row's secondary line
pub(crate) fn joined_line<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    parts
        .into_iter()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" \u{b7} ")
}

#[derive(Clone, Copy, PartialEq)]
pub enum CopyStatus {
    Loaded,
    Skipped,
}

#[derive(Clone, PartialEq)]
pub struct PackageEntry {
    pub package_id: String,
    pub bundle_name: Option<String>,
    pub provider: ProviderId,
    pub github_hosted: bool,
    pub github_url: Option<String>,
    pub name: String,
    pub file_name: String,
    pub cluster_file_name: String,
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
    /// False for uninstalled rows of bundles the cluster never took keeps them out of the All tab
    pub opted_in: bool,
    pub advanced: bool,
    pub deleted: bool,
    /// Only set for browser-installed content bundle packages use the bundle update flow
    pub update_available: bool,
    pub copy_status: Option<CopyStatus>,
    /// Other copies of this mod the loader ignores, folded into this row
    pub copies: Vec<PackageEntry>,
    /// Recency badge state cleared once the user views the list
    pub seen_status: SeenStatus,
    pub essential: Option<&'static EssentialPackage>,
    pub bisect_role: Option<BisectRole>,
    pub locked: bool,
    pub modpack: Option<String>,
    /// RFC 3339 so string order matches time order
    pub added_at: Option<String>,
    /// When the installed version was released RFC 3339
    pub published_at: Option<String>,
}

pub fn folder_pack_id(name: &str, enabled: bool) -> String {
    let state = if enabled { "on" } else { "off" };
    format!("folder:{state}:{name}")
}

pub fn parse_folder_pack_id(id: &str) -> Option<(&str, bool)> {
    let (state, name) = id.strip_prefix("folder:")?.split_once(':')?;
    match state {
        "on" => Some((name, true)),
        "off" => Some((name, false)),
        _ => None,
    }
}

impl PackageEntry {
    pub fn folder_pack(&self) -> Option<(&str, bool)> {
        (self.package_id.starts_with("folder:") && self.hash.is_none() && !self.is_remote())
            .then_some((self.file_name.as_str(), self.enabled))
    }

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

    pub fn update_checkable(&self) -> bool {
        self.installed && self.hash.is_some() && self.is_remote() && !self.in_bundle()
    }

    pub fn recency_badge(&self) -> Option<Element> {
        match self.seen_status {
            SeenStatus::New => Some(new_badge()),
            SeenStatus::Updated => Some(updated_badge()),
            SeenStatus::Seen => None,
        }
    }

    pub fn added_label(&self) -> Option<String> {
        self.copy_status?;
        let added = chrono::DateTime::parse_from_rfc3339(self.added_at.as_deref()?).ok()?;
        Some(tf(
            "oneclient.components.package_row.added",
            &[&crate::ui::fmt_local_datetime(added)],
        ))
    }

    pub fn copy_badge(&self) -> Option<Element> {
        match self.copy_status? {
            CopyStatus::Loaded => Some(status_tag(
                t("oneclient.components.package_row.loaded"),
                colors::success(),
            )),
            CopyStatus::Skipped => Some(pill(
                None,
                t("oneclient.components.package_row.skipped").to_string(),
                colors::fg_secondary(),
            )),
        }
    }

    pub fn bisect_badge(&self) -> Option<Element> {
        let role = self.bisect_role?;
        let accent = match role {
            BisectRole::Testing | BisectRole::Helper | BisectRole::Required => colors::brand(),
            BisectRole::Waiting | BisectRole::KeptOff | BisectRole::Omitted => {
                colors::fg_secondary()
            }
            BisectRole::Cleared => colors::success(),
            BisectRole::Found | BisectRole::Problem | BisectRole::Unresolvable => colors::danger(),
        };
        Some(status_tag(role.label(), accent))
    }
}

#[derive(PartialEq)]
pub struct PackageRow {
    item: PackageEntry,
    cluster_id: i64,
    package_type: &'static str,
    layout: CardLayout,
    on_context: EventHandler<(f32, f32, Vec<String>)>,
    selected: bool,
    selecting: bool,
    on_select: EventHandler<()>,
    on_copies: Option<EventHandler<()>>,
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
            selected: false,
            selecting: false,
            on_select: (|()| {}).into(),
            on_copies: None,
            key: DiffKey::None,
        }
    }

    pub fn layout(mut self, layout: CardLayout) -> Self {
        self.layout = layout;
        self
    }

    pub fn on_context(mut self, handler: impl Into<EventHandler<(f32, f32, Vec<String>)>>) -> Self {
        self.on_context = handler.into();
        self
    }

    pub fn on_copies(mut self, on_copies: impl Into<EventHandler<()>>) -> Self {
        self.on_copies = Some(on_copies.into());
        self
    }

    pub fn selection(
        mut self,
        selected: bool,
        selecting: bool,
        on_select: impl Into<EventHandler<()>>,
    ) -> Self {
        self.selected = selected;
        self.selecting = selecting;
        self.on_select = on_select.into();
        self
    }
}

impl KeyExt for PackageRow {
    fn key_mut(&mut self) -> &mut DiffKey {
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
        let warnings_query = use_disable_warnings();
        let hovered = use_state(|| false);

        let icon_query = use_cached_image(item.icon_url.clone(), 256);
        let icon = package_icon(&item, &icon_query, ROW_ICON);

        let on_toggle: EventHandler<()> = {
            let action = toggle_action(&item, cluster_id, !item.enabled).filter(|_| !item.locked);
            let item = item.clone();
            let warnings = disable_warnings(&warnings_query);
            let pending_disable = use_disable_confirm();
            let dispatch = use_dispatch();
            (move |()| {
                let Some(action) = action.clone() else {
                    return;
                };
                // Switching one off can take other packages with it, so it goes
                // through the impact check rather than straight to the flag.
                // Switching one on never does.
                if item.enabled && item.folder_pack().is_none() {
                    guard_disable(
                        disable_request(&[&item], cluster_id, item.name.clone()),
                        &[&item],
                        warnings.clone(),
                        guard,
                        pending_disable,
                        dispatch.clone(),
                    );
                    return;
                }
                cluster.mutate(action);
            })
            .into()
        };

        let hidden_badges = use_state(Vec::<String>::new);
        let on_context = self.on_context.clone();
        let on_context = Some(EventHandler::new(move |(x, y)| {
            let hidden = if layout == CardLayout::List {
                hidden_badges.peek().clone()
            } else {
                Vec::new()
            };
            on_context.call((x, y, hidden));
        }));
        let on_copies = self.on_copies.clone().filter(|_| !item.copies.is_empty());

        let radius = ROW_RADIUS;
        let card = match layout {
            CardLayout::List => list_card(
                &item,
                package_type,
                cluster_id,
                icon,
                on_toggle,
                on_context.clone(),
                on_copies,
                hovered,
                hidden_badges,
            ),
            CardLayout::Grid => render_grid_card(
                &item,
                package_type,
                cluster_id,
                icon,
                GridToggle::Switch(on_toggle),
                true,
                on_context.clone(),
                on_copies,
                hovered,
            ),
        };

        let on_select = self.on_select.clone();
        let highlight = (self.selecting && self.selected).then(|| {
            rect()
                .position(Position::new_absolute().top(0.).left(0.))
                .width(Size::percent(100.))
                .height(Size::percent(100.))
                .layer(Layer::Relative(SELECTION_LAYER))
                .corner_radius(rounded(radius))
                .background(colors::brand().with_a(28))
                .border(border_all_color(2., colors::brand()).alignment(BorderAlignment::Inner))
                .into_element()
        });
        let overlay = self.selecting.then(|| {
            rect()
                .position(Position::new_absolute().top(0.).left(0.))
                .width(Size::percent(100.))
                .height(Size::percent(100.))
                .layer(Layer::Relative(SELECTION_INPUT_LAYER))
                .corner_radius(rounded(radius))
                .cursor(CursorIcon::Pointer)
                .on_press(move |_| on_select.call(()))
                .on_secondary_down(on_secondary(on_context))
                .into_element()
        });

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .child(card)
            .maybe_child(highlight)
            .maybe_child(overlay)
    }
}

pub(crate) fn toggle_action(
    item: &PackageEntry,
    cluster_id: i64,
    enabled: bool,
) -> Option<ClusterAction> {
    if let Some((name, _)) = item.folder_pack() {
        return Some(ClusterAction::SetFolderPackEnabled {
            cluster_id,
            name: name.to_string(),
            enabled,
        });
    }
    set_enabled_action(
        cluster_id,
        item.hash.as_deref(),
        item.bundle_name.as_deref(),
        &item.package_id,
        item.manifest_default,
        enabled,
    )
}

pub(crate) fn disable_request(
    items: &[&PackageEntry],
    cluster_id: i64,
    name: String,
) -> DisableRequest {
    let roots = items
        .iter()
        .map(|item| match &item.hash {
            Some(hash) => DisableRoot::Artifact(hash.clone()),
            None => DisableRoot::Package(item.provider, item.package_id.clone()),
        })
        .collect();
    let bundles = items
        .iter()
        .filter(|item| item.hash.is_none())
        .filter_map(|item| {
            Some(BundlePackage {
                bundle_name: item.bundle_name.clone()?,
                package_id: item.package_id.clone(),
            })
        })
        .collect();

    DisableRequest {
        cluster_id,
        name,
        roots,
        bundles,
        warned: Vec::new(),
    }
}

pub(crate) fn set_enabled_action(
    cluster_id: i64,
    hash: Option<&str>,
    bundle_name: Option<&str>,
    package_id: &str,
    manifest_default: bool,
    enabled: bool,
) -> Option<ClusterAction> {
    if let Some(hash) = hash {
        Some(ClusterAction::SetArtifactEnabled {
            cluster_id,
            hash: hash.to_string(),
            enabled,
        })
    } else {
        bundle_name.map(|bundle_name| ClusterAction::SetBundlePackageEnabled {
            cluster_id,
            bundle_name: bundle_name.to_string(),
            package_id: package_id.to_string(),
            enabled,
            manifest_default,
        })
    }
}

pub(crate) fn bundled_delete_action(item: &PackageEntry, cluster_id: i64) -> Option<ClusterAction> {
    if let Some(bundle_name) = item.bundle_name.clone() {
        return Some(ClusterAction::SetBundlePackageRemoved {
            cluster_id,
            bundle_name,
            package_id: item.package_id.clone(),
            hash: item.hash.clone(),
            removed: !item.deleted,
        });
    }
    let hash = item.hash.clone()?;
    Some(if item.deleted {
        ClusterAction::SetArtifactEnabled {
            cluster_id,
            hash,
            enabled: true,
        }
    } else {
        ClusterAction::DeleteBundledArtifact { cluster_id, hash }
    })
}

pub(crate) fn disable_warning_body(
    item: &PackageEntry,
    warnings: Option<oneclient_core::DisableWarnings>,
) -> Option<String> {
    let bundled = item.in_bundle() && item.provider == ProviderId::Modrinth;

    match warnings {
        Some(warnings) if bundled => warnings.body_for(&item.package_id).map(str::to_string),
        _ => item
            .essential
            .map(|package| (package.disable_body)().to_string()),
    }
}

pub(crate) fn run_guarded(
    action: ClusterAction,
    targets: &[&PackageEntry],
    name: String,
    kind: EssentialGuardKind,
    warnings: Option<oneclient_core::DisableWarnings>,
    cluster: UseMutation<ClusterMutation>,
    mut guard: State<Option<PendingEssential>>,
) {
    let Some((body, _)) = guard_body(targets, &name, warnings) else {
        cluster.mutate(action);
        return;
    };
    guard.set(Some(PendingEssential {
        name,
        body,
        kind,
        then: Guarded::Mutate(action),
    }));
}

pub(crate) fn guard_disable(
    mut request: DisableRequest,
    targets: &[&PackageEntry],
    warnings: Option<oneclient_core::DisableWarnings>,
    mut guard: State<Option<PendingEssential>>,
    pending_disable: State<Option<disable::PendingDisable>>,
    dispatch: Actions,
) {
    let acknowledged = crate::launcher::state()
        .map(|state| state.settings.read().acknowledged_disable_warnings.clone())
        .unwrap_or_default();
    let targets: Vec<&PackageEntry> = targets
        .iter()
        .copied()
        .filter(|p| !acknowledged.contains(&p.package_id))
        .collect();
    let Some((body, warned)) = guard_body(&targets, &request.name, warnings) else {
        disable::request(request, pending_disable, dispatch);
        return;
    };
    request.warned = warned;
    guard.set(Some(PendingEssential {
        name: request.name.clone(),
        body,
        kind: EssentialGuardKind::Disable,
        then: Guarded::Disable(request),
    }));
}

fn guard_body(
    targets: &[&PackageEntry],
    name: &str,
    warnings: Option<oneclient_core::DisableWarnings>,
) -> Option<(String, Vec<String>)> {
    let warned: Vec<(&PackageEntry, String)> = targets
        .iter()
        .filter_map(|p| Some((*p, disable_warning_body(p, warnings.clone())?)))
        .collect();
    let body = match warned.as_slice() {
        [] => return None,
        [(p, body)] if p.name == name => body.clone(),
        _ => warned
            .iter()
            .map(|(p, body)| format!("**{}**\n\n{body}", p.name))
            .collect::<Vec<_>>()
            .join("\n\n"),
    };
    let ids = warned.iter().map(|(p, _)| p.package_id.clone()).collect();
    Some((body, ids))
}

#[allow(clippy::too_many_arguments)]
pub fn package_context_menu(
    x: f32,
    y: f32,
    item: &PackageEntry,
    cluster_id: i64,
    package_type: &'static str,
    content_type: ContentType,
    on_delete: EventHandler<(String, String)>,
    on_bundled: Option<EventHandler<()>>,
    on_select: EventHandler<()>,
    on_check_update: Option<EventHandler<()>>,
    hidden_badges: Vec<String>,
) -> ContextMenu {
    let mut menu = ContextMenu::new(x, y).title(item.name.clone()).action(
        IconType::Check,
        t("oneclient.components.package_row.select"),
        on_select,
    );

    if item.is_remote() {
        let provider = item.provider;
        let package_id = item.package_id.clone();
        let package_type = package_type.to_string();
        menu = menu.action(
            IconType::LinkExternal01,
            t("oneclient.components.package_row.view_in_browser"),
            move |()| {
                let _ = RouterContext::get().push(Route::BrowserPackage {
                    cluster_id,
                    package_type: package_type.clone(),
                    package_id: format!("{}:{}", provider as u8, package_id),
                    add_to_cluster: false,
                    in_instance: true,
                });
            },
        );
    }

    if let Some(url) = item.github_url.clone() {
        menu = menu.action(
            IconType::LinkExternal01,
            t("oneclient.components.package_row.view_in_browser"),
            move |()| {
                crate::platform::open_url(&url);
            },
        );
    }

    if item.hash.is_some() {
        let file_name = item.cluster_file_name.clone();
        menu = menu.action(
            IconType::Folder,
            t("oneclient.components.package_row.view_in_folder"),
            EventHandler::new_current(move |()| {
                reveal_in_cluster(cluster_id, content_type, file_name.clone())
            }),
        );
    }

    if let Some(on_check_update) = on_check_update {
        menu = menu.action(
            IconType::RefreshCw01,
            t("oneclient.components.package_row.check_update"),
            on_check_update,
        );
    }

    if let Some(on_bundled) =
        on_bundled.filter(|_| item.in_bundle() && item.opted_in && !item.locked)
    {
        menu = if item.deleted {
            menu.separator().action(
                IconType::RefreshCcw02,
                t("oneclient.components.package_row.restore"),
                on_bundled,
            )
        } else {
            menu.separator().danger_action(
                IconType::Trash01,
                t("oneclient.common.delete"),
                on_bundled,
            )
        };
    }

    if !hidden_badges.is_empty() {
        menu = menu.separator();
        for text in hidden_badges {
            let label = text.clone();
            menu = menu.action(IconType::Copy01, label, move |()| {
                if let Err(err) = Clipboard::set(text.clone()) {
                    tracing::warn!("clipboard copy failed: {err:?}");
                }
            });
        }
    }

    let delete_key = item.hash.clone().or_else(|| {
        item.folder_pack()
            .map(|(name, enabled)| folder_pack_id(name, enabled))
    });
    if item.installed
        && !item.in_bundle()
        && let Some(hash) = delete_key
    {
        let name = item.name.clone();
        menu = menu.separator().danger_action(
            IconType::Trash01,
            t("oneclient.common.delete"),
            move |()| on_delete.call((name.clone(), hash.clone())),
        );
    }

    menu
}

pub(crate) fn on_secondary(
    handler: Option<EventHandler<(f32, f32)>>,
) -> impl FnMut(Event<PressEventData>) {
    move |e: Event<PressEventData>| {
        if let (Some(handler), PressEventData::Mouse(m)) = (handler.as_ref(), e.data()) {
            e.stop_propagation();
            handler.call((m.global_location.x as f32, m.global_location.y as f32));
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
    on_context: Option<EventHandler<(f32, f32)>>,
    on_copies: Option<EventHandler<()>>,
    hovered: State<bool>,
    hidden_badges: State<Vec<String>>,
) -> Element {
    let copies = item.copies.len();
    row_surface(hovered)
        .maybe(item.copy_status == Some(CopyStatus::Skipped), |el| {
            el.opacity(0.5)
        })
        .on_secondary_down(on_secondary(on_context.clone()))
        .child(package_info(
            item,
            package_type,
            cluster_id,
            icon,
            hidden_badges,
        ))
        .maybe_child(on_copies.map(|on_press| copies_button(copies, on_press)))
        .child(meta_size(item.size))
        .child(toggle_controlled(item.enabled, on_toggle).disabled(item.locked))
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
    on_copies: Option<EventHandler<()>>,
    hovered: State<bool>,
) -> Element {
    render_grid_card(
        item,
        package_type,
        cluster_id,
        icon,
        GridToggle::Body(on_toggle),
        navigable,
        on_context,
        on_copies,
        hovered,
    )
}

enum GridToggle {
    Body(EventHandler<()>),
    Switch(EventHandler<()>),
}

#[allow(clippy::too_many_arguments)]
fn render_grid_card(
    item: &PackageEntry,
    package_type: &'static str,
    cluster_id: i64,
    icon: impl IntoElement,
    toggle: GridToggle,
    navigable: bool,
    on_context: Option<EventHandler<(f32, f32)>>,
    on_copies: Option<EventHandler<()>>,
    hovered: State<bool>,
) -> Element {
    let enabled = item.enabled;
    let (body_press, switch) = match toggle {
        GridToggle::Body(on_toggle) => (Some(on_toggle), None),
        GridToggle::Switch(on_toggle) => (
            None,
            Some(PackageGridSwitch {
                enabled,
                locked: item.locked,
                on_toggle,
            }),
        ),
    };

    let alpha = if enabled && item.copy_status != Some(CopyStatus::Skipped) {
        255u8
    } else {
        DISABLED_ALPHA
    };
    let tone = if enabled {
        CardTone::Enabled
    } else {
        CardTone::Disabled
    };
    let title_lines = if switch.is_some() { 2 } else { 1 };
    let meta = grid_meta(item, package_type, cluster_id, navigable, alpha);
    let details = match switch {
        Some(switch) => rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(8.)
            .child(rect().width(Size::flex(1.)).child(meta))
            .child(switch)
            .into_element(),
        None => meta,
    };

    let header = rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(12.)
        .content(Content::Flex)
        .child(rect().opacity(alpha as f32 / 255.).child(icon))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(2.)
                .child(HoverTitle {
                    title: item.name.clone(),
                    color: if enabled {
                        colors::fg_strong()
                    } else {
                        colors::fg_primary().with_a(alpha)
                    },
                    lines: title_lines,
                })
                .child(details),
        )
        .maybe_child(on_context.clone().map(kebab_button))
        .into_element();

    let description = (!item.description.trim().is_empty()).then(|| GridDescription {
        text: item.description.clone(),
        color: colors::fg_primary().with_a(scale_a(alpha, 0.72)),
    });

    let badged = item.is_outdated()
        || item.copy_status.is_some()
        || item.bisect_role.is_some()
        || item.modpack.is_some()
        || item.recency_badge().is_some()
        || on_copies.is_some();
    let copies = item.copies.len();
    let floating = badged.then(|| {
        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(BADGE_STRIP_H))
            .position(
                Position::new_absolute()
                    .bottom(-(GRID_CARD_PADDING + BADGE_STRIP_H / 2.) + BADGE_STRIP_LIFT),
            )
            .layer(Layer::Relative(BADGE_STRIP_LAYER))
            .main_align(Alignment::End)
            .cross_align(Alignment::Center)
            .spacing(4.)
            .maybe_child(on_copies.map(|on_press| copies_button(copies, on_press)))
            .maybe_child(item.modpack.as_deref().map(modpack_badge))
            .maybe_child(item.is_outdated().then(outdated_badge))
            .maybe_child(item.copy_badge())
            .maybe_child(item.bisect_badge())
            .maybe_child(item.recency_badge())
            .into_element()
    });

    card_surface(hovered, tone)
        .on_secondary_down(on_secondary(on_context))
        .maybe(body_press.is_some(), move |surface| {
            surface.cursor(CursorIcon::Pointer).on_press(move |_| {
                if let Some(handler) = &body_press {
                    handler.call(());
                }
            })
        })
        .child(header)
        .maybe_child(description)
        .maybe_child(floating)
        .into_element()
}

#[derive(PartialEq)]
struct GridDescription {
    text: String,
    color: Color,
}

impl Component for GridDescription {
    fn render(&self) -> impl IntoElement {
        let mut lines = use_state(|| 2);
        let two_lines_height = text_size(12.) * DESCRIPTION_LINE_HEIGHT * 2.;
        rect()
            .width(Size::fill())
            .height(Size::flex(1.0))
            .overflow(Overflow::Clip)
            .on_sized(move |event: Event<SizedEventData>| {
                let fits_two = event.data().area.height() >= two_lines_height;
                lines.set_if_modified(if fits_two { 2 } else { 1 });
            })
            .child(card_description_label(
                self.text.clone(),
                self.color,
                lines(),
            ))
    }
}

#[derive(PartialEq)]
struct PackageGridSwitch {
    enabled: bool,
    locked: bool,
    on_toggle: EventHandler<()>,
}

impl Component for PackageGridSwitch {
    fn render(&self) -> impl IntoElement {
        let tooltip = use_tooltip_anchor(
            self.locked
                .then(|| t("oneclient.components.package_row.locked").into()),
            TooltipPlacement::Top,
        );
        tooltip.attach(
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(4.)
                .maybe_child(self.locked.then(|| {
                    Icon::new(IconType::Lock01)
                        .size(14.)
                        .color(colors::fg_secondary())
                }))
                .child(
                    toggle_controlled(self.enabled, self.on_toggle.clone()).disabled(self.locked),
                ),
        )
    }
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
    let muted = colors::fg_primary().with_a(scale_a(alpha, 0.5));

    let source = if let Some(url) = item.github_url.clone() {
        SourceLink {
            text: "GitHub".to_string(),
            on_press: EventHandler::new(move |()| crate::platform::open_url(&url)),
            alpha,
        }
        .into_element()
    } else if item.github_hosted {
        meta_text("GitHub".to_string(), muted)
    } else if item.is_remote() && navigable {
        let provider = item.provider;
        let package_id = item.package_id.clone();
        SourceLink {
            text: provider.to_string(),
            on_press: EventHandler::new(move |()| {
                let _ = RouterContext::get().push(Route::BrowserPackage {
                    cluster_id,
                    package_type: package_type.to_string(),
                    package_id: format!("{}:{}", provider as u8, package_id),
                    add_to_cluster: false,
                    in_instance: true,
                });
            }),
            alpha,
        }
        .into_element()
    } else if item.is_remote() {
        meta_text(item.provider.to_string(), muted)
    } else {
        meta_text(
            t("oneclient.components.package_row.local_file").to_string(),
            muted,
        )
    };

    let mut parts: Vec<Element> = Vec::new();
    if !item.author.is_empty() {
        parts.push(meta_text(
            tf(
                "oneclient.components.package_row.by_author",
                &[&item.author],
            ),
            muted,
        ));
    }
    if let Some(added) = item.added_label() {
        parts.push(meta_text(added, muted));
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
    text: String,
    on_press: EventHandler<()>,
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
            colors::fg_primary().with_a(scale_a(self.alpha, 0.68))
        };

        let on_press = self.on_press.clone();

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
                on_press.call(());
            })
            .child(meta_text(self.text.clone(), color))
    }
}

pub(crate) fn meta_text(text: String, color: Color) -> Element {
    label()
        .text(text)
        .font_size(text_size(12.))
        .max_lines(1)
        .color(color)
        .into_element()
}

pub(crate) fn kebab_button(on_context: EventHandler<(f32, f32)>) -> Element {
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
            .corner_radius(rounded(6.))
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

fn reveal_in_cluster(cluster_id: i64, content_type: ContentType, file_name: String) {
    spawn(async move {
        let Ok(state) = crate::launcher::state() else {
            return;
        };
        use oneclient_content::packages::{PackageStore, store::materialized_root};
        let cluster = match PackageStore::get_cluster(cluster_id, &state.services.content()).await {
            Ok(cluster) => cluster,
            Err(err) => {
                tracing::warn!(%err, "failed to look up cluster for reveal");
                return;
            }
        };
        let Some((root, _)) = materialized_root(&cluster, content_type).await else {
            return;
        };
        let dir = root.join(content_type.folder_name());
        let file = [file_name.clone(), format!("{file_name}.disabled")]
            .into_iter()
            .map(|name| dir.join(name))
            .find(|path| path.exists());
        match file {
            Some(file) => crate::platform::reveal_file(file),
            None => crate::platform::open_path(&dir.to_string_lossy()),
        }
    });
}

const TITLE_BADGE_GAP: f32 = 8.;

fn title_badge_budget(
    info_width: f32,
    desired_title_width: f32,
    badges: &[f32],
) -> (f32, Vec<bool>) {
    let mut title_width = info_width;
    let visible = badges
        .iter()
        .map(|&width| {
            let cost = width + TITLE_BADGE_GAP;
            let fits = width > 0. && title_width >= desired_title_width + cost;
            if fits {
                title_width -= cost;
            }
            fits
        })
        .collect();
    (title_width, visible)
}

struct TitleBadge {
    text: String,
    element: Element,
}

fn title_badges(item: &PackageEntry) -> Vec<TitleBadge> {
    let (source, element) = if item.github_url.is_some() || item.github_hosted {
        ("GitHub".to_string(), github_badge())
    } else if item.is_remote() {
        (item.provider.to_string(), provider_badge(item.provider))
    } else {
        (
            t("oneclient.components.package_row.local_file").into(),
            local_badge(),
        )
    };
    let mut badges = vec![TitleBadge {
        text: source,
        element,
    }];
    let mut add = |text: String, element| badges.push(TitleBadge { text, element });
    if let Some(name) = &item.modpack {
        add(name.clone(), modpack_badge(name));
    }
    if item.is_outdated() {
        add(
            t("oneclient.components.package_row.update_available").into(),
            outdated_badge(),
        );
    }
    if let Some(status) = item.copy_status
        && let Some(badge) = item.copy_badge()
    {
        let text = match status {
            CopyStatus::Loaded => t("oneclient.components.package_row.loaded"),
            CopyStatus::Skipped => t("oneclient.components.package_row.skipped"),
        };
        add(text.into(), badge);
    }
    if let Some(role) = item.bisect_role
        && let Some(badge) = item.bisect_badge()
    {
        add(role.label().into(), badge);
    }
    if let Some(badge) = item.recency_badge() {
        let text = if item.seen_status == SeenStatus::New {
            t("oneclient.components.package_row.new")
        } else {
            t("oneclient.components.package_row.updated")
        };
        add(text.into(), badge);
    }
    badges
}

pub(crate) fn row_title(title: String, color: Color) -> Element {
    HoverTitle {
        title,
        color,
        lines: 1,
    }
    .into_element()
}

#[derive(PartialEq)]
struct HoverTitle {
    title: String,
    color: Color,
    lines: usize,
}

impl Component for HoverTitle {
    fn render(&self) -> impl IntoElement {
        use_tooltip_anchor(Some(self.title.clone().into()), TooltipPlacement::Top).attach(
            rect().width(Size::fill()).child(
                row_title_label(self.title.clone(), self.color)
                    .width(Size::fill())
                    .max_lines(self.lines),
            ),
        )
    }
}

#[derive(PartialEq)]
struct PackageTitle {
    item: PackageEntry,
    hidden_badges: State<Vec<String>>,
}

impl Component for PackageTitle {
    fn render(&self) -> impl IntoElement {
        let mut info_width = use_state(|| 0f32);
        let mut title_width = use_state(|| 0f32);
        let mut measured_badges = use_state(BTreeMap::<(usize, String), f32>::new);
        let item = &self.item;
        let badges = title_badges(item);
        let widths: Vec<_> = {
            let widths = measured_badges.read();
            badges
                .iter()
                .enumerate()
                .map(|(index, badge)| {
                    widths
                        .get(&(index, badge.text.clone()))
                        .copied()
                        .unwrap_or(0.)
                })
                .collect()
        };
        let (title_budget, visible) = title_badge_budget(info_width(), title_width(), &widths);
        let hidden = badges
            .iter()
            .zip(&visible)
            .filter(|(_, visible)| !**visible)
            .map(|(badge, _)| badge.text.clone())
            .collect::<Vec<_>>();
        let mut hidden_badges = self.hidden_badges;
        use_side_effect_with_deps(&hidden, move |hidden| {
            hidden_badges.set_if_modified(hidden.clone())
        });

        // These probes only need natural widths. Global positioning keeps them out of
        // Freya's scroll translation, which otherwise sends every probe a Sized event per tick.
        let measurement = rect()
            .position(Position::new_global().left(0.).top(0.))
            .width(Size::window_percent(100.))
            .height(Size::px(0.))
            .interactive(false)
            .overflow(Overflow::Clip)
            .opacity(0.)
            .child(
                rect()
                    .on_sized(move |e: Event<SizedEventData>| {
                        title_width.set_if_modified(e.area.width());
                    })
                    .child(row_title_label(item.name.clone(), colors::fg_primary())),
            )
            .children(badges.iter().enumerate().map(|(index, badge)| {
                let key = (index, badge.text.clone());
                rect()
                    .key(format!("{index}:{}", badge.text))
                    .on_sized(move |e: Event<SizedEventData>| {
                        let width = e.area.width();
                        if measured_badges.peek().get(&key) != Some(&width) {
                            measured_badges.write().insert(key.clone(), width);
                        }
                    })
                    .child(badge.element.clone())
                    .into_element()
            }));

        rect()
            .horizontal()
            .width(Size::fill())
            .cross_align(Alignment::Center)
            .spacing(TITLE_BADGE_GAP)
            .on_sized(move |e: Event<SizedEventData>| {
                info_width.set_if_modified(e.area.width());
            })
            .child(measurement)
            .child(
                rect()
                    .max_width(Size::px(title_budget))
                    .child(row_title_label(item.name.clone(), colors::fg_primary())),
            )
            .children(badges.into_iter().zip(visible).enumerate().filter_map(
                |(index, (badge, visible))| {
                    visible.then(|| {
                        if let Some(url) = item.github_url.clone().filter(|_| index == 0) {
                            rect()
                                .cursor(CursorIcon::Pointer)
                                .on_press(move |e: Event<PressEventData>| {
                                    e.stop_propagation();
                                    crate::platform::open_url(&url);
                                })
                                .child(badge.element)
                                .into_element()
                        } else {
                            badge.element
                        }
                    })
                },
            ))
    }
}

fn package_info(
    item: &PackageEntry,
    package_type: &'static str,
    cluster_id: i64,
    icon: impl IntoElement,
    hidden_badges: State<Vec<String>>,
) -> impl IntoElement {
    let remote = item.is_remote();
    let provider = item.provider;
    let package_id = item.package_id.clone();
    let package_type = package_type.to_string();
    let author = (!item.author.is_empty()).then(|| {
        tf(
            "oneclient.components.package_row.by_author",
            &[&item.author],
        )
    });
    // Meta and description share one line so every list row keeps the same height
    let added = item.added_label();
    let mut line = joined_line(
        author
            .iter()
            .chain(added.iter())
            .map(String::as_str)
            .chain([item.description.as_str()]),
    );
    if line.is_empty() && item.file_name != item.name {
        line = item.file_name.clone();
    }

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
                    add_to_cluster: false,
                    in_instance: true,
                });
            })
        })
        .child(icon)
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(4.)
                .child(PackageTitle {
                    item: item.clone(),
                    hidden_badges,
                })
                .maybe(!line.is_empty(), |el| {
                    el.child(row_line(line, colors::fg_secondary()))
                }),
        )
        .into_element()
}

pub(crate) fn package_icon(
    item: &PackageEntry,
    icon_query: &freya::query::UseQuery<crate::hooks::CachedImageQuery>,
    size: f32,
) -> Element {
    remote_icon(item.icon_url.as_deref(), icon_query, size)
}

pub(crate) fn remote_icon(
    icon_url: Option<&str>,
    icon_query: &freya::query::UseQuery<crate::hooks::CachedImageQuery>,
    size: f32,
) -> Element {
    let loaded = loaded_image(icon_url, icon_query);

    match loaded {
        Some((url, bytes)) => ImageViewer::new((url, bytes))
            .width(Size::px(size))
            .height(Size::px(size))
            .aspect_ratio(AspectRatio::Min)
            .corner_radius(rounded(8.))
            .fallback(icon_box(IconType::DotsGrid, size))
            .into_element(),

        None if icon_url.is_some() => icon_box(IconType::DotsGrid, size),
        None => icon_box(IconType::HelpCircle, size),
    }
}

pub(crate) fn icon_box(icon: IconType, size: f32) -> Element {
    rect()
        .center()
        .width(Size::px(size))
        .height(Size::px(size))
        .corner_radius(rounded(8.))
        .background(colors::component_bg_hover())
        .border(border_all_color(1., colors::component_border()).alignment(BorderAlignment::Inner))
        .child(
            Icon::new(icon)
                .size(size * 0.4)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

pub(crate) fn meta_size(size: u64) -> impl IntoElement {
    rect()
        .maybe_child((size > 0).then(|| {
            label()
                .text(format_size(size))
                .font_size(text_size(12.))
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

pub fn github_badge() -> Element {
    badge(
        Icon::new(IconType::Github).size(12.).into_element(),
        "GitHub".to_string(),
    )
}

fn outdated_badge() -> Element {
    status_tag(
        t("oneclient.components.package_row.update_available"),
        colors::brand(),
    )
}

fn copies_button(ignored: usize, on_press: EventHandler<()>) -> Element {
    rect()
        .cursor(CursorIcon::Pointer)
        .on_press(move |e: Event<PressEventData>| {
            e.stop_propagation();
            on_press.call(());
        })
        .child(accent_badge(
            Icon::new(IconType::Copy01)
                .size(12.)
                .color(colors::brand())
                .into_element(),
            tf("oneclient.components.package_row.copies", &[&(ignored + 1)]),
            colors::brand(),
        ))
        .into_element()
}

fn new_badge() -> Element {
    status_tag(t("oneclient.components.package_row.new"), colors::success())
}

fn updated_badge() -> Element {
    status_tag(
        t("oneclient.components.package_row.updated"),
        colors::success(),
    )
}

fn status_tag(text: impl Into<String>, accent: Color) -> Element {
    rect()
        .corner_radius(rounded(4.))
        .background(colors::component_bg())
        .child(
            rect()
                .padding(Gaps::new(1., 6., 2., 6.))
                .corner_radius(rounded(4.))
                .background(accent.with_a(38))
                .border(border_all_color(1., accent.with_a(96)))
                .child(
                    label()
                        .text(text.into())
                        .font_size(text_size(10.))
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(accent),
                ),
        )
        .into_element()
}

const MODPACK_TAG_CHARS: usize = 10;
const MODPACK_TAG_ELLIPSIS: &str = "...";

fn modpack_badge(name: &str) -> Element {
    status_tag(
        crate::utils::shorten(name, MODPACK_TAG_CHARS, MODPACK_TAG_ELLIPSIS),
        colors::code_info(),
    )
}

fn local_badge() -> Element {
    badge(
        Icon::new(IconType::File02)
            .size(12.)
            .color(colors::fg_secondary())
            .into_element(),
        t("oneclient.components.package_row.local_file").to_string(),
    )
}

pub(crate) fn badge(icon: impl IntoElement, text: String) -> Element {
    accent_badge(icon, text, colors::fg_secondary())
}

fn accent_badge(icon: impl IntoElement, text: String, accent: Color) -> Element {
    pill(Some(icon.into_element()), text, accent)
}

pub(crate) fn pill(icon: Option<Element>, text: String, accent: Color) -> Element {
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(4.)
        .padding(Gaps::new_symmetric(2., 8.))
        .corner_radius(rounded(999.))
        .border(border_all_color(1., colors::component_border()))
        .background(colors::component_bg())
        .maybe_child(icon)
        .child(
            label()
                .text(text)
                .font_size(text_size(10.))
                .font_weight(FontWeight::MEDIUM)
                .color(accent),
        )
        .into_element()
}

#[cfg(test)]
mod title_tests {
    use super::*;

    #[test]
    fn title_budget_reserves_only_visible_badge_widths_and_gaps() {
        assert_eq!(
            title_badge_budget(252., 190., &[54., 60.]),
            (190., vec![true, false])
        );
        assert_eq!(
            title_badge_budget(251., 190., &[54., 30.]),
            (213., vec![false, true])
        );
        assert_eq!(
            title_badge_budget(180., 190., &[54., 30.]),
            (180., vec![false, false])
        );
    }
}
