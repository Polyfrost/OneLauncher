use super::*;

use freya::router::RouterContext;
use oneclient_content::packages::ContentType;
use oneclient_core::settings::ViewLayout;

use crate::components::{
    Button, CardLayout, ChevronToggle, ContextMenu, FilterMenu, FilterOption, Icon, IconType,
    LazySection, PackageEntry, PackageRow, ScrollArea, Segment, SegmentedControl, TextInput,
    package_context_menu, use_shared_delete,
};
use crate::hooks::{ClusterAction, Selection, use_cluster_mutation, use_dispatch};
use crate::routes::Route;
use crate::theme::colors;
use crate::{Actions, utils};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum SortMode {
    NameAsc,
    NameDesc,
    SizeAsc,
    SizeDesc,
}

impl SortMode {
    const ALL: [SortMode; 4] = [
        SortMode::NameAsc,
        SortMode::NameDesc,
        SortMode::SizeAsc,
        SortMode::SizeDesc,
    ];

    fn key(self) -> &'static str {
        match self {
            SortMode::NameAsc => "name_asc",
            SortMode::NameDesc => "name_desc",
            SortMode::SizeAsc => "size_asc",
            SortMode::SizeDesc => "size_desc",
        }
    }

    pub(super) fn from_key(key: &str) -> Option<Self> {
        SortMode::ALL.into_iter().find(|s| s.key() == key)
    }

    fn label(self) -> &'static str {
        match self {
            SortMode::NameAsc => "Name (A-Z)",
            SortMode::NameDesc => "Name (Z-A)",
            SortMode::SizeAsc => "Size (Smallest)",
            SortMode::SizeDesc => "Size (Largest)",
        }
    }

    pub(super) fn sort(self, rows: &mut [PackageEntry]) {
        let title = |p: &PackageEntry| p.name.to_lowercase();

        match self {
            SortMode::NameDesc => rows.sort_by_key(|p| std::cmp::Reverse(title(p))),
            SortMode::SizeAsc => rows.sort_by_key(|p| p.size),
            SortMode::SizeDesc => rows.sort_by_key(|p| std::cmp::Reverse(p.size)),
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
    const ALL: [EnabledFilter; 3] = [
        EnabledFilter::All,
        EnabledFilter::Enabled,
        EnabledFilter::Disabled,
    ];

    fn label(self) -> &'static str {
        match self {
            EnabledFilter::All => "All packages",
            EnabledFilter::Enabled => "Enabled only",
            EnabledFilter::Disabled => "Disabled only",
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
    const ALL: [HiddenFilter; 2] = [HiddenFilter::Hide, HiddenFilter::Show];

    fn label(self) -> &'static str {
        match self {
            HiddenFilter::Hide => "Hide",
            HiddenFilter::Show => "Show",
        }
    }

    pub(super) fn keep(self, p: &PackageEntry) -> bool {
        match self {
            HiddenFilter::Hide => !p.hidden,
            HiddenFilter::Show => true,
        }
    }
}

/// The view state persists this choice as a plain flag so the toolbar and the
/// list have to agree on what it means
impl From<bool> for HiddenFilter {
    fn from(show: bool) -> Self {
        if show {
            HiddenFilter::Show
        } else {
            HiddenFilter::Hide
        }
    }
}

#[derive(Clone, PartialEq)]
pub(super) struct Bulk {
    pub(super) selection: Selection<String>,
    pub(super) order: Vec<String>,
    pub(super) count: usize,
    pub(super) deletable: usize,
    pub(super) set_enabled: EventHandler<bool>,
    pub(super) delete: EventHandler<()>,
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
            (IconType::XClose, "Unselect all")
        } else {
            (IconType::Check, "Select all")
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
                .font_size(12.)
                .child(Icon::new(icon).size(15.))
                .text(text)
        };

        vec![
            label()
                .text(format!("{} selected", self.count))
                .font_size(12.)
                .max_lines(1)
                .color(colors::fg_secondary())
                .into_element(),
            button(toggle_icon, toggle_label.to_string())
                .on_press(move |_| selection.toggle_all(&order))
                .into_element(),
            button(IconType::CheckCircle, "Enable".to_string())
                .enabled(self.count > 0)
                .on_press(move |_| enable.call(true))
                .into_element(),
            button(IconType::Minus, "Disable".to_string())
                .enabled(self.count > 0)
                .on_press(move |_| disable.call(false))
                .into_element(),
            button(IconType::Trash01, format!("Delete ({})", self.deletable))
                .danger()
                .enabled(self.deletable > 0)
                .on_press(move |_| delete.call(()))
                .into_element(),
            Button::new()
                .ghost()
                .icon()
                .height(Size::px(34.))
                .tooltip("Exit select mode")
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
        let (toggle_icon, toggle_label) = self.toggle_all_label();

        let (icon, text) = if selection.is_selected(&key) {
            (IconType::XClose, "Unselect")
        } else {
            (IconType::Check, "Select")
        };

        let mut menu = ContextMenu::new(x, y)
            .title(format!("{} selected", self.count))
            .action(icon, text, move |()| selection.toggle(key.clone()))
            .separator();
        if self.count > 0 {
            menu = menu
                .action(IconType::CheckCircle, "Enable Selected", move |()| {
                    enable.call(true)
                })
                .action(IconType::Minus, "Disable Selected", move |()| {
                    disable.call(false)
                })
                .separator();
        }
        menu = menu.action(toggle_icon, toggle_label, move |()| {
            selection.toggle_all(&order)
        });
        if self.count > 0 && !self.all_selected() {
            menu = menu.action(IconType::XClose, "Clear selection", move |()| {
                selection.clear()
            });
        }

        if self.deletable == 0 {
            return menu;
        }
        menu.separator().danger_action(
            IconType::Trash01,
            format!("Delete {}", self.deletable),
            move |()| delete.call(()),
        )
    }
}

