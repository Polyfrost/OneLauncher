use std::collections::BTreeMap;
use std::path::PathBuf;

use super::*;

use freya::router::RouterContext;
use freya::text_edit::Clipboard;
use oneclient_content::packages::ContentType;
use oneclient_core::settings::ViewLayout;

use crate::components::{
    Button, CardLayout, ChevronToggle, ContextMenu, CopyStatus, Dropdown, FilterMenu, FilterOption,
    Icon, IconType, LazySection, OVERLAY_BASE_LEVEL, OverlayPopup, PackageEntry, PackageRow,
    ROW_GAP, ROW_RADIUS, bundled_delete_action, package_context_menu, parse_folder_pack_id,
    run_guarded, use_shared_delete,
};
use crate::hooks::{ClusterAction, Selection, use_cluster_mutation, use_dispatch};
use crate::i18n::{t, tf};
use crate::routes::Route;
use crate::theme::{colors, rounded, text_size};
use crate::ui::border_all_color;
use crate::{Actions, utils};

use super::super::folder_list::{
    CONTROL_H, content_area, content_empty, content_grid, content_loading, content_scroll,
    content_toolbar, layout_toggle, open_folder_action, open_folder_button, scope_chip,
    toolbar_action, toolbar_secondary,
};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum SortMode {
    NameAsc,
    NameDesc,
    SizeAsc,
    SizeDesc,
    AddedDesc,
    UpdatedDesc,
}

impl SortMode {
    const ALL: [SortMode; 6] = [
        SortMode::NameAsc,
        SortMode::NameDesc,
        SortMode::SizeAsc,
        SortMode::SizeDesc,
        SortMode::AddedDesc,
        SortMode::UpdatedDesc,
    ];

    fn key(self) -> &'static str {
        match self {
            SortMode::NameAsc => "name_asc",
            SortMode::NameDesc => "name_desc",
            SortMode::SizeAsc => "size_asc",
            SortMode::SizeDesc => "size_desc",
            SortMode::AddedDesc => "added_desc",
            SortMode::UpdatedDesc => "updated_desc",
        }
    }

    pub(super) fn from_key(key: &str) -> Option<Self> {
        SortMode::ALL.into_iter().find(|s| s.key() == key)
    }

    fn label(self) -> &'static str {
        match self {
            SortMode::NameAsc => t("oneclient.cluster.package_manager.sort.name_asc"),
            SortMode::NameDesc => t("oneclient.cluster.package_manager.sort.name_desc"),
            SortMode::SizeAsc => t("oneclient.cluster.package_manager.sort.size_asc"),
            SortMode::SizeDesc => t("oneclient.cluster.package_manager.sort.size_desc"),
            SortMode::AddedDesc => t("oneclient.cluster.package_manager.sort.added_desc"),
            SortMode::UpdatedDesc => t("oneclient.cluster.package_manager.sort.updated_desc"),
        }
    }

    pub(super) fn sort(self, rows: &mut [PackageEntry]) {
        let title = |p: &PackageEntry| p.name.to_lowercase();

        match self {
            SortMode::NameDesc => rows.sort_by_key(|p| std::cmp::Reverse(title(p))),
            SortMode::SizeAsc => rows.sort_by_key(|p| p.size),
            SortMode::SizeDesc => rows.sort_by_key(|p| std::cmp::Reverse(p.size)),
            // Undated rows (not installed, or added before dates were kept) sink to the bottom
            SortMode::AddedDesc => {
                rows.sort_by_key(|p| (std::cmp::Reverse(p.added_at.clone()), title(p)))
            }
            SortMode::UpdatedDesc => {
                rows.sort_by_key(|p| (std::cmp::Reverse(p.published_at.clone()), title(p)))
            }
            _ => rows.sort_by_key(title),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum EnabledFilter {
    All,
    Enabled,
    Disabled,
}

impl EnabledFilter {
    pub(super) const STORAGE_KEY: &'static str = "show";

    const ALL: [EnabledFilter; 3] = [
        EnabledFilter::All,
        EnabledFilter::Enabled,
        EnabledFilter::Disabled,
    ];

    fn key(self) -> &'static str {
        match self {
            EnabledFilter::All => "all",
            EnabledFilter::Enabled => "enabled",
            EnabledFilter::Disabled => "disabled",
        }
    }

    pub(super) fn from_key(key: &str) -> Option<Self> {
        EnabledFilter::ALL.into_iter().find(|f| f.key() == key)
    }

    fn label(self) -> &'static str {
        match self {
            EnabledFilter::All => t("oneclient.cluster.package_manager.filter.show.all"),
            EnabledFilter::Enabled => t("oneclient.cluster.package_manager.filter.show.enabled"),
            EnabledFilter::Disabled => t("oneclient.cluster.package_manager.filter.show.disabled"),
        }
    }

    pub(super) fn keep(self, p: &PackageEntry) -> bool {
        match self {
            EnabledFilter::All => true,
            EnabledFilter::Enabled => p.enabled,
            EnabledFilter::Disabled => !p.enabled,
        }
    }
}

