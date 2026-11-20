use oneclient_content::packages::markdown::{Iframe, iframes};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use super::embed::Embed;

#[derive(Clone, PartialEq)]
pub enum Block {
    Heading {
        level: HeadingLevel,
        spans: Vec<TextSpan>,
    },
    Paragraph {
        content: Vec<Inline>,
    },
    Code {
        code: String,
    },
    List(List),
    Image {
        url: String,
        alt: String,
    },
    Link {
        url: String,
        title: Option<String>,
        content: Vec<Inline>,
    },
    Blockquote {
        content: Vec<Inline>,
    },
    Table {
        headers: Vec<Vec<TextSpan>>,
        rows: Vec<Vec<Vec<TextSpan>>>,
    },
    /// A video or similar from a known provider, drawn as a card rather than played
    Embed(Embed),
    Rule,
}

#[derive(Clone, PartialEq)]
pub struct List {
    pub start: Option<u64>,
    pub items: Vec<ListItem>,
}

#[derive(Clone, PartialEq)]
pub struct ListItem {
    pub content: Vec<Inline>,
    pub nested_lists: Vec<List>,
}

#[derive(Clone, PartialEq)]
pub enum Inline {
    Span(TextSpan),
    Image {
        url: String,
        alt: String,
    },
    Link {
        url: String,
        title: Option<String>,
        content: Vec<Inline>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextSpan {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
}

impl TextSpan {
    fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            bold: false,
            italic: false,
            code: false,
        }
    }
}

pub fn parse(content: &str) -> Vec<Block> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);

    Walker::default().walk(Parser::new_ext(content, options))
}

#[derive(Default)]
struct Walker {
    blocks: Vec<Block>,

    spans: Vec<TextSpan>,
    content: Vec<Inline>,

    lists: Vec<List>,
    items: Vec<ListItem>,

    heading: Option<HeadingLevel>,
    in_paragraph: bool,

    in_code_block: bool,
    code: String,

    in_blockquote: bool,
    blockquote: Vec<Inline>,

    in_table_cell: bool,
    headers: Vec<Vec<TextSpan>>,
    rows: Vec<Vec<Vec<TextSpan>>>,
    row: Vec<Vec<TextSpan>>,
    cell: Vec<TextSpan>,

    in_link: bool,
    link_url: Option<String>,
    link_title: Option<String>,
    link_content: Vec<Inline>,

    in_image: bool,
    image_url: String,
    image_title: String,
    image_alt: String,

    bold: bool,
    italic: bool,
}

