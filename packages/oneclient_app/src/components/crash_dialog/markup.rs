use crate::components::clean_line;

const MAX_LINE_COLS: usize = 400;
const ELLIPSIS: &str = "...";
const PATH_INDICATOR: &str = "%pathindicator%";

fn clamp_cols(line: &str) -> String {
    if line.chars().nth(MAX_LINE_COLS).is_none() {
        return line.to_string();
    }

    let kept: String = line.chars().take(MAX_LINE_COLS - ELLIPSIS.len()).collect();

    kept + ELLIPSIS
}

pub(super) fn display_line(raw: &str) -> String {
    clamp_cols(&clean_line(raw))
}

pub(super) fn resolve_placeholders(text: &str, game_dir: Option<&str>) -> String {
    let dir = game_dir.unwrap_or("your game folder");

    text.replace("%profileroot%", dir)
        .replace("%gameroot%", dir)
        .replace(PATH_INDICATOR, "`")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SegmentStyle {
    Plain,
    Code,
    Link,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TextSegment {
    pub(super) text: String,
    pub(super) style: SegmentStyle,
}

pub(super) fn segments(line: &str) -> Vec<TextSegment> {
    let mut out: Vec<TextSegment> = Vec::new();
    let mut plain = String::new();
    let mut rest = line;

    while !rest.is_empty() {
        if let Some(open) = rest.find('`') {
            let (before, after) = rest.split_at(open);
            plain.push_str(before);
            let after = &after[1..];

            let (code, remainder) = match after.find('`') {
                Some(close) => (&after[..close], &after[close + 1..]),
                None => (after, ""),
            };

            push_plain(&mut out, &mut plain);
            if !code.is_empty() {
                out.push(TextSegment {
                    text: code.to_string(),
                    style: SegmentStyle::Code,
                });
            }
            rest = remainder;
            continue;
        }

        plain.push_str(rest);
        break;
    }

    push_plain(&mut out, &mut plain);
    split_links(out)
}

fn push_plain(out: &mut Vec<TextSegment>, plain: &mut String) {
    let text = std::mem::take(plain);
    if !text.is_empty() {
        out.push(TextSegment {
            text,
            style: SegmentStyle::Plain,
        });
    }
}

fn push_text(out: &mut Vec<TextSegment>, text: &str, style: SegmentStyle) {
    if !text.is_empty() {
        out.push(TextSegment {
            text: text.to_string(),
            style,
        });
    }
}

fn split_links(segments: Vec<TextSegment>) -> Vec<TextSegment> {
    let mut out = Vec::new();

    for segment in segments {
        if segment.style != SegmentStyle::Plain {
            out.push(segment);
            continue;
        }

        let mut rest = segment.text.as_str();
        while let Some(found) = url_at(rest) {
            let before = &rest[..found.start];
            let leading = before.strip_suffix('<').unwrap_or(before);
            let trailing = found.raw[found.url.len()..].trim_start_matches('>');

            push_text(&mut out, leading, SegmentStyle::Plain);
            push_text(&mut out, found.url, SegmentStyle::Link);
            push_text(&mut out, trailing, SegmentStyle::Plain);

            rest = &rest[found.end()..];
        }

        push_text(&mut out, rest, SegmentStyle::Plain);
    }

    out
}

struct FoundUrl<'a> {
    start: usize,
    raw: &'a str,
    url: &'a str,
}

impl FoundUrl<'_> {
    fn end(&self) -> usize {
        self.start + self.raw.len()
    }
}

fn url_at(text: &str) -> Option<FoundUrl<'_>> {
    let start = [text.find("https://"), text.find("http://")]
        .into_iter()
        .flatten()
        .min()?;

    let from_url = &text[start..];
    let end = from_url.find(char::is_whitespace).unwrap_or(from_url.len());
    let raw = &from_url[..end];

    Some(FoundUrl {
        start,
        raw,
        url: trim_url(raw),
    })
}

fn trim_url(raw: &str) -> &str {
    raw.trim_end_matches(['>', '.', ',', ')', '"', '\'', ':', ';', '!', '?'])
}

pub(super) fn links_in(text: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut rest = text;

    while let Some(hit) = url_at(rest) {
        if !found.iter().any(|seen| seen == hit.url) {
            found.push(hit.url.to_string());
        }
        rest = &rest[hit.end()..];
    }

    found
}

pub(super) fn host_of(url: &str) -> String {
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or(url)
        .trim_start_matches("www.")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_placeholders_become_a_code_span() {
        let out = resolve_placeholders(
            "Remove it:\n%pathindicator%%profileroot%/mods/%pathindicator%",
            Some("C:/games/skyblock"),
        );

        assert_eq!(out, "Remove it:\n`C:/games/skyblock/mods/`");
    }

    #[test]
    fn a_missing_game_dir_still_reads_as_a_sentence() {
        let out = resolve_placeholders("Open %profileroot%/mods", None);
        assert_eq!(out, "Open your game folder/mods");
    }

    #[test]
    fn a_code_span_is_split_out() {
        let out = segments("Navigate to `mods/` then stop");

        assert_eq!(
            out,
            vec![
                TextSegment {
                    text: "Navigate to ".into(),
                    style: SegmentStyle::Plain
                },
                TextSegment {
                    text: "mods/".into(),
                    style: SegmentStyle::Code
                },
                TextSegment {
                    text: " then stop".into(),
                    style: SegmentStyle::Plain
                },
            ]
        );
    }

    #[test]
    fn an_unterminated_code_span_runs_to_the_end() {
        let out = segments("Open `mods/");

        assert_eq!(out.last().unwrap().style, SegmentStyle::Code);
        assert_eq!(out.last().unwrap().text, "mods/");
    }

    #[test]
    fn discord_no_embed_brackets_are_stripped() {
        let out = segments("Update it: <https://modrinth.com/mod/scc>");

        let link = out
            .iter()
            .find(|segment| segment.style == SegmentStyle::Link)
            .unwrap();
        assert_eq!(link.text, "https://modrinth.com/mod/scc");

        assert!(
            out.iter().all(|segment| !segment.text.contains('<')),
            "{out:?}"
        );
        assert!(
            out.iter().all(|segment| !segment.text.contains('>')),
            "{out:?}"
        );
    }

    #[test]
    fn trailing_sentence_punctuation_stays_out_of_the_url() {
        let links = links_in("Get it at https://github.com/Skytils/SkytilsMod/releases/latest.");

        assert_eq!(
            links,
            vec!["https://github.com/Skytils/SkytilsMod/releases/latest"]
        );
    }

    #[test]
    fn every_distinct_link_is_collected_once() {
        let links = links_in(
            "See <https://polyfrost.org> and <https://polyfrost.org> and https://modrinth.com/x",
        );

        assert_eq!(
            links,
            vec!["https://polyfrost.org", "https://modrinth.com/x"]
        );
    }

    #[test]
    fn a_line_with_no_link_yields_none() {
        assert!(links_in("Reset the Controls to List Players and Open Chat").is_empty());
    }

    #[test]
    fn the_host_is_what_labels_the_chip() {
        assert_eq!(host_of("https://www.modrinth.com/mod/scc"), "modrinth.com");
        assert_eq!(host_of("https://discord.gg/dg"), "discord.gg");
    }
}
