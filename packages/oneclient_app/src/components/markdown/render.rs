use freya::prelude::*;

use super::MarkdownStyle;
use super::image::MarkdownImage;
use super::parse::{Block, Inline, List, TextSpan};
use crate::hooks::LinkConfirmState;

pub fn render_block(block: &Block, key: usize, style: &MarkdownStyle) -> Element {
    match block {
        Block::Heading { level, spans } => {
            render_spans(spans, style.heading_size(*level), style.color, style.color_code)
                .font_weight(FontWeight::BOLD)
                .key(key)
                .into()
        }
        Block::Paragraph { content } => render_content(content, style.paragraph_size, style)
            .key(key)
            .into(),
        Block::Code { code } => rect()
            .key(key)
            .width(Size::fill())
            .background(style.background_code)
            .corner_radius(CornerRadius::new_all(6.))
            .padding(Gaps::new_all(12.))
            .child(
                label()
                    .text(code.clone())
                    .font_family(style.code_font_family.clone())
                    .font_size(style.code_font_size)
                    .color(style.color_code),
            )
            .into(),
        Block::List(list) => render_list(list, style).key(key).into(),
        Block::Image { url, alt } => rect()
            .key(key)
            .child(MarkdownImage::new(url.clone(), alt.clone()))
            .into(),
        Block::Link {
            url,
            title,
            content,
        } => MarkdownLink::new(url.clone(), title.clone(), content.clone(), style.clone())
            .key(key)
            .into(),
        Block::Blockquote { content } => rect()
            .key(key)
            .width(Size::fill())
            .padding(Gaps::new(12., 12., 12., 16.))
            .border(
                Border::new()
                    .width(4.)
                    .fill(style.border_blockquote)
                    .alignment(BorderAlignment::Inner),
            )
            .background(style.background_blockquote)
            .child(render_content(content, style.paragraph_size, style).font_slant(FontSlant::Italic))
            .into(),
        Block::Rule => rect()
            .key(key)
            .width(Size::fill())
            .height(Size::px(1.))
            .background(style.background_divider)
            .into(),
        Block::Table { headers, rows } => render_table(headers, rows, style).key(key).into(),
    }
}

fn styled_span(span: &TextSpan, text_color: Color, code_color: Color) -> Span<'static> {
    let mut styled = Span::new(span.text.clone());
    if span.bold {
        styled = styled.font_weight(FontWeight::BOLD);
    }
    if span.italic {
        styled = styled.font_slant(FontSlant::Italic);
    }
    if span.code {
        styled.font_family("monospace").color(code_color)
    } else {
        styled.color(text_color)
    }
}

fn render_spans(
    spans: &[TextSpan],
    base_font_size: f32,
    text_color: Color,
    code_color: Color,
) -> Paragraph {
    paragraph().font_size(base_font_size).spans_iter(
        spans
            .iter()
            .map(|span| styled_span(span, text_color, code_color)),
    )
}

fn render_content(content: &[Inline], base_font_size: f32, style: &MarkdownStyle) -> Paragraph {
    let mut result = paragraph().font_size(base_font_size);

    for item in content {
        result = match item {
            Inline::Span(span) => result.span(styled_span(span, style.color, style.color_code)),
            Inline::Image { url, alt } => {
                result.child(MarkdownImage::new(url.clone(), alt.clone()))
            }
            Inline::Link {
                url,
                title,
                content,
            } => result.child(MarkdownLink::new(
                url.clone(),
                title.clone(),
                content.clone(),
                style.clone(),
            )),
        };
    }

    result
}

