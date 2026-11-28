//! Browse inside the instance pop-up. The categories read as the pop-up's rail and the toolbar and
//! results as its content pages, so adding content never looks like leaving the instance.

use std::collections::HashMap;

use oneclient_content::packages::types::ProjectSummary;

use super::cards::instance_row;
use super::skeletons::InstanceSkeletonRow;
use super::*;
use crate::components::{CARD_H as ROW_H, ROW_GAP, ROW_RADIUS};
use crate::hooks::SearchOverride;
use crate::layout::ClusterShell;
use crate::theme::rounded;
use crate::ui::border_all_color;
use crate::utils::capitalize;

/// The instance shell's space between the header divider and every page
const CONTENT_TOP: f32 = 16.;
/// Toolbar controls, as on the content pages
pub(super) const CONTROL_H: f32 = 34.;
/// Between controls of one toolbar group
const CONTROL_GAP: f32 = 8.;
/// Between toolbar groups, and between the toolbar and the results
const GROUP_GAP: f32 = 12.;
const SEARCH_MIN_W: f32 = 160.;
const SEARCH_MAX_W: f32 = 280.;
const TARGET_CHIP_H: f32 = 28.;
const PICKER_W: f32 = 132.;
/// The provider switch with its names, before it has been measured
const PROVIDER_LABELS_W: f32 = 240.;
/// The provider switch as icons only, before it has been measured. Deliberately generous: a low
/// guess would let the line overflow and the controls overlap
const PROVIDER_ICONS_W: f32 = 124.;
/// Breathing room kept on a single toolbar line, so measuring noise never makes it overlap
const ONE_LINE_SLACK: f32 = 8.;
/// A rail row's text inset
const RAIL_INSET: f32 = 10.;
/// Below this the rows drop their downloads count and the pager shows fewer pages
const COMPACT_W: f32 = 620.;
/// Room the result count needs beside the centered pager
const SUMMARY_W: f32 = 190.;

type InstalledMap = HashMap<(ProviderId, String), Installed>;

/// How the toolbar lays out at a measured width
#[derive(Clone, Copy, Debug, PartialEq)]
struct ToolbarFit {
    stacked: bool,
    search_w: f32,
    provider_labels: bool,
}

/// Keep everything on one line for as long as possible: first the search shrinks to its minimum,
/// then the provider switch drops its names for icons, and only then do the pickers move to a
/// second line. `controls_w` is the pickers' group with the provider names showing.
fn toolbar_fit(available: f32, chips_w: f32, controls_w: f32, icon_controls_w: f32) -> ToolbarFit {
    let chips = if chips_w > 0. {
        chips_w + GROUP_GAP
    } else {
        0.
    };
    let one_line = SEARCH_MIN_W + chips + GROUP_GAP + controls_w + ONE_LINE_SLACK;
    let one_line_icons = SEARCH_MIN_W + chips + GROUP_GAP + icon_controls_w + ONE_LINE_SLACK;
    if available >= one_line {
        ToolbarFit {
            stacked: false,
            search_w: (available - (one_line - SEARCH_MIN_W)).clamp(SEARCH_MIN_W, SEARCH_MAX_W),
            provider_labels: true,
        }
    } else if available >= one_line_icons {
        ToolbarFit {
            stacked: false,
            search_w: (available - (one_line_icons - SEARCH_MIN_W))
                .clamp(SEARCH_MIN_W, SEARCH_MAX_W),
            provider_labels: false,
        }
    } else {
        ToolbarFit {
            stacked: true,
            search_w: (available - chips).clamp(0., SEARCH_MAX_W),
            provider_labels: available >= controls_w,
        }
    }
}

/// The pickers' group with the provider names: as last measured with them, or an estimate
fn labelled_controls_w(measured: f32, pickers: usize) -> f32 {
    if measured > 0. {
        measured
    } else {
        PROVIDER_LABELS_W + pickers as f32 * (PICKER_W + CONTROL_GAP)
    }
}

