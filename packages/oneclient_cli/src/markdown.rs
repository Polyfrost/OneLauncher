use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::output::{accent, bold, dim, heading};

pub(crate) fn render(markdown: &str) -> String {
    let mut out = String::new();
    let mut lists: Vec<Option<u64>> = Vec::new();
    let mut quote = 0usize;
    let mut link: Option<String> = None;
    let mut styled: Option<fn(&str) -> String> = None;
    let mut code = false;
    let mut image: Option<String> = None;

    let newline = |out: &mut String, quote: usize| {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&"│ ".repeat(quote));
    };

    for event in Parser::new_ext(markdown, Options::all()) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                out.push('\n');
                newline(&mut out, quote);
                styled = Some(if level <= HeadingLevel::H2 {
                    |text| heading(text)
                } else {
                    |text| bold(text)
                });
            }
            Event::End(TagEnd::Heading(_)) => {
                styled = None;
                out.push('\n');
            }
            Event::Start(Tag::Paragraph) => newline(&mut out, quote),
            Event::End(TagEnd::Paragraph) => out.push('\n'),
            Event::Start(Tag::BlockQuote(_)) => quote += 1,
            Event::End(TagEnd::BlockQuote(_)) => quote = quote.saturating_sub(1),
            Event::Start(Tag::List(start)) => lists.push(start),
            Event::End(TagEnd::List(_)) => {
                lists.pop();
                if lists.is_empty() {
                    out.push('\n');
                }
            }
            Event::Start(Tag::Item) => {
                newline(&mut out, quote);
                out.push_str(&"  ".repeat(lists.len().saturating_sub(1)));
                match lists.last_mut() {
                    Some(Some(n)) => {
                        out.push_str(&format!("{n}. "));
                        *n += 1;
                    }
                    _ => out.push_str("• "),
                }
            }
            Event::Start(Tag::CodeBlock(_)) => {
                newline(&mut out, quote);
                code = true;
            }
            Event::End(TagEnd::CodeBlock) => code = false,
            Event::Start(Tag::Strong) => styled = Some(|text| bold(text)),
            Event::Start(Tag::Emphasis) => styled = Some(|text| dim(text)),
            Event::End(TagEnd::Strong | TagEnd::Emphasis) => styled = None,
            Event::Start(Tag::Link { dest_url, .. }) => link = Some(dest_url.into_string()),
            Event::End(TagEnd::Link) => {
                if let Some(url) = link.take().filter(|url| url.starts_with("http")) {
                    out.push_str(&dim(format!(" <{url}>")));
                }
            }
            Event::Start(Tag::Image { .. }) => image = Some(String::new()),
            Event::End(TagEnd::Image) => {
                let alt = image.take().unwrap_or_default();
                let alt = alt.trim();
                out.push_str(&dim(if alt.is_empty() {
                    "[image]".to_string()
                } else {
                    format!("[image: {alt}]")
                }));
                out.push(' ');
            }
            Event::Text(text) => {
                let text = text.into_string();
                if let Some(alt) = image.as_mut() {
                    alt.push_str(&text);
                    continue;
                }
                if code {
                    for line in text.lines() {
                        out.push_str(&format!("    {}\n", accent(line)));
                    }
                    continue;
                }
                match styled {
                    Some(paint) => out.push_str(&paint(&text)),
                    None if link.is_some() => out.push_str(&accent(&text)),
                    None => {
                        let prefix = format!("\n{}", "│ ".repeat(quote));
                        out.push_str(&text.replace('\n', &prefix));
                    }
                }
            }
            Event::Code(snippet) => out.push_str(&accent(&snippet)),
            Event::SoftBreak => out.push(' '),
            Event::HardBreak => newline(&mut out, quote),
            Event::Rule => {
                newline(&mut out, quote);
                out.push_str(&dim("─".repeat(40)));
                out.push('\n');
            }
            Event::TaskListMarker(done) => out.push_str(if done { "[x] " } else { "[ ] " }),
            _ => {}
        }
    }

    let mut collapsed = String::new();
    let mut blank = 0;
    for line in out.trim().lines() {
        blank = if line.trim().is_empty() { blank + 1 } else { 0 };
        if blank < 2 {
            collapsed.push_str(line.trim_end());
            collapsed.push('\n');
        }
    }
    collapsed
}
