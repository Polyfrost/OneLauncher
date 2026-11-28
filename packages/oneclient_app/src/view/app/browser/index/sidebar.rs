use super::*;

use crate::components::ScrollArea;
use crate::i18n::t;
use crate::theme::{colors, text_size};
use crate::ui::border_all_color;
use crate::utils::capitalize;

const CATEGORY_SCROLLBAR_GUTTER: f32 = 14.;

#[derive(PartialEq)]
pub(super) struct CategorySidebar {
    pub(super) categories: Vec<String>,
    pub(super) selected: State<Vec<String>>,
    pub(super) fade_surface: Color,
}

impl Component for CategorySidebar {
    fn render(&self) -> impl IntoElement {
        let selected = self.selected;
        // A one-press way back to every category, as in the instance's Add Content rail
        let all = {
            let mut selected = selected;
            CategoryRow {
                name: t("oneclient.clusters.page.filter.all").to_string(),
                selected: selected.read().is_empty(),
                on_toggle: (move |()| selected.set(Vec::new())).into(),
                key: DiffKey::None,
            }
            .key("all")
            .into_element()
        };
        // Bottom inset lets the last category scroll clear of the window edge the list now runs to
        let list = rect()
            .vertical()
            .width(Size::fill())
            .spacing(1.)
            .padding(Gaps::new(0., 0., SCROLL_END_INSET, 0.))
            .child(all)
            .children(self.categories.clone().into_iter().map(move |cat| {
                let is_selected = selected.read().contains(&cat);
                let mut selected = selected;
                let cat_for_press = cat.clone();
                let on_toggle: EventHandler<()> = (move |()| {
                    let mut current = selected.read().clone();
                    if let Some(i) = current.iter().position(|c| c == &cat_for_press) {
                        current.remove(i);
                    } else {
                        current.push(cat_for_press.clone());
                    }
                    selected.set(current);
                })
                .into();
                CategoryRow {
                    name: cat.clone(),
                    selected: is_selected,
                    on_toggle,
                    key: DiffKey::None,
                }
                .key(cat)
                .into_element()
            }));

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::flex(1.0))
            .spacing(12.)
            .child(
                // Same height as the results toolbar so the first category lines up with the first card
                rect()
                    .width(Size::fill())
                    .height(Size::px(PICKER_H))
                    .padding(Gaps::new(0., 0., 0., 11.))
                    .cross_align(Alignment::Center)
                    .child(
                        label()
                            .text(t("oneclient.browser.index.categories"))
                            .font_size(text_size(11.))
                            .font_weight(FontWeight::MEDIUM)
                            .letter_spacing(1.4)
                            .color(colors::fg_secondary()),
                    ),
            )
            .child(
                ScrollArea::new()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    // Reserve the thumb's track even when the list fits.
                    .padding(Gaps::new(0., CATEGORY_SCROLLBAR_GUTTER, 0., 0.))
                    .bottom_fade(self.fade_surface)
                    .children([list.into_element()]),
            )
    }
}

#[derive(PartialEq)]
pub(super) struct CategoryRow {
    pub(super) name: String,
    pub(super) selected: bool,
    pub(super) on_toggle: EventHandler<()>,
    pub(super) key: DiffKey,
}

impl KeyExt for CategoryRow {
    fn key_mut(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for CategoryRow {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);

        let selected = self.selected;
        let on_toggle = self.on_toggle.clone();
        let hovered = *hovering.read();
        let focused = focus().is_focused();
        let color = if selected || hovered || focused {
            colors::fg_primary()
        } else {
            colors::fg_secondary()
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(29.))
            .cross_align(Alignment::Center)
            .padding(Gaps::new_symmetric(0., 10.))
            .corner_radius(control_radius(29., 8.))
            .overflow(Overflow::Clip)
            .maybe(selected, |el| {
                el.background(colors::brand().with_a(40))
                    .border(border_all_color(1., colors::brand().with_a(115)))
            })
            .maybe(!selected && hovered, |el| {
                el.background(colors::ghost_overlay())
            })
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .cursor(CursorIcon::Pointer)
            .on_pointer_enter(move |_| hovering.set(true))
            .on_pointer_leave(move |_| hovering.set(false))
            .on_press(move |_| on_toggle.call(()))
            .child(
                label()
                    .text(capitalize(&self.name))
                    .font_size(text_size(12.))
                    .width(Size::fill())
                    .max_lines(1)
                    .text_overflow(TextOverflow::Ellipsis)
                    .font_weight(if selected {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::NORMAL
                    })
                    .color(color),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use freya_testing::TestingRunner;

    use super::*;

    #[test]
    fn category_rows_keep_the_same_gutter_with_and_without_overflow() {
        for count in [1, 30] {
            let (mut test, ()) = TestingRunner::new(
                move || {
                    rect()
                        .vertical()
                        .width(Size::px(CATEGORY_SIDEBAR_W))
                        .height(Size::fill())
                        .child(CategorySidebar {
                            categories: (0..count).map(|i| format!("Category {i}")).collect(),
                            selected: use_state(Vec::new),
                            fade_surface: colors::card(),
                        })
                },
                (800., 180.).into(),
                |_| (),
                1.,
            );
            test.poll_n(Duration::ZERO, 5);

            let row = test
                .find(|node, element| {
                    (element.accessibility().builder.role() == AccessibilityRole::Button)
                        .then(|| node.layout().area)
                })
                .unwrap();
            assert_eq!(row.min_x(), 0.);
            assert_eq!(row.max_x(), CATEGORY_SIDEBAR_W - CATEGORY_SCROLLBAR_GUTTER);
        }
    }
}