/// The pickers' group with the provider switch as icons: as last measured that way, or an estimate
fn icon_controls_w(measured: f32, pickers: usize) -> f32 {
    if measured > 0. {
        measured
    } else {
        PROVIDER_ICONS_W + pickers as f32 * (PICKER_W + CONTROL_GAP)
    }
}

/// Whether the result count fits beside a pager of `window` pages centered in `width`
fn summary_fits(width: f32, window: usize) -> bool {
    // first, previous, the window, both ends with their gaps, next and last
    let pager = (window + 6) as f32 * 32. + 40. + (window + 9) as f32 * 6.;
    (width - pager) / 2. >= SUMMARY_W
}

fn track_width(mut state: State<f32>) -> impl FnMut(Event<SizedEventData>) {
    move |event: Event<SizedEventData>| {
        let next = event.data().area.width();
        if (*state.peek() - next).abs() > 0.5 {
            state.set(next);
        }
    }
}

pub(super) struct InstanceBrowse {
    pub cluster_id: i64,
    pub package_type: String,
    pub target: Option<(&'static str, String)>,
    pub override_button: Option<OverrideButton>,
    pub provider: State<ProviderId>,
    pub curseforge_blocked: bool,
    pub query: State<String>,
    pub categories: Vec<String>,
    pub selected_categories: State<Vec<String>>,
    pub sort: State<SearchSort>,
    pub loader: Option<State<Option<GameLoader>>>,
    pub side: Option<State<Option<SearchSide>>>,
    /// Mods can be filtered by side on some providers. The toolbar keeps the picker's room either
    /// way, so switching provider never slides the provider switch or the sort picker.
    pub side_slot: bool,
    pub packages: Vec<ProjectSummary>,
    pub installed: InstalledMap,
    pub ownership_ready: bool,
    pub add_to_cluster: bool,
    pub search_override: Option<SearchOverride>,
    pub pending: bool,
    /// The search failed and nothing is on screen: Retry, not "No results"
    pub failed: bool,
    pub on_retry: EventHandler<()>,
    /// Changes with every effective search, so any change scrolls back to the top
    pub scroll_key: u64,
    pub total: usize,
    pub page: State<usize>,
    pub pages: Option<usize>,
}

impl InstanceBrowse {
    /// Called from the browser's render, so these hooks always run in the same order
    pub(super) fn render(self) -> Element {
        let padding = ClusterShell::workspace_padding();
        let rail_width = ClusterShell::workspace_rail_width();
        let (row_height, row_gap) = ClusterShell::workspace_rail_rows();

        let pane_width = use_state(|| 0f32);
        let toolbar_width = use_state(|| 0f32);
        let chips_width = use_state(|| 0f32);
        let controls_width = use_state(|| 0f32);
        let icon_controls_width = use_state(|| 0f32);

        let width = *pane_width.read();
        let compact = width > 0. && width < COMPACT_W;
        let current_page = *self.page.read();
        let cluster_id = self.cluster_id;
        let package_type = self.package_type.clone();
        let add_to_cluster = self.add_to_cluster;
        let search_override = self.search_override.clone();
        let installed = self.installed;
        let ownership_ready = self.ownership_ready;
        let packages = self.packages;

        let list = ScrollArea::new()
            .width(Size::fill())
            .height(Size::fill())
            .auto_hide_scrollbar()
            .reset_key(self.scroll_key);
        let results = if !packages.is_empty() {
            list.lazy(packages.len(), ROW_H, ROW_GAP, move |i| {
                instance_row(
                    packages[i].clone(),
                    cluster_id,
                    &package_type,
                    &installed,
                    ownership_ready,
                    add_to_cluster,
                    search_override.clone(),
                    compact,
                )
                .into_element()
            })
            .into_element()
        } else if self.pending {
            list.lazy(BROWSE_PAGE_SIZE, ROW_H, ROW_GAP, |_| {
                InstanceSkeletonRow.into_element()
            })
            .into_element()
        } else if self.failed {
            let on_retry = self.on_retry.clone();
            result_surface(super::load_failed(
                t("oneclient.browser.index.load_failed"),
                move |()| on_retry.call(()),
            ))
        } else {
            empty_panel()
        };

        let window = if compact { 3 } else { 5 };
        let summary = self
            .pages
            .filter(|_| !self.pending && summary_fits(width, window))
            .map(|pages| {
                tf(
                    "oneclient.browser.index.summary",
                    &[
                        &abbreviate_number(self.total as u64),
                        &(current_page + 1),
                        &pages,
                    ],
                )
            });
        let pager = self.pages.map(|pages| {
            rect()
                .width(Size::fill())
                .height(Size::px(32.))
                .child(
                    Pagination::new(self.page, pages)
                        .window(window)
                        .enabled(!self.pending),
                )
                .maybe_child(summary.map(|summary| {
                    rect()
                        .position(Position::new_absolute().left(0.).top(0.))
                        .height(Size::px(32.))
                        .main_align(Alignment::Center)
                        .child(
                            label()
                                .text(summary)
                                .max_lines(1)
                                .font_size(text_size(12.))
                                .color(colors::fg_secondary()),
                        )
                }))
        });

        let toolbar = toolbar(
            Toolbar {
                placeholder: tf(
                    "oneclient.browser.index.search_type",
                    &[&type_title(&self.package_type).to_lowercase()],
                ),
                target: self.target,
                override_button: self.override_button,
                provider: self.provider,
                curseforge_blocked: self.curseforge_blocked,
                query: self.query,
                sort: self.sort,
                loader: self.loader,
                side: self.side,
                side_slot: self.side_slot,
            },
            toolbar_width,
            chips_width,
            controls_width,
            icon_controls_width,
        );

        rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::fill())
            .padding(Gaps::new(CONTENT_TOP, padding, padding, padding))
            .child(CategoryRail {
                categories: self.categories,
                selected: self.selected_categories,
                width: rail_width,
                gap: padding,
                row_height,
                row_gap,
            })
            .child(
                rect()
                    .vertical()
                    .content(Content::Flex)
                    .width(Size::flex(1.))
                    .height(Size::fill())
                    .spacing(GROUP_GAP)
                    .on_sized(track_width(pane_width))
                    .child(toolbar)
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::flex(1.))
                            .child(results),
                    )
                    .maybe_child(pager),
            )
            .into_element()
    }
}

