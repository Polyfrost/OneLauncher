use freya::prelude::*;

use crate::components::{Icon, IconType, ScrollArea};
use crate::theme::colors;
use crate::ui::border_all_color;

const OPTION_HEIGHT: f32 = 28.;
const OPTION_SPACING: f32 = 2.;
const MAX_VISIBLE_OPTIONS: usize = 8;

#[derive(Clone, PartialEq)]
pub struct Dropdown {
    selected: String,
    options: Vec<String>,
    on_select: Option<EventHandler<usize>>,
    width: Size,
    height: Size,
    key: DiffKey,
}

impl Dropdown {
    pub fn new(selected: impl Into<String>, options: Vec<String>) -> Self {
        Self {
            selected: selected.into(),
            options,
            on_select: None,
            width: Size::px(72.),
            height: Size::px(24.),
            key: DiffKey::None,
        }
    }

    #[allow(dead_code)]
    pub fn width(mut self, width: impl Into<Size>) -> Self {
        self.width = width.into();
        self
    }

    #[allow(dead_code)]
    pub fn height(mut self, height: impl Into<Size>) -> Self {
        self.height = height.into();
        self
    }

    pub fn on_select(mut self, handler: impl Into<EventHandler<usize>>) -> Self {
        self.on_select = Some(handler.into());
        self
    }
}

impl KeyExt for Dropdown {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for Dropdown {
    fn render(&self) -> impl IntoElement {
        let mut open = use_state(|| false);
        let mut hovering = use_state(|| false);

        let mut button_area = use_state(|| None::<Area>);
        let mut list_size = use_state(|| None::<Size2D>);

        let selected = self.selected.clone();
        let options = self.options.clone();
        let on_select = self.on_select.clone();
        let is_open = open();

        if !is_open && list_size().is_some() {
            let _ = list_size.take();
        }

        let trigger_bg = if hovering() {
            colors::ghost_overlay_hover()
        } else {
            colors::ghost_overlay()
        };

        let visible = options.len().min(MAX_VISIBLE_OPTIONS);
        let list_h = visible as f32 * OPTION_HEIGHT
            + visible.saturating_sub(1) as f32 * OPTION_SPACING;

        let offset_y = match (button_area(), list_size()) {
            (Some(button), Some(list)) => {
                let root_height = Platform::get().root_size.peek().height;
                let space_below = root_height - button.max_y();
                let space_above = button.min_y();
                if list.height > space_below && list.height <= space_above {
                    -(button.height() + list.height + 8.)
                } else {
                    0.
                }
            }
            _ => 0.,
        };

        let list_width = button_area()
            .map(|b| Size::px(b.width()))
            .unwrap_or_else(|| self.width.clone());

        let on_global_pointer_press = move |_: Event<PointerEventData>| {
            open.set_if_modified(false);
        };

        rect()
            .width(self.width.clone())
            .child(
                rect()
                    .width(Size::fill())
                    .height(self.height.clone())
                    .horizontal()
                    .center()
                    .spacing(4.)
                    .padding(Gaps::new_symmetric(0., 8.))
                    .corner_radius(CornerRadius::new_all(6.))
                    .background(trigger_bg)
                    .on_pointer_enter(move |_| hovering.set(true))
                    .on_pointer_leave(move |_| hovering.set(false))
                    .on_global_pointer_press(on_global_pointer_press)
                    .on_sized(move |e: Event<SizedEventData>| {
                        button_area.set_if_modified(Some(e.area));
                    })
                    .on_press(move |e: Event<PressEventData>| {
                        e.prevent_default();
                        e.stop_propagation();
                        open.toggle();
                    })
                    .child(
                        label()
                            .text(selected)
                            .font_size(12.)
                            .color(colors::fg_primary()),
                    )
                    .child(Icon::new(
                        IconType::ChevronDown)
                        .size(14.)
                        .color(colors::fg_secondary()),
                    ),
            )
            .maybe_child(is_open.then(|| {
                rect().width(Size::px(0.)).height(Size::px(0.)).child(
                    rect()
                        .layer(Layer::Overlay)
                        .width(list_width)
                        .margin(Gaps::new(4., 0., 0., 0.))
                        .offset_y(offset_y)
                        .content(Content::Fit)
                        .on_sized(move |e: Event<SizedEventData>| {
                            list_size.set_if_modified(Some(e.area.size));
                        })
                        .child(
                            rect()
                                .width(Size::fill())
                                .padding(4.)
                                .corner_radius(CornerRadius::new_all(8.))
                                .border(border_all_color(1., colors::component_border()))
                                .background(colors::page_elevated())
                                .overflow(Overflow::Clip)
                                .child(
                                    ScrollArea::new()
                                        .height(Size::px(list_h))
                                        .spacing(OPTION_SPACING)
                                        .children(options.into_iter().enumerate().map(
                                            |(idx, option)| {
                                                let on_select = on_select.clone();
                                                DropdownOption::new(option, move |_| {
                                                    if let Some(handler) = &on_select {
                                                        handler.call(idx);
                                                    }
                                                    open.set(false);
                                                })
                                                .into_element()
                                            },
                                        )),
                                ),
                        ),
                )
            }))
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

struct DropdownOption {
    text: String,
    on_press: EventHandler<Event<PressEventData>>,
}

impl DropdownOption {
    pub fn new(
        text: impl Into<String>,
        on_press: impl Into<EventHandler<Event<PressEventData>>>,
    ) -> Self {
        Self {
            text: text.into(),
            on_press: on_press.into(),
        }
    }
}

impl PartialEq for DropdownOption {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl Component for DropdownOption {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);

        rect()
            .width(Size::fill())
            .padding(Gaps::new_symmetric(6., 8.))
            .corner_radius(CornerRadius::new_all(6.))
            .background(if hovering() {
                colors::ghost_overlay_hover()
            } else {
                Color::TRANSPARENT
            })
            .on_pointer_enter(move |_| hovering.set(true))
            .on_pointer_leave(move |_| hovering.set(false))
            .on_press(self.on_press.clone())
            .child(
                label()
                    .text(self.text.clone())
                    .font_size(12.)
                    .color(colors::fg_primary()),
            )
    }
}
