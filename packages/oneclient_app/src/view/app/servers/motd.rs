//! Minecraft's section-sign formatting codes, as the server list renders them

use freya::prelude::*;

use crate::components::Button;
use crate::i18n::t;
use crate::theme::{colors, rounded, text_size};
use crate::ui::border_all_color;

pub const SECTION: char = '\u{a7}';

/// The 16 chat colors, in code order 0-9 then a-f
pub const COLORS: [(char, Color); 16] = [
    ('0', Color::from_rgb(0, 0, 0)),
    ('1', Color::from_rgb(0, 0, 170)),
    ('2', Color::from_rgb(0, 170, 0)),
    ('3', Color::from_rgb(0, 170, 170)),
    ('4', Color::from_rgb(170, 0, 0)),
    ('5', Color::from_rgb(170, 0, 170)),
    ('6', Color::from_rgb(255, 170, 0)),
    ('7', Color::from_rgb(170, 170, 170)),
    ('8', Color::from_rgb(85, 85, 85)),
    ('9', Color::from_rgb(85, 85, 255)),
    ('a', Color::from_rgb(85, 255, 85)),
    ('b', Color::from_rgb(85, 255, 255)),
    ('c', Color::from_rgb(255, 85, 85)),
    ('d', Color::from_rgb(255, 85, 255)),
    ('e', Color::from_rgb(255, 255, 85)),
    ('f', Color::from_rgb(255, 255, 255)),
];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Run {
    pub text: String,
    pub color: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub obfuscated: bool,
}

/// Splits text into styled runs. A color code clears the formats before it, like in game
pub fn parse(text: &str) -> Vec<Run> {
    let mut runs = Vec::new();
    let mut current = Run::default();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != SECTION {
            current.text.push(c);
            continue;
        }
        let Some(code) = chars.next().map(|c| c.to_ascii_lowercase()) else {
            break;
        };
        let style = Run {
            text: String::new(),
            ..current.clone()
        };
        if !current.text.is_empty() {
            runs.push(std::mem::take(&mut current));
        }
        current = style;
        if let Some((_, color)) = COLORS.iter().find(|(k, _)| *k == code) {
            current = Run {
                color: Some(*color),
                ..Run::default()
            };
            continue;
        }
        match code {
            'k' => current.obfuscated = true,
            'l' => current.bold = true,
            'm' => current.strike = true,
            'n' => current.underline = true,
            'o' => current.italic = true,
            'r' => current = Run::default(),
            _ => {}
        }
    }
    if !current.text.is_empty() {
        runs.push(current);
    }
    runs
}

/// shortcut: obfuscated text is scrambled once, the game keeps cycling the glyphs
fn scramble(text: &str) -> String {
    const GLYPHS: &[char] = &['#', '%', '&', '$', '@', 'X', 'W', 'M', '8', '?'];
    text.chars()
        .enumerate()
        .map(|(i, c)| {
            if c.is_whitespace() {
                c
            } else {
                GLYPHS[(i * 7 + c as usize) % GLYPHS.len()]
            }
        })
        .collect()
}

pub fn motd_text(text: &str, default: Color, line_height: f32) -> Element {
    paragraph()
        .max_lines(2)
        .line_height(line_height)
        .spans_iter(parse(text).into_iter().map(|run| {
            let body = if run.obfuscated {
                scramble(&run.text)
            } else {
                run.text
            };
            let mut span = Span::new(body).color(run.color.unwrap_or(default));
            if run.bold {
                span = span.font_weight(FontWeight::BOLD);
            }
            if run.italic {
                span = span.font_slant(FontSlant::Italic);
            }
            // shortcut: one decoration per span, underline wins when both are set
            if run.underline {
                span = span.text_decoration(TextDecoration::Underline);
            } else if run.strike {
                span = span.text_decoration(TextDecoration::LineThrough);
            }
            span
        }))
        .into_element()
}

/// Color swatches and format buttons that add a code to the end of the MOTD
pub fn code_picker(mut motd: State<String>) -> Element {
    let mut add = move |code: char| motd.write().push_str(&format!("{SECTION}{code}"));

    let swatches = COLORS.iter().map(|(code, color)| {
        let code = *code;
        rect()
            .width(Size::px(22.))
            .height(Size::px(22.))
            .corner_radius(rounded(5.))
            .background(*color)
            .border(border_all_color(1., colors::component_border()))
            .cursor(CursorIcon::Pointer)
            .a11y_role(AccessibilityRole::Button)
            .on_press(move |_| add(code))
            .into_element()
    });

    // Each button shows its own name with the effect it adds, obfuscated stays readable
    let formats = [
        ('l', true, t("oneclient.servers.settings.motd.bold")),
        ('o', true, t("oneclient.servers.settings.motd.italic")),
        ('n', true, t("oneclient.servers.settings.motd.underline")),
        ('m', true, t("oneclient.servers.settings.motd.strikethrough")),
        ('k', false, t("oneclient.servers.settings.motd.obfuscated")),
        ('r', false, t("oneclient.servers.settings.motd.reset")),
    ]
    .into_iter()
    .map(|(code, styled, name)| {
        let sample = if styled { format!("{SECTION}{code}{name}") } else { name.to_string() };
        Button::new()
            .secondary()
            .small()
            .on_press(move |_| add(code))
            .child(motd_text(&sample, colors::fg_primary(), 1.2))
            .into_element()
    });

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(8.)
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::wrap_spacing(6.))
                .spacing(6.)
                .children(swatches),
        )
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::wrap_spacing(6.))
                .spacing(6.)
                .children(formats),
        )
        .child(
            label()
                .text(t("oneclient.servers.settings.motd.hint"))
                .font_size(text_size(11.))
                .color(colors::fg_secondary()),
        )
        .into_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_reset_formats_and_r_resets_everything() {
        let runs = parse("§lBold§cRed§oItalic§rPlain");
        assert_eq!(runs.len(), 4);
        assert!(runs[0].bold && runs[0].color.is_none());
        assert_eq!(runs[1].color, Some(Color::from_rgb(255, 85, 85)));
        assert!(!runs[1].bold);
        assert!(runs[2].italic && runs[2].color == Some(Color::from_rgb(255, 85, 85)));
        assert_eq!(
            runs[3],
            Run {
                text: "Plain".into(),
                ..Run::default()
            }
        );
    }
}