struct Toolbar {
    placeholder: String,
    target: Option<(&'static str, String)>,
    override_button: Option<OverrideButton>,
    provider: State<ProviderId>,
    curseforge_blocked: bool,
    query: State<String>,
    sort: State<SearchSort>,
    loader: Option<State<Option<GameLoader>>>,
    side: Option<State<Option<SearchSide>>>,
    side_slot: bool,
}

fn toolbar(
    parts: Toolbar,
    width: State<f32>,
    chips_width: State<f32>,
    controls_width: State<f32>,
    icon_controls_width: State<f32>,
) -> Element {
    let has_chips = parts.target.is_some() || parts.override_button.is_some();
    let pickers =
        usize::from(parts.loader.is_some()) + usize::from(parts.side.is_some() || parts.side_slot);
    let mut fit = toolbar_fit(
        *width.read(),
        *chips_width.read(),
        labelled_controls_w(*controls_width.read(), pickers),
        icon_controls_w(*icon_controls_width.read(), pickers),
    );
    // Until the chips report their natural width they get the search's line to themselves
    fit.stacked |= has_chips && *chips_width.read() == 0.;

    let search = TextInput::new(parts.query)
        .placeholder(parts.placeholder)
        .width(Size::px(fit.search_w))
        .height(Size::px(CONTROL_H))
        .leading(
            Icon::new(IconType::SearchMd)
                .size(14.)
                .color(colors::fg_secondary())
                .into_element(),
        );
    let chips = has_chips.then(|| {
        rect()
            .horizontal()
            .height(Size::px(CONTROL_H))
            .cross_align(Alignment::Center)
            .spacing(CONTROL_GAP)
            .on_sized(track_width(chips_width))
            .maybe_child(
                parts
                    .target
                    .map(|(verb, name)| target_chip(verb, &name).into_element()),
            )
            .maybe_child(parts.override_button)
            .into_element()
    });

    let curseforge_blocked = parts.curseforge_blocked;
    let labels = fit.provider_labels;
    let providers = SegmentedControl::new(parts.provider)
        .no_tint()
        .static_indicator()
        .height(CONTROL_H);
    let providers = if labels {
        providers
    } else {
        providers.equal_width(CONTROL_H)
    };
    let providers = providers.segments(ProviderId::REMOTE_PROVIDERS.iter().map(move |provider| {
        let mut segment = Segment::new(*provider)
            .icon(IconType::from(*provider))
            .tooltip(provider.to_string());
        if curseforge_blocked && *provider == ProviderId::CurseForge {
            segment = segment
                .disabled(true)
                .tooltip(t("oneclient.browser.index.curseforge_blocked"));
        }
        if labels {
            segment.label(provider.to_string())
        } else {
            segment
        }
    }));
    let controls = rect()
        .horizontal()
        .height(Size::px(CONTROL_H))
        .cross_align(Alignment::Center)
        .spacing(CONTROL_GAP)
        // Only a measurement with the names showing says how much room the names need
        .on_sized(move |event: Event<SizedEventData>| {
            let next = event.data().area.width();
            // each form of the switch keeps its own measurement
            let mut target = if labels {
                controls_width
            } else {
                icon_controls_width
            };
            if (*target.peek() - next).abs() > 0.5 {
                target.set(next);
            }
        })
        // The side picker leads the group, so on a provider without it the reserved room reads as
        // part of the gap before the controls instead of a hole between them
        .maybe_child(match parts.side {
            Some(side) => Some(
                SidePicker {
                    side,
                    width: PICKER_W,
                    height: CONTROL_H,
                }
                .into_element(),
            ),
            None => parts.side_slot.then(|| {
                rect()
                    .width(Size::px(PICKER_W))
                    .height(Size::px(CONTROL_H))
                    .into_element()
            }),
        })
        .child(providers)
        .maybe_child(parts.loader.map(|loader| LoaderPicker {
            loader,
            width: PICKER_W,
            height: CONTROL_H,
        }))
        .child(SortPicker {
            sort: parts.sort,
            width: PICKER_W,
            height: CONTROL_H,
        });

    // No flex here: a squeezed group would report less than it needs and never restack
    let line = || {
        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(CONTROL_H))
            .cross_align(Alignment::Center)
    };
    let find = rect()
        .horizontal()
        .height(Size::px(CONTROL_H))
        .cross_align(Alignment::Center)
        .spacing(GROUP_GAP)
        .child(search)
        .maybe_child(chips);
    let inner = if fit.stacked {
        rect()
            .vertical()
            .width(Size::fill())
            .spacing(CONTROL_GAP)
            .child(line().child(find))
            .child(line().main_align(Alignment::End).child(controls))
    } else {
        rect().width(Size::fill()).child(
            line()
                .main_align(Alignment::SpaceBetween)
                .child(find)
                .child(controls),
        )
    };

