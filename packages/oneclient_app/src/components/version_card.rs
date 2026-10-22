use freya::prelude::*;
use oneclient_core::clusters::Cluster;

use crate::components::cluster_menu::open_menu_at;
use crate::components::{ART_PREVIEW_EDGE, ClusterContextMenu, ClusterMenuTarget, DynamicArt};
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::utils::{GridSelection, ReleaseLine, line_art_key, line_title};

const CARD_HEIGHT_PX: f32 = 150.;

pub struct VersionCard {
    pub key: GridSelection,
    pub art: DynamicArt,
    pub title: String,
    pub caption: String,
    pub count: usize,
    pub selected: bool,
    pub on_press: EventHandler<Event<PressEventData>>,
    pub menu: Option<ClusterMenuTarget>,
}

impl VersionCard {
    pub fn menu(mut self, target: Option<ClusterMenuTarget>) -> Self {
        self.menu = target;
        self
    }

    pub fn new(
        line: ReleaseLine,
        clusters: &[Cluster],
        caption: String,
        selected: bool,
        on_press: impl Into<EventHandler<Event<PressEventData>>>,
    ) -> Self {
        Self {
            key: GridSelection::Line(line),
            art: DynamicArt::for_version(line.major, line_art_key(line, clusters), None)
                .max_edge(ART_PREVIEW_EDGE),
            title: line_title(line, clusters),
            caption,
            count: clusters.len(),
            selected,
            on_press: on_press.into(),
            menu: None,
        }
    }

    pub fn for_instance(
        cluster: &Cluster,
        selected: bool,
        on_press: impl Into<EventHandler<Event<PressEventData>>>,
    ) -> Self {
        Self {
            key: GridSelection::Instance(cluster.id),
            art: DynamicArt::for_cluster(cluster).max_edge(ART_PREVIEW_EDGE),
            title: cluster.name.clone(),
            caption: format!("{} \u{b7} {}", cluster.mc_version, cluster.mc_loader),
            count: 1,
            selected,
            on_press: on_press.into(),
            menu: None,
        }
    }
}

impl PartialEq for VersionCard {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.art == other.art
            && self.title == other.title
            && self.caption == other.caption
            && self.count == other.count
            && self.selected == other.selected
            && self.menu == other.menu
    }
}

impl Component for VersionCard {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);
        let menu_at = use_state(|| None::<(f32, f32)>);

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);

        let selected = self.selected;
        let hovered = *hovering.read();
        let focused = focus().is_focused();
        let on_press = self.on_press.clone();

        let art_opacity = if selected {
            1.0
        } else if hovered || focused {
            0.85
        } else {
            0.6
        };

        let border = if selected || focused {
            border_all_color(2., colors::brand())
        } else if hovered {
            border_all_color(1., colors::component_border_hover())
        } else {
            border_all_color(1., colors::component_border())
        };

        rect()
            .key(self.key)
            .width(Size::fill())
            .height(Size::px(CARD_HEIGHT_PX))
            .corner_radius(CornerRadius::new_all(16.))
            .overflow(Overflow::Clip)
            .background(colors::page_elevated())
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
                    .width(Size::fill())
                    .height(Size::fill())
                    .position(Position::new_absolute())
                    .opacity(art_opacity)
                    .child(self.art.clone()),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .padding(12.)
                    .spacing(5.)
                    .main_align(Alignment::End)
                    .corner_radius(CornerRadius::new_all(16.))
                    .border(border.alignment(BorderAlignment::Inner))
                    .layer(Layer::Relative(3))
                    .background(
                        LinearGradient::new()
                            .angle(0.)
                            .stop((Color::from_af32rgb(0.1, 11, 16, 19), 0.))
                            .stop((Color::from_af32rgb(0.6, 11, 16, 19), 55.))
                            .stop((Color::from_af32rgb(0.95, 11, 16, 19), 100.)),
                    )
                    .child(
                        label()
                            .text(self.title.clone())
                            .font_size(22.)
                            .font_weight(FontWeight::SEMI_BOLD)
                            .color(Color::WHITE),
                    )
                    .child(
                        label()
                            .text(self.caption.clone())
                            .font_size(11.)
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .color(colors::fg_secondary()),
                    ),
            )
            .maybe_child((self.count > 1).then(|| {
                rect()
                    .position(Position::new_absolute().top(8.).right(8.))
                    .layer(Layer::Relative(4))
                    .padding(Gaps::new_symmetric(4., 8.))
                    .corner_radius(CornerRadius::new_all(6.))
                    .background(Color::from_af32rgb(0.66, 11, 16, 19))
                    .child(
                        label()
                            .text(format!("{} instances", self.count))
                            .font_size(11.)
                            .font_weight(FontWeight::MEDIUM)
                            .color(colors::fg_primary()),
                    )
                    .into_element()
            }))
    }
}
