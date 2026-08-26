use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

const BREAK: &str = "\u{e000}";
const BREAK_MARKDOWN: &str = "  \n";
const BREAK_IN_TABLE: &str = " ";

pub fn normalize_markdown(input: &str) -> String {
    let islands = html_islands(input);
    if islands.is_empty() {
        return input.to_owned();
    }

    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;

    for island in islands {
        out.push_str(&input[cursor..island.range.start]);
        out.push_str(&convert(&input[island.range.clone()], &island));
        cursor = island.range.end;
    }
    out.push_str(&input[cursor..]);

    out
}

struct HtmlIsland {
    range: Range<usize>,
    block: bool,
    in_table: bool,
}

fn html_islands(input: &str) -> Vec<HtmlIsland> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);

    let mut islands: Vec<HtmlIsland> = Vec::new();
    let mut tables = 0usize;

    for (event, range) in Parser::new_ext(input, options).into_offset_iter() {
        let block = match event {
            Event::Start(Tag::Table(_)) => {
                tables += 1;
                continue;
            }
            Event::End(TagEnd::Table) => {
                tables = tables.saturating_sub(1);
                continue;
            }
            Event::Html(_) => true,
            Event::InlineHtml(_) => false,
            _ => continue,
        };

        match islands.last_mut() {
            Some(last) if last.range.end >= range.start => {
                last.range.end = range.end;
                last.block |= block;
            }
            _ => islands.push(HtmlIsland {
                range,
                block,
                in_table: tables > 0,
            }),
        }
    }

    islands
}

fn convert(html: &str, island: &HtmlIsland) -> String {
    let Ok(markdown) = htmd::convert(&replace_break_tags(html)) else {
        return String::new();
    };

    let markdown = markdown.replace(
        BREAK,
        if island.in_table {
            BREAK_IN_TABLE
        } else {
            BREAK_MARKDOWN
        },
    );

    if !island.block {
        return markdown;
    }

    match markdown.trim() {
        "" => "\n\n".to_owned(),
        trimmed => format!("\n\n{trimmed}\n\n"),
    }
}

fn replace_break_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;

    while let Some(open) = rest.find('<') {
        let Some(close) = rest[open..].find('>').map(|end| open + end) else {
            break;
        };

        out.push_str(&rest[..open]);
        if is_break_tag(&rest[open + 1..close]) {
            out.push_str(BREAK);
        } else {
            out.push_str(&rest[open..=close]);
        }
        rest = &rest[close + 1..];
    }

    out.push_str(rest);
    out
}

fn is_break_tag(tag: &str) -> bool {
    let tag = tag.trim_end_matches('/');
    let name = tag
        .split_once(|c: char| c.is_ascii_whitespace())
        .map_or(tag, |(name, _)| name);

    name.eq_ignore_ascii_case("br")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaves_plain_markdown_untouched() {
        let input = "# Title\n\nSome *text* with a [link](https://example.com).\n\n- a\n- b\n";
        assert_eq!(normalize_markdown(input), input);
    }

    #[test]
    fn converts_a_centered_header_block() {
        let out = normalize_markdown(
            "<p align=\"center\">\n<img src=\"banner.png\" alt=\"Banner\">\n</p>\n\nBody text here.",
        );

        assert!(out.contains("![Banner](banner.png)"), "{out}");
        assert!(!out.contains("<p"), "{out}");
        assert!(out.contains("\n\nBody text here."), "{out}");
    }

    #[test]
    fn breaks_lines_on_inline_br() {
        let out = normalize_markdown("Line one<br>Line two<br/>Line three");

        assert!(!out.contains("<br"), "{out}");
        assert_eq!(out.matches("  \n").count(), 2, "{out}");
    }

    #[test]
    fn keeps_a_break_flat_inside_a_table() {
        let out = normalize_markdown("| a | b |\n| - | - |\n| one<br>two | c |\n");

        assert!(!out.contains("<br"), "{out}");
        assert_eq!(out.lines().count(), 3, "{out}");
    }

    #[test]
    fn leaves_html_inside_fenced_code_alone() {
        let input = "Example:\n\n```html\n<br>\n<p align=\"center\">x</p>\n```\n";
        assert_eq!(normalize_markdown(input), input);
    }

    #[test]
    fn unwraps_a_div_of_headings_and_links() {
        let out = normalize_markdown(
            "<div align=\"center\">\n<h1>Mod</h1>\n<h3>Tagline</h3>\n<a href=\"https://example.com\"><img src=\"badge.svg\" alt=\"Badge\"></a>\n</div>\n\nAfter.",
        );

        assert!(out.contains("# Mod"), "{out}");
        assert!(out.contains("### Tagline"), "{out}");
        assert!(out.contains("[![Badge](badge.svg)](https://example.com)"), "{out}");
        assert!(!out.contains("<div"), "{out}");
    }

    #[test]
    fn converts_a_details_block() {
        let out = normalize_markdown(
            "## Changelog\n\n<details>\n<summary>1.0.0</summary>\n\nFixed a crash.\n\n</details>\n",
        );

        assert!(!out.contains("<details>"), "{out}");
        assert!(out.contains("Fixed a crash."), "{out}");
    }

    #[test]
    fn does_not_mistake_other_tags_for_breaks() {
        assert_eq!(replace_break_tags("<brick>x</brick>"), "<brick>x</brick>");
        assert_eq!(replace_break_tags("a<br class=\"x\">b"), format!("a{BREAK}b"));
    }
}