/// Hidden files are dependencies the bundle manages so the list leaves them out until asked
#[derive(Clone, Copy, PartialEq)]
pub(super) enum HiddenFilter {
    Hide,
    Show,
}

impl HiddenFilter {
    pub(super) const STORAGE_KEY: &'static str = "hidden";

    const ALL: [HiddenFilter; 2] = [HiddenFilter::Hide, HiddenFilter::Show];

    pub(super) fn key(self) -> &'static str {
        match self {
            HiddenFilter::Hide => "hide",
            HiddenFilter::Show => "show",
        }
    }

    pub(super) fn from_key(key: &str) -> Option<Self> {
        HiddenFilter::ALL.into_iter().find(|f| f.key() == key)
    }

    fn label(self) -> &'static str {
        match self {
            HiddenFilter::Hide => t("oneclient.cluster.package_manager.filter.hidden.hide"),
            HiddenFilter::Show => t("oneclient.cluster.package_manager.filter.hidden.show"),
        }
    }

    pub(super) fn keep(self, p: &PackageEntry) -> bool {
        match self {
            HiddenFilter::Hide => !p.hidden,
            HiddenFilter::Show => true,
        }
    }
}

#[derive(Clone, PartialEq)]
pub(super) struct Bulk {
    pub(super) selection: Selection<String>,
    pub(super) order: Vec<String>,
    pub(super) count: usize,
    pub(super) deletable: usize,
    pub(super) checkable: usize,
    pub(super) set_enabled: EventHandler<bool>,
    pub(super) delete: EventHandler<()>,
    pub(super) check_updates: EventHandler<()>,
}

impl Bulk {
    pub(super) fn active(&self) -> bool {
        self.selection.is_active()
    }

    fn all_selected(&self) -> bool {
        self.count > 0 && self.count == self.order.len()
    }

    fn toggle_all_label(&self) -> (IconType, &'static str) {
        if self.all_selected() {
            (
                IconType::XClose,
                t("oneclient.cluster.package_manager.select.unselect_all"),
            )
        } else {
            (
                IconType::Check,
                t("oneclient.cluster.package_manager.select.select_all"),
            )
        }
    }

    fn controls(&self) -> Vec<Element> {
        let selection = self.selection;
        let order = self.order.clone();
        let enable = self.set_enabled.clone();
        let disable = self.set_enabled.clone();
        let delete = self.delete.clone();
        let (toggle_icon, toggle_label) = self.toggle_all_label();
        let button = |icon: IconType, text: String| {
            Button::new()
                .secondary()
                .height(Size::px(34.))
                .font_size(text_size(12.))
                .child(Icon::new(icon).size(15.))
                .text(text)
        };

        vec![
            label()
                .text(tf(
                    "oneclient.cluster.package_manager.select.count",
                    &[&self.count],
                ))
                .font_size(text_size(12.))
                .max_lines(1)
                .color(colors::fg_secondary())
                .into_element(),
            button(toggle_icon, toggle_label.to_string())
                .on_press(move |_| selection.toggle_all(&order))
                .into_element(),
            button(
                IconType::CheckCircle,
                t("oneclient.cluster.package_manager.select.enable").to_string(),
            )
            .enabled(self.count > 0)
            .on_press(move |_| enable.call(true))
            .into_element(),
            button(
                IconType::Minus,
                t("oneclient.cluster.package_manager.select.disable").to_string(),
            )
            .enabled(self.count > 0)
            .on_press(move |_| disable.call(false))
            .into_element(),
            button(
                IconType::Trash01,
                tf(
                    "oneclient.cluster.package_manager.select.delete",
                    &[&self.deletable],
                ),
            )
            .danger()
            .enabled(self.deletable > 0)
            .on_press(move |_| delete.call(()))
            .into_element(),
            Button::new()
                .ghost()
                .icon()
                .height(Size::px(34.))
                .tooltip(t("oneclient.cluster.package_manager.select.exit"))
                .on_press(move |_| selection.exit())
                .child(Icon::new(IconType::XClose).size(15.))
                .into_element(),
        ]
    }