    rect()
        .vertical()
        .width(Size::fill())
        .on_sized(track_width(width))
        .child(inner)
        .into_element()
}

/// Where installs go; the content pages' scope chip carries the same kind of fact
fn target_chip(verb: &'static str, name: &str) -> impl IntoElement {
    rect()
        .horizontal()
        .height(Size::px(TARGET_CHIP_H))
        .max_width(Size::px(240.))
        .padding(Gaps::new_symmetric(0., 10.))
        .cross_align(Alignment::Center)
        .spacing(6.)
        .corner_radius(rounded(999.))
        .background(colors::component_bg())
        .border(border_all_color(1., colors::component_border()))
        .child(
            Icon::new(IconType::Download01)
                .size(14.)
                .color(colors::fg_secondary()),
        )
        .child(
            label()
                .text(format!("{verb} {name}"))
                .font_size(text_size(12.))
                .font_weight(FontWeight::MEDIUM)
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .color(colors::fg_primary()),
        )
}

/// No results: the content pages' empty surface, its message about 30% down
fn empty_panel() -> Element {
    result_surface(
        rect()
            .vertical()
            .width(Size::fill())
            .cross_align(Alignment::Center)
            .spacing(8.)
            .child(
                Icon::new(IconType::SearchMd)
                    .size(28.)
                    .color(colors::fg_secondary()),
            )
            .child(
                label()
                    .text(t("oneclient.browser.index.no_results"))
                    .text_align(TextAlign::Center)
                    .font_size(text_size(14.))
                    .font_weight(FontWeight::SEMI_BOLD)
                    .color(colors::fg_primary()),
            )
            .into_element(),
    )
}

