use std::sync::Arc;

use freya::prelude::*;
use freya::router::*;

use oneclient_common::constants::DISCORD_URL;
use oneclient_events::{CrashRemedy, GameCrash};

use crate::Route;
use crate::components::{Button, Icon, IconType, OverlayPopup, ScrollArea, clean_line};
use crate::hooks::{use_dispatch, use_link_confirm};
use crate::theme;
use crate::theme::colors;
use crate::utils::format_duration_hms;

const DIALOG_BG: Color = Color::from_rgb(21, 28, 34);
const CODE_BG: Color = Color::from_rgb(13, 18, 22);

const EXCERPT_HEIGHT: f32 = 190.;

const FIXES_HEIGHT: f32 = 250.;

const MAX_LINK_CHIPS: usize = 3;

const CHAR_W_TEXT: f32 = 7.8;
const CHAR_W_MONO: f32 = 7.3;
const FIX_LINE_H: f32 = 20.;

const V_SCROLLBAR_W: f32 = 16.;
const H_SCROLLBAR_H: f32 = 14.;

#[derive(PartialEq)]
pub struct CrashDialog {
    pub crash: Arc<GameCrash>,
}

impl Component for CrashDialog {
    fn render(&self) -> impl IntoElement {
        let crash = Arc::clone(&self.crash);
        let dispatch = use_dispatch();
        let confirm = use_link_confirm();

        let close = dispatch.clone();
        let outside_close = dispatch.clone();

        OverlayPopup::new()
            .on_close(move |_| close.dismiss_game_crash())
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .on_press(move |_| outside_close.dismiss_game_crash())
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(560.))
                            .max_height(Size::window_percent(92.))
                            .background(DIALOG_BG)
                            .corner_radius(CornerRadius::new_all(16.))
                            .padding(Gaps::new_all(24.))
                            .spacing(16.)
                            .border(crate::ui::border_all_color(1., colors::component_border()))
                            .on_press(|e: Event<PressEventData>| e.stop_propagation())
                            .child(header(&crash))
                            .child(subtitle(&crash))
                            .maybe_child(crash.cause.as_ref().map(|cause| cause_card(cause)))
                            .maybe_child(fixes_section(&crash, confirm))
                            .maybe_child(discord_line(confirm))
                            .maybe_child(excerpt_box(&crash))
                            .child(actions(&crash, dispatch)),
                    ),
            )
    }
}

fn header(crash: &GameCrash) -> impl IntoElement {
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(10.)
        .child(
            Icon::new(IconType::AlertTriangle)
                .size(20.)
                .color(colors::danger()),
        )
        .child(
            label()
                .text(crash.title.clone())
                .font_size(18.)
                .font_weight(FontWeight::SEMI_BOLD)
                .width(Size::fill())
                .max_lines(2)
                .color(colors::fg_primary()),
        )
}

fn subtitle(crash: &GameCrash) -> impl IntoElement {
    let mut parts = vec![crash.cluster_name.clone()];
    if !crash.exit.is_empty() {
        parts.push(crash.exit.clone());
    }
    if crash.played_secs > 0 {
        parts.push(format!(
            "after {}",
            format_duration_hms(crash.played_secs as i64)
        ));
    }

    label()
        .text(parts.join("  ·  "))
        .font_size(12.)
        .width(Size::fill())
        .max_lines(2)
        .color(colors::fg_secondary())
}

fn cause_card(cause: &str) -> Element {
    rect()
        .width(Size::fill())
        .padding(Gaps::new_all(12.))
        .corner_radius(CornerRadius::new_all(8.))
        .background(colors::component_bg())
        .border(crate::ui::border_all_color(1., colors::component_border()))
        .child(
            label()
                .text(cause.to_string())
                .font_size(13.)
                .width(Size::fill())
                .color(colors::fg_primary()),
        )
        .into_element()
}