    fn menu(&self, x: f32, y: f32, key: String) -> ContextMenu {
        let selection = self.selection;
        let order = self.order.clone();
        let enable = self.set_enabled.clone();
        let disable = self.set_enabled.clone();
        let delete = self.delete.clone();
        let check_updates = self.check_updates.clone();
        let (toggle_icon, toggle_label) = self.toggle_all_label();

        let (icon, text) = if selection.is_selected(&key) {
            (
                IconType::XClose,
                t("oneclient.cluster.package_manager.select.unselect"),
            )
        } else {
            (
                IconType::Check,
                t("oneclient.cluster.package_manager.select.select"),
            )
        };

        let mut menu = ContextMenu::new(x, y)
            .title(tf(
                "oneclient.cluster.package_manager.select.count",
                &[&self.count],
            ))
            .action(icon, text, move |()| selection.toggle(key.clone()))
            .separator();
        if self.count > 0 {
            menu = menu
                .action(
                    IconType::CheckCircle,
                    t("oneclient.cluster.package_manager.select.enable_selected"),
                    move |()| enable.call(true),
                )
                .action(
                    IconType::Minus,
                    t("oneclient.cluster.package_manager.select.disable_selected"),
                    move |()| disable.call(false),
                );
            if self.checkable > 0 {
                menu = menu.action(
                    IconType::RefreshCw01,
                    tf(
                        "oneclient.cluster.package_manager.select.check_updates_selected",
                        &[&self.checkable],
                    ),
                    move |()| check_updates.call(()),
                );
            }
            menu = menu.separator();
        }
        menu = menu.action(toggle_icon, toggle_label, move |()| {
            selection.toggle_all(&order)
        });
        if self.count > 0 && !self.all_selected() {
            menu = menu.action(
                IconType::XClose,
                t("oneclient.cluster.package_manager.select.clear"),
                move |()| selection.clear(),
            );
        }

        if self.deletable == 0 {
            return menu;
        }
        menu.separator().danger_action(
            IconType::Trash01,
            tf(
                "oneclient.cluster.package_manager.select.delete_count",
                &[&self.deletable],
            ),
            move |()| delete.call(()),
        )
    }
}

const CATEGORY_W: f32 = 180.;

fn folder_tip(content_type: ContentType, shared: bool) -> String {
    let scope = t(match content_type {
        ContentType::Shader => "oneclient.cluster.folder.tip.shaders",
        ContentType::ResourcePack => "oneclient.cluster.folder.tip.textures",
        _ => "oneclient.cluster.folder.tip.mods",
    });
    if shared {
        format!("{scope} {}", t("oneclient.cluster.folder.tip.shared"))
    } else {
        scope.to_string()
    }
}

