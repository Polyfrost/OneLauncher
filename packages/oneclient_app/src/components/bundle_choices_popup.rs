use std::collections::HashSet;

use freya::prelude::*;

use crate::components::{Button, Icon, IconType, OverlayPopup, toggle_controlled};
use crate::hooks::{use_dispatch, use_notifications_snapshot};
use crate::theme::colors;
use crate::ui::border_all_color;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);
const DIALOG_W: f32 = 460.;
const LIST_MAX_H: f32 = 306.;

#[derive(PartialEq)]
pub struct BundleChoicesPopup;

impl Component for BundleChoicesPopup {
    fn render(&self) -> impl IntoElement {
        let snapshot = use_notifications_snapshot();
        let dispatch = use_dispatch();
        let mut chosen = use_state(HashSet::<String>::new);

        let Some(choices) = snapshot.bundle_choices.clone() else {
            return rect().into_element();
        };

        let mut list = rect().vertical().width(Size::fill()).spacing(6.);
        for (name, title) in &choices.bundles {
            let key = name.clone();
            let on_toggle: EventHandler<()> = (move |()| {
                let mut next = chosen.read().clone();
                if !next.remove(&key) {
                    next.insert(key.clone());
                }
                chosen.set(next);
            })
            .into();
            list = list.child(
                rect()
                    .key(name.clone())
                    .horizontal()
                    .width(Size::fill())
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .padding(Gaps::new_symmetric(8., 10.))
                    .corner_radius(CornerRadius::new_all(8.))
                    .background(colors::component_bg())
                    .child(
                        label()
                            .text(title.clone())
                            .font_size(13.)
                            .max_lines(1)
                            .width(Size::flex(1.0))
                            .color(colors::fg_primary()),
                    )
                    .child(toggle_controlled(
                        choices.held.contains(name) != chosen.read().contains(name),
                        on_toggle,
                    )),
            );
        }

        let names: Vec<String> = choices
            .bundles
            .iter()
            .map(|(name, _)| name.clone())
            .collect();
        let held = choices.held.clone();
        let later = dispatch.clone();
        let close = dispatch.clone();

        OverlayPopup::new()
            .on_close(move |_| {
                chosen.set(HashSet::new());
                close.close_bundle_choices(None);
            })
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(DIALOG_W))
                            .height(Size::Inner)
                            .max_width(Size::window_percent(95.))
                            .max_height(Size::window_percent(85.))
                            .padding(Gaps::new_all(22.))
                            .spacing(14.)
                            .corner_radius(CornerRadius::new_all(16.))
                            .background(CARD_BG)
                            .border(border_all_color(1., colors::component_border()))
                            .child(
                                label()
                                    .text("Choose your bundles")
                                    .font_size(17.)
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .color(colors::fg_primary()),
                            )
                            .child(
                                label()
                                    .text(format!(
                                        "{} has optional bundles you have not chosen yet. Turn on the ones you want, turning one off removes its mods.",
                                        choices.cluster_name
                                    ))
                                    .font_size(12.5)
                                    .max_lines(4)
                                    .color(colors::fg_secondary()),
                            )
                            .child(
                                ScrollView::new()
                                    .width(Size::fill())
                                    .height(Size::Inner)
                                    .max_height(Size::px(LIST_MAX_H))
                                    .child(list),
                            )
                            .child(
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .main_align(Alignment::End)
                                    .spacing(8.)
                                    .child(
                                        Button::new()
                                            .ghost()
                                            .on_press(move |_| {
                                                chosen.set(HashSet::new());
                                                later.close_bundle_choices(None);
                                            })
                                            .text("Ask me later"),
                                    )
                                    .child(
                                        Button::new()
                                            .primary()
                                            .on_press(move |_| {
                                                let flipped = chosen.read().clone();
                                                let picked = names
                                                    .iter()
                                                    .filter(|name| {
                                                        held.contains(*name)
                                                            != flipped.contains(*name)
                                                    })
                                                    .cloned()
                                                    .collect();
                                                chosen.set(HashSet::new());
                                                dispatch.close_bundle_choices(Some(picked));
                                            })
                                            .child(Icon::new(IconType::Check).size(15.))
                                            .text("Confirm & launch"),
                                    ),
                            ),
                    ),
            )
            .into_element()
    }
}
