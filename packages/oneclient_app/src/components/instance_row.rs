use freya::prelude::*;
use oneclient_core::clusters::Cluster;

use crate::components::cluster_menu::{
    InstanceOverviewButton, InstanceOverviewSurface, OVERVIEW_BUTTON_PX, use_instance_menu_trigger,
};
use crate::components::instance_play::INSTANCE_PLAY_PX;
use crate::components::{ART_PREVIEW_EDGE, ClusterContextMenu, ClusterMenuTarget, DynamicArt};
use crate::theme::{colors, rounded, text_size};
use crate::ui::{border_all_color, last_played_label};
use crate::utils::{GridSelection, ReleaseLine, line_art_key, line_title};

const ROW_HEIGHT_PX: f32 = 56.;
const THUMB_PX: f32 = 40.;
/// Fixed, right-aligned column for the last played label, wide enough for "Yesterday"
const PLAYED_PX: f32 = 76.;
/// Gap between the menu button and Play
const ACTION_GAP_PX: f32 = 6.;
const ACTIONS_PX: f32 = OVERVIEW_BUTTON_PX + ACTION_GAP_PX + INSTANCE_PLAY_PX;
/// Rows narrower than this fold the last played label into the subtitle
const ROOMY_MIN_WIDTH_PX: f32 = 520.;

pub struct InstanceRow {
    key: GridSelection,
    title: String,
    subtitle: String,
    played: String,
    play_id: i64,
    uses_bundles: bool,
    art: DynamicArt,
    selected: bool,
    on_press: EventHandler<Event<PressEventData>>,
    menu: Option<ClusterMenuTarget>,
}

impl InstanceRow {
    pub fn menu(mut self, target: Option<ClusterMenuTarget>) -> Self {
        self.menu = target;
        self
    }

    pub fn new(
        cluster: &Cluster,
        selected: bool,
        on_press: impl Into<EventHandler<Event<PressEventData>>>,
    ) -> Self {
        let branded = crate::utils::builtin_oneclient_title(cluster);
        let subtitle = if branded.is_some() {
            cluster.mc_loader.to_string()
        } else {
            super::VersionCard::instance_caption(cluster)
        };
        Self {
            key: GridSelection::Instance(cluster.id),
            play_id: cluster.id,
            uses_bundles: cluster.uses_bundles(),
            title: branded.unwrap_or_else(|| cluster.name.clone()),
            subtitle,
            played: last_played_label(cluster.last_played),
            art: DynamicArt::for_cluster(cluster).max_edge(ART_PREVIEW_EDGE),
            selected,
            on_press: on_press.into(),
            menu: None,
        }
    }

    pub fn for_line(
        line: ReleaseLine,
        clusters: &[Cluster],
        target: &Cluster,
        selected: bool,
        on_press: impl Into<EventHandler<Event<PressEventData>>>,
    ) -> Self {
        Self {
            key: GridSelection::Line(line),
            play_id: target.id,
            uses_bundles: target.uses_bundles(),
            title: line_title(line, clusters),
            subtitle: if clusters.len() > 1 {
                format!(
                    "{} \u{b7} {}",
                    target.mc_loader,
                    crate::i18n::tf("oneclient.clusters.page.instance_count", &[&clusters.len()])
                )
            } else {
                target.mc_loader.to_string()
            },
            played: last_played_label(clusters.iter().filter_map(|c| c.last_played).max()),
            art: DynamicArt::for_version(line.major, line_art_key(line, clusters), None)
                .max_edge(ART_PREVIEW_EDGE),
            selected,
            on_press: on_press.into(),
            menu: None,
        }
    }
}

impl PartialEq for InstanceRow {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.title == other.title
            && self.subtitle == other.subtitle
            && self.played == other.played
            && self.play_id == other.play_id
            && self.uses_bundles == other.uses_bundles
            && self.art == other.art
            && self.selected == other.selected
            && self.menu == other.menu
    }
}