/// Whether the page's content is shared with other instances, with the sentence that explains it
pub(super) struct Scope {
    pub(super) shared: bool,
    pub(super) tip: String,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn toolbar_bar(
    tabs: &[Tab],
    active_idx: usize,
    active: State<usize>,
    search: State<String>,
    sort: State<Option<String>>,
    current_sort: SortMode,
    filters: State<BTreeMap<String, String>>,
    show: EnabledFilter,
    hidden: HiddenFilter,
    layout: State<ViewLayout>,
    cluster_id: i64,
    package_type: &'static str,
    content_type: ContentType,
    dispatch: Actions,
    bulk: &Bulk,
    noun_plural: &'static str,
    export: Vec<PackageEntry>,
    header: Option<String>,
    folder: Option<PathBuf>,
    scope: Option<Scope>,
) -> Element {
    let local_tab = matches!(tabs.get(active_idx), Some(Tab::Local));

    let labels: Vec<String> = tabs.iter().map(Tab::label).collect();
    let current = labels.get(active_idx).cloned().unwrap_or_default();

    // One compact dropdown replaces the row of category chips; it names the active value
    let category = {
        let mut active = active;
        Dropdown::new(
            tf("oneclient.cluster.package_manager.category", &[&current]),
            labels,
        )
        .outlined()
        .solid_menu()
        .width(Size::px(CATEGORY_W))
        .height(Size::px(CONTROL_H))
        .tooltip(t("oneclient.cluster.package_manager.category.tooltip"))
        .on_select(move |idx: usize| active.set(idx))
        .into_element()
    };
    let mut view_controls = vec![category];
    if !bulk.active() {
        view_controls.push(filter_button(sort, current_sort, filters, show, hidden).into_element());
        view_controls.push(layout_toggle(layout));
    }

    let actions = if bulk.active() {
        bulk.controls()
    } else {
        let shared = scope.as_ref().is_some_and(|scope| scope.shared);
        let mut controls = Vec::new();
        if let Some(scope) = scope {
            controls.push(scope_chip(scope.shared, scope.tip));
        }
        controls.push(copy_list_button(
            dispatch.clone(),
            export.len(),
            noun_one(content_type),
            noun_plural,
            move || package_list_text(&export, content_type, header.as_deref()),
        ));
        controls.push(open_folder_action(folder, folder_tip(content_type, shared)));
        controls.push(add_button(
            local_tab,
            cluster_id,
            package_type,
            content_type,
            dispatch,
        ));
        controls
    };

    content_toolbar(Some(search), view_controls, actions)
}

/// The page's primary action: browse providers, or pick files on the Local tab
fn add_button(
    local_tab: bool,
    cluster_id: i64,
    package_type: &'static str,
    content_type: ContentType,
    dispatch: Actions,
) -> Element {
    if local_tab {
        import_button(cluster_id, content_type, dispatch, true).into_element()
    } else {
        toolbar_action(
            IconType::Plus,
            t("oneclient.cluster.package_manager.add_content"),
        )
        .on_press(move |_| open_browser(cluster_id, package_type))
        .into_element()
    }
}

fn import_button(
    cluster_id: i64,
    content_type: ContentType,
    dispatch: Actions,
    primary: bool,
) -> Button {
    let icon = IconType::FilePlus02;
    let text = t("oneclient.cluster.package_manager.add_from_file");
    let button = if primary {
        toolbar_action(icon, text)
    } else {
        toolbar_secondary(icon, text)
    };
    button.on_press(move |_| import_from_file(cluster_id, content_type, dispatch.clone()))
}

pub(crate) fn copy_list_button(
    dispatch: Actions,
    count: usize,
    noun: &'static str,
    noun_plural: &'static str,
    text: impl Fn() -> String + 'static,
) -> Element {
    let noun = if count == 1 { noun } else { noun_plural };
    Button::new()
        .secondary()
        .icon()
        .width(Size::px(CONTROL_H))
        .height(Size::px(CONTROL_H))
        .enabled(count > 0)
        .on_press(move |_| {
            if let Err(err) = Clipboard::set(text()) {
                tracing::warn!("clipboard copy failed: {err:?}");
                dispatch
                    .notify(t("oneclient.cluster.package_manager.copy.failed.title"))
                    .body(tf(
                        "oneclient.cluster.package_manager.copy.failed.body",
                        &[&noun_plural],
                    ))
                    .error()
                    .send();
            } else {
                dispatch
                    .notify(t("oneclient.cluster.package_manager.copy.done.title"))
                    .body(tf(
                        "oneclient.cluster.package_manager.copy.done.body",
                        &[&count, &noun],
                    ))
                    .info()
                    .icon(IconType::ClipboardCheck)
                    .send();
            }
        })
        .child(
            Icon::new(IconType::Copy01)
                .size(16.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

pub(super) fn running_notice(noun_plural: &'static str, content_type: ContentType) -> String {
    match content_type {
        ContentType::ResourcePack => tf(
            "oneclient.cluster.package_manager.notice.running.resource_pack",
            &[&noun_plural],
        ),
        ContentType::Shader => tf(
            "oneclient.cluster.package_manager.notice.running.shader",
            &[&noun_plural],
        ),
        _ => tf(
            "oneclient.cluster.package_manager.notice.running",
            &[&noun_plural],
        ),
    }
}

pub(super) fn bisect_notice(noun_plural: &'static str) -> String {
    tf(
        "oneclient.cluster.package_manager.notice.bisect",
        &[&noun_plural],
    )
}

pub(super) fn global_notice(noun_plural: &'static str) -> String {
    tf(
        "oneclient.cluster.package_manager.notice.global",
        &[&noun_plural],
    )
}

pub(in crate::view::app::cluster) fn instance_only_notice(noun_plural: &str) -> String {
    tf(
        "oneclient.cluster.package_manager.notice.instance_only",
        &[&noun_plural],
    )
}

pub(super) fn essential_notice(names: &[&'static str]) -> String {
    match names {
        [only] => tf(
            "oneclient.cluster.package_manager.notice.essential.one",
            &[only],
        ),
        _ => tf(
            "oneclient.cluster.package_manager.notice.essential.other",
            &[&names.join(", ")],
        ),
    }
}

pub(crate) fn notice_bar(text: String) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .content(Content::Flex)
        .spacing(10.)
        .padding(Gaps::new_symmetric(9., 12.))
        .corner_radius(rounded(ROW_RADIUS))
        .background(colors::brand().with_a(30))
        .border(crate::ui::border_all_color(1., colors::brand().with_a(90)))
        .child(
            Icon::new(IconType::InfoCircle)
                .size(16.)
                .color(colors::brand()),
        )
        .child(
            label()
                .text(text)
                .font_size(text_size(12.))
                .width(Size::flex(1.0))
                .color(colors::fg_primary()),
        )
        .into_element()
}

fn filter_button(
    mut sort: State<Option<String>>,
    current_sort: SortMode,
    mut filters: State<BTreeMap<String, String>>,
    show: EnabledFilter,
    hidden: HiddenFilter,
) -> FilterMenu {
    // Hiding hidden packages is the default so it does not count as the filters being touched
    let active = current_sort != SortMode::NameAsc
        || show != EnabledFilter::All
        || hidden != HiddenFilter::Hide;

    FilterMenu::new(active)
        .section(
            t("oneclient.cluster.package_manager.filter.sort.title"),
            SortMode::ALL.map(|mode| {
                FilterOption::new(mode.label(), mode == current_sort, move |()| {
                    sort.set(Some(mode.key().to_string()));
                })
            }),
        )
        .section(
            t("oneclient.cluster.package_manager.filter.show.title"),
            EnabledFilter::ALL.map(|filter| {
                FilterOption::new(filter.label(), filter == show, move |()| {
                    filters
                        .write()
                        .insert(EnabledFilter::STORAGE_KEY.into(), filter.key().into());
                })
            }),
        )
        .section(
            t("oneclient.cluster.package_manager.filter.hidden.title"),
            HiddenFilter::ALL.map(|filter| {
                FilterOption::new(filter.label(), filter == hidden, move |()| {
                    filters
                        .write()
                        .insert(HiddenFilter::STORAGE_KEY.into(), filter.key().into());
                })
            }),
        )
}

fn import_from_file(cluster_id: i64, content_type: ContentType, dispatch: Actions) {
    spawn(async move {
        if let Some(handles) = rfd::AsyncFileDialog::new()
            .set_title(t("oneclient.cluster.package_manager.import.picker_title"))
            .pick_files()
            .await
        {
            dispatch.import_local_files(
                cluster_id,
                handles
                    .iter()
                    .map(|handle| (handle.path().to_path_buf(), content_type))
                    .collect(),
            );
        }
    });
}

fn open_browser(cluster_id: i64, package_type: &'static str) {
    let _ = RouterContext::get().push(Route::Browser {
        cluster_id,
        package_type: package_type.to_string(),
        pick_cluster: false,
        in_instance: true,
    });
}

fn browse_button(cluster_id: i64, package_type: &'static str) -> Element {
    toolbar_action(
        IconType::SearchMd,
        t("oneclient.cluster.package_manager.browse_ellipsis"),
    )
    .on_press(move |_| open_browser(cluster_id, package_type))
    .into_element()
}

#[derive(Clone, PartialEq)]
pub(super) enum ContentKind {
    Browser,
    Local,
    Other,
    Filtered(State<BTreeMap<String, String>>),
    /// `scope` names the tab that was searched when it is narrower than "All"
    NoMatches {
        scope: Option<String>,
    },
}

const SECTION_HEADER_H: f32 = 36.;

#[derive(Clone, Copy, PartialEq)]
pub(super) struct AdvancedSection {
    pub(super) open: State<bool>,
    pub(super) forced: bool,
}

impl AdvancedSection {
    pub(super) fn expanded(self) -> bool {
        self.forced || *self.open.read()
    }
}

#[derive(PartialEq)]
struct SectionHeader {
    label: &'static str,
    count: usize,
    section: AdvancedSection,
}

impl Component for SectionHeader {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let section = self.section;
        let mut open = section.open;
        let expanded = section.expanded();
        let interactive = !section.forced;

        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::fill())
            .cross_align(Alignment::Center)
            .spacing(8.)
            .padding(Gaps::new_symmetric(0., 12.))
            .corner_radius(rounded(ROW_RADIUS))
            .background(if interactive && *hovered.read() {
                colors::ghost_overlay_hover()
            } else {
                Color::TRANSPARENT
            })
            .maybe(interactive, |el| {
                el.cursor(CursorIcon::Pointer)
                    .on_pointer_enter(move |_| hovered.set(true))
                    .on_pointer_leave(move |_| hovered.set(false))
                    .on_press(move |_| open.toggle())
            })
            .child(ChevronToggle { expanded })
            .child(
                label()
                    .text(self.label)
                    .font_size(text_size(13.))
                    .font_weight(FontWeight::SEMI_BOLD)
                    .color(colors::fg_primary()),
            )
            .child(
                label()
                    .text(self.count.to_string())
                    .font_size(text_size(12.))
                    .color(colors::fg_secondary()),
            )
    }
}

#[derive(PartialEq)]
pub(super) struct ContentBox {
    items: Vec<PackageEntry>,
    advanced: Vec<PackageEntry>,
    section: AdvancedSection,
    noun_plural: &'static str,
    package_type: &'static str,
    content_type: ContentType,
    cluster_id: i64,
    kind: ContentKind,
    layout: CardLayout,
    bulk: Bulk,
    notices: Vec<String>,
    loading: bool,
    folder: Option<PathBuf>,
}

impl ContentBox {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        items: Vec<PackageEntry>,
        advanced: Vec<PackageEntry>,
        section: AdvancedSection,
        noun_plural: &'static str,
        package_type: &'static str,
        content_type: ContentType,
        cluster_id: i64,
        kind: ContentKind,
        layout: CardLayout,
        bulk: Bulk,
    ) -> Self {
        Self {
            items,
            advanced,
            section,
            noun_plural,
            package_type,
            content_type,
            cluster_id,
            kind,
            layout,
            bulk,
            notices: Vec::new(),
            loading: false,
            folder: None,
        }
    }

    pub(super) fn notices(mut self, notices: Vec<String>) -> Self {
        self.notices = notices;
        self
    }

    /// The content query has not answered yet, so an empty list is not "nothing installed"
    pub(super) fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// The folder the empty state's "Open folder" opens
    pub(super) fn folder(mut self, folder: Option<PathBuf>) -> Self {
        self.folder = folder;
        self
    }
}

impl Component for ContentBox {
    fn render(&self) -> impl IntoElement {
        let section = self.section;
        let expanded = section.expanded();
        let advanced_count = self.advanced.len();
        let mut items = self.items.clone();
        let normal_count = items.len();
        if expanded {
            items.extend(self.advanced.iter().cloned());
        }
        let package_type = self.package_type;
        let content_type = self.content_type;
        let cluster_id = self.cluster_id;
        let noun_plural = self.noun_plural;
        let kind = &self.kind;
        let layout = self.layout;

        let dispatch = use_dispatch();
        let cluster = use_cluster_mutation();
        let guard = use_essential_guard();
        let warnings_query = use_disable_warnings();
        let mut menu = use_state(|| None::<(f32, f32, PackageEntry, Vec<String>)>);
        let mut copies_of = use_state(|| None::<String>);
        // Copy groups an import asked to show, waiting for the open one to close
        let mut queued_copies = use_state(Vec::<String>::new);
        // Every group, hidden Advanced ones included, so only the import decides what opens
        let copy_groups: Vec<(String, Vec<String>)> = self
            .items
            .iter()
            .chain(self.advanced.iter())
            .filter(|item| !item.copies.is_empty())
            .map(|item| {
                let hashes = item
                    .hash
                    .iter()
                    .chain(item.copies.iter().filter_map(|copy| copy.hash.as_ref()))
                    .cloned()
                    .collect();
                (item.package_id.clone(), hashes)
            })
            .collect();
        use_side_effect_with_deps(&copy_groups, move |groups| {
            let mut request = crate::hooks::COPIES_REQUEST.lock().unwrap();
            let Some((_, wanted)) = request
                .as_ref()
                .filter(|(cluster, _)| *cluster == cluster_id)
            else {
                return;
            };
            let mut found: Vec<String> = groups
                .iter()
                .filter(|(_, hashes)| hashes.iter().any(|hash| wanted.contains(hash)))
                .map(|(package_id, _)| package_id.clone())
                .collect();
            if found.is_empty() {
                return;
            }
            *request = None;
            if copies_of.peek().is_none() {
                copies_of.set(Some(found.remove(0)));
            }
            queued_copies.write().extend(found);
        });
        use_side_effect(move || {
            if copies_of.read().is_none() && !queued_copies.peek().is_empty() {
                let next = queued_copies.write().remove(0);
                copies_of.set(Some(next));
            }
        });
        let (on_delete, delete_dialog) =
            use_shared_delete(cluster_id, move |(_, hash): (String, String)| {
                cluster.mutate(match parse_folder_pack_id(&hash) {
                    Some((name, enabled)) => ClusterAction::RemoveFolderPack {
                        cluster_id,
                        name: name.to_string(),
                        enabled,
                    },
                    None => ClusterAction::RemoveArtifact { cluster_id, hash },
                });
            });

        let bulk = self.bulk.clone();
        let selection = bulk.selection;
        let active = bulk.active();
        let selecting = active || selection.modifier_held();
        let selected: Vec<bool> = items
            .iter()
            .map(|p| selection.is_selected(&p.package_id))
            .collect();

        let row = {
            let items = items.clone();
            let order = bulk.order.clone();
            move |i: usize| {
                let item: PackageEntry = items[i].clone();
                let key = item.package_id.clone();
                let for_menu = item.clone();
                let is_selected = selected[i];
                let order = order.clone();
                let click_key = key.clone();
                let copies_key = key.clone();
                PackageRow::new(item, cluster_id, package_type)
                    .layout(layout)
                    .selection(is_selected, selecting, move |()| {
                        selection.click(click_key.clone(), &order)
                    })
                    .on_context(move |(x, y, hidden)| {
                        menu.set(Some((x, y, for_menu.clone(), hidden)))
                    })
                    .on_copies(move |()| copies_of.set(Some(copies_key.clone())))
                    .key(key)
                    .into_element()
            }
        };

        let count = normal_count + advanced_count;
        let scroll = (count > 0).then(|| {
            let mut sections = vec![LazySection {
                header: false,
                count: normal_count,
            }];
            if advanced_count > 0 {
                sections.push(LazySection {
                    header: true,
                    count: if expanded { advanced_count } else { 0 },
                });
            }
            let header = move |_| {
                SectionHeader {
                    label: t("oneclient.cluster.package_manager.advanced"),
                    count: advanced_count,
                    section,
                }
                .into_element()
            };
            match layout {
                CardLayout::List => content_scroll().lazy_sections(
                    sections,
                    CARD_H,
                    ROW_GAP,
                    0.,
                    1,
                    SECTION_HEADER_H,
                    row,
                    header,
                ),
                CardLayout::Grid => content_grid(
                    sections,
                    items
                        .iter()
                        .map(|item| !item.description.trim().is_empty())
                        .collect(),
                    SECTION_HEADER_H,
                    row,
                    header,
                ),
            }
            .into_element()
        });

        let copies_target = copies_of
            .read()
            .as_ref()
            .and_then(|key| {
                items
                    .iter()
                    .chain(self.advanced.iter())
                    .find(|item| &item.package_id == key)
            })
            .filter(|item| !item.copies.is_empty())
            .cloned();
        let copies_open = copies_target.is_some();
        let copies_popup = copies_target.map(|item| {
            let mut loaded = item.clone();
            if item.copies.iter().any(|copy| copy.copy_status.is_some()) {
                loaded.copy_status = Some(CopyStatus::Loaded);
            }
            let rows = std::iter::once(loaded)
                .chain(item.copies.iter().cloned())
                .map(|entry| {
                    let for_menu = entry.clone();
                    let key = entry.package_id.clone();
                    rect()
                        .width(Size::fill())
                        .height(Size::px(CARD_H))
                        .child(
                            PackageRow::new(entry, cluster_id, package_type)
                                .on_context(move |(x, y, hidden)| menu.set(Some((x, y, for_menu.clone(), hidden))))
                                .key(key),
                        )
                        .into_element()
                });
            OverlayPopup::new()
                .on_close(move |_| copies_of.set(None))
                .child(
                    rect()
                        .width(Size::window_percent(100.))
                        .height(Size::window_percent(100.))
                        .on_global_file_hover(move |_| copies_of.set(None))
                        .on_pointer_press(move |_| copies_of.set(None))
                        .center()
                        .child(
                            rect()
                                .vertical()
                                .width(Size::px(720.))
                                .max_width(Size::window_percent(90.))
                                .spacing(CARD_SPACING)
                                .padding(Gaps::new_all(20.))
                                .corner_radius(rounded(ROW_RADIUS))
                                .background(colors::page_elevated())
                                .border(border_all_color(1., colors::component_border()))
                                .on_pointer_press(|e: Event<PointerEventData>| e.stop_propagation())
                                .child(
                                    rect()
                                        .horizontal()
                                        .width(Size::fill())
                                        .content(Content::Flex)
                                        .cross_align(Alignment::Center)
                                        .spacing(10.)
                                        .child(
                                            label()
                                                .text(tf(
                                                    "oneclient.cluster.package_manager.copies.title",
                                                    &[&(item.copies.len() + 1), &item.name],
                                                ))
                                                .width(Size::flex(1.0))
                                                .font_size(text_size(16.))
                                                .font_weight(FontWeight::SEMI_BOLD)
                                                .color(colors::fg_strong()),
                                        )
                                        .child(
                                            Button::new()
                                                .ghost()
                                                .icon()
                                                .tooltip(t("oneclient.common.close"))
                                                .on_press(move |_| copies_of.set(None))
                                                .child(Icon::new(IconType::X).size(18.)),
                                        ),
                                )
                                .child(
                                    label()
                                        .text(t("oneclient.cluster.package_manager.copies.body"))
                                        .font_size(text_size(12.))
                                        .color(colors::fg_secondary()),
                                )
                                .children(rows),
                        ),
                )
                .into_element()
        });

        let menu_overlay = menu.read().clone().map(|(x, y, item, hidden_badges)| {
            let menu_for = if bulk.active() {
                bulk.menu(x, y, item.package_id.clone())
            } else {
                let key = item.package_id.clone();
                let on_bundled = (content_type == ContentType::Mod).then(|| {
                    let item = item.clone();
                    let warnings = disable_warnings(&warnings_query);
                    let handler: EventHandler<()> = (move |()| {
                        let Some(action) = bundled_delete_action(&item, cluster_id) else {
                            return;
                        };
                        let only = [&item];
                        run_guarded(
                            action,
                            if item.deleted { &[] } else { &only[..] },
                            item.name.clone(),
                            EssentialGuardKind::Remove,
                            warnings.clone(),
                            cluster,
                            guard,
                        );
                    })
                    .into();
                    handler
                });
                package_context_menu(
                    x,
                    y,
                    &item,
                    cluster_id,
                    package_type,
                    content_type,
                    on_delete,
                    on_bundled,
                    (move |()| selection.toggle(key.clone())).into(),
                    item.hash
                        .clone()
                        .filter(|_| item.update_checkable())
                        .map(|hash| {
                            let dispatch = dispatch.clone();
                            let package = (hash, item.name.clone());
                            (move |()| {
                                dispatch.check_package_updates(cluster_id, vec![package.clone()])
                            })
                            .into()
                        }),
                    hidden_badges,
                )
            };
            menu_for
                .on_close(move |_| menu.set(None))
                .overlay_level(OVERLAY_BASE_LEVEL + if copies_open { 3 } else { 0 })
                .into_element()
        });

        let folder = self.folder.clone();
        let empty = (count == 0).then(|| match kind {
            _ if self.loading => content_loading(),
            ContentKind::Browser => browser_empty(cluster_id, package_type, folder),
            ContentKind::Local => local_empty(
                cluster_id,
                content_type,
                dispatch.clone(),
                noun_plural,
                folder,
            ),
            ContentKind::Other => empty_state(
                cluster_id,
                package_type,
                content_type,
                noun_plural,
                dispatch.clone(),
            ),
            ContentKind::Filtered(filters) => filtered_empty(noun_plural, *filters),
            ContentKind::NoMatches { scope } => search_empty(noun_plural, scope.as_deref()),
        });

        // File drops are handled window-wide by the app shell so this box does not claim them
        // Overlays sit beside the list area, so opening one never takes a gap out of the list
        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::flex(1.0))
            .content(Content::Flex)
            .child(
                content_area(self.notices.clone())
                    .maybe_child(scroll)
                    .maybe_child(empty.map(|empty| {
                        rect()
                            .width(Size::fill())
                            .height(Size::flex(1.0))
                            .child(empty)
                    })),
            )
            .maybe_child(copies_popup)
            .maybe_child(menu_overlay)
            .maybe_child(delete_dialog)
    }
}