fn render_list(list: &List, style: &MarkdownStyle) -> Rect {
    rect()
        .vertical()
        .spacing(4.)
        .padding(Gaps::new(0., 0., 0., 20.))
        .children(list.items.iter().enumerate().map(|(item_idx, item)| {
            rect()
                .key(item_idx)
                .horizontal()
                .cross_align(Alignment::Start)
                .spacing(8.)
                .child(
                    label()
                        .text(match list.start {
                            Some(start) => format!("{}.", start + item_idx as u64),
                            None => "•".to_string(),
                        })
                        .font_size(style.paragraph_size)
                        .color(style.color),
                )
                .child(
                    rect()
                        .vertical()
                        .spacing(4.)
                        .child(render_content(&item.content, style.paragraph_size, style))
                        .children(
                            item.nested_lists
                                .iter()
                                .map(|nested| render_list(nested, style)),
                        ),
                )
        }))
}

fn render_table(
    headers: &[Vec<TextSpan>],
    rows: &[Vec<Vec<TextSpan>>],
    style: &MarkdownStyle,
) -> Rect {
    rect()
        .vertical()
        .width(Size::fill())
        .corner_radius(CornerRadius::new_all(8.))
        .border(
            Border::new()
                .width(1.)
                .fill(style.background_divider)
                .alignment(BorderAlignment::Inner),
        )
        .child(table_row(headers, style, true))
        .children(rows.iter().enumerate().flat_map(|(row_idx, row)| {
            [
                rect()
                    .key(format!("divider-{row_idx}"))
                    .width(Size::fill())
                    .height(Size::px(1.))
                    .background(style.background_divider)
                    .into_element(),
                table_row(row, style, false).key(row_idx).into_element(),
            ]
        }))
}

fn table_row(cells: &[Vec<TextSpan>], style: &MarkdownStyle, header: bool) -> Rect {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .maybe(header, |row| row.background(style.background_code))
        .children(cells.iter().enumerate().map(|(col_idx, spans)| {
            rect()
                .key(col_idx)
                .width(Size::flex(1.))
                .padding(Gaps::new_all(8.))
                .child(
                    render_spans(spans, style.table_font_size, style.color, style.color_code)
                        .maybe(header, |cell| cell.font_weight(FontWeight::BOLD)),
                )
        }))
}

#[derive(PartialEq)]
struct MarkdownLink {
    url: String,
    title: Option<String>,
    content: Vec<Inline>,
    style: MarkdownStyle,
    key: DiffKey,
}

impl KeyExt for MarkdownLink {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl MarkdownLink {
    fn new(
        url: String,
        title: Option<String>,
        content: Vec<Inline>,
        style: MarkdownStyle,
    ) -> Self {
        Self {
            url,
            title,
            content,
            style,
            key: DiffKey::None,
        }
    }
}

impl Component for MarkdownLink {
    fn render(&self) -> impl IntoElement {
        let pending = try_consume_root_context::<LinkConfirmState>().map(|state| state.0);
        let mut hovering = use_state(|| false);

        let url = self.url.clone();
        let alt = self.title.clone().filter(|title| !title.is_empty());

        let mut text = paragraph().font_size(self.style.paragraph_size);
        for item in &self.content {
            text = match item {
                Inline::Span(span) => {
                    text.span(styled_span(span, self.style.color_link, self.style.color_code))
                }
                Inline::Image { url, alt } => {
                    text.child(MarkdownImage::new(url.clone(), alt.clone()))
                }
                Inline::Link { content, .. } => content.iter().fold(text, |text, item| match item {
                    Inline::Span(span) => {
                        text.span(styled_span(span, self.style.color_link, self.style.color_code))
                    }
                    _ => text,
                }),
            };
        }

        rect()
            .cursor(CursorIcon::Pointer)
            .on_pointer_enter(move |_| hovering.set(true))
            .on_pointer_leave(move |_| hovering.set(false))
            .on_press(move |_| match pending {
                Some(mut pending) => pending.set(Some(url.clone())),
                None => tracing::warn!("no link confirmation available, not opening {url}"),
            })
            .maybe(hovering(), |element| {
                element.text_decoration(TextDecoration::Underline)
            })
            .a11y_role(AccessibilityRole::Link)
            .map(alt, |element, alt| element.a11y_alt(alt))
            .child(text)
    }
}
