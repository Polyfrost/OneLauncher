use super::*;

use crate::components::ScrollArea;
use crate::i18n::t;
use crate::theme::{colors, text_size};
use crate::ui::border_all_color;
use crate::utils::capitalize;

#[derive(PartialEq)]
pub(super) struct CategorySidebar {
    pub(super) categories: Vec<String>,
    pub(super) selected: State<Vec<String>>,
    pub(super) fade_surface: Color,
}

impl Component for CategorySidebar {
    fn render(&self) -> impl IntoElement {
        let selected = self.selected;
        // Bottom inset lets the last category scroll clear of the window edge the list now runs to
        let list = rect()
            .vertical()
            .width(Size::fill())
            .spacing(1.)
            .padding(Gaps::new(0., 0., SCROLL_END_INSET, 0.))
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
