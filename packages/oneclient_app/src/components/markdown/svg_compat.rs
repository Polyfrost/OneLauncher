//! Rewrites the parts of an svg Skia's svg module can't read.
//!
//! shields.io embeds badge logos as `<image href="data:...">`. Skia only reads
//! the SVG 1.1 `xlink:href`, and its `<image>` only decodes bitmaps, so svg logos
//! (`logo=discord`, `logo=modrinth`, ...) are inlined as nested `<svg>` elements instead.

use std::borrow::Cow;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

const XLINK_NS: &str = "http://www.w3.org/1999/xlink";

/// Logos are a few kilobytes, anything bigger is left for Skia to skip
const MAX_INLINED_SVG: usize = 256 * 1024;
const MAX_INLINED_IMAGES: usize = 16;

/// The `<image>` attributes that position the picture, carried onto the inlined `<svg>`
const PLACEMENT: [&str; 5] = ["x", "y", "width", "height", "preserveAspectRatio"];

pub(super) fn prepare_svg(bytes: &[u8]) -> Cow<'_, [u8]> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Cow::Borrowed(bytes);
    };

    match rewrite_images(text) {
        Some(rewritten) => Cow::Owned(rewritten.into_bytes()),
        None => Cow::Borrowed(bytes),
    }
}

fn rewrite_images(text: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut changed = false;
    let mut uses_xlink = false;
    let mut inlined = 0;

    while let Some(at) = find_tag(rest, "image") {
        out.push_str(&rest[..at]);
        let from_tag = &rest[at..];

        let Some(tag) = Tag::parse(from_tag, "image") else {
            out.push_str(from_tag);
            rest = "";
            break;
        };
        rest = &from_tag[tag.len..];

        if inlined < MAX_INLINED_IMAGES
            && let Some(nested) = inline_svg_image(&tag, &mut rest, inlined)
        {
            out.push_str(&nested);
            inlined += 1;
            changed = true;
            continue;
        }

        if tag.attr("href").is_some() && tag.attr("xlink:href").is_none() {
            out.push_str(&tag.renamed("href", "xlink:href").to_source());
            uses_xlink = true;
            changed = true;
        } else {
            out.push_str(&from_tag[..tag.len]);
        }
    }
    out.push_str(rest);

    if !changed {
        return None;
    }

    if uses_xlink {
        declare_xlink(&mut out);
    }

    Some(out)
}

/// Replaces an `<image>` holding an svg data url with that svg, placed where the image was
fn inline_svg_image(tag: &Tag<'_>, rest: &mut &str, index: usize) -> Option<String> {
    let href = tag.attr("href").or_else(|| tag.attr("xlink:href"))?;
    // the logo's ids now share the badge's document, where shields.io already uses `r`, `s` and `blur`
    let document = prefix_ids(&decode_svg_data_url(href)?, &format!("logo{index}-"));

    let start = find_tag(&document, "svg")?;
    let inner = Tag::parse(&document[start..], "svg")?;
    let body = &document[start + inner.len..];

    // `<image ...></image>` has a closing tag the nested svg must swallow
    if !tag.self_closing {
        let trimmed = rest.trim_start();
        *rest = trimmed.strip_prefix("</image>")?;
    }

    let mut nested = inner.without(&PLACEMENT);
    for name in PLACEMENT {
        if let Some(value) = tag.attr(name) {
            nested.attrs.push((name, value, '"'));
        }
    }

    let mut out = nested.to_source();
    out.push_str(body);

    Some(match tag.attr("transform") {
        Some(transform) => format!("<g transform=\"{transform}\">{out}</g>"),
        None => out,
    })
}

fn decode_svg_data_url(href: &str) -> Option<String> {
    let payload = href.trim().strip_prefix("data:image/svg+xml")?;
    let (params, data) = payload.split_once(',')?;
    if !params.split(';').any(|param| param == "base64") {
        return None;
    }

    let data: String = data.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    // base64 grows by a third, so the encoded length bounds the decoded one
    if data.len() / 4 * 3 > MAX_INLINED_SVG {
        return None;
    }

    let decoded = String::from_utf8(STANDARD.decode(data).ok()?).ok()?;
    // one level of nesting, an inlined logo can't pull in further svgs
    (!decoded.contains("data:image/svg+xml")).then_some(decoded)
}

