use std::borrow::Cow;

use freya::prelude::*;
use pulldown_cmark::HeadingLevel;

mod image;
mod parse;
mod render;

#[derive(Clone, PartialEq)]
pub struct MarkdownStyle {
    pub color: Color,
    pub color_link: Color,
    pub color_code: Color,
    pub background_code: Color,
    pub background_blockquote: Color,
    pub border_blockquote: Color,
    pub background_divider: Color,
    pub headings: [f32; 6],
    pub paragraph_size: f32,
    pub code_font_size: f32,
    pub table_font_size: f32,
    pub code_font_family: Cow<'static, str>,
}

impl Default for MarkdownStyle {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            color_link: Color::from_rgb(90, 160, 255),
            color_code: Color::WHITE,
            background_code: Color::from_rgb(32, 32, 32),
            background_blockquote: Color::from_rgb(32, 32, 32),
            border_blockquote: Color::from_rgb(80, 80, 80),
            background_divider: Color::from_rgb(64, 64, 64),
            headings: [32., 28., 24., 20., 18., 16.],
            paragraph_size: 16.,
            code_font_size: 14.,
            table_font_size: 14.,
            code_font_family: Cow::Borrowed("Jetbrains Mono"),
        }
    }
}

impl MarkdownStyle {
    fn heading_size(&self, level: HeadingLevel) -> f32 {
        self.headings[match level {
            HeadingLevel::H1 => 0,
            HeadingLevel::H2 => 1,
            HeadingLevel::H3 => 2,
            HeadingLevel::H4 => 3,
            HeadingLevel::H5 => 4,
            HeadingLevel::H6 => 5,
        }]
    }
}

#[derive(PartialEq)]
pub struct Markdown {
    content: String,
    style: MarkdownStyle,
    layout: LayoutData,
    key: DiffKey,
}

impl Markdown {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            style: MarkdownStyle::default(),
            layout: LayoutData::default(),
            key: DiffKey::None,
        }
    }

    pub fn style(mut self, style: MarkdownStyle) -> Self {
        self.style = style;
        self
    }
}

impl KeyExt for Markdown {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl LayoutExt for Markdown {
    fn get_layout(&mut self) -> &mut LayoutData {
        &mut self.layout
    }
}

impl ContainerExt for Markdown {}

impl Component for Markdown {
    fn render(&self) -> impl IntoElement {
        let content = use_reactive(&self.content);
        let blocks = use_memo(move || parse::parse(&content.read()));
        let blocks = blocks.read();

        rect()
            .vertical()
            .layout(self.layout.clone())
            .spacing(12.)
            .children(
                blocks
                    .iter()
                    .enumerate()
                    .map(|(idx, block)| render::render_block(block, idx, &self.style)),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