fn fixes_section(crash: &GameCrash, confirm: State<Option<String>>) -> Option<Element> {
    if crash.fixes.is_empty() {
        return None;
    }

    let cards: Vec<Element> = crash
        .fixes
        .iter()
        .map(|fix| fix_card(&fix.text, &fix.kind, crash.game_dir.as_deref(), confirm))
        .collect();

    if cards.len() == 1 {
        return cards.into_iter().next();
    }

    Some(
        rect()
            .vertical()
            .width(Size::fill())
            .spacing(8.)
            .child(
                label()
                    .text(format!("{} suggestions", cards.len()))
                    .font_size(11.)
                    .font_weight(FontWeight::SEMI_BOLD)
                    .color(colors::fg_secondary()),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::px(FIXES_HEIGHT))
                    .overflow(Overflow::Clip)
                    .child(
                        ScrollArea::new()
                            .width(Size::fill())
                            .height(Size::fill())
                            .spacing(10.)
                            .padding(Gaps::new(0., V_SCROLLBAR_W, 0., 0.))
                            .children(cards),
                    ),
            )
            .into_element(),
    )
}

fn fix_card(
    text: &str,
    kind: &str,
    game_dir: Option<&str>,
    confirm: State<Option<String>>,
) -> Element {
    let resolved = resolve_placeholders(text, game_dir);
    let links = links_in(&resolved);

    let mut card = rect()
        .vertical()
        .width(Size::fill())
        .spacing(8.)
        .padding(Gaps::new_all(12.))
        .corner_radius(CornerRadius::new_all(8.))
        .background(colors::component_bg())
        .border(crate::ui::border_all_color(1., colors::component_border()))
        .child(
            label()
                .text(kind.to_uppercase())
                .font_size(10.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::brand()),
        );

    let lines: Vec<String> = resolved
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(clean_line)
        .collect();

    if !lines.is_empty() {
        let content_w = lines
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0) as f32
            * CHAR_W_TEXT
            + 8.;
        let height = lines.len() as f32 * FIX_LINE_H + H_SCROLLBAR_H;

        card = card.child(
            rect()
                .width(Size::fill())
                .height(Size::px(height))
                .overflow(Overflow::Clip)
                .child(
                    ScrollArea::new()
                        .horizontal(content_w)
                        .width(Size::fill())
                        .height(Size::px(height))
                        .children(lines.iter().map(|line| fix_line(line, content_w))),
                ),
        );
    }

    if !links.is_empty() {
        let mut row = rect().horizontal().width(Size::fill()).spacing(8.);
        for url in links.into_iter().take(MAX_LINK_CHIPS) {
            row = row.child(link_chip(url, confirm));
        }
        card = card.child(row);
    }

    card.into_element()
}

fn fix_line(line: &str, content_w: f32) -> Element {
    let mut text = paragraph()
        .width(Size::px(content_w))
        .height(Size::px(FIX_LINE_H))
        .max_lines(1)
        .font_size(13.);

    for segment in segments(line) {
        let span = Span::new(segment.text);
        text = text.span(match segment.style {
            SegmentStyle::Plain => span.color(colors::fg_primary()),
            SegmentStyle::Code => span
                .font_family(theme::MONO_FONT)
                .font_size(12.)
                .color(colors::code_info()),
            SegmentStyle::Link => span
                .color(colors::code_info())
                .text_decoration(TextDecoration::Underline),
        });
    }

    text.into_element()
}

fn link_chip(url: String, mut confirm: State<Option<String>>) -> impl IntoElement {
    let target = url.clone();

    Button::new()
        .secondary()
        .small()
        .on_press(move |_| confirm.set(Some(target.clone())))
        .child(Icon::new(IconType::LinkExternal01).size(12.))
        .text(host_of(&url))
}

fn discord_line(mut confirm: State<Option<String>>) -> Option<Element> {
    if DISCORD_URL.is_empty() {
        return None;
    }

    Some(
        rect()
            .horizontal()
            .width(Size::fill())
            .cross_align(Alignment::Center)
            .spacing(8.)
            .child(
                Icon::new(IconType::MessageTextSquare01)
                    .size(14.)
                    .color(colors::fg_secondary()),
            )
            .child(
                label()
                    .text("Still stuck? Come to our discord:")
                    .font_size(12.)
                    .color(colors::fg_secondary()),
            )
            .child(
                Button::new()
                    .ghost()
                    .small()
                    .on_press(move |_| confirm.set(Some(DISCORD_URL.to_string())))
                    .child(Icon::new(IconType::LinkExternal01).size(12.))
                    .text(host_of(DISCORD_URL)),
            )
            .into_element(),
    )
}