impl Walker {
    fn walk(mut self, parser: Parser<'_>) -> Vec<Block> {
        for event in parser {
            match event {
                Event::Start(tag) => self.start(tag),
                Event::End(tag) => self.end(tag),
                Event::Text(text) => self.text(&text),
                Event::Code(code) => self.code(&code),
                Event::SoftBreak => self.break_of(" "),
                Event::HardBreak => self.break_of("\n"),
                Event::InlineHtml(_) => {}
                Event::Html(html) => self.html_block(&html),
                Event::Rule => self.blocks.push(Block::Rule),
                _ => {}
            }
        }

        self.blocks
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Heading { level, .. } => {
                self.heading = Some(level);
                self.spans.clear();
            }
            Tag::Paragraph => {
                if !self.in_blockquote && self.items.is_empty() {
                    self.in_paragraph = true;
                    self.spans.clear();
                    self.content.clear();
                }
            }
            Tag::CodeBlock(_) => {
                self.in_code_block = true;
                self.code.clear();
            }
            Tag::List(start) => self.lists.push(List {
                start,
                items: Vec::new(),
            }),
            Tag::Item => self.items.push(ListItem {
                content: Vec::new(),
                nested_lists: Vec::new(),
            }),
            Tag::Strong => self.bold = true,
            Tag::Emphasis => self.italic = true,
            Tag::BlockQuote(_) => {
                self.in_blockquote = true;
                self.blockquote.clear();
            }
            Tag::Image {
                dest_url, title, ..
            } => {
                self.in_image = true;
                self.image_url = dest_url.to_string();
                self.image_title = title.to_string();
                self.image_alt.clear();
            }
            Tag::Link {
                dest_url, title, ..
            } => {
                self.in_link = true;
                self.link_url = Some(dest_url.to_string());
                self.link_title = Some(title.to_string());
                self.link_content.clear();
            }
            Tag::Table(_) => {
                self.headers.clear();
                self.rows.clear();
                self.row.clear();
            }
            Tag::TableRow => self.row.clear(),
            Tag::TableCell => {
                self.in_table_cell = true;
                self.cell.clear();
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Heading(_) => {
                if let Some(level) = self.heading.take() {
                    let spans = std::mem::take(&mut self.spans);
                    self.blocks.push(Block::Heading { level, spans });
                }
            }
            TagEnd::Paragraph => {
                let spans: Vec<Inline> = self.spans.drain(..).map(Inline::Span).collect();
                if self.in_blockquote {
                    self.blockquote.extend(spans);
                } else if let Some(item) = self.items.last_mut() {
                    item.content.extend(spans);
                } else if self.in_paragraph {
                    self.in_paragraph = false;
                    self.content.extend(spans);
                    let content = std::mem::take(&mut self.content);
                    self.blocks.push(paragraph_block(content));
                }
            }
            TagEnd::CodeBlock => {
                self.in_code_block = false;
                let code = std::mem::take(&mut self.code);
                self.blocks.push(Block::Code {
                    code: code.trim_end().to_string(),
                });
            }
            TagEnd::List(_) => {
                if let Some(list) = self.lists.pop() {
                    match self.items.last_mut() {
                        Some(item) => item.nested_lists.push(list),
                        None => self.blocks.push(Block::List(list)),
                    }
                }
            }
            TagEnd::Item => {
                if let (Some(item), Some(list)) = (self.items.pop(), self.lists.last_mut()) {
                    list.items.push(item);
                }
            }
            TagEnd::Strong => self.bold = false,
            TagEnd::Emphasis => self.italic = false,
            TagEnd::BlockQuote(_) => {
                self.in_blockquote = false;
                let content = std::mem::take(&mut self.blockquote);
                self.blocks.push(Block::Blockquote { content });
            }
            TagEnd::Table => self.blocks.push(Block::Table {
                headers: std::mem::take(&mut self.headers),
                rows: std::mem::take(&mut self.rows),
            }),
            TagEnd::TableHead => self.headers = std::mem::take(&mut self.row),
            TagEnd::TableRow => {
                let row = std::mem::take(&mut self.row);
                self.rows.push(row);
            }
            TagEnd::TableCell => {
                self.in_table_cell = false;
                let cell = std::mem::take(&mut self.cell);
                self.row.push(cell);
            }
            TagEnd::Image => {
                self.in_image = false;
                let url = std::mem::take(&mut self.image_url);
                let alt = if self.image_alt.is_empty() {
                    std::mem::take(&mut self.image_title)
                } else {
                    std::mem::take(&mut self.image_alt)
                };
                self.place(Inline::Image { url, alt }, |url, alt| Block::Image {
                    url,
                    alt,
                });
            }
            TagEnd::Link => {
                self.in_link = false;
                let Some(url) = self.link_url.take() else {
                    return;
                };
                let title = self.link_title.take();
                let content = std::mem::take(&mut self.link_content);

                if self.in_table_cell {
                    for item in content {
                        flatten(item, &mut self.cell);
                    }
                    return;
                }

                let inline = Inline::Link {
                    url: url.clone(),
                    title: title.clone(),
                    content: content.clone(),
                };
                if self.in_blockquote {
                    self.blockquote.push(inline);
                } else if let Some(item) = self.items.last_mut() {
                    item.content.push(inline);
                } else if self.in_paragraph {
                    self.content.extend(self.spans.drain(..).map(Inline::Span));
                    self.content.push(inline);
                } else if self.heading.is_some() {
                    for item in content {
                        flatten(item, &mut self.spans);
                    }
                } else {
                    self.blocks.push(Block::Link {
                        url,
                        title,
                        content,
                    });
                }
            }
            _ => {}
        }
    }

    fn place(&mut self, inline: Inline, as_block: impl FnOnce(String, String) -> Block) {
        if self.in_table_cell {
            flatten(inline, &mut self.cell);
        } else if self.in_link {
            self.link_content.push(inline);
        } else if self.in_blockquote {
            self.blockquote.push(inline);
        } else if let Some(item) = self.items.last_mut() {
            item.content.push(inline);
        } else if self.in_paragraph {
            self.content.extend(self.spans.drain(..).map(Inline::Span));
            self.content.push(inline);
        } else if self.heading.is_some() {
            flatten(inline, &mut self.spans);
        } else if let Inline::Image { url, alt } = inline {
            self.blocks.push(as_block(url, alt));
        }
    }

    fn text(&mut self, text: &str) {
        if self.in_code_block {
            self.code.push_str(text);
            return;
        }
        if self.in_image {
            self.image_alt.push_str(text);
            return;
        }

        self.push_span(TextSpan {
            text: text.to_string(),
            bold: self.bold,
            italic: self.italic,
            code: false,
        });
    }