pub(crate) fn empty_shell(icon: IconType) -> Rect {
    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .center()
        .padding(Gaps::new_all(48.))
        .spacing(8.)
        .child(Icon::new(icon).size(28.).color(colors::fg_secondary()))
}

pub(crate) fn empty_title(text: impl Into<String>) -> impl IntoElement {
    label()
        .text(text.into())
        .font_size(text_size(14.))
        .color(colors::fg_secondary())
}

pub(crate) fn empty_hint(text: impl Into<String>) -> impl IntoElement {
    label()
        .text(text.into())
        .font_size(text_size(12.))
        .color(colors::fg_secondary())
}

/// The page's own icon, the one its rail entry uses
fn content_icon(content_type: ContentType) -> IconType {
    match content_type {
        ContentType::Shader => IconType::PaintPour,
        ContentType::ResourcePack => IconType::LayersThree01,
        _ => IconType::Cube01,
    }
}

fn empty_state(
    cluster_id: i64,
    package_type: &'static str,
    content_type: ContentType,
    noun_plural: &'static str,
    dispatch: Actions,
) -> Element {
    content_empty(
        content_icon(content_type),
        tf(
            "oneclient.cluster.package_manager.empty.title",
            &[&noun_plural],
        ),
        Some(t("oneclient.cluster.package_manager.empty.hint").to_string()),
        vec![
            browse_button(cluster_id, package_type),
            import_button(cluster_id, content_type, dispatch, false).into_element(),
        ],
    )
}

