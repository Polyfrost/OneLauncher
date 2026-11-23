//! Minecraft's section-sign formatting codes, as the server list renders them

use freya::prelude::*;
use freya::text_edit::*;

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

fn advance(c: char, bold: bool) -> f32 {
    let base = match c {
        '!' | ',' | '.' | ':' | ';' | 'i' | '|' | '\'' => 2.,
        'l' | '`' => 3.,
        ' ' | 'I' | 't' | '[' | ']' => 4.,
        '"' | '(' | ')' | '*' | '<' | '>' | 'f' | 'k' | '{' | '}' => 5.,
        '@' | '~' => 7.,
        _ => 6.,
    };
    if bold { base + 1. } else { base }
}

fn wrap(runs: &mut [Run], max: f32) {
    let mut chars: Vec<(usize, char)> = runs
        .iter()
        .enumerate()
        .flat_map(|(i, run)| run.text.chars().map(move |c| (i, c)))
        .collect();
    let (mut i, mut line_start, mut width, mut space) = (0, 0, 0., None);
    while i < chars.len() {
        let (run, c) = chars[i];
        if c == '\n' {
            (i, line_start, width, space) = (i + 1, i + 1, 0., None);
            continue;
        }
        if c == ' ' {
            space = Some(i);
        }
        width += advance(c, runs[run].bold);
        if width > max && i > line_start {
            match space {
                Some(s) => chars[s].1 = '\n',
                None => chars.insert(i, (run, '\n')),
            }
            i = space.unwrap_or(i) + 1;
            (line_start, width, space) = (i, 0., None);
            continue;
        }
        i += 1;
    }
    for run in runs.iter_mut() {
        run.text.clear();
    }
    for (run, c) in chars {
        runs[run].text.push(c);
    }
}

