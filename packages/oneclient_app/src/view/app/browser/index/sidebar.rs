use super::*;

use crate::components::ScrollArea;
use crate::theme::colors;
use crate::utils::capitalize;

#[derive(PartialEq)]
pub(super) struct CategorySidebar {
    pub(super) categories: Vec<String>,
    pub(super) selected: State<Vec<String>>,
}

impl Component for CategorySidebar {
    fn render(&self) -> impl IntoElement {
        let selected = self.selected;
        let list = rect().vertical().width(Size::fill()).spacing(2.).children(
            self.categories.clone().into_iter().map(move |cat| {
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
            }),
        );

        rect()
            .vertical()
            .width(Size::px(190.))
            .height(Size::fill())
            .spacing(8.)
            .child(
                label()
                    .text("CATEGORIES")
                    .font_size(11.)
                    .font_weight(FontWeight::LIGHT)
                    .color(colors::fg_secondary()),
            )
            .child(
                ScrollArea::new()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .children([list.into_element()]),
            )
    }
}

#[derive(PartialEq)]
struct CategoryRow {
    name: String,
    selected: bool,
    on_toggle: EventHandler<()>,
    key: DiffKey,
}

impl KeyExt for CategoryRow {
    fn write_key(&mut self) -> &mut DiffKey {
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
            .width(Size::fill())
            .padding(Gaps::new_symmetric(3., 0.))
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .on_pointer_enter(move |_| {
                hovering.set(true);
                Cursor::set(CursorIcon::Pointer);
            })
            .on_pointer_leave(move |_| {
                hovering.set(false);
                Cursor::set(CursorIcon::default());
            })
            .on_all_press(move |_| on_toggle.call(()))
            .child(
                label()
                    .text(capitalize(&self.name))
                    .font_size(13.)
                    .max_lines(1)
                    .font_weight(if selected {
                        FontWeight::SEMI_BOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .color(color),
            )
    }
}
