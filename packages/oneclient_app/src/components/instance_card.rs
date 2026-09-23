use freya::prelude::*;
use oneclient_core::clusters::Cluster;

use crate::components::{ART_PREVIEW_EDGE, DynamicArt};
use crate::theme::colors;
use crate::ui::border_all_color;

const CARD_HEIGHT_PX: f32 = 240.;

pub struct InstanceCard {
    id: i64,
    title: String,
    tags: Vec<String>,
    cover: Option<std::path::PathBuf>,
    art: DynamicArt,
    selected: bool,
    on_press: EventHandler<Event<PressEventData>>,
}

impl InstanceCard {
    pub fn new(
        cluster: &Cluster,
        selected: bool,
        on_press: impl Into<EventHandler<Event<PressEventData>>>,
    ) -> Self {
        Self {
            id: cluster.id,
            title: cluster.name.clone(),
            tags: cluster.tags.clone(),
            cover: cluster.cover_file(),
            art: DynamicArt::for_cluster(cluster).max_edge(ART_PREVIEW_EDGE),
            selected,
            on_press: on_press.into(),
        }
    }
}

impl PartialEq for InstanceCard {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.title == other.title
            && self.tags == other.tags
            && self.cover == other.cover
            && self.selected == other.selected
    }
}

impl Component for InstanceCard {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);

        let selected = self.selected;
        let hovered = *hovering.read();
        let focused = focus().is_focused();
        let on_press = self.on_press.clone();

        let opacity = if selected || hovered || focused {
            if selected { 1.0 } else { 0.85 }
        } else {
            0.6
        };

        let border_color = if selected || focused {
            colors::brand()
        } else if hovered {
            colors::component_border_hover()
        } else {
            colors::component_border()
        };

        rect()
            .key(self.id)
            .width(Size::flex(1.0))
            .height(Size::px(CARD_HEIGHT_PX))
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .on_press(move |e| on_press.call(e))
            .on_pointer_enter(move |_| {
                *hovering.write() = true;
            })
            .on_pointer_leave(move |_| {
                *hovering.write() = false;
            })
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .overflow(Overflow::Clip)
                    .corner_radius(CornerRadius::new_all(12.))
                    .opacity(opacity)
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::fill())
                            .position(Position::new_absolute())
                            .child(self.art.clone()),
                    )
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::fill())
                            .padding(Gaps::new_symmetric(12., 16.))
                            .main_align(Alignment::SpaceBetween)
                            .corner_radius(CornerRadius::new_all(12.))
                            .cross_align(Alignment::Start)
                            .border(
                                border_all_color(1., border_color)
                                    .alignment(BorderAlignment::Inner),
                            )
                            .layer(Layer::Relative(3))
                            .background(
                                LinearGradient::new()
                                    .angle(0.)
                                    .stop((Color::from_af32rgb(0.8, 0, 0, 0), 0.))
                                    .stop((Color::from_af32rgb(0.3, 0, 0, 0), 20.))
                                    .stop((Color::from_af32rgb(0.3, 0, 0, 0), 60.))
                                    .stop((Color::from_af32rgb(0.8, 0, 0, 0), 100.)),
                            )
                            .child(
                                rect()
                                    .horizontal()
                                    .spacing(8.)
                                    .children(self.tags.iter().map(|tag| {
                                        rect()
                                            .padding(Gaps::new_symmetric(4., 8.))
                                            .corner_radius(CornerRadius::new_all(999.))
                                            .background(colors::fg_primary())
                                            .child(
                                                label()
                                                    .text(tag.clone())
                                                    .font_size(12.)
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .color(colors::brand()),
                                            )
                                            .into_element()
                                    })),
                            )
                            .child(
                                label()
                                    .text(self.title.clone())
                                    .font_size(28.)
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .color(colors::fg_primary()),
                            ),
                    ),
            )
    }
}