/// Prefixes every `id` attribute and the `url(#id)` / `href="#id"` references to it
fn prefix_ids(svg: &str, prefix: &str) -> String {
    let mut ids = Vec::new();
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;

    while let Some(at) = rest.find("id=") {
        let attr_start = at > 0 && rest[..at].ends_with(|c: char| c.is_ascii_whitespace());
        let (before, after) = rest.split_at(at + "id=".len());
        out.push_str(before);
        rest = after;

        let Some(quote) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            continue;
        };
        let Some(len) = rest[1..].find(quote) else {
            continue;
        };
        if !attr_start || len == 0 {
            continue;
        }

        let id = &rest[1..1 + len];
        out.push(quote);
        out.push_str(prefix);
        out.push_str(id);
        out.push(quote);
        ids.push(id);
        rest = &rest[len + 2..];
    }
    out.push_str(rest);

    for id in ids {
        for (from, to) in [
            ("url(#", ")"),
            ("url('#", "')"),
            ("url(\"#", "\")"),
            ("href=\"#", "\""),
            ("href='#", "'"),
        ] {
            out = out.replace(
                &format!("{from}{id}{to}"),
                &format!("{from}{prefix}{id}{to}"),
            );
        }
    }

    out
}

fn declare_xlink(svg: &mut String) {
    let Some(root) = find_tag(svg, "svg") else {
        return;
    };
    let Some(tag) = Tag::parse(&svg[root..], "svg") else {
        return;
    };
    if tag.attr("xmlns:xlink").is_some() {
        return;
    }

    svg.insert_str(root + "<svg".len(), &format!(" xmlns:xlink=\"{XLINK_NS}\""));
}

/// Byte offset of the next `<name` start tag, skipping longer names sharing the prefix
fn find_tag(text: &str, name: &str) -> Option<usize> {
    let open = format!("<{name}");
    let mut from = 0;

    while let Some(found) = text[from..].find(&open) {
        let at = from + found;
        let next = text[at + open.len()..].chars().next();
        if matches!(next, Some(c) if c.is_ascii_whitespace() || c == '/' || c == '>') {
            return Some(at);
        }
        from = at + open.len();
    }

    None
}

/// A start tag, values kept as written so re-emitting them needs no escaping
#[derive(Debug, PartialEq)]
struct Tag<'a> {
    name: &'a str,
    attrs: Vec<(&'a str, &'a str, char)>,
    self_closing: bool,
    /// Bytes of the source the tag spans, `>` included
    len: usize,
}

impl<'a> Tag<'a> {
    fn parse(source: &'a str, name: &'a str) -> Option<Self> {
        let mut at = source.strip_prefix('<')?.strip_prefix(name)?.len();
        at = source.len() - at;
        let mut attrs = Vec::new();

        loop {
            at += whitespace(&source[at..]);
            let rest = &source[at..];

            if let Some(after) = rest.strip_prefix("/>") {
                let len = source.len() - after.len();
                return Some(Self {
                    name,
                    attrs,
                    self_closing: true,
                    len,
                });
            }
            if rest.starts_with('>') {
                return Some(Self {
                    name,
                    attrs,
                    self_closing: false,
                    len: at + 1,
                });
            }

            let name_len =
                rest.find(|c: char| c == '=' || c.is_ascii_whitespace() || c == '>' || c == '/')?;
            if name_len == 0 {
                return None;
            }
            let attr = &rest[..name_len];
            at += name_len;
            at += whitespace(&source[at..]);
            source[at..].strip_prefix('=')?;
            at += 1;
            at += whitespace(&source[at..]);

            let quote = source[at..]
                .chars()
                .next()
                .filter(|c| *c == '"' || *c == '\'')?;
            at += 1;
            let value_len = source[at..].find(quote)?;
            attrs.push((attr, &source[at..at + value_len], quote));
            at += value_len + 1;
        }
    }

    fn attr(&self, name: &str) -> Option<&'a str> {
        self.attrs
            .iter()
            .find(|(attr, _, _)| *attr == name)
            .map(|(_, value, _)| *value)
    }

    fn renamed(mut self, from: &str, to: &'a str) -> Self {
        for attr in &mut self.attrs {
            if attr.0 == from {
                attr.0 = to;
            }
        }
        self
    }

    fn without(&self, names: &[&str]) -> Self {
        Self {
            name: self.name,
            attrs: self
                .attrs
                .iter()
                .filter(|(attr, _, _)| !names.contains(attr))
                .copied()
                .collect(),
            self_closing: self.self_closing,
            len: self.len,
        }
    }

    fn to_source(&self) -> String {
        let mut out = format!("<{}", self.name);
        for (attr, value, quote) in &self.attrs {
            out.push_str(&format!(" {attr}={quote}{value}{quote}"));
        }
        out.push_str(if self.self_closing { "/>" } else { ">" });
        out
    }
}