/// The content pages' empty surface with `message` about 30% down
fn result_surface(message: Element) -> Element {
    rect()
        .vertical()
        .content(Content::Flex)
        .width(Size::fill())
        .height(Size::fill())
        .padding(Gaps::new_all(32.))
        .corner_radius(rounded(ROW_RADIUS))
        .background(colors::card())
        .border(border_all_color(1., colors::component_border()).alignment(BorderAlignment::Inner))
        .child(rect().width(Size::fill()).height(Size::flex(3.)))
        .child(message)
        .child(rect().width(Size::fill()).height(Size::flex(7.)))
        .into_element()
}

/// The categories as a second rail: the rail's width, rows and heading, scrolling on its own with
/// its thumb in the gap beside it, like the rail's
#[derive(PartialEq)]
struct CategoryRail {
    categories: Vec<String>,
    selected: State<Vec<String>>,
    width: f32,
    /// The gap to the results, which holds the scrollbar
    gap: f32,
    row_height: f32,
    row_gap: f32,
}

impl Component for CategoryRail {
    fn render(&self) -> impl IntoElement {
        let mut selected = self.selected;
        let none_selected = selected.read().is_empty();
        let row_height = self.row_height;

        let all = RailCategory {
            name: t("oneclient.clusters.page.filter.all").to_string(),
            selected: none_selected,
            check: false,
            height: row_height,
            on_press: (move |()| selected.set(Vec::new())).into(),
            key: DiffKey::None,
        }
        .key("all")
        .into_element();
        let rows = self.categories.iter().map(move |category| {
            let is_selected = selected.read().contains(category);
            let toggled = category.clone();
            RailCategory {
                name: capitalize(category),
                selected: is_selected,
                check: true,
                height: row_height,
                on_press: (move |()| {
                    let mut current = selected.read().clone();
                    if let Some(i) = current.iter().position(|c| *c == toggled) {
                        current.remove(i);
                    } else {
                        current.push(toggled.clone());
                    }
                    selected.set(current);
                })
                .into(),
                key: DiffKey::None,
            }
            .key(category.clone())
            .into_element()
        });

        let list = rect()
            .vertical()
            .width(Size::fill())
            .spacing(self.row_gap)
            .child(all)
            .children(rows);
        let scroll = ScrollArea::new()
            .width(Size::fill())
            .height(Size::flex(1.))
            .padding(Gaps::new(0., self.gap, 0., 0.));
        // Glass dialogs have no solid surface to fade into
        let scroll = if colors::active_theme().liquid_glass {
            scroll
        } else {
            scroll
                .bottom_fade(colors::page_elevated())
                .bottom_fade_height(crate::view::app::settings::SCROLL_FADE_HEIGHT)
        };

        rect()
            .vertical()
            .content(Content::Flex)
            .width(Size::px(self.width + self.gap))
            .height(Size::fill())
            .spacing(GROUP_GAP)
            .child(
                // As tall as the toolbar, so the heading sits on the search's centre line
                rect()
                    .width(Size::fill())
                    .height(Size::px(CONTROL_H))
                    .padding(Gaps::new(0., self.gap, 0., RAIL_INSET))
                    .main_align(Alignment::Center)
                    .child(
                        label()
                            .text(t("oneclient.browser.index.categories"))
                            .font_size(text_size(11.))
                            .font_weight(FontWeight::SEMI_BOLD)
                            .color(ClusterShell::workspace_rail_ink()),
                    ),
            )
            .child(scroll.children([list.into_element()]))
    }
}

