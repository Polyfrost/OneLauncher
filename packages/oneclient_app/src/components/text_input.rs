use freya::prelude::*;

use crate::components::{Button, Icon, IconType, OverlayPopup};
use crate::theme::colors;
use crate::ui::clamp_to_window;

const GAP_VERT: f32 = 8.0;
const GAP_HORI: f32 = 12.0;

const EXPANDED_WIDTH: f32 = 440.;
const EXPANDED_HEIGHT: f32 = 180.;

fn text_input(value: impl Into<Writable<String>>) -> Input {
    Input::new(value)
        .padding(Gaps::new_symmetric(GAP_VERT, GAP_HORI))
        .corner_radius(CornerRadius::new_all(8.))
        .background(colors::component_bg())
        .focus_background(colors::component_bg_pressed())
        .border_fill(colors::component_border())
        .focus_border_fill(colors::component_border_pressed())
        .color(colors::fg_primary())
        .placeholder_color(colors::fg_secondary())
}

pub fn validate_number(validator: InputValidator) {
    let text = validator.text();

    let valid = text.is_empty() || text.parse::<u32>().is_ok();

    validator.set_valid(valid);
}

/// Validates the input if user has entered more MB of memory than his machine has
pub fn validate_memory(validator: InputValidator) {
    let text = validator.text();

    let valid = text.is_empty()
        || text
            .parse::<u32>()
            .is_ok_and(|mb| mb <= crate::utils::total_ram_mb());

    validator.set_valid(valid);
}

#[derive(Clone, PartialEq)]
pub struct TextInput {
    input: Input,
    value: Writable<String>,
    on_submit: Option<EventHandler<String>>,
    multiline: bool,
    expandable: bool,
    layout: LayoutData,
    style: StyleState,
    text_style: TextStyleData,
    elements: Vec<Element>,
    key: DiffKey,
}

#[allow(dead_code)]
impl TextInput {
    pub fn new(value: impl Into<Writable<String>>) -> Self {
        let value = value.into();
        Self {
            input: text_input(value.clone()),
            value,
            on_submit: None,
            multiline: false,
            expandable: false,
            layout: LayoutData::default(),
            style: StyleState::default(),
            text_style: TextStyleData::default(),
            elements: Vec::new(),
            key: DiffKey::None,
        }
        .font_size(14.)
    }

    pub fn enabled(mut self, enabled: impl Into<bool>) -> Self {
        self.input = self.input.enabled(enabled);
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.input = self.input.placeholder(placeholder);
        self
    }

    pub fn on_validate(mut self, on_validate: impl Into<EventHandler<InputValidator>>) -> Self {
        self.input = self.input.on_validate(on_validate);
        self
    }

    pub fn on_submit(mut self, on_submit: impl Into<EventHandler<String>>) -> Self {
        let on_submit = on_submit.into();
        self.input = self.input.on_submit(on_submit.clone());
        self.on_submit = Some(on_submit);
        self
    }

    pub fn compact(mut self) -> Self {
        self.input = self
            .input
            .inner_margin(Gaps::new_symmetric(GAP_VERT / 2., GAP_HORI / 1.5));
        self.font_size(12.)
    }

    pub fn multiline(mut self, multiline: bool) -> Self {
        self.input = self.input.multiline(multiline);
        self.multiline = multiline;
        self
    }

    pub fn expandable(mut self, expandable: bool) -> Self {
        self.expandable = expandable;
        self
    }

    pub fn mode(mut self, mode: InputMode) -> Self {
        self.input = self.input.mode(mode);
        self
    }

    pub fn auto_focus(mut self, auto_focus: impl Into<bool>) -> Self {
        self.input = self.input.auto_focus(auto_focus);
        self
    }

    pub fn leading(mut self, leading: impl Into<Element>) -> Self {
        self.input = self.input.leading(leading);
        self
    }

    pub fn trailing(mut self, trailing: impl Into<Element>) -> Self {
        self.input = self.input.trailing(trailing);
        self
    }

    pub fn on_pre_key_down(
        mut self,
        on_pre_key_down: impl Into<Callback<Event<KeyboardEventData>, bool>>,
    ) -> Self {
        self.input = self.input.on_pre_key_down(on_pre_key_down);
        self
    }
}