pub fn motd_text(text: &str, default: Color, line_height: f32, wrap_width: Option<f32>) -> Element {
    let mut runs = parse(text);
    if let Some(max) = wrap_width {
        wrap(&mut runs, max);
    }
    paragraph()
        .max_lines(2)
        .line_height(line_height)
        .spans_iter(runs.into_iter().map(|run| {
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
#[derive(Clone, Copy, PartialEq)]
pub struct MotdEditor {
    pub value: State<String>,
    pub editable: UseEditable,
    pub a11y_id: AccessibilityId,
    pub keep_focus: State<bool>,
}

pub fn use_motd_editor(initial: String) -> MotdEditor {
    let value = use_state({
        let initial = initial.clone();
        move || initial
    });
    let mut editable = use_editable(move || initial, EditableConfig::new);
    use_hook(move || {
        let mut editor = editable.editor_mut().write();
        let end = editor.rope().len_utf16_cu();
        *editor.selection_mut() = TextSelection::new_cursor(end);
    });
    MotdEditor {
        value,
        editable,
        a11y_id: use_a11y(),
        keep_focus: use_state(|| false),
    }
}

impl MotdEditor {
    fn insert(mut self, code: char) {
        let text = {
            let mut editor = self.editable.editor_mut().write();
            if let Some((start, end)) = editor.get_selection_range() {
                editor.remove(start..end);
                *editor.selection_mut() = TextSelection::new_cursor(start);
            }
            let at = editor.cursor_pos();
            let len = editor.insert(&format!("{SECTION}{code}"), at);
            *editor.selection_mut() = TextSelection::new_cursor(at + len);
            editor.committed_text()
        };
        self.value.set(text);
        self.a11y_id.request_focus();
    }
}

#[derive(PartialEq)]
pub struct MotdInput {
    pub editor: MotdEditor,
    pub placeholder: &'static str,
}

impl Component for MotdInput {
    fn render(&self) -> impl IntoElement {
        let MotdEditor {
            mut value,
            mut editable,
            a11y_id,
            mut keep_focus,
        } = self.editor;
        let focus = use_focus(a11y_id);
        let holder = use_state(ParagraphHolder::default);
        let mut area = use_state(Area::default);
        let mut dragging = use_state(|| false);
        let focused = focus() != Focus::Not;
        let (mut blink, cursor_color) = use_cursor_blink(focused, colors::fg_primary());

        if *value.read() != editable.editor().read().committed_text() {
            let mut editor = editable.editor_mut().write();
            editor.clear_preedit();
            editor.set(&value.read());
            editor.editor_history_mut().clear();
            editor.clear_selection();
        }

        let on_key_down = move |e: Event<KeyboardEventData>| {
            match &e.key {
                Key::Named(NamedKey::Escape) => {
                    a11y_id.request_unfocus();
                    return;
                }
                Key::Named(NamedKey::Tab | NamedKey::Enter) => return,
                _ => {}
            }
            e.stop_propagation();
            e.prevent_default();
            blink.reset();
            editable.process_event(EditableEvent::KeyDown {
                key: &e.key,
                modifiers: e.modifiers,
                editor_line: Some(EditorLine::SingleParagraph),
                holder: Some(&holder.read()),
            });
            let text = editable.editor().read().committed_text();
            if *value.peek() != text {
                value.set(text);
            }
        };

        let on_key_up = move |e: Event<KeyboardEventData>| {
            e.stop_propagation();
            editable.process_event(EditableEvent::KeyUp { key: &e.key });
        };

        let on_ime_preedit = move |e: Event<ImePreeditEventData>| {
            let mut editor = editable.editor_mut().write();
            if e.data().text.is_empty() {
                editor.clear_preedit();
            } else {
                editor.set_preedit(&e.data().text);
            }
        };

        let on_focus_press = move |e: Event<FocusPressEventData>| {
            e.stop_propagation();
            e.prevent_default();
            dragging.set_if_modified(true);
            blink.reset();
            let text_area = area.read().to_f64();
            let global = e.global_location().clamp(text_area.min(), text_area.max());
            editable.process_event(EditableEvent::Down {
                location: (global - text_area.min()).to_point(),
                editor_line: EditorLine::SingleParagraph,
                holder: &holder.read(),
            });
            a11y_id.request_focus();
        };

        let on_global_pointer_move = move |e: Event<PointerEventData>| {
            if a11y_id.is_focused() && *dragging.read() {
                let text_area = area.read().to_f64();
                editable.process_event(EditableEvent::Move {
                    location: (e.global_location() - text_area.min()).to_point(),
                    editor_line: EditorLine::SingleParagraph,
                    holder: &holder.read(),
                });
            }
        };

        let on_global_pointer_up = move |_: Event<PointerEventData>| {
            if a11y_id.is_focused() {
                editable.process_event(EditableEvent::Release);
            }
            if *keep_focus.peek() {
                keep_focus.set(false);
                dragging.set_if_modified(false);
            } else if *dragging.peek() {
                dragging.set(false);
            } else if a11y_id.is_focused() {
                a11y_id.request_unfocus();
            }
        };

        let text = value.read().clone();
        let placeholder = text.is_empty() && !focused;
        let (cursor_index, highlights) = if focused {
            let editor = editable.editor().read();
            (
                Some(editor.cursor_pos()),
                editor.get_visible_selection(EditorLine::SingleParagraph),
            )
        } else {
            (None, None)
        };
        let border = if focused {
            Border::new()
                .fill(colors::component_border_pressed())
                .width(2.)
                .alignment(BorderAlignment::Inner)
        } else {
            Border::new()
                .fill(colors::component_border())
                .width(1.)
                .alignment(BorderAlignment::Inner)
        };

        rect()
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::TextInput)
            .a11y_alt(text.clone())
            .on_key_down(on_key_down)
            .on_key_up(on_key_up)
            .on_ime_preedit(on_ime_preedit)
            .on_focus_press(on_focus_press)
            .on_global_pointer_move(on_global_pointer_move)
            .on_global_pointer_up(on_global_pointer_up)
            .cursor(CursorIcon::Text)
            .width(Size::fill())
            .corner_radius(rounded(8.))
            .background(if focused {
                colors::component_bg_pressed()
            } else {
                colors::component_bg()
            })
            .border(border)
            .child(
                paragraph()
                    .holder(holder.read().clone())
                    .on_sized(move |e: Event<SizedEventData>| area.set_if_modified(e.area))
                    .width(Size::fill())
                    .padding(Gaps::new_symmetric(8., 12.))
                    .font_family(crate::theme::mono_font())
                    .font_size(text_size(14.))
                    .cursor_index(cursor_index)
                    .cursor_color(cursor_color)
                    .highlights(highlights.map(|h| vec![h]))
                    .color(if placeholder {
                        colors::fg_secondary()
                    } else {
                        colors::fg_primary()
                    })
                    .span(if placeholder {
                        self.placeholder.to_string()
                    } else {
                        text
                    }),
            )
    }
}

pub const DEFAULT_MOTD: &str = "A Minecraft Server";

pub fn motd_card(editor: MotdEditor, preview: Element, action: Option<Element>) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(12.)
        .padding(Gaps::new_symmetric(12., 16.))
        .corner_radius(rounded(12.))
        .background(colors::page_elevated())
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::Flex)
                .cross_align(Alignment::Center)
                .spacing(16.)
                .child(crate::components::Icon::new(
                    crate::components::IconType::MessageTextSquare01,
                ))
                .child(
                    rect()
                        .vertical()
                        .width(Size::flex(1.0))
                        .spacing(2.)
                        .child(
                            label()
                                .text(t("oneclient.servers.settings.motd.title"))
                                .font_size(text_size(16.))
                                .font_weight(FontWeight::MEDIUM)
                                .color(colors::fg_primary()),
                        )
                        .child(
                            label()
                                .text(t("oneclient.servers.settings.motd.description"))
                                .font_size(text_size(12.))
                                .color(colors::fg_secondary()),
                        ),
                )
                .maybe_child(action),
        )
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(10.)
                .padding(Gaps::new(0., 0., 0., 40.))
                .child(preview)
                .child(MotdInput {
                    editor,
                    placeholder: DEFAULT_MOTD,
                })
                .child(code_picker(editor)),
        )
        .into_element()
}

pub fn code_picker(editor: MotdEditor) -> Element {
    let mut keep_focus = editor.keep_focus;
    let add = move |code: char| editor.insert(code);

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
        (
            'm',
            true,
            t("oneclient.servers.settings.motd.strikethrough"),
        ),
        ('k', false, t("oneclient.servers.settings.motd.obfuscated")),
        ('r', false, t("oneclient.servers.settings.motd.reset")),
    ]
    .into_iter()
    .map(|(code, styled, name)| {
        let sample = if styled {
            format!("{SECTION}{code}{name}")
        } else {
            name.to_string()
        };
        Button::new()
            .secondary()
            .small()
            .on_press(move |_| add(code))
            .child(
                rect()
                    .font_family(crate::theme::MINECRAFT_FONT)
                    .font_size(20.)
                    .child(motd_text(&sample, colors::fg_primary(), 1.2, None)),
            )
            .into_element()
    });

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(8.)
        .on_pointer_down(move |_| keep_focus.set(true))
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
    fn overflowing_word_moves_down_whole() {
        let mut runs = parse("hi §cabcdef");
        wrap(&mut runs, 30.);
        assert_eq!(runs[0].text, "hi\n");
        assert_eq!(runs[1].text, "abcde\nf");
    }

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
