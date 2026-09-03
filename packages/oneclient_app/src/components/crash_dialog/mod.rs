use std::sync::Arc;

use freya::prelude::*;
use freya::router::*;

use oneclient_common::constants::DISCORD_URL;
use oneclient_events::{CrashFix, CrashRemedy, GameCrash};

use crate::Route;
use crate::components::{Button, Icon, IconType, OverlayPopup, ScrollArea};
use crate::hooks::{use_dispatch, use_link_confirm};
use crate::theme;
use crate::theme::colors;
use crate::utils::format_duration_hms;

mod markup;

use markup::{SegmentStyle, display_line, host_of, links_in, resolve_placeholders, segments};

const DIALOG_BG: Color = Color::from_rgb(21, 28, 34);
const CODE_BG: Color = Color::from_rgb(13, 18, 22);
const DISCORD_BG: Color = Color::from_argb(46, 43, 75, 255);
const DISCORD_BORDER: Color = Color::from_argb(120, 43, 75, 255);

const CRASH_DIALOG_LEVEL: u8 = 14;

const MAX_LINK_CHIPS: usize = 3;

const CHAR_W_TEXT: f32 = 7.8;
const CHAR_W_MONO: f32 = 7.3;
const FIX_LINE_H: f32 = 20.;

const V_SCROLLBAR_W: f32 = 16.;
const H_SCROLLBAR_H: f32 = 14.;

const DIALOG_W: f32 = 560.;
const DIALOG_PADDING: f32 = 24.;
const DIALOG_SPACING: f32 = 16.;
const BODY_SPACING: f32 = 16.;
const CARD_PADDING: f32 = 12.;
const CARD_SPACING: f32 = 8.;
const FIXES_SPACING: f32 = 10.;

const HEADER_H: f32 = 26.;
const SUBTITLE_H: f32 = 18.;
const ACTIONS_H: f32 = 32.;
const DISCORD_H: f32 = 68.;
const KIND_LABEL_H: f32 = 14.;
const SUGGESTIONS_LABEL_H: f32 = 14.;
const LINK_ROW_H: f32 = 26.;
const CAUSE_LINE_H: f32 = 18.;
const CAUSE_CHARS_PER_LINE: f32 = 66.;
const EXCERPT_LINE_H: f32 = 17.;
const SUSPECT_HINT_H: f32 = 16.;
const SUSPECT_MAX_LINES: f32 = 2.;

const SUSPECT_HINT: &str = "Named in the crash report. Try removing or updating them first.";

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
            .level(CRASH_DIALOG_LEVEL)
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
                            .width(Size::px(DIALOG_W))
                            .height(Size::px(dialog_height(&crash)))
                            .max_height(Size::window_percent(92.))
                            .content(Content::Flex)
                            .overflow(Overflow::Clip)
                            .background(DIALOG_BG)
                            .corner_radius(CornerRadius::new_all(16.))
                            .padding(Gaps::new_all(DIALOG_PADDING))
                            .spacing(DIALOG_SPACING)
                            .border(crate::ui::border_all_color(1., colors::component_border()))
                            .on_press(|e: Event<PressEventData>| e.stop_propagation())
                            .child(header(&crash))
                            .child(subtitle(&crash))
                            .child(body(&crash, confirm))
                            .child(discord_card(confirm))
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

enum Block<'a> {
    Suspects,
    Cause(&'a str),
    Fixes,
    Excerpt,
}

impl Block<'_> {
    fn height(&self, crash: &GameCrash) -> f32 {
        match self {
            Self::Suspects => suspects_height(crash),
            Self::Cause(cause) => CARD_PADDING * 2. + wrapped_lines(cause) * CAUSE_LINE_H,
            Self::Fixes => fixes_height(crash),
            Self::Excerpt => excerpt_height(crash.excerpt.len()),
        }
    }

    fn element(&self, crash: &GameCrash, confirm: State<Option<String>>) -> Element {
        match self {
            Self::Suspects => suspects_card(crash),
            Self::Cause(cause) => cause_card(cause),
            Self::Fixes => fixes_section(crash, confirm),
            Self::Excerpt => excerpt_box(crash),
        }
    }
}