const TOOLBAR_STACK_W: f32 = 640.;
const TABS_ROW_H: f32 = 34.;

fn tabs_scrollbar_theme() -> ScrollBarThemePartial {
    ScrollBarThemePartial {
        thumb_background: Some(colors::fg_secondary().with_a(120).into()),
        hover_thumb_background: Some(colors::fg_secondary().with_a(190).into()),
        active_thumb_background: Some(colors::fg_primary().into()),
        ..Default::default()
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn toolbar_bar(
    tabs: &[Tab],
    active_idx: usize,
    active: State<usize>,
    search: State<String>,
    sort: State<Option<String>>,
    current_sort: SortMode,
    enabled_filter: State<EnabledFilter>,
    show_hidden: State<bool>,
    uses_bundles: bool,
    layout: State<ViewLayout>,
    cluster_id: i64,
    package_type: &'static str,
    mut toolbar_width: State<f32>,
    bulk: &Bulk,
) -> impl IntoElement {
    let chips = tabs.iter().enumerate().map(|(i, tab)| {
        let mut active = active;
        CategoryChip {
            label: tab.label(),
            selected: i == active_idx,
            on_press: (move |_| *active.write() = i).into(),
        }
        .into_element()
    });

    let mut top_corners = CornerRadius::new_all(0.);
    top_corners.fill_top(12.);

    let measured = *toolbar_width.read();
    let stacked = measured > 0. && measured < TOOLBAR_STACK_W;

    let filter_tabs = ScrollView::new()
        .direction(Direction::Horizontal)
        .invert_scroll_wheel(true)
        .show_scrollbar(true)
        .scrollbar(|context| ScrollBar::new(context).theme(tabs_scrollbar_theme()).into())
        .width(if stacked {
            Size::fill()
        } else {
            Size::flex(1.0)
        })
        .height(Size::px(TABS_ROW_H))
        .child(
            rect()
                .horizontal()
                .width(Size::auto())
                .height(Size::fill())
                .cross_align(Alignment::Center)
                .spacing(6.)
                .children(chips),
        )
        .into_element();

    let mut controls: Vec<Element> = vec![
        TextInput::new(search)
            .placeholder("Search...")
            .width(Size::px(180.))
            .leading(
                Icon::new(IconType::SearchMd)
                    .size(14.)
                    .color(colors::fg_secondary())
                    .into_element(),
            )
            .into_element(),
        filter_button(
            sort,
            current_sort,
            enabled_filter,
            show_hidden,
            uses_bundles,
        )
        .into_element(),
    ];

    controls.push(
        SegmentedControl::new(layout)
            .height(34.)
            .icon_size(15.)
            .equal_width(34.)
            .segment(Segment::new(ViewLayout::List).icon(IconType::ParagraphWrap))
            .segment(Segment::new(ViewLayout::Grid).icon(IconType::DotsGrid))
            .into_element(),
    );
    controls.push(
        Button::new()
            .primary()
            .height(Size::px(34.))
            .font_size(12.)
            .on_press(move |_| open_browser(cluster_id, package_type))
            .child(Icon::new(IconType::Plus).size(15.))
            .text("Add Content")
            .into_element(),
    );
    let controls = if bulk.active() {
        bulk.controls()
    } else {
        controls
    };

    let inner = if stacked {
        rect()
            .vertical()
            .width(Size::fill())
            .spacing(8.)
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .height(Size::px(TABS_ROW_H))
                    .cross_align(Alignment::Center)
                    .child(filter_tabs),
            )
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .cross_align(Alignment::Center)
                    .spacing(8.)
                    .content(Content::Flex)
                    .children(controls),
            )
            .into_element()
    } else {
        rect()
            .horizontal()
            .width(Size::fill())
            .cross_align(Alignment::Center)
            .spacing(8.)
            .content(Content::Flex)
            .child(filter_tabs)
            .children(controls)
            .into_element()
    };

    rect()
        .vertical()
        .width(Size::fill())
        .overflow(Overflow::Clip)
        .padding(Gaps::new_symmetric(8., 12.))
        .corner_radius(top_corners)
        .background(colors::page_elevated())
        .on_sized(move |event: Event<SizedEventData>| {
            let next = event.data().area.width();
            if (*toolbar_width.peek() - next).abs() > 0.5 {
                toolbar_width.set(next);
            }
        })
        .child(inner)
        .into_element()
}

