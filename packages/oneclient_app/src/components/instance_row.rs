use freya::prelude::*;
use oneclient_core::clusters::Cluster;

use crate::components::cluster_menu::open_menu_at;
use crate::components::{ART_PREVIEW_EDGE, ClusterContextMenu, ClusterMenuTarget, DynamicArt};
use crate::theme::colors;
use crate::ui::{border_all_color, last_played_label};
use crate::utils::{GridSelection, ReleaseLine, line_art_key, line_title};

const ROW_HEIGHT_PX: f32 = 56.;
const THUMB_PX: f32 = 40.;

pub struct InstanceRow {
    key: GridSelection,
    title: String,
    subtitle: String,
    played: String,
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
        Self {
            key: GridSelection::Instance(cluster.id),
            title: cluster.name.clone(),
            subtitle: format!("{} \u{b7} {}", cluster.mc_version, cluster.mc_loader),
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
        caption: String,
        selected: bool,
        on_press: impl Into<EventHandler<Event<PressEventData>>>,
    ) -> Self {
        Self {
            key: GridSelection::Line(line),
            title: line_title(line, clusters),
            subtitle: caption,
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
            && self.art == other.art
            && self.selected == other.selected
            && self.menu == other.menu
    }
}

impl Component for InstanceRow {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);
        let menu_at = use_state(|| None::<(f32, f32)>);

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);

        let selected = self.selected;
        let hovered = *hovering.read();
        let focused = focus().is_focused();
        let on_press = self.on_press.clone();

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

        rect()
            .key(self.key)
            .horizontal()
            .width(Size::fill())
            .height(Size::px(ROW_HEIGHT_PX))
            .padding(Gaps::new(0., 14., 0., 8.))
            .spacing(14.)
            .cross_align(Alignment::Center)
            .content(Content::Flex)
            .corner_radius(CornerRadius::new_all(12.))
            .background(background)
            .border(border_all_color(1., border_color).alignment(BorderAlignment::Inner))
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .on_press(move |e| on_press.call(e))
            .maybe(self.menu.is_some(), |el| el.on_secondary_down(open_menu_at(menu_at)))
            .on_pointer_enter(move |_| hovering.set(true))
            .on_pointer_leave(move |_| hovering.set(false))
            .maybe_child(self.menu.clone().map(|target| {
                ClusterContextMenu {
                    target,
                    position: menu_at,
                }
                .into_element()
            }))
            .child(
                rect()
                    .width(Size::px(THUMB_PX))
                    .height(Size::px(THUMB_PX))
                    .corner_radius(CornerRadius::new_all(8.))
                    .overflow(Overflow::Clip)
                    .child(self.art.clone()),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(3.)
                    .child(
                        label()
                            .text(self.title.clone())
                            .font_size(14.)
                            .font_weight(FontWeight::MEDIUM)
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text(self.subtitle.clone())
                            .font_size(12.)
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .color(colors::fg_secondary()),
                    ),
            )
            .child(
                label()
                    .text(self.played.clone())
                    .font_size(12.)
                    .max_lines(1)
                    .color(colors::fg_secondary()),
            )
    }
}