fn excerpt_box(crash: &GameCrash) -> Option<Element> {
    if crash.excerpt.is_empty() {
        return None;
    }

    let cleaned: Vec<String> = crash.excerpt.iter().map(|line| clean_line(line)).collect();

    let content_w = cleaned
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0) as f32
        * CHAR_W_MONO
        + V_SCROLLBAR_W;

    let lines: Vec<Element> = cleaned
        .into_iter()
        .map(|line| {
            label()
                .text(line)
                .font_family(theme::MONO_FONT)
                .font_size(12.)
                .width(Size::px(content_w))
                .max_lines(1)
                .color(colors::fg_primary())
                .into_element()
        })
        .collect();

    Some(
        rect()
            .width(Size::fill())
            .height(Size::px(EXCERPT_HEIGHT))
            .background(CODE_BG)
            .corner_radius(CornerRadius::new_all(8.))
            .padding(Gaps::new_all(12.))
            .overflow(Overflow::Clip)
            .child(
                ScrollArea::new()
                    .horizontal(content_w)
                    .width(Size::fill())
                    .height(Size::fill())
                    .padding(Gaps::new(0., 0., H_SCROLLBAR_H, 0.))
                    .children(lines),
            )
            .into_element(),
    )
}

fn actions(crash: &GameCrash, dispatch: crate::Actions) -> impl IntoElement {
    let cluster_id = crash.cluster_id;
    let close = dispatch.clone();

    let mut row = rect()
        .horizontal()
        .width(Size::fill())
        .main_align(Alignment::End)
        .cross_align(Alignment::Center)
        .spacing(10.)
        .child(copy_button(crash, dispatch.clone()))
        .child(logs_button(cluster_id, dispatch.clone()));

    if let Some(remedy) = crash.remedy {
        row = row.child(remedy_button(remedy, cluster_id, dispatch.clone()));
    }

    row.child(
        Button::new()
            .primary()
            .on_press(move |_| close.dismiss_game_crash())
            .text("Close"),
    )
}

fn copy_button(crash: &GameCrash, dispatch: crate::Actions) -> impl IntoElement {
    let report = report_text(crash);

    Button::new()
        .secondary()
        .on_press(move |_| {
            if let Err(err) = freya::text_edit::Clipboard::set(report.clone()) {
                tracing::warn!("clipboard copy failed: {err:?}");
                dispatch
                    .notify("Copy failed")
                    .body("Could not copy the crash details to the clipboard.")
                    .error()
                    .send();
            } else {
                dispatch
                    .notify("Copied to clipboard")
                    .body("Crash details copied to your clipboard.")
                    .info()
                    .icon(IconType::ClipboardCheck)
                    .send();
            }
        })
        .child(Icon::new(IconType::Copy01).size(14.))
        .text("Copy")
}

fn logs_button(cluster_id: i64, dispatch: crate::Actions) -> impl IntoElement {
    Button::new()
        .secondary()
        .on_press(move |_| {
            dispatch.dismiss_game_crash();
            spawn(async move {
                let _ = RouterContext::get().push(Route::ClusterLogs { cluster_id });
            });
        })
        .child(Icon::new(IconType::Terminal).size(14.))
        .text("View logs")
}

