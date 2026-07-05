use freya::prelude::*;

use crate::theme::colors;

const GAP_VERT: f32 = 8.0;
const GAP_HORI: f32 = 12.0;

fn text_input(value: impl Into<Writable<String>>) -> Input {
    Input::new(value)
        .inner_margin(Gaps::new_symmetric(GAP_VERT, GAP_HORI))
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

#[derive(Clone, PartialEq)]
pub struct TextInput {
    input: Input,
    layout: LayoutData,
    style: StyleState,
    text_style: TextStyleData,
    elements: Vec<Element>,
    key: DiffKey,
}

#[allow(dead_code)]
impl TextInput {
    pub fn new(value: impl Into<Writable<String>>) -> Self {
        Self::from_input(text_input(value))
    }

    pub fn from_input(input: Input) -> Self {
        Self {
            input,
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
        self.input = self.input.on_submit(on_submit);
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

impl Component for TextInput {
    fn render(&self) -> impl IntoElement {
        let mut card = rect()
            .layout(self.layout.clone())
            .text_style(self.text_style.clone())
            .children(self.elements.clone());

        card.get_style().clone_from(&self.style);

        card.child(
            self.input
                .clone()
                .map(self.text_style.text_align, |mut el, text_align| {
                    el = el.text_align(text_align);

                    if text_align == TextAlign::Center {
                        el = el.inner_margin(Gaps::new_symmetric(GAP_VERT, 0.));
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