    fn code(&mut self, code: &str) {
        if self.in_image {
            self.image_alt.push_str(code);
            return;
        }

        self.push_span(TextSpan {
            text: code.to_string(),
            bold: self.bold,
            italic: self.italic,
            code: true,
        });
    }

    fn break_of(&mut self, text: &str) {
        if self.in_image {
            self.image_alt.push(' ');
            return;
        }

        self.push_span(TextSpan::new(text));
    }

    fn html_block(&mut self, html: &str) {
        let text = strip_tags(html);
        if !text.trim().is_empty() {
            self.blocks.push(Block::Paragraph {
                content: vec![Inline::Span(TextSpan::new(text))],
            });
        }

        // Only the tag is read, so an iframe becomes a card or a link, never a running page
        self.blocks
            .extend(iframes(html).into_iter().map(iframe_block));
    }

    fn push_span(&mut self, span: TextSpan) {
        if self.in_table_cell {
            self.cell.push(span);
        } else if self.in_link {
            self.link_content.push(Inline::Span(span));
        } else if self.in_blockquote && !self.in_paragraph {
            self.blockquote.push(Inline::Span(span));
        } else if let Some(item) = self.items.last_mut()
            && !self.in_paragraph
        {
            item.content.push(Inline::Span(span));
        } else {
            self.spans.push(span);
        }
    }
}

fn iframe_block(frame: Iframe) -> Block {
    match Embed::from_url(&frame.src, frame.title.as_deref(), true) {
        Some(embed) => Block::Embed(embed),
        None => {
            let label = frame.title.clone().unwrap_or_else(|| frame.src.clone());
            Block::Link {
                url: frame.src,
                title: frame.title,
                content: vec![Inline::Span(TextSpan::new(label))],
            }
        }
    }
}

/// A paragraph that is only a link to a video is that video, so it can show as one.
///
/// A link the author wrote with their own words around a page url stays a link: only the
/// embed form of a url, or a page url standing as its own text, is taken for an embed.
fn paragraph_block(content: Vec<Inline>) -> Block {
    if let [
        Inline::Link {
            url, content: text, ..
        },
    ] = content.as_slice()
    {
        let mut label = String::new();
        let plain = text.iter().all(|inline| match inline {
            Inline::Span(span) => {
                label.push_str(&span.text);
                true
            }
            _ => false,
        });

        if plain {
            let label = label.trim();
            let allow_page = label.is_empty() || label == url.trim();
            if let Some(embed) = Embed::from_url(url, Some(label), allow_page) {
                return Block::Embed(embed);
            }
        }
    }

    Block::Paragraph { content }
}

/// Headings and table cells hold plain spans, so links and images collapse to their text.
fn flatten(inline: Inline, out: &mut Vec<TextSpan>) {
    match inline {
        Inline::Span(span) => out.push(span),
        Inline::Image { alt, .. } => {
            if !alt.is_empty() {
                out.push(TextSpan::new(alt));
            }
        }
        Inline::Link { content, .. } => {
            for item in content {
                flatten(item, out);
            }
        }
    }
}

fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;

    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        match rest[open..].find('>') {
            Some(close) => rest = &rest[open + close + 1..],
            None => {
                out.push_str(&rest[open..]);
                return out;
            }
        }
    }
    out.push_str(rest);

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocks(markdown: &str) -> Vec<Block> {
        parse(markdown)
    }

    fn paragraph_text(block: &Block) -> String {
        let Block::Paragraph { content } = block else {
            panic!("expected a paragraph");
        };
        content
            .iter()
            .map(|inline| match inline {
                Inline::Span(span) => span.text.clone(),
                Inline::Image { alt, .. } => format!("[img:{alt}]"),
                Inline::Link { content, .. } => content
                    .iter()
                    .map(|inline| match inline {
                        Inline::Span(span) => span.text.clone(),
                        Inline::Image { alt, .. } => format!("[img:{alt}]"),
                        Inline::Link { .. } => String::new(),
                    })
                    .collect(),
            })
            .collect()
    }

    #[test]
    fn hard_break_is_a_line_break_and_soft_break_is_a_space() {
        assert_eq!(paragraph_text(&blocks("one  \ntwo")[0]), "one\ntwo");
        assert_eq!(paragraph_text(&blocks("one\ntwo")[0]), "one two");
    }

    #[test]
    fn code_block_keeps_its_lines() {
        let Block::Code { code } = &blocks("```rust\nlet a = 1;\nlet b = 2;\n```")[0] else {
            panic!("expected a code block");
        };
        assert_eq!(code, "let a = 1;\nlet b = 2;");
    }

    #[test]
    fn inline_html_keeps_the_text_it_wrapped() {
        assert_eq!(
            paragraph_text(&blocks("a <b>bold</b> word")[0]),
            "a bold word"
        );
    }

    #[test]
    fn block_html_keeps_its_text() {
        assert_eq!(
            paragraph_text(&blocks("<div>\nhello\n</div>")[0]).trim(),
            "hello"
        );
    }

    #[test]
    fn images_arrive_as_paragraph_content() {
        assert_eq!(paragraph_text(&blocks("![alt](a.png)")[0]), "[img:alt]");
        assert_eq!(
            paragraph_text(&blocks("see ![alt](a.png) here")[0]),
            "see [img:alt] here"
        );
    }

    #[test]
    fn an_image_inside_a_link_stays_in_the_link() {
        assert_eq!(
            paragraph_text(&blocks("text [![badge](b.svg)](https://example.com)")[0]),
            "text [img:badge]"
        );
    }

    #[test]
    fn nested_lists_keep_their_nesting() {
        let Block::List(list) = &blocks("- one\n  - inner\n- two")[0] else {
            panic!("expected a list");
        };
        assert_eq!(list.items.len(), 2);
        assert_eq!(list.items[0].nested_lists.len(), 1);
        assert_eq!(list.items[0].nested_lists[0].items.len(), 1);
    }

    #[test]
    fn ordered_lists_keep_their_start() {
        let Block::List(list) = &blocks("3. three\n4. four")[0] else {
            panic!("expected a list");
        };
        assert_eq!(list.start, Some(3));
    }

    #[test]
    fn tables_split_head_from_body() {
        let Block::Table { headers, rows } =
            &blocks("| a | b |\n| - | - |\n| 1 | 2 |\n| 3 | 4 |")[0]
        else {
            panic!("expected a table");
        };
        assert_eq!(headers.len(), 2);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1][1][0].text, "4");
    }

    #[test]
    fn emphasis_and_code_carry_their_style() {
        let Block::Paragraph { content } = &blocks("**b** *i* `c`")[0] else {
            panic!("expected a paragraph");
        };
        let spans: Vec<&TextSpan> = content
            .iter()
            .filter_map(|inline| match inline {
                Inline::Span(span) => Some(span),
                _ => None,
            })
            .collect();
        assert!(spans.iter().any(|s| s.text == "b" && s.bold));
        assert!(spans.iter().any(|s| s.text == "i" && s.italic));
        assert!(spans.iter().any(|s| s.text == "c" && s.code));
    }

    #[test]
    fn a_linked_heading_keeps_its_text_and_leaks_no_link_block() {
        let parsed = blocks("## [Docs](https://example.com)");
        assert_eq!(parsed.len(), 1);
        let Block::Heading { spans, .. } = &parsed[0] else {
            panic!("expected a heading");
        };
        assert_eq!(spans[0].text, "Docs");
    }

    #[test]
    fn a_badge_table_keeps_its_badges_inside_the_table() {
        let parsed = blocks(
            "| a | b |\n| - | - |\n| [![CF](cf.svg)](https://curseforge.com) | [![MR](mr.svg)](https://modrinth.com) |\n",
        );
        assert_eq!(parsed.len(), 1);
        let Block::Table { rows, .. } = &parsed[0] else {
            panic!("expected a table");
        };
        assert_eq!(rows[0][0][0].text, "CF");
        assert_eq!(rows[0][1][0].text, "MR");
    }

    #[test]
    fn headings_blockquotes_and_rules_are_their_own_blocks() {
        let parsed = blocks("# Title\n\n> quoted\n\n---\n");
        assert!(matches!(parsed[0], Block::Heading { .. }));
        assert!(matches!(parsed[1], Block::Blockquote { .. }));
        assert!(matches!(parsed[2], Block::Rule));
    }

    const WWAYPOINTS: &str = "- Highlights blocks, making it the #1 Trapping mod
# Watch Sheet's Showcase (0.6.0):