fn remedy_button(
    remedy: CrashRemedy,
    cluster_id: i64,
    dispatch: crate::Actions,
) -> impl IntoElement {
    Button::new()
        .secondary()
        .on_press(move |_| match remedy {
            CrashRemedy::VerifyFiles => dispatch.repair_cluster(cluster_id),
            CrashRemedy::RaiseMemory => {
                dispatch.dismiss_game_crash();
                spawn(async move {
                    let _ = RouterContext::get().push(Route::ClusterSettings { cluster_id });
                });
            }
            CrashRemedy::OpenJavaSettings => {
                dispatch.dismiss_game_crash();
                spawn(async move {
                    let _ = RouterContext::get().push(Route::SettingsJava {});
                });
            }
        })
        .child(
            Icon::new(match remedy {
                CrashRemedy::VerifyFiles => IconType::FolderCheck,
                CrashRemedy::RaiseMemory | CrashRemedy::OpenJavaSettings => IconType::Settings01,
            })
            .size(14.),
        )
        .text(remedy.label())
}

fn report_text(crash: &GameCrash) -> String {
    let mut out = format!("{}\n{}", crash.title, crash.cluster_name);
    if !crash.exit.is_empty() {
        out.push_str(&format!(" — {}", crash.exit));
    }

    if let Some(cause) = &crash.cause {
        out.push_str(&format!("\n\n{cause}"));
    }

    for fix in &crash.fixes {
        out.push_str(&format!("\n\n[{}] {}", fix.kind, fix.text));
    }

    if !crash.excerpt.is_empty() {
        out.push_str("\n\n");
        out.push_str(&crash.excerpt.join("\n"));
    }

    out
}

const PATH_INDICATOR: &str = "%pathindicator%";

fn resolve_placeholders(text: &str, game_dir: Option<&str>) -> String {
    let dir = game_dir.unwrap_or("your game folder");

    text.replace("%profileroot%", dir)
        .replace("%gameroot%", dir)
        .replace(PATH_INDICATOR, "`")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegmentStyle {
    Plain,
    Code,
    Link,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextSegment {
    text: String,
    style: SegmentStyle,
}

fn segments(line: &str) -> Vec<TextSegment> {
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
    if !plain.is_empty() {
        out.push(TextSegment {
            text: std::mem::take(plain),
            style: SegmentStyle::Plain,
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
        while let Some(start) = find_url(rest) {
            let (before, from_url) = rest.split_at(start);
            let end = from_url
                .find(|c: char| c.is_whitespace())
                .unwrap_or(from_url.len());
            let (raw, remainder) = from_url.split_at(end);
            let url = trim_url(raw);

            let mut leading = before.to_string();
            if leading.ends_with('<') {
                leading.pop();
            }
            if !leading.is_empty() {
                out.push(TextSegment {
                    text: leading,
                    style: SegmentStyle::Plain,
                });
            }

            out.push(TextSegment {
                text: url.to_string(),
                style: SegmentStyle::Link,
            });

            let trailing = raw[url.len()..].trim_start_matches('>');
            rest = remainder;
            if !trailing.is_empty() {
                out.push(TextSegment {
                    text: trailing.to_string(),
                    style: SegmentStyle::Plain,
                });
            }
        }

        if !rest.is_empty() {
            out.push(TextSegment {
                text: rest.to_string(),
                style: SegmentStyle::Plain,
            });
        }
    }

    out
}

fn find_url(text: &str) -> Option<usize> {
    let https = text.find("https://");
    let http = text.find("http://");

    match (https, http) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn trim_url(raw: &str) -> &str {
    raw.trim_end_matches(['>', '.', ',', ')', '"', '\'', ':', ';', '!', '?'])
}

fn links_in(text: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut rest = text;

    while let Some(start) = find_url(rest) {
        let from_url = &rest[start..];
        let end = from_url
            .find(|c: char| c.is_whitespace())
            .unwrap_or(from_url.len());
        let url = trim_url(&from_url[..end]).to_string();

        if !url.is_empty() && !found.contains(&url) {
            found.push(url);
        }
        rest = &from_url[end..];
    }

    found
}

fn host_of(url: &str) -> String {
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
                TextSegment { text: "Navigate to ".into(), style: SegmentStyle::Plain },
                TextSegment { text: "mods/".into(), style: SegmentStyle::Code },
                TextSegment { text: " then stop".into(), style: SegmentStyle::Plain },
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

        assert_eq!(links, vec!["https://polyfrost.org", "https://modrinth.com/x"]);
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