fn filtered_empty(
    noun_plural: &'static str,
    mut filters: State<BTreeMap<String, String>>,
) -> Element {
    content_empty(
        IconType::SearchMd,
        tf(
            "oneclient.cluster.package_manager.filtered_empty.title",
            &[&noun_plural],
        ),
        None,
        vec![
            toolbar_secondary(
                IconType::XClose,
                t("oneclient.cluster.package_manager.filtered_empty.clear"),
            )
            .on_press(move |_| {
                filters.write().remove(EnabledFilter::STORAGE_KEY);
            })
            .into_element(),
        ],
    )
}

/// The search only looks inside the active tab so name it and point at the tab that covers everything
fn search_empty(noun_plural: &'static str, scope: Option<&str>) -> Element {
    let (title, hint) = match scope {
        Some(tab) => (
            tf(
                "oneclient.cluster.package_manager.search_empty.scoped.title",
                &[&noun_plural, &tab],
            ),
            t("oneclient.cluster.package_manager.search_empty.scoped.hint"),
        ),
        None => (
            tf(
                "oneclient.cluster.package_manager.search_empty.title",
                &[&noun_plural],
            ),
            t("oneclient.cluster.package_manager.search_empty.hint"),
        ),
    };

    content_empty(
        IconType::SearchMd,
        title,
        Some(hint.to_string()),
        Vec::new(),
    )
}

fn browser_empty(cluster_id: i64, package_type: &'static str, folder: Option<PathBuf>) -> Element {
    content_empty(
        IconType::SearchMd,
        t("oneclient.cluster.package_manager.browser_empty.title"),
        Some(t("oneclient.cluster.package_manager.browser_empty.hint").to_string()),
        vec![
            browse_button(cluster_id, package_type),
            open_folder_button(folder),
        ],
    )
}

fn local_empty(
    cluster_id: i64,
    content_type: ContentType,
    dispatch: Actions,
    noun_plural: &'static str,
    folder: Option<PathBuf>,
) -> Element {
    let hint = (!utils::is_wayland())
        .then(|| t("oneclient.cluster.package_manager.local_empty.hint").to_string());

    content_empty(
        IconType::FilePlus02,
        tf(
            "oneclient.cluster.package_manager.local_empty.title",
            &[&noun_plural],
        ),
        hint,
        vec![
            import_button(cluster_id, content_type, dispatch, true).into_element(),
            open_folder_button(folder),
        ],
    )
}
