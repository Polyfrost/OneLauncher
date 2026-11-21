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
    let markdown = match htmd::convert(&replace_iframes(&replace_break_tags(html))) {
        Ok(markdown) => markdown,
        Err(err) => {
            tracing::warn!("failed to convert an html block to markdown, keeping it as-is: {err}");
            return html.to_owned();
        }
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

/// An iframe the author wrote into a description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Iframe {
    /// Always an absolute `http(s)` url
    pub src: String,
    pub title: Option<String>,
}

/// Every iframe in a piece of html that points at a web page, in order.
///
/// Nothing is fetched or run, this only reads the tag. Iframes without an `http(s)` source, such
/// as `srcdoc` or `javascript:` ones, are not returned.
pub fn iframes(html: &str) -> Vec<Iframe> {
    scan_iframes(html)
        .into_iter()
        .filter_map(|found| found.iframe)
        .collect()
}

struct IframeSpan {
    range: Range<usize>,
    iframe: Option<Iframe>,
}

fn scan_iframes(html: &str) -> Vec<IframeSpan> {
    let lower = html.to_ascii_lowercase();
    let mut found = Vec::new();
    let mut cursor = 0;

    while let Some(offset) = lower[cursor..].find("<iframe") {
        let start = cursor + offset;
        let after_name = start + "<iframe".len();
        if !html[after_name..]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_whitespace() || c == '>' || c == '/')
        {
            cursor = after_name;
            continue;
        }

        let Some(tag_end) = tag_end(html, after_name) else {
            break;
        };

        let end = match lower[tag_end..].find("</iframe") {
            Some(close) => match lower[tag_end + close..].find('>') {
                Some(gt) => tag_end + close + gt + 1,
                None => tag_end,
            },
            None => tag_end,
        };

        let attrs = &html[after_name..tag_end - 1];
        found.push(IframeSpan {
            range: start..end,
            iframe: attribute(attrs, "src")
                .and_then(|src| web_url(&src))
                .map(|src| Iframe {
                    src,
                    title: attribute(attrs, "title").filter(|title| !title.trim().is_empty()),
                }),
        });
        cursor = end;
    }

    found
}

/// The offset just past the `>` closing the tag whose attributes start at `from`.
fn tag_end(html: &str, from: usize) -> Option<usize> {
    let mut quote = None;

    for (offset, c) in html[from..].char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(open), c) if c == open => quote = None,
            (None, '>') => return Some(from + offset + 1),
            _ => {}
        }
    }

    None
}

/// The decoded value of one attribute out of a tag's attribute text.
fn attribute(attrs: &str, name: &str) -> Option<String> {
    let lower = attrs.to_ascii_lowercase();
    let mut from = 0;

    while let Some(offset) = lower[from..].find(name) {
        let start = from + offset;
        let end = start + name.len();
        from = end;

        let boundary = attrs[..start]
            .chars()
            .next_back()
            .is_none_or(|c| c.is_ascii_whitespace() || c == '"' || c == '\'');
        if !boundary {
            continue;
        }

        let Some(rest) = attrs[end..].trim_start().strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim_start();

        let value = match rest.chars().next() {
            Some(quote @ ('"' | '\'')) => {
                let inner = &rest[1..];
                &inner[..inner.find(quote)?]
            }
            Some(_) => {
                &rest[..rest
                    .find(|c: char| c.is_ascii_whitespace())
                    .unwrap_or(rest.len())]
            }
            None => return None,
        };

        return Some(decode_entities(value));
    }

    None
}