impl Component for InstanceRow {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);
        let mut row_width = use_state(|| None::<f32>);
        let menu_at = use_state(|| None);

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);

        let selected = self.selected;
        let hovered = *hovering.read();
        let focused = focus().is_focused();

        let background = if selected {
            colors::brand().with_a(36)
        } else if hovered {
            colors::component_bg_hover()
        } else {
            colors::component_bg()
        };

        let border_color = if selected || focused {
            colors::brand()
        } else if hovered {
            colors::component_border_hover()
        } else {
            colors::component_border()
        };

        // Roomy rows keep the last played label in its own column. Compact rows fold it into
        // the subtitle so the title keeps the room the actions need.
        let roomy = row_width().is_some_and(|width| width >= ROOMY_MIN_WIDTH_PX);
        let has_played = self.played != crate::i18n::t("oneclient.ui.last_played.never");
        let subtitle = if roomy || !has_played {
            self.subtitle.clone()
        } else {
            format!("{} \u{b7} {}", self.subtitle, self.played)
        };

        let row = use_instance_menu_trigger(menu_at, self.menu.is_some(), self.on_press.clone())
            .horizontal()
            .width(Size::fill())
            .height(Size::fill())
            .padding(Gaps::new_symmetric(0., 10.))
            .spacing(14.)
            .cross_align(Alignment::Center)
            .content(Content::Flex)
            .corner_radius(rounded(12.))
            .background(background)
            .border(border_all_color(1., border_color).alignment(BorderAlignment::Inner))
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button);

        // The measuring wrapper never depends on its content's width, so it cannot loop
        rect()
            .key(self.key)
            .width(Size::fill())
            .height(Size::px(ROW_HEIGHT_PX))
            .on_sized(move |e: Event<SizedEventData>| {
                row_width.set_if_modified(Some(e.area.width()));
            })
            .child(
                row.on_pointer_over(move |_| hovering.set_if_modified(true))
                    .on_pointer_move(move |_| hovering.set_if_modified(true))
                    .on_pointer_leave(move |_| hovering.set(false))
                    .child(
                        rect()
                            .width(Size::px(THUMB_PX))
                            .height(Size::px(THUMB_PX))
                            .min_width(Size::px(THUMB_PX))
                            .corner_radius(rounded(8.))
                            .overflow(Overflow::Clip)
                            .child(super::art_zoom::ArtZoom {
                                highlighted: hovered || focus() == Focus::Keyboard,
                                art: self.art.clone().into_element(),
                            }),
                    )
                    .child(
                        rect()
                            .vertical()
                            .width(Size::flex(1.0))
                            .spacing(3.)
                            .child(
                                label()
                                    .text(self.title.clone())
                                    .font_size(text_size(14.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .max_lines(1)
                                    .text_overflow(TextOverflow::Ellipsis)
                                    .color(colors::fg_primary()),
                            )
                            .child(
                                label()
                                    .text(subtitle)
                                    .font_size(text_size(12.))
                                    .max_lines(1)
                                    .text_overflow(TextOverflow::Ellipsis)
                                    .color(colors::fg_secondary()),
                            ),
                    )
                    .maybe(roomy && has_played, |el| {
                        el.child(
                            label()
                                .text(self.played.clone())
                                .width(Size::px(PLAYED_PX))
                                .min_width(Size::px(PLAYED_PX))
                                .text_align(TextAlign::Right)
                                .font_size(text_size(12.))
                                .max_lines(1)
                                .text_overflow(TextOverflow::Ellipsis)
                                .color(colors::fg_secondary()),
                        )
                    })
                    // Reserve the Overview slot so Play lines up even on rows without it.
                    .child(
                        rect()
                            .horizontal()
                            .width(Size::px(ACTIONS_PX))
                            .min_width(Size::px(ACTIONS_PX))
                            .height(Size::px(INSTANCE_PLAY_PX))
                            .spacing(ACTION_GAP_PX)
                            .cross_align(Alignment::Center)
                            .child(match &self.menu {
                                Some(target) => InstanceOverviewButton {
                                    surface: InstanceOverviewSurface::List,
                                    cluster_id: target.cluster_id,
                                    name: self.title.clone(),
                                }
                                .into_element(),
                                None => rect()
                                    .width(Size::px(OVERVIEW_BUTTON_PX))
                                    .height(Size::px(OVERVIEW_BUTTON_PX))
                                    .min_width(Size::px(OVERVIEW_BUTTON_PX))
                                    .into_element(),
                            })
                            .child(super::InstancePlay {
                                cluster_id: self.play_id,
                                uses_bundles: self.uses_bundles,
                                name: self.title.clone(),
                                list: true,
                            }),
                    ),
            )
            .maybe_child(self.menu.clone().map(|target| {
                ClusterContextMenu {
                    target,
                    position: menu_at,
                }
                .into_element()
            }))
    }
}