fn blocks(crash: &GameCrash) -> Vec<Block<'_>> {
    let mut blocks = Vec::new();

    if !crash.suspects.is_empty() {
        blocks.push(Block::Suspects);
    }
    if let Some(cause) = &crash.cause {
        blocks.push(Block::Cause(cause));
    }
    if !crash.fixes.is_empty() {
        blocks.push(Block::Fixes);
    }
    if !crash.excerpt.is_empty() {
        blocks.push(Block::Excerpt);
    }

    blocks
}

fn body(crash: &GameCrash, confirm: State<Option<String>>) -> impl IntoElement {
    let elements = blocks(crash)
        .into_iter()
        .map(|block| block.element(crash, confirm));

    ScrollArea::new()
        .width(Size::fill())
        .height(Size::flex(1.0))
        .spacing(BODY_SPACING)
        .padding(Gaps::new(0., V_SCROLLBAR_W, 0., 0.))
        .children(elements)
}

fn dialog_height(crash: &GameCrash) -> f32 {
    let blocks = blocks(crash);
    let body = blocks.iter().map(|block| block.height(crash)).sum::<f32>()
        + stacked(blocks.len(), BODY_SPACING);

    DIALOG_PADDING * 2. + HEADER_H + SUBTITLE_H + body + DISCORD_H + ACTIONS_H + DIALOG_SPACING * 4.
}

fn stacked(count: usize, spacing: f32) -> f32 {
    spacing * count.saturating_sub(1) as f32
}

fn wrapped_lines(text: &str) -> f32 {
    (text.chars().count() as f32 / CAUSE_CHARS_PER_LINE)
        .ceil()
        .max(1.)
}

fn suspects_height(crash: &GameCrash) -> f32 {
    let names = wrapped_lines(&crash.suspects.join(", ")).min(SUSPECT_MAX_LINES);

    CARD_PADDING * 2.
        + KIND_LABEL_H
        + CARD_SPACING
        + names * CAUSE_LINE_H
        + CARD_SPACING
        + SUSPECT_HINT_H
}

fn excerpt_height(lines: usize) -> f32 {
    CARD_PADDING * 2. + lines as f32 * EXCERPT_LINE_H + H_SCROLLBAR_H
}

fn fixes_height(crash: &GameCrash) -> f32 {
    let cards: f32 = crash
        .fixes
        .iter()
        .map(|fix| fix_card_height(fix, crash.game_dir.as_deref()))
        .sum();

    let stack = cards + stacked(crash.fixes.len(), FIXES_SPACING);

    if crash.fixes.len() == 1 {
        stack
    } else {
        stack + SUGGESTIONS_LABEL_H + FIXES_SPACING
    }
}

fn fix_card_height(fix: &CrashFix, game_dir: Option<&str>) -> f32 {
    let resolved = resolve_placeholders(&fix.text, game_dir);
    let lines = resolved
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();

    let mut height = CARD_PADDING * 2. + KIND_LABEL_H;
    if lines > 0 {
        height += CARD_SPACING + lines as f32 * FIX_LINE_H + H_SCROLLBAR_H;
    }
    if !links_in(&resolved).is_empty() {
        height += CARD_SPACING + LINK_ROW_H;
    }

    height
}

fn card() -> Rect {
    rect()
        .vertical()
        .width(Size::fill())
        .padding(Gaps::new_all(CARD_PADDING))
        .corner_radius(CornerRadius::new_all(8.))
        .background(colors::component_bg())
        .border(crate::ui::border_all_color(1., colors::component_border()))
}

fn clip_box(height: f32) -> Rect {
    rect()
        .width(Size::fill())
        .height(Size::px(height))
        .overflow(Overflow::Clip)
}

fn content_width(lines: &[String], char_w: f32, padding: f32) -> f32 {
    let longest = lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);

    longest as f32 * char_w + padding
}

fn cause_card(cause: &str) -> Element {
    card()
        .child(
            label()
                .text(cause.to_string())
                .font_size(13.)
                .width(Size::fill())
                .color(colors::fg_primary()),
        )
        .into_element()
}