#[derive(PartialEq)]
struct RailCategory {
    name: String,
    selected: bool,
    /// Categories combine, so each picked one shows a check; "All" doesn't need one
    check: bool,
    height: f32,
    on_press: EventHandler<()>,
    key: DiffKey,
}

impl KeyExt for RailCategory {
    fn key_mut(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for RailCategory {
    fn render(&self) -> impl IntoElement {
        let mut hover = use_state(|| false);
        let id = use_a11y();
        let focus = use_focus(id);
        let keyboard = focus() == Focus::Keyboard;
        let selected = self.selected;
        let ink = if selected || hover() || keyboard {
            colors::fg_primary()
        } else {
            ClusterShell::workspace_rail_ink()
        };
        let on_press = self.on_press.clone();

        rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::px(self.height))
            .padding(Gaps::new_symmetric(0., RAIL_INSET))
            .spacing(10.)
            .cross_align(Alignment::Center)
            .corner_radius(rounded(8.))
            .background(if selected {
                colors::brand().with_a(if colors::is_light() { 34 } else { 67 })
            } else if hover() {
                colors::ghost_overlay_hover()
            } else {
                Color::TRANSPARENT
            })
            .maybe(keyboard, |el| {
                el.border(border_all_color(1., colors::fg_primary()))
            })
            .a11y_id(id)
            .a11y_role(AccessibilityRole::Button)
            .a11y_focusable(true)
            .a11y_alt(self.name.clone())
            .cursor(CursorIcon::Pointer)
            .on_pointer_enter(move |_| hover.set_if_modified(true))
            .on_pointer_leave(move |_| hover.set_if_modified(false))
            .on_press(move |_| on_press.call(()))
            .child(
                label()
                    .text(self.name.clone())
                    .width(Size::flex(1.))
                    .max_lines(1)
                    .text_overflow(TextOverflow::Ellipsis)
                    .font_size(text_size(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .color(ink),
            )
            .maybe_child(
                (selected && self.check).then(|| Icon::new(IconType::Check).size(14.).color(ink)),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_toolbar_keeps_one_line_while_every_control_fits() {
        // search 160, chips 260 and a 520 pickers group
        let one_line = SEARCH_MIN_W + 260. + GROUP_GAP + GROUP_GAP + 520. + ONE_LINE_SLACK;
        let fit = toolbar_fit(one_line, 260., 520., 400.);
        assert!(!fit.stacked && fit.provider_labels);
        assert_eq!(fit.search_w, SEARCH_MIN_W);
        // Extra room widens the search up to the content pages' maximum
        assert_eq!(toolbar_fit(one_line + 60., 260., 520., 400.).search_w, 220.);
        assert_eq!(
            toolbar_fit(one_line + 400., 260., 520., 400.).search_w,
            SEARCH_MAX_W
        );
    }

    #[test]
    fn the_provider_names_go_before_the_toolbar_wraps() {
        // 972 is one line with names; with a 400 icon group 852 still fits on one line
        let fit = toolbar_fit(900., 260., 520., 400.);
        assert!(!fit.stacked && !fit.provider_labels);
        assert_eq!(fit.search_w, SEARCH_MIN_W + 48.);
        let fit = toolbar_fit(852., 260., 520., 400.);
        assert!(!fit.stacked && !fit.provider_labels);
        assert_eq!(fit.search_w, SEARCH_MIN_W);
    }

    #[test]
    fn a_narrow_toolbar_stacks_and_drops_the_provider_names_last() {
        let fit = toolbar_fit(780., 260., 520., 400.);
        assert!(fit.stacked && fit.provider_labels);
        assert_eq!(fit.search_w, SEARCH_MAX_W);
        // A pane narrower than the 800x600 one: the search takes what the chips leave and the
        // switch shows icons
        let fit = toolbar_fit(508., 260., 520., 400.);
        assert!(fit.stacked && !fit.provider_labels);
        assert_eq!(fit.search_w, 236.);
    }

    #[test]
    fn the_pickers_are_estimated_until_measured_with_names() {
        assert_eq!(labelled_controls_w(0., 2), PROVIDER_LABELS_W + 280.);
        assert_eq!(labelled_controls_w(498., 2), 498.);
    }

    #[test]
    fn the_result_count_only_shows_with_room_beside_the_pager() {
        assert!(!summary_fits(508., 3));
        assert!(summary_fits(1000., 5));
    }
}

#[cfg(test)]
mod toolbar_tests {
    use super::*;
    use freya_testing::TestingRunner;
    use std::time::Duration;

    /// The side picker only exists on Modrinth; it must not push the switch that toggles it
    #[test]
    fn switching_provider_leaves_the_switch_and_sort_in_place() {
        fn app() -> impl IntoElement {
            crate::hooks::use_provide_station(|| {
                let mut app = crate::state::AppState::default();
                app.settings.settings.animations_enabled = false;
                app
            });
            let provider = use_consume::<State<ProviderId>>();
            let side = use_state(|| None::<SearchSide>);
            let parts = Toolbar {
                placeholder: "Search mods".into(),
                target: None,
                override_button: None,
                provider,
                curseforge_blocked: false,
                query: use_state(String::new),
                sort: use_state(|| SearchSort::Relevance),
                loader: None,
                side: (provider() == ProviderId::Modrinth).then_some(side),
                side_slot: true,
            };
            toolbar(
                parts,
                use_state(|| 0.),
                use_state(|| 0.),
                use_state(|| 0.),
                use_state(|| 0.),
            )
        }
        for width in [1400., 700.] {
            let (mut runner, provider) = TestingRunner::new(
                app,
                (width, 300.).into(),
                |runner| runner.provide_root_context(|| State::create(ProviderId::Modrinth)),
                1.,
            );
            let area = |runner: &TestingRunner, text: &str| {
                runner
                    .find(|node, element| {
                        Label::try_downcast(element)
                            .filter(|label| label.text.as_ref() == text)
                            .map(|_| node.layout().area)
                    })
                    .unwrap_or_else(|| panic!("no {text} at {width}"))
            };
            runner.poll_n(Duration::ZERO, 8);
            let names = ["CurseForge", "Relevance"];
            let before = names.map(|name| area(&runner, name));
            let target = before[0].center();
            runner.click_cursor((f64::from(target.x), f64::from(target.y)));
            runner.poll_n(Duration::ZERO, 8);
            runner.run_in(|| assert_eq!(*provider.peek(), ProviderId::CurseForge));
            let after = names.map(|name| area(&runner, name));
            for (a, b) in before.iter().zip(&after) {
                // The label's weight changes when selected, so compare centres
                assert!((a.center().x - b.center().x).abs() < 3., "{a:?} {b:?}");
                assert_eq!(a.min_y(), b.min_y());
            }
        }
    }
}