#[derive(PartialEq)]
struct CategoryChip {
    label: String,
    selected: bool,
    on_press: EventHandler<Event<PressEventData>>,
}

impl Component for CategoryChip {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let selected = self.selected;

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let focused = focus().is_focused();

        let background = if selected {
            colors::brand()
        } else if *hovered.read() {
            colors::component_bg_hover()
        } else {
            colors::component_bg()
        };

        rect()
            .horizontal()
            .height(Size::px(32.))
            .center()
            .padding(Gaps::new_symmetric(0., 12.))
            .corner_radius(CornerRadius::new_all(8.))
            .background(background)
            .border(crate::ui::border_all_color(
                1.,
                if selected || focused {
                    colors::brand()
                } else {
                    colors::component_border()
                },
            ))
            .cursor(CursorIcon::Pointer)
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Tab)
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_leave(move |_| hovered.set(false))
            .on_press(self.on_press.clone())
            .child(
                label()
                    .text(self.label.clone())
                    .font_size(12.)
                    .font_weight(FontWeight::MEDIUM)
                    .max_lines(1)
                    .color(if selected {
                        Color::WHITE
                    } else {
                        colors::fg_primary()
                    }),
            )
    }
}

pub(super) fn running_notice(noun_plural: &'static str, content_type: ContentType) -> String {
    match content_type {
        ContentType::ResourcePack => format!(
            "Minecraft is running. New {noun_plural} usually go in right away, open Options → Resource Packs in game to turn them on. OneClient tells you when one has to wait for the next launch."
        ),
        ContentType::Shader => format!(
            "Minecraft is running. New {noun_plural} usually go in right away, open the shader pack screen in game to turn them on. OneClient tells you when one has to wait for the next launch."
        ),
        _ => format!(
            "Minecraft is running. Changes to your {noun_plural} are saved, and take effect the next time you launch this version."
        ),
    }
}

pub(super) fn global_notice(noun_plural: &'static str) -> String {
    format!(
        "These {noun_plural} are shared across all your OneClient clusters. Adding one here makes it available in all of them, and turning one off removes it from all of them."
    )
}

pub(super) fn instance_only_notice(noun_plural: &'static str) -> String {
    format!(
        "These {noun_plural} belong to this instance alone. Adding one here does not touch your other instances."
    )
}