fn suspects_card(crash: &GameCrash) -> Element {
    card()
        .spacing(CARD_SPACING)
        .child(
            label()
                .text("SUSPECTED MODS")
                .font_size(10.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::danger()),
        )
        .child(
            label()
                .text(crash.suspects.join(", "))
                .font_size(13.)
                .width(Size::fill())
                .max_lines(SUSPECT_MAX_LINES as usize)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text(SUSPECT_HINT)
                .font_size(11.)
                .width(Size::fill())
                .max_lines(1)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn fixes_section(crash: &GameCrash, confirm: State<Option<String>>) -> Element {
    let mut cards: Vec<Element> = crash
        .fixes
        .iter()
        .map(|fix| fix_card(fix, crash.game_dir.as_deref(), confirm))
        .collect();

    if cards.len() == 1 {
        return cards.remove(0);
    }

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(FIXES_SPACING)
        .child(
            label()
                .text(format!("{} suggestions", cards.len()))
                .font_size(11.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_secondary()),
        )
        .children(cards)
        .into_element()
}

fn fix_card(fix: &CrashFix, game_dir: Option<&str>, confirm: State<Option<String>>) -> Element {
    let resolved = resolve_placeholders(&fix.text, game_dir);
    let links = links_in(&resolved);

    let mut fix = card().spacing(CARD_SPACING).child(
        label()
            .text(fix.kind.to_uppercase())
            .font_size(10.)
            .font_weight(FontWeight::SEMI_BOLD)
            .color(colors::brand()),
    );

    let lines: Vec<String> = resolved
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(display_line)
        .collect();

    if !lines.is_empty() {
        let content_w = content_width(&lines, CHAR_W_TEXT, 8.);
        let height = lines.len() as f32 * FIX_LINE_H + H_SCROLLBAR_H;

        fix = fix.child(
            clip_box(height).child(
                ScrollArea::new()
                    .horizontal(content_w)
                    .width(Size::fill())
                    .height(Size::px(height))
                    .children(lines.iter().map(|line| fix_line(line, content_w))),
            ),
        );
    }

    if !links.is_empty() {
        let mut row = rect()
            .horizontal()
            .width(Size::fill())
            .spacing(CARD_SPACING);
        for url in links.into_iter().take(MAX_LINK_CHIPS) {
            row = row.child(link_chip(url, confirm));
        }
        fix = fix.child(row);
    }

    fix.into_element()
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

fn discord_card(mut confirm: State<Option<String>>) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(12.)
        .content(Content::Flex)
        .padding(Gaps::new_all(14.))
        .corner_radius(CornerRadius::new_all(10.))
        .background(DISCORD_BG)
        .border(crate::ui::border_all_color(1., DISCORD_BORDER))
        .child(
            Icon::new(IconType::MessageTextSquare01)
                .size(22.)
                .color(colors::brand()),
        )
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(2.)
                .child(
                    label()
                        .text("Still stuck? Ask us on Discord")
                        .font_size(14.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .max_lines(1)
                        .width(Size::fill())
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text("Copy the crash details and we will help you fix it.")
                        .font_size(12.)
                        .max_lines(1)
                        .width(Size::fill())
                        .color(colors::fg_secondary()),
                ),
        )
        .child(
            Button::new()
                .primary()
                .on_press(move |_| confirm.set(Some(DISCORD_URL.to_string())))
                .child(Icon::new(IconType::LinkExternal01).size(14.))
                .text("Join Discord"),
        )
}

fn excerpt_box(crash: &GameCrash) -> Element {
    let cleaned: Vec<String> = crash
        .excerpt
        .iter()
        .map(|line| display_line(line))
        .collect();
    let content_w = content_width(&cleaned, CHAR_W_MONO, V_SCROLLBAR_W);
    let height = excerpt_height(cleaned.len());

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

    clip_box(height)
        .background(CODE_BG)
        .corner_radius(CornerRadius::new_all(8.))
        .padding(Gaps::new_all(CARD_PADDING))
        .child(
            ScrollArea::new()
                .horizontal(content_w)
                .width(Size::fill())
                .height(Size::fill())
                .padding(Gaps::new(0., 0., H_SCROLLBAR_H, 0.))
                .children(lines),
        )
        .into_element()
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
        .child(logs_button(cluster_id, dispatch.clone()));

    if let Some(remedy) = crash.remedy {
        row = row.child(remedy_button(remedy, cluster_id, dispatch.clone()));
    }

    row.child(
        Button::new()
            .ghost()
            .on_press(move |_| close.dismiss_game_crash())
            .text("Close"),
    )
}

fn logs_button(cluster_id: i64, dispatch: crate::Actions) -> impl IntoElement {
    Button::new()
        .secondary()
        .on_press(move |_| {
            dispatch.dismiss_game_crash();
            let _ = RouterContext::get().push(Route::ClusterLogs { cluster_id });
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
        .on_press(move |_| {
            let route = match remedy {
                CrashRemedy::VerifyFiles => {
                    dispatch.repair_cluster(cluster_id);
                    return;
                }
                CrashRemedy::RaiseMemory => Route::ClusterSettings { cluster_id },
                CrashRemedy::OpenJavaSettings => Route::SettingsJava {},
                CrashRemedy::OpenMods => Route::ClusterMods { cluster_id },
            };

            dispatch.dismiss_game_crash();
            let _ = RouterContext::get().push(route);
        })
        .child(
            Icon::new(match remedy {
                CrashRemedy::VerifyFiles => IconType::FolderCheck,
                CrashRemedy::RaiseMemory | CrashRemedy::OpenJavaSettings => IconType::Settings01,
                CrashRemedy::OpenMods => IconType::DotsGrid,
            })
            .size(14.),
        )
        .text(remedy.label())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crash() -> GameCrash {
        GameCrash {
            cluster_id: 1,
            cluster_name: "Skyblock".to_string(),
            title: "Minecraft ran out of memory".to_string(),
            exit: "exit code: 1".to_string(),
            played_secs: 252,
            cause: None,
            suspects: Vec::new(),
            remedy: None,
            fixes: Vec::new(),
            excerpt: Vec::new(),
            game_dir: None,
        }
    }

    fn fix(text: &str) -> CrashFix {
        CrashFix {
            text: text.to_string(),
            kind: "Solution".to_string(),
        }
    }

    #[test]
    fn every_block_adds_to_the_measured_height() {
        let bare = dialog_height(&crash());

        let mut with_cause = crash();
        with_cause.cause = Some("The game asked for more memory than it was allowed.".to_string());
        assert!(dialog_height(&with_cause) > bare);

        let mut with_fix = crash();
        with_fix.fixes = vec![fix("Raise the memory allocation")];
        assert!(dialog_height(&with_fix) > bare);

        let mut with_excerpt = crash();
        with_excerpt.excerpt = vec!["at net.minecraft.client.Minecraft.run".to_string(); 20];
        assert!(dialog_height(&with_excerpt) > bare + 20. * EXCERPT_LINE_H);

        let mut with_suspects = crash();
        with_suspects.suspects = vec!["Biomes O' Plenty (biomesoplenty)".to_string()];
        assert!(dialog_height(&with_suspects) > bare);
    }

    #[test]
    fn a_long_suspect_list_stays_within_its_measured_lines() {
        let mut many = crash();
        many.suspects = vec!["Some Very Long Mod Name (someverylongmodname)".to_string(); 6];

        let names = many.suspects.join(", ");
        assert!(wrapped_lines(&names) > SUSPECT_MAX_LINES);
        assert_eq!(
            suspects_height(&many),
            CARD_PADDING * 2.
                + KIND_LABEL_H
                + CARD_SPACING
                + SUSPECT_MAX_LINES * CAUSE_LINE_H
                + CARD_SPACING
                + SUSPECT_HINT_H
        );
    }

    #[test]
    fn a_full_crash_measures_past_the_smallest_window() {
        let mut full = crash();
        full.cause = Some("x".repeat(140));
        full.fixes = vec![
            fix("Remove the mod"),
            fix("Update it: https://modrinth.com/mod/scc"),
            fix("Line one\nLine two\nLine three"),
        ];
        full.excerpt = vec!["at java.util.zip.ZipFile.open".to_string(); 27];

        assert!(
            dialog_height(&full) > 552.,
            "{} fits in the 800x600 minimum window, so the body would not scroll",
            dialog_height(&full)
        );
    }
}
