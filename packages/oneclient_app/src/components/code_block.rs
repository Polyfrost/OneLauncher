use std::borrow::Cow;

use freya::prelude::*;

use crate::components::ScrollArea;
use crate::theme::{self, colors};
use crate::ui::border_all_color;

#[derive(PartialEq)]
pub struct CodeBlock {
    code: String,
    color: Color,
    background: Color,
    font_family: Cow<'static, str>,
    font_size: f32,
    max_lines: Option<usize>,
    height: Option<f32>,
    key: DiffKey,
}

impl KeyExt for CodeBlock {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl CodeBlock {
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            color: colors::fg_primary(),
            background: colors::component_bg(),
            font_family: Cow::Borrowed(theme::MONO_FONT),
            font_size: 12.,
            max_lines: None,
            height: None,
            key: DiffKey::None,
        }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn background(mut self, background: Color) -> Self {
        self.background = background;
        self
    }

    pub fn font_family(mut self, font_family: Cow<'static, str>) -> Self {
        self.font_family = font_family;
        self
    }

    pub fn font_size(mut self, font_size: f32) -> Self {
        self.font_size = font_size;
        self
    }

    pub fn max_lines(mut self, max_lines: usize) -> Self {
        self.max_lines = Some(max_lines);
        self
    }

    /// Fixes the block's height and scrolls the code inside it
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }
}

impl Component for CodeBlock {
    fn render(&self) -> impl IntoElement {
        let text = SelectableText::new()
            .span(self.code.clone())
            .font_family(self.font_family.clone())
            .font_size(self.font_size)
            .max_lines(self.max_lines)
            .width(Size::fill())
            .color(self.color);

        let block = rect()
            .width(Size::fill())
            .padding(Gaps::new_all(12.))
            .corner_radius(CornerRadius::new_all(8.))
            .background(self.background)
            .border(border_all_color(1., colors::component_border()));

        match self.height {
            Some(height) => block
                .height(Size::px(height))
                .overflow(Overflow::Clip)
                .child(
                    ScrollArea::new()
                        .width(Size::fill())
                        .height(Size::fill())
                        .child(text),
                ),
            None => block.child(text),
        }
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}