[YouTube video player](https://www.youtube-nocookie.com/embed/WaRLLta2Edg)

After the video.";

    fn embed_of(block: &Block) -> &Embed {
        let Block::Embed(embed) = block else {
            panic!("expected an embed");
        };
        embed
    }

    #[test]
    fn the_wwaypoints_embed_follows_its_heading_and_keeps_what_comes_after() {
        let parsed = blocks(WWAYPOINTS);

        assert!(matches!(parsed[0], Block::List(_)));
        let Block::Heading { spans, .. } = &parsed[1] else {
            panic!("expected the showcase heading");
        };
        assert_eq!(spans[0].text, "Watch Sheet's Showcase (0.6.0):");
        assert_eq!(
            embed_of(&parsed[2]).provider,
            super::super::embed::Provider::YouTube
        );
        assert_eq!(
            embed_of(&parsed[2]).open_url,
            "https://www.youtube.com/watch?v=WaRLLta2Edg"
        );
        assert_eq!(embed_of(&parsed[2]).title, None);
        assert_eq!(paragraph_text(&parsed[3]), "After the video.");
        assert_eq!(parsed.len(), 4);
    }

    #[test]
    fn the_real_wwaypoints_description_ends_in_its_youtube_card() {
        let body = oneclient_content::packages::markdown::normalize_markdown(
            "### Officially Allowed on Hoplite\n\nwWaypoints is the new best waypoint mod.\n- Highlights blocks, making it the #1 Trapping mod\n# Watch Sheet's Showcase (0.6.0):\n<iframe width=\"560\" height=\"315\" src=\"https://www.youtube-nocookie.com/embed/WaRLLta2Edg\" title=\"YouTube video player\" frameborder=\"0\" allow=\"accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share\" allowfullscreen></iframe>",
        );
        let parsed = blocks(&body);

        let Block::Heading { spans, .. } = &parsed[parsed.len() - 2] else {
            panic!("the showcase heading should come right before the embed");
        };
        assert_eq!(spans[0].text, "Watch Sheet's Showcase (0.6.0):");
        assert_eq!(
            embed_of(parsed.last().unwrap()).open_url,
            "https://www.youtube.com/watch?v=WaRLLta2Edg"
        );
    }

    #[test]
    fn a_raw_iframe_that_was_never_normalised_still_becomes_an_embed() {
        let parsed = blocks(
            "# Showcase
<iframe width=\"560\" src=\"https://www.youtube-nocookie.com/embed/WaRLLta2Edg\" title=\"YouTube video player\" allowfullscreen></iframe>

after",
        );

        assert!(matches!(parsed[0], Block::Heading { .. }));
        assert_eq!(
            embed_of(&parsed[1]).open_url,
            "https://www.youtube.com/watch?v=WaRLLta2Edg"
        );
        assert_eq!(paragraph_text(&parsed[2]), "after");
    }

    #[test]
    fn an_iframe_from_an_unknown_site_is_a_link_never_an_embed() {
        let parsed = blocks("<iframe src=\"https://example.com/player\" title=\"Demo\"></iframe>");

        let Block::Link { url, content, .. } = &parsed[0] else {
            panic!("expected a link");
        };
        assert_eq!(url, "https://example.com/player");
        assert!(matches!(&content[0], Inline::Span(span) if span.text == "Demo"));
    }

    #[test]
    fn a_script_or_javascript_iframe_renders_nothing() {
        assert!(blocks("<iframe src=\"javascript:alert(1)\"></iframe>").is_empty());
        assert!(
            blocks("<script>alert(1)</script>")
                .iter()
                .all(|block| { !matches!(block, Block::Embed(_) | Block::Link { .. }) })
        );
    }

    #[test]
    fn a_lone_video_page_link_is_a_card_only_when_it_is_its_own_text() {
        assert!(matches!(
            blocks("<https://youtu.be/WaRLLta2Edg>")[0],
            Block::Embed(_)
        ));
        assert!(matches!(
            blocks("[https://youtu.be/WaRLLta2Edg](https://youtu.be/WaRLLta2Edg)")[0],
            Block::Embed(_)
        ));
        assert_eq!(
            paragraph_text(&blocks("[Watch it here](https://youtu.be/WaRLLta2Edg)")[0]),
            "Watch it here"
        );
    }

    #[test]
    fn a_titled_embed_link_keeps_its_title() {
        let parsed = blocks("[Sheet's Showcase](https://www.youtube.com/embed/WaRLLta2Edg)");
        assert_eq!(
            embed_of(&parsed[0]).title.as_deref(),
            Some("Sheet's Showcase")
        );
    }

    #[test]
    fn a_video_link_inside_a_sentence_or_list_stays_a_link() {
        assert!(matches!(
            blocks("watch [it](https://www.youtube.com/embed/WaRLLta2Edg) now")[0],
            Block::Paragraph { .. }
        ));
        assert!(matches!(
            blocks("- [it](https://www.youtube.com/embed/WaRLLta2Edg)")[0],
            Block::List(_)
        ));
    }

    #[test]
    fn a_plain_link_paragraph_is_unchanged() {
        assert!(matches!(
            blocks("[Docs](https://example.com)")[0],
            Block::Paragraph { .. }
        ));
    }
}