impl ChildrenExt for TextInput {
    fn get_children(&mut self) -> &mut Vec<Element> {
        &mut self.elements
    }
}

impl KeyExt for TextInput {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl LayoutExt for TextInput {
    fn get_layout(&mut self) -> &mut LayoutData {
        &mut self.layout
    }
}

impl TextStyleExt for TextInput {
    fn get_text_style_data(&mut self) -> &mut TextStyleData {
        &mut self.text_style
    }
}

impl ContainerExt for TextInput {}
impl ContainerWithContentExt for TextInput {}

impl StyleExt for TextInput {
    fn get_style(&mut self) -> &mut StyleState {
        &mut self.style
    }
}

fn submit_on_modifier_enter(
    value: Writable<String>,
    on_submit: EventHandler<String>,
) -> Callback<Event<KeyboardEventData>, bool> {
    Callback::new(move |e: Event<KeyboardEventData>| match &e.key {
        Key::Named(NamedKey::Enter) if e.modifiers.contains(Modifiers::ctrl_or_meta()) => {
            e.stop_propagation();
            e.prevent_default();
            on_submit.call(value.read().clone());
            false
        }
        Key::Named(NamedKey::Escape) | Key::Named(NamedKey::Shift) => true,
        Key::Named(NamedKey::Tab) => false,
        _ => {
            e.stop_propagation();
            e.prevent_default();
            true
        }
    })
}

impl Component for TextInput {
    fn render(&self) -> impl IntoElement {
        let mut expanded = use_state(|| false);
        let mut area = use_state(Area::default);

        let mut card = rect()
            .layout(self.layout.clone())
            .text_style(self.text_style.clone())
            .children(self.elements.clone())
            .maybe(self.expandable, |el| {
                el.on_sized(move |e: Event<SizedEventData>| area.set_if_modified(e.area))
            });

        card.get_style().clone_from(&self.style);

        let mut input = self.input.clone();
        if self.multiline
            && let Some(on_submit) = self.on_submit.clone()
        {
            input = input.on_pre_key_down(submit_on_modifier_enter(self.value.clone(), on_submit));
        }
        if !matches!(self.layout.height, Size::Inner) {
            input = input.height(Size::fill());
        }

        let expanded_input = (self.expandable && expanded()).then(|| {
            let area = area();
            let (left, top) = clamp_to_window(
                area.max_x() - EXPANDED_WIDTH,
                area.min_y(),
                EXPANDED_WIDTH,
                EXPANDED_HEIGHT,
            );

            OverlayPopup::new()
                .backdrop(false)
                .position(Position::new_global().left(left).top(top))
                .on_close(move |_| expanded.set(false))
                .child(
                    rect()
                        .corner_radius(CornerRadius::new_all(8.))
                        .shadow(Shadow::from((
                            0.,
                            8.,
                            32.,
                            0.,
                            Color::from_argb(120, 0, 0, 0),
                        )))
                        .child(
                            self.input
                                .clone()
                                .multiline(true)
                                .auto_focus(true)
                                .width(Size::px(EXPANDED_WIDTH))
                                .height(Size::px(EXPANDED_HEIGHT)),
                        ),
                )
        });

        if self.expandable {
            input = input.trailing(
                rect()
                    .on_pointer_down(|e: Event<PointerEventData>| e.stop_propagation())
                    .child(
                        Button::new()
                            .ghost()
                            .icon()
                            .width(Size::px(22.))
                            .height(Size::px(22.))
                            .padding(3.)
                            .tooltip("Expand")
                            .on_press(move |e: Event<PressEventData>| {
                                e.stop_propagation();
                                expanded.set(true);
                            })
                            .child(
                                Icon::new(IconType::Maximize01)
                                    .size(14.)
                                    .color(colors::fg_secondary()),
                            ),
                    ),
            );
        }

        card.maybe_child(expanded_input).child(
            input
                .map(self.text_style.text_align, |mut el, text_align| {
                    el = el.text_align(text_align);

                    if text_align == TextAlign::Center {
                        el = el.padding(Gaps::new_symmetric(GAP_VERT, 0.));
                    }

                    el
                })
                .width(Size::fill()),
        )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}