pub(super) fn essential_notice(names: &[&'static str]) -> String {
    match names {
        [only] => format!("{only} is turned off, so its features will not work in game."),
        _ => format!(
            "{} are turned off, so their features will not work in game.",
            names.join(", ")
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
        .margin(Gaps::new(0., 0., 8., 0.))
        .padding(Gaps::new_symmetric(9., 12.))
        .corner_radius(CornerRadius::new_all(10.))
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
                .font_size(12.)
                .width(Size::flex(1.0))
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn filter_button(
    mut sort: State<Option<String>>,
    current_sort: SortMode,
    mut enabled_filter: State<EnabledFilter>,
    mut show_hidden: State<bool>,
    uses_bundles: bool,
) -> FilterMenu {
    let show = *enabled_filter.read();
    let hidden = HiddenFilter::from(*show_hidden.read());

    // Hiding hidden packages is the default so it does not count as the filters being touched
    let active = current_sort != SortMode::NameAsc
        || show != EnabledFilter::All
        || hidden != HiddenFilter::Hide;

    let menu = FilterMenu::new(active)
        .section(
            "Sort by",
            SortMode::ALL.map(|mode| {
                FilterOption::new(mode.label(), mode == current_sort, move |()| {
                    sort.set(Some(mode.key().to_string()));
                })
            }),
        )
        .section(
            "Show",
            EnabledFilter::ALL.map(|filter| {
                FilterOption::new(filter.label(), filter == show, move |()| {
                    enabled_filter.set(filter);
                })
            }),
        );

    if !uses_bundles {
        return menu;
    }
    menu.section(
        "Hidden packages",
        HiddenFilter::ALL.map(|filter| {
            FilterOption::new(filter.label(), filter == hidden, move |()| {
                show_hidden.set(filter == HiddenFilter::Show);
            })
        }),
    )
}

fn add_from_file_button(
    cluster_id: i64,
    content_type: ContentType,
    dispatch: Actions,
) -> impl IntoElement {
    Button::new()
        .primary()
        .on_press(move |_| {
            let dispatch = dispatch.clone();
            spawn(async move {
                if let Some(handles) = rfd::AsyncFileDialog::new()
                    .set_title("Select files to import")
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
        })
        .child(Icon::new(IconType::FilePlus02).size(14.))
        .text("Add from file")
}

fn open_browser(cluster_id: i64, package_type: &'static str) {
    let _ = RouterContext::get().push(Route::Browser {
        cluster_id,
        package_type: package_type.to_string(),
        pick_cluster: false,
    });
}

fn browse_button(cluster_id: i64, package_type: &'static str) -> impl IntoElement {
    Button::new()
        .primary()
        .on_press(move |_| open_browser(cluster_id, package_type))
        .child(Icon::new(IconType::SearchMd).size(14.))
        .text("Browse Content")
}

#[derive(Clone, PartialEq)]
pub(super) enum ContentKind {
    Browser,
    Local,
    Other,
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
            .corner_radius(CornerRadius::new_all(10.))
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
                    .font_size(13.)
                    .font_weight(FontWeight::SEMI_BOLD)
                    .color(colors::fg_primary()),
            )
            .child(
                label()
                    .text(self.count.to_string())
                    .font_size(12.)
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
        }
    }

    pub(super) fn notices(mut self, notices: Vec<String>) -> Self {
        self.notices = notices;
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
        let mut menu = use_state(|| None::<(f32, f32, PackageEntry)>);
        let (on_delete, delete_dialog) = use_shared_delete(cluster_id, move |(_, hash)| {
            cluster.mutate(ClusterAction::RemoveArtifact { cluster_id, hash });
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
                PackageRow::new(item, cluster_id, package_type)
                    .layout(layout)
                    .selection(is_selected, selecting, move |()| {
                        selection.click(click_key.clone(), &order)
                    })
                    .on_context(move |(x, y)| menu.set(Some((x, y, for_menu.clone()))))
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
            let (item_height, gap, min_width, max_cols) = match layout {
                CardLayout::List => (CARD_H, CARD_SPACING, 0., 1),
                CardLayout::Grid => (CARD_GRID_H, GRID_GAP, GRID_MIN_W, GRID_MAX_COLS),
            };
            ScrollArea::new()
                .width(Size::fill())
                .height(Size::flex(1.0))
                .scrollbar_gutter(true)
                .lazy_sections(
                    sections,
                    item_height,
                    gap,
                    min_width,
                    max_cols,
                    SECTION_HEADER_H,
                    row,
                    move |_| {
                        SectionHeader {
                            label: "Advanced",
                            count: advanced_count,
                            section,
                        }
                        .into_element()
                    },
                )
                .into_element()
        });

        let menu_overlay = menu.read().clone().map(|(x, y, item)| {
            let menu_for = if bulk.active() {
                bulk.menu(x, y, item.package_id.clone())
            } else {
                let key = item.package_id.clone();
                package_context_menu(
                    x,
                    y,
                    &item,
                    cluster_id,
                    package_type,
                    on_delete,
                    (move |()| selection.toggle(key.clone())).into(),
                )
            };
            menu_for.on_close(move |_| menu.set(None)).into_element()
        });

        let empty = (count == 0).then(|| match kind {
            ContentKind::Browser => browser_empty(cluster_id, package_type).into_element(),
            ContentKind::Local => {
                local_empty(cluster_id, content_type, dispatch.clone(), noun_plural).into_element()
            }
            ContentKind::Other => empty_state(noun_plural).into_element(),
            ContentKind::NoMatches { scope } => {
                search_empty(noun_plural, scope.as_deref()).into_element()
            }
        });

        let header = (count > 0)
            .then(|| match kind {
                // The toolbar already carries a permanent "Add Content" button
                ContentKind::Browser => None,
                ContentKind::Local => Some(
                    action_header(add_from_file_button(
                        cluster_id,
                        content_type,
                        dispatch.clone(),
                    ))
                    .into_element(),
                ),
                ContentKind::Other | ContentKind::NoMatches { .. } => None,
            })
            .flatten();

        let mut bottom_corners = CornerRadius::new_all(0.);
        bottom_corners.fill_bottom(12.);

        // File drops are handled window-wide by the app shell so this box does not claim them
        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::flex(1.0))
            .padding(Gaps::new(0., 12., 12., 12.))
            .corner_radius(bottom_corners)
            .background(colors::page_elevated())
            .overflow(Overflow::Clip)
            .content(Content::Flex)
            .children(self.notices.iter().map(|text| notice_bar(text.clone())))
            .maybe_child(header)
            .maybe_child(scroll)
            .maybe_child(empty.map(|empty| {
                rect()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .child(empty)
            }))
            .maybe_child(menu_overlay)
            .maybe_child(delete_dialog)
    }
}

fn action_header(button: impl IntoElement) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .content(Content::Flex)
        .child(rect().width(Size::flex(1.0)))
        .child(button)
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
        .font_size(14.)
        .color(colors::fg_secondary())
}

pub(crate) fn empty_hint(text: impl Into<String>) -> impl IntoElement {
    label()
        .text(text.into())
        .font_size(12.)
        .color(colors::fg_secondary())
}

fn empty_state(noun_plural: &'static str) -> impl IntoElement {
    empty_shell(IconType::DotsGrid)
        .child(empty_title(format!("No {noun_plural} here yet.")))
        .child(empty_hint(
            "Add one from a file or browse provider content.",
        ))
        .into_element()
}

/// The search only looks inside the active tab so name it and point at the tab that covers everything
fn search_empty(noun_plural: &'static str, scope: Option<&str>) -> impl IntoElement {
    let (title, hint) = match scope {
        Some(tab) => (
            format!("No {noun_plural} in \"{tab}\" match your search."),
            "Try a different term, or search from the All tab.",
        ),
        None => (
            format!("No {noun_plural} match your search."),
            "Try a different term.",
        ),
    };

    empty_shell(IconType::SearchMd)
        .child(empty_title(title))
        .child(empty_hint(hint))
        .into_element()
}

fn browser_empty(cluster_id: i64, package_type: &'static str) -> impl IntoElement {
    empty_shell(IconType::SearchMd)
        .child(empty_title("No content installed from the browser."))
        .child(empty_hint(
            "Browse providers to add mods, resource packs and more.",
        ))
        .child(rect().height(Size::px(6.)))
        .child(browse_button(cluster_id, package_type))
        .into_element()
}

fn local_empty(
    cluster_id: i64,
    content_type: ContentType,
    dispatch: Actions,
    noun_plural: &'static str,
) -> impl IntoElement {
    let is_wayland = utils::is_wayland();

    empty_shell(IconType::FilePlus02)
        .child(empty_title(format!("No local {noun_plural} yet.")))
        .maybe(!is_wayland, |e| {
            e.child(empty_hint(
                "Tip: drag files onto the window to install them.",
            ))
        })
        .child(rect().height(Size::px(6.)))
        .child(add_from_file_button(cluster_id, content_type, dispatch))
        .into_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_flag_decides_which_hidden_packages_are_listed() {
        assert!(HiddenFilter::from(false) == HiddenFilter::Hide);
        assert!(HiddenFilter::from(true) == HiddenFilter::Show);
    }
}
