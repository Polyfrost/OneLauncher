use super::*;

use oneclient_core::clusters::Cluster;

use crate::components::{
    Button, DynamicArt, Icon, IconType,
};
use crate::theme::colors;
use crate::ui::border_all_color;

pub(super) struct BundleCard {
    pub(super) display_name: String,
    pub(super) art_cluster: Option<Cluster>,
    pub(super) mod_count: usize,
    pub(super) selected: bool,
    pub(super) on_toggle: EventHandler<()>,
    pub(super) on_open: EventHandler<()>,
}

impl PartialEq for BundleCard {
    fn eq(&self, other: &Self) -> bool {
        self.display_name == other.display_name
            && self.mod_count == other.mod_count
            && self.selected == other.selected
            && self.art_cluster.as_ref().map(|c| c.id) == other.art_cluster.as_ref().map(|c| c.id)
    }
}

impl Component for BundleCard {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);

        let selected = self.selected;
        let hovered = *hovering.read();
        let on_toggle = self.on_toggle.clone();
        let on_open = self.on_open.clone();

        let opacity = if selected {
            1.0
        } else if hovered {
            0.9
        } else {
            0.65
        };
        let border_color = if selected {
            colors::brand()
        } else if hovered {
            colors::component_border_hover()
        } else {
            colors::component_border()
        };

        let art: Element = match &self.art_cluster {
            Some(cluster) => DynamicArt::for_cluster(cluster)
                .max_edge(512)
                .into_element(),
            None => rect()
                .width(Size::fill())
                .height(Size::fill())
                .background(colors::component_bg())
                .into_element(),
        };

        let badge_text = if selected {
            format!("{} Mods Selected", self.mod_count)
        } else {
            format!("{} Mods", self.mod_count)
        };

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .corner_radius(CornerRadius::new_all(12.))
            .opacity(opacity)
            .a11y_role(AccessibilityRole::Button)
            .on_pointer_enter(move |_| {
                *hovering.write() = true;
                Cursor::set(CursorIcon::Pointer);
            })
            .on_pointer_leave(move |_| {
                *hovering.write() = false;
                Cursor::set(CursorIcon::default());
            })
            .on_press(move |_| on_toggle.call(()))
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .position(Position::new_absolute())
                    .child(art),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .height(Size::fill())
                    .padding(Gaps::new_symmetric(10., 12.))
                    .main_align(Alignment::SpaceBetween)
                    .corner_radius(CornerRadius::new_all(12.))
                    .border(border_all_color(2., border_color).alignment(BorderAlignment::Inner))
                    .layer(Layer::Relative(3))
                    .background(
                        LinearGradient::new()
                            .angle(0.)
                            .stop((Color::from_af32rgb(0.75, 0, 0, 0), 0.))
                            .stop((Color::from_af32rgb(0.2, 0, 0, 0), 45.))
                            .stop((Color::from_af32rgb(0.6, 0, 0, 0), 100.)),
                    )
                    .child(
                        rect()
                            .horizontal()
                            .width(Size::fill())
                            .main_align(Alignment::End)
                            .child(
                                rect()
                                    .padding(Gaps::new_symmetric(3., 8.))
                                    .corner_radius(CornerRadius::new_all(999.))
                                    .background(CARD_NAME)
                                    .child(
                                        label()
                                            .text(badge_text)
                                            .font_size(11.)
                                            .font_weight(FontWeight::MEDIUM)
                                            .color(colors::brand()),
                                    ),
                            ),
                    )
                    .child(
                        rect()
                            .horizontal()
                            .width(Size::fill())
                            .cross_align(Alignment::Center)
                            .content(Content::Flex)
                            .child(
                                label()
                                    .text(self.display_name.clone())
                                    .width(Size::flex(1.0))
                                    .font_size(20.)
                                    .font_weight(FontWeight::BOLD)
                                    .max_lines(1)
                                    .color(colors::fg_primary()),
                            )
                            .child(
                                Button::new()
                                    .ghost()
                                    .icon()
                                    .small()
                                    .on_press(move |e: Event<PressEventData>| {
                                        e.stop_propagation();
                                        on_open.call(());
                                    })
                                    .child(
                                        Icon::new(IconType::DotsVertical)
                                            .size(18.)
                                            .color(colors::fg_primary()),
                                    ),
                            ),
                    ),
            )
    }
}