fn decode_entities(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn encode_entities(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// An absolute `http(s)` url, with protocol-relative ones (`//host/path`) taken as https.
fn web_url(src: &str) -> Option<String> {
    let src = src.trim();
    let absolute = match src.strip_prefix("//") {
        Some(rest) => format!("https://{rest}"),
        None => src.to_owned(),
    };

    let url = url::Url::parse(&absolute).ok()?;
    (matches!(url.scheme(), "http" | "https") && url.host_str().is_some()).then(|| url.to_string())
}

/// Turns each iframe into a plain link to its source, which htmd would otherwise drop with the
/// rest of the elements it cannot express as markdown.
fn replace_iframes(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut cursor = 0;

    for found in scan_iframes(html) {
        out.push_str(&html[cursor..found.range.start]);
        if let Some(iframe) = found.iframe {
            let label = iframe.title.as_deref().unwrap_or(&iframe.src);
            out.push_str(&format!(
                "<a href=\"{}\">{}</a>",
                encode_entities(&iframe.src),
                encode_entities(label)
            ));
        }
        cursor = found.range.end;
    }
    out.push_str(&html[cursor..]);

    out
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
        assert!(
            out.contains("[![Badge](badge.svg)](https://example.com)"),
            "{out}"
        );
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

    const WWAYPOINTS: &str = "### Officially Allowed on Hoplite\n\nwWaypoints is the new best waypoint mod.\n\n- Easy to use with clean, optimized menus\n- Highlights blocks, making it the #1 Trapping mod\n# Watch Sheet's Showcase (0.6.0):\n<iframe width=\"560\" height=\"315\" src=\"https://www.youtube-nocookie.com/embed/WaRLLta2Edg\" title=\"YouTube video player\" frameborder=\"0\" allow=\"accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share\" allowfullscreen></iframe>";

    #[test]
    fn keeps_the_wwaypoints_youtube_embed_as_a_link_after_its_heading() {
        let out = normalize_markdown(WWAYPOINTS);

        assert!(!out.contains("<iframe"), "{out}");
        assert!(
            out.contains("# Watch Sheet's Showcase (0.6.0):"),
            "heading kept: {out}"
        );
        assert!(
            out.contains(
                "[YouTube video player](https://www.youtube-nocookie.com/embed/WaRLLta2Edg)"
            ),
            "{out}"
        );
        assert!(out.contains("- Highlights blocks"), "list kept: {out}");

        let heading = out.find("# Watch Sheet").unwrap();
        let embed = out.find("youtube-nocookie.com").unwrap();
        assert!(embed > heading, "embed follows its heading: {out}");
    }

    #[test]
    fn reads_an_iframes_source_and_title() {
        let found = iframes(
            "<p>x</p><IFRAME src='//player.vimeo.com/video/76979871?h=a&amp;b=1' title=\"A &amp; B\"></IFRAME>",
        );

        assert_eq!(
            found,
            vec![Iframe {
                src: "https://player.vimeo.com/video/76979871?h=a&b=1".into(),
                title: Some("A & B".into()),
            }]
        );
    }

    #[test]
    fn ignores_iframes_that_are_not_web_pages() {
        assert!(iframes("<iframe src=\"javascript:alert(1)\"></iframe>").is_empty());
        assert!(iframes("<iframe srcdoc=\"<script>1</script>\"></iframe>").is_empty());
        assert!(iframes("<iframe></iframe>").is_empty());
        assert!(iframes("<iframes src=\"https://example.com\">").is_empty());
        assert!(iframes("<iframe data-src=\"https://example.com/x\"></iframe>").is_empty());
    }

    #[test]
    fn a_dangerous_iframe_is_dropped_not_converted() {
        let out =
            normalize_markdown("before\n\n<iframe src=\"javascript:alert(1)\"></iframe>\n\nafter");

        assert!(!out.contains("javascript"), "{out}");
        assert!(out.contains("before") && out.contains("after"), "{out}");
    }

    #[test]
    fn an_inline_iframe_keeps_its_surrounding_text() {
        let out = normalize_markdown(
            "see <iframe src=\"https://www.youtube.com/embed/WaRLLta2Edg\"></iframe> here",
        );

        assert!(out.contains("see "), "{out}");
        assert!(
            out.contains("https://www.youtube.com/embed/WaRLLta2Edg"),
            "{out}"
        );
        assert!(out.contains(" here"), "{out}");
    }

    #[test]
    fn leaves_an_iframe_inside_a_code_fence_alone() {
        let input =
            "```html\n<iframe src=\"https://www.youtube.com/embed/WaRLLta2Edg\"></iframe>\n```\n";
        assert_eq!(normalize_markdown(input), input);
    }

    #[test]
    fn does_not_mistake_other_tags_for_breaks() {
        assert_eq!(replace_break_tags("<brick>x</brick>"), "<brick>x</brick>");
        assert_eq!(
            replace_break_tags("a<br class=\"x\">b"),
            format!("a{BREAK}b")
        );
    }
}
