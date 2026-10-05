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
        let mut flipped = use_state(HashSet::<String>::new);

        let Some(choices) = snapshot.bundle_choices.clone() else {
            return rect().into_element();
        };

        let chosen: HashSet<String> = choices
            .bundles
            .iter()
            .filter(|(name, held)| *held != flipped.read().contains(name))
            .map(|(name, _)| name.clone())
            .collect();

        let mut list = rect().vertical().width(Size::fill()).spacing(6.);
        for (name, _) in &choices.bundles {
            let key = name.clone();
            let on_toggle: EventHandler<()> = (move |()| {
                let mut next = flipped.read().clone();
                if !next.remove(&key) {
                    next.insert(key.clone());
                }
                flipped.set(next);
            })
            .into();
            list = list.child(
                rect()
                    .key(name.clone())
                    .horizontal()
                    .width(Size::fill())
                    .cross_align(Alignment::Center)
                    .padding(Gaps::new_symmetric(8., 10.))
                    .corner_radius(CornerRadius::new_all(8.))
                    .background(colors::component_bg())
                    .child(
                        label()
                            .text(name.clone())
                            .font_size(13.)
                            .max_lines(1)
                            .width(Size::flex(1.0))
                            .color(colors::fg_primary()),
                    )
                    .child(toggle_controlled(chosen.contains(name), on_toggle)),
            );
        }

        let later = dispatch.clone();
        let close = dispatch.clone();

        OverlayPopup::new()
            .on_close(move |_| {
                flipped.set(HashSet::new());
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
                                        "{} was set up with optional bundles you were never asked about. Keep the ones you want, the rest are removed.",
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
                                                flipped.set(HashSet::new());
                                                later.close_bundle_choices(None);
                                            })
                                            .text("Ask me later"),
                                    )
                                    .child(
                                        Button::new()
                                            .primary()
                                            .on_press(move |_| {
                                                flipped.set(HashSet::new());
                                                dispatch.close_bundle_choices(Some(chosen.clone()));
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
