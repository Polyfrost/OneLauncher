use freya::prelude::*;
use oneclient_core::VersionKey;
use oneclient_core::clusters::Cluster;

use crate::components::ClusterLandscapeArt;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::utils::{ReleaseLine, line_art_key, line_title, loader_tags};

const CARD_HEIGHT_PX: f32 = 240.;

pub struct VersionCard {
    pub line: ReleaseLine,
    pub art_key: Option<VersionKey>,
    pub tags: Vec<String>,
    pub title: String,
    pub selected: bool,
    pub on_press: EventHandler<Event<PressEventData>>,
}

impl VersionCard {
    pub fn new(
        line: ReleaseLine,
        clusters: &[Cluster],
        selected: bool,
        on_press: impl Into<EventHandler<Event<PressEventData>>>,
    ) -> Self {
        Self {
            line,
            art_key: line_art_key(line, clusters),
            tags: loader_tags(clusters),
            title: line_title(line, clusters),
            selected,
            on_press: on_press.into(),
        }
    }
}

impl PartialEq for VersionCard {
    fn eq(&self, other: &Self) -> bool {
        self.line == other.line
            && self.art_key == other.art_key
            && self.tags == other.tags
            && self.title == other.title
            && self.selected == other.selected
    }
}

impl Component for VersionCard {
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
            .key(self.line)
            .width(Size::flex(1.0))
            .height(Size::px(CARD_HEIGHT_PX))
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .on_all_press(move |e| on_press.call(e))
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
                            .child(ClusterLandscapeArt::for_version(
                                self.line.major,
                                self.art_key,
                                None,
                                false,
                            )),
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
                                    .text(format!("Version {}", self.title))
                                    .font_size(32.)
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .color(colors::fg_primary()),
                            ),
                    ),
            )
    }
}
