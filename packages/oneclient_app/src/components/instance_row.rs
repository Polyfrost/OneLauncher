use freya::prelude::*;
use oneclient_core::clusters::Cluster;

use crate::components::{ART_PREVIEW_EDGE, DynamicArt};
use crate::theme::colors;
use crate::ui::{border_all_color, last_played_label};

const ROW_HEIGHT_PX: f32 = 56.;
const THUMB_PX: f32 = 40.;

pub struct InstanceRow {
    id: i64,
    title: String,
    subtitle: String,
    played: String,
    art: DynamicArt,
    selected: bool,
    on_press: EventHandler<Event<PressEventData>>,
}

impl InstanceRow {
    pub fn new(
        cluster: &Cluster,
        selected: bool,
        on_press: impl Into<EventHandler<Event<PressEventData>>>,
    ) -> Self {
        Self {
            id: cluster.id,
            title: cluster.name.clone(),
            subtitle: format!("{} \u{b7} {}", cluster.mc_version, cluster.mc_loader),
            played: last_played_label(cluster.last_played),
            art: DynamicArt::for_cluster(cluster).max_edge(ART_PREVIEW_EDGE),
            selected,
            on_press: on_press.into(),
        }
    }
}

impl PartialEq for InstanceRow {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.title == other.title
            && self.subtitle == other.subtitle
            && self.played == other.played
            && self.art == other.art
            && self.selected == other.selected
    }
}

impl Component for InstanceRow {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);

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
            .key(self.id)
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
            .on_pointer_enter(move |_| hovering.set(true))
            .on_pointer_leave(move |_| hovering.set(false))
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