fn whitespace(text: &str) -> usize {
    text.len()
        - text
            .trim_start_matches(|c: char| c.is_ascii_whitespace())
            .len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepared(svg: &str) -> String {
        String::from_utf8(prepare_svg(svg.as_bytes()).into_owned()).expect("utf8")
    }

    fn svg_data_url(svg: &str) -> String {
        format!("data:image/svg+xml;base64,{}", STANDARD.encode(svg))
    }

    #[test]
    fn leaves_an_svg_without_images_untouched() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><rect width="1" height="1"/></svg>"#;
        assert!(matches!(prepare_svg(svg.as_bytes()), Cow::Borrowed(_)));
    }

    #[test]
    fn renames_href_and_declares_xlink_once() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><image x="5" href="data:image/png;base64,AAAA"/><image href="b.png"/></svg>"#;
        assert_eq!(
            prepared(svg),
            r#"<svg xmlns:xlink="http://www.w3.org/1999/xlink" xmlns="http://www.w3.org/2000/svg"><image x="5" xlink:href="data:image/png;base64,AAAA"/><image xlink:href="b.png"/></svg>"#
        );
    }

    #[test]
    fn keeps_an_existing_xlink_declaration() {
        let svg = r#"<svg xmlns:xlink="http://www.w3.org/1999/xlink"><image href="a.png"/></svg>"#;
        assert_eq!(
            prepared(svg),
            r#"<svg xmlns:xlink="http://www.w3.org/1999/xlink"><image xlink:href="a.png"/></svg>"#
        );
    }

    #[test]
    fn inlines_an_svg_logo_where_the_image_was() {
        let logo = r##"<?xml version="1.0"?><svg fill="#5865F2" viewBox="0 0 24 24" width="24" xmlns="http://www.w3.org/2000/svg"><path d="M0 0h24v24H0z"/></svg>"##;
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><image x="5" y="3" width="14" height="14" href="{}"/><text>Discord</text></svg>"#,
            svg_data_url(logo)
        );

        assert_eq!(
            prepared(&svg),
            r##"<svg xmlns="http://www.w3.org/2000/svg"><svg fill="#5865F2" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" x="5" y="3" width="14" height="14"><path d="M0 0h24v24H0z"/></svg><text>Discord</text></svg>"##
        );
    }

    #[test]
    fn swallows_the_closing_tag_of_an_open_image() {
        let logo = r#"<svg viewBox="0 0 1 1"/>"#;
        let svg = format!(
            r#"<svg><image width="2" href="{}"> </image></svg>"#,
            svg_data_url(logo)
        );

        assert_eq!(
            prepared(&svg),
            r#"<svg><svg viewBox="0 0 1 1" width="2"/></svg>"#
        );
    }

    #[test]
    fn malformed_base64_falls_back_to_the_xlink_rename() {
        let svg = r#"<svg><image href="data:image/svg+xml;base64,@@@"/></svg>"#;
        assert_eq!(
            prepared(svg),
            r#"<svg xmlns:xlink="http://www.w3.org/1999/xlink"><image xlink:href="data:image/svg+xml;base64,@@@"/></svg>"#
        );
    }

    #[test]
    fn does_not_inline_a_logo_that_nests_another_svg() {
        let inner = svg_data_url(r#"<svg viewBox="0 0 1 1"/>"#);
        let logo = format!(r#"<svg viewBox="0 0 1 1"><image href="{inner}"/></svg>"#);
        let svg = format!(r#"<svg><image href="{}"/></svg>"#, svg_data_url(&logo));

        assert!(prepared(&svg).contains("<image xlink:href=\"data:image/svg+xml;base64,"));
    }

    #[test]
    fn prefixes_the_ids_of_an_inlined_logo() {
        let logo = r##"<svg viewBox="0 0 1 1"><clipPath id="r"><rect width="1" height="1"/></clipPath><linearGradient id='s'/><rect grid="r" clip-path="url(#r)" fill="url('#s')"/><use href="#r"/><use xlink:href="#s"/><rect fill="url(#rr)"/></svg>"##;
        let svg = format!(r#"<svg><image href="{}"/></svg>"#, svg_data_url(logo));

        assert_eq!(
            prepared(&svg),
            r##"<svg><svg viewBox="0 0 1 1"><clipPath id="logo0-r"><rect width="1" height="1"/></clipPath><linearGradient id='logo0-s'/><rect grid="r" clip-path="url(#logo0-r)" fill="url('#logo0-s')"/><use href="#logo0-r"/><use xlink:href="#logo0-s"/><rect fill="url(#rr)"/></svg></svg>"##
        );
    }

    #[test]
    fn each_inlined_logo_gets_its_own_prefix() {
        let logo = svg_data_url(r#"<svg><g id="a"/></svg>"#);
        let svg = format!(r#"<svg><image href="{logo}"/><image href="{logo}"/></svg>"#);

        assert_eq!(
            prepared(&svg),
            r#"<svg><svg><g id="logo0-a"/></svg><svg><g id="logo1-a"/></svg></svg>"#
        );
    }

    #[test]
    fn ignores_tags_that_only_share_the_prefix() {
        let svg = r#"<svg><imageset href="a"/></svg>"#;
        assert!(matches!(prepare_svg(svg.as_bytes()), Cow::Borrowed(_)));
    }

    #[test]
    fn unterminated_tag_is_left_alone() {
        let svg = r#"<svg><image href="a.png"#;
        assert!(matches!(prepare_svg(svg.as_bytes()), Cow::Borrowed(_)));
    }
}
