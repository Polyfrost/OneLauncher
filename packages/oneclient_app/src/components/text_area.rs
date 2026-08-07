use std::borrow::Cow;
use std::time::Duration;

use freya::animation::{AnimNum, OnChange, OnCreation, OnFinish, use_animation_with_dependencies};
use freya::prelude::*;
use freya::sdk::{Timeout, use_timeout};
use freya::text_edit::{EditableConfig, EditableEvent, EditorLine, TextEditor, use_editable};

use crate::theme::{self, colors};

/// Padding between the border and the text. Lives on the paragraph's *margin*,
/// exactly like Freya's own `Input` does it, so that the paragraph's
/// `visible_area` — the box the caret and the highlights are measured against —
/// still starts where the first glyph is painted.
const GAP_VERT: f32 = 10.0;
const GAP_HORI: f32 = 12.0;

const DEFAULT_FONT_SIZE: f32 = 14.0;
/// Line height as a multiple of the font size. The grow-to-fit maths below
/// depends on it, so the paragraph has to be drawn with the same number.
const DEFAULT_LINE_HEIGHT: f32 = 1.5;
const DEFAULT_MIN_LINES: usize = 3;
const DEFAULT_MAX_LINES: usize = 16;

// Cursor blink, ported from `freya_components::cursor_blink`, which `Input` uses
// but which Freya does not re-export through `freya::prelude`.
const BLINK_FADE: Duration = Duration::from_millis(100);
const BLINK_HOLD: Duration = Duration::from_millis(750);
/// How long after the last keystroke or click the caret starts blinking again.
const BLINK_IDLE: Duration = Duration::from_millis(500);

/// Keeps the caret solid while the user is busy and blinking once they stop.
/// Returns the timeout to [`Timeout::reset`] on every keystroke or press.
fn use_cursor_blink(enabled: bool, color: Color) -> (Timeout, Color) {
    let idle = use_timeout(|| BLINK_IDLE);

    let blink = use_animation_with_dependencies(
        &(enabled, idle.elapsed()),
        |conf, (enabled, gone_idle)| {
            if *enabled && *gone_idle {
                conf.on_creation(OnCreation::Run);
                conf.on_change(OnChange::Rerun);
                conf.on_finish(OnFinish::reverse_with_delay(BLINK_HOLD));
            }
            AnimNum::new(255., 0.).duration(BLINK_FADE)
        },
    );

    (idle, color.with_a(blink.get().value() as u8))
}

/// What `Enter` does, and what submits.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SubmitBinding {
    /// `Ctrl`/`Cmd`+`Enter` submits, a bare `Enter` breaks the line. The default,
    /// because in a multi-line field a bare `Enter` fires half-written text far
    /// too easily.
    #[default]
    ModifierEnter,
    /// `Enter` submits, `Shift`+`Enter` breaks the line. The chat-box bargain.
    ShiftEnter,
    /// Nothing submits; `Enter` always breaks the line.
    Never,
}

/// What the field does with a key, decided before any of it touches the editor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum KeyAction {
    /// Left alone, so it can do whatever it does outside the field — `Tab` still
    /// has to be able to move the focus on.
    Bubble,
    /// Handed to the text editor.
    Edit,
    /// Fires `on_submit` with the current text.
    Submit,
    /// Drops the focus.
    Unfocus,
}

pub(crate) fn key_action(key: &Key, modifiers: Modifiers, binding: SubmitBinding) -> KeyAction {
    match key {
        Key::Named(NamedKey::Tab) => KeyAction::Bubble,
        Key::Named(NamedKey::Escape) => KeyAction::Unfocus,
        Key::Named(NamedKey::Enter) => match binding {
            SubmitBinding::ModifierEnter if modifiers.contains(Modifiers::ctrl_or_meta()) => {
                KeyAction::Submit
            }
            SubmitBinding::ShiftEnter if !modifiers.contains(Modifiers::SHIFT) => KeyAction::Submit,
            _ => KeyAction::Edit,
        },
        _ => KeyAction::Edit,
    }
}

/// How many text rows the field shows before the inner scroll takes over.
///
/// Hard breaks are counted rather than `lines()`, so a trailing newline already
/// opens up the row it puts the caret on. Soft-wrapped long lines are not
/// counted — that is what the inner scroll is there for.
pub(crate) fn visible_lines(text: &str, min_lines: usize, max_lines: usize) -> usize {
    let min = min_lines.max(1);
    let max = max_lines.max(min);
    let hard_breaks = text.bytes().filter(|byte| *byte == b'\n').count();
    (hard_breaks + 1).clamp(min, max)
}

/// Outer height of the field for a given number of visible rows.
pub(crate) fn field_height(rows: usize, font_size: f32, line_height: f32) -> f32 {
    rows as f32 * (font_size * line_height) + GAP_VERT * 2.
}

/// Multi-line sibling of [`TextInput`](super::TextInput).
///
/// Freya's `Input` cannot be talked into doing this: it clamps its paragraph to
/// a single line, scrolls horizontally, and spends `Enter` on submitting. So this
/// drives `use_editable` directly, which is what `Input` itself does one layer
/// down.
///
/// The field grows with the text between [`TextArea::min_lines`] and
/// [`TextArea::max_lines`], then scrolls.
///
/// ```ignore
/// TextArea::new(query)
///     .monospace()
///     .font_size(13.)
///     .placeholder("SELECT * FROM …")
///     .on_submit(move |sql| run(sql));
/// ```
#[derive(Clone, PartialEq)]
pub struct TextArea {
    value: Writable<String>,
    placeholder: Option<Cow<'static, str>>,
    on_submit: Option<EventHandler<String>>,
    submit_binding: SubmitBinding,
    font_family: Cow<'static, str>,
    font_size: f32,
    line_height: f32,
    min_lines: usize,
    max_lines: usize,
    width: Size,
    read_only: bool,
    auto_focus: bool,
    key: DiffKey,
}

#[allow(dead_code)]
impl TextArea {
    pub fn new(value: impl Into<Writable<String>>) -> Self {
        Self {
            value: value.into(),
            placeholder: None,
            on_submit: None,
            submit_binding: SubmitBinding::default(),
            font_family: Cow::Borrowed(theme::DEFAULT_FONT),
            font_size: DEFAULT_FONT_SIZE,
            line_height: DEFAULT_LINE_HEIGHT,
            min_lines: DEFAULT_MIN_LINES,
            max_lines: DEFAULT_MAX_LINES,
            width: Size::fill(),
            read_only: false,
            auto_focus: false,
            key: DiffKey::None,
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<Cow<'static, str>>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Called with the whole text when [`SubmitBinding`] says so.
    pub fn on_submit(mut self, on_submit: impl Into<EventHandler<String>>) -> Self {
        self.on_submit = Some(on_submit.into());
        self
    }

    pub fn submit_binding(mut self, submit_binding: SubmitBinding) -> Self {
        self.submit_binding = submit_binding;
        self
    }

    /// Shortcut for [`SubmitBinding::ShiftEnter`]: `Enter` submits, `Shift`+`Enter`
    /// breaks the line.
    pub fn submit_on_enter(self) -> Self {
        self.submit_binding(SubmitBinding::ShiftEnter)
    }

    /// Draws the text in the bundled JetBrains Mono, for code-ish content.
    pub fn monospace(self) -> Self {
        self.font_family(theme::MONO_FONT)
    }

    pub fn font_family(mut self, font_family: impl Into<Cow<'static, str>>) -> Self {
        self.font_family = font_family.into();
        self
    }

    pub fn font_size(mut self, font_size: f32) -> Self {
        self.font_size = font_size;
        self
    }

    /// Line height as a multiple of the font size. Also drives the grow-to-fit
    /// height, so the two cannot drift apart.
    pub fn line_height(mut self, line_height: f32) -> Self {
        self.line_height = line_height;
        self
    }

    /// Height the field keeps while empty. Clamped to at least one line.
    pub fn min_lines(mut self, min_lines: usize) -> Self {
        self.min_lines = min_lines;
        self
    }

    /// Height the field stops growing at, scrolling instead. Never below
    /// [`Self::min_lines`].
    pub fn max_lines(mut self, max_lines: usize) -> Self {
        self.max_lines = max_lines;
        self
    }

    /// Shortcut for [`Self::min_lines`] plus [`Self::max_lines`].
    pub fn lines(self, min_lines: usize, max_lines: usize) -> Self {
        self.min_lines(min_lines).max_lines(max_lines)
    }

    /// Fixes the field at exactly `lines` rows.
    pub fn fixed_lines(self, lines: usize) -> Self {
        self.lines(lines, lines)
    }

    pub fn width(mut self, width: impl Into<Size>) -> Self {
        self.width = width.into();
        self
    }

    /// Selectable and copyable, but not editable.
    ///
    /// Read once, when the field first renders — the underlying `EditableConfig`
    /// is built one time and Freya offers no way to swap it afterwards. Toggling
    /// this on a mounted field needs a new [`DiffKey`].
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn auto_focus(mut self, auto_focus: bool) -> Self {
        self.auto_focus = auto_focus;
        self
    }
}

impl KeyExt for TextArea {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for TextArea {
    fn render(&self) -> impl IntoElement {
        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let holder = use_state(ParagraphHolder::default);
        let mut area = use_state(Area::default);
        let mut hovering = use_state(|| false);
        let mut is_dragging = use_state(|| false);
        let read_only = self.read_only;
        let mut editable = use_editable(
            || self.value.read().to_string(),
            move || EditableConfig::new().with_allow_changes(!read_only),
        );
        let mut value = self.value.clone();

        let focused = focus().is_focused();
        let (mut idle_timeout, cursor_color) = use_cursor_blink(focused, colors::fg_primary());

        use_drop(move || {
            if hovering() {
                Cursor::set(CursorIcon::default());
            }
        });

        // The value is owned by the caller, so a write from anywhere else has to
        // be pulled back into the editor.
        if *value.read() != editable.editor().read().committed_text() {
            let mut editor = editable.editor_mut().write();
            editor.clear_preedit();
            editor.set(&value.read());
            editor.editor_history().clear();
            editor.clear_selection();
        }

        let display_placeholder = value.read().is_empty()
            && self.placeholder.is_some()
            && !editable.editor().read().has_preedit();

        let on_ime_preedit = move |e: Event<ImePreeditEventData>| {
            let mut editor = editable.editor_mut().write();
            if e.data().text.is_empty() {
                editor.clear_preedit();
            } else {
                editor.set_preedit(&e.data().text);
            }
        };

        let on_submit = self.on_submit.clone();
        let submit_binding = self.submit_binding;
        let on_key_down = move |e: Event<KeyboardEventData>| {
            let key = e.key.clone();
            let modifiers = e.modifiers;
            let action = key_action(&key, modifiers, submit_binding);

            if action == KeyAction::Bubble {
                return;
            }

            // Escape and Shift keep bubbling: one closes whatever is around the
            // field, the other is a bare modifier nobody else acts on.
            match &key {
                Key::Named(NamedKey::Escape) | Key::Named(NamedKey::Shift) => {}
                _ => {
                    e.stop_propagation();
                    e.prevent_default();
                }
            }

            match action {
                KeyAction::Submit => {
                    if let Some(on_submit) = &on_submit {
                        on_submit.call(editable.editor().peek().committed_text());
                    }
                }
                KeyAction::Unfocus => {
                    a11y_id.request_unfocus();
                    Cursor::set(CursorIcon::default());
                }
                KeyAction::Edit => {
                    idle_timeout.reset();
                    editable.process_event(EditableEvent::KeyDown {
                        key: &key,
                        modifiers,
                    });
                    if !read_only {
                        *value.write() = editable.editor().read().committed_text();
                    }
                }
                KeyAction::Bubble => unreachable!("returned above"),
            }
        };

        let on_key_up = move |e: Event<KeyboardEventData>| {
            e.stop_propagation();
            editable.process_event(EditableEvent::KeyUp { key: &e.key });
        };

        // Pressing the padding around the text still has to land the caret, so
        // the outer press is mapped back into the paragraph's own coordinates.
        let on_field_focus_press = move |e: Event<FocusPressEventData>| {
            e.stop_propagation();
            e.prevent_default();
            is_dragging.set_if_modified(true);
            idle_timeout.reset();
            if !display_placeholder {
                let area = area.read().to_f64();
                let global_location = e.global_location().clamp(area.min(), area.max());
                editable.process_event(EditableEvent::Down {
                    location: (global_location - area.min()).to_point(),
                    editor_line: EditorLine::SingleParagraph,
                    holder: &holder.read(),
                });
            }
            a11y_id.request_focus();
        };

        let on_text_focus_press = move |e: Event<FocusPressEventData>| {
            e.stop_propagation();
            e.prevent_default();
            is_dragging.set_if_modified(true);
            idle_timeout.reset();
            if !display_placeholder {
                editable.process_event(EditableEvent::Down {
                    location: e.element_location(),
                    editor_line: EditorLine::SingleParagraph,
                    holder: &holder.read(),
                });
            }
            a11y_id.request_focus();
        };

        let on_global_pointer_move = move |e: Event<PointerEventData>| {
            if a11y_id.is_focused() && *is_dragging.read() {
                let mut location = e.global_location();
                location.x -= area.read().min_x() as f64;
                location.y -= area.read().min_y() as f64;
                editable.process_event(EditableEvent::Move {
                    location,
                    editor_line: EditorLine::SingleParagraph,
                    holder: &holder.read(),
                });
            }
        };

        let on_global_pointer_press = move |_: Event<PointerEventData>| {
            if hovering() || a11y_id.is_focused() {
                editable.process_event(EditableEvent::Release);
            }

            if a11y_id.is_focused() {
                if *is_dragging.read() {
                    is_dragging.set(false);
                } else {
                    // Focused, not dragging: the press landed outside the field.
                    a11y_id.request_unfocus();
                }
            }
        };

        let on_pointer_press = move |e: Event<PointerEventData>| {
            e.stop_propagation();
            e.prevent_default();
            editable.process_event(EditableEvent::Release);
            if a11y_id.is_focused() {
                is_dragging.set_if_modified(false);
            }
        };

        let on_pointer_enter = move |_| {
            hovering.set_if_modified(true);
            Cursor::set(CursorIcon::Text);
        };

        let on_pointer_leave = move |_| {
            if hovering() {
                Cursor::set(CursorIcon::default());
                hovering.set(false);
            }
        };

        let (cursor_index, text_selection) = if focused {
            (
                Some(editable.editor().read().cursor_pos()),
                editable
                    .editor()
                    .read()
                    .get_visible_selection(EditorLine::SingleParagraph),
            )
        } else {
            (None, None)
        };

        let text = editable.editor().read().rope().to_string();
        let rows = visible_lines(&text, self.min_lines, self.max_lines);
        let height = field_height(rows, self.font_size, self.line_height);

        rect()
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_auto_focus(self.auto_focus)
            .a11y_role(AccessibilityRole::TextInput)
            .a11y_alt(match &self.placeholder {
                Some(placeholder) if display_placeholder => placeholder.to_string(),
                _ => text.clone(),
            })
            .on_key_down(on_key_down)
            .on_key_up(on_key_up)
            .on_ime_preedit(on_ime_preedit)
            .on_focus_press(on_field_focus_press)
            .on_pointer_press(on_pointer_press)
            .on_global_pointer_press(on_global_pointer_press)
            .on_global_pointer_move(on_global_pointer_move)
            .on_pointer_enter(on_pointer_enter)
            .on_pointer_leave(on_pointer_leave)
            .width(self.width.clone())
            .height(Size::px(height))
            .overflow(Overflow::Clip)
            .corner_radius(CornerRadius::new_all(8.))
            .background(if focused {
                colors::component_bg_pressed()
            } else {
                colors::component_bg()
            })
            .border(
                Border::new()
                    .fill(if focused {
                        colors::component_border_pressed()
                    } else {
                        colors::component_border()
                    })
                    .width(if focused { 2. } else { 1. })
                    .alignment(BorderAlignment::Inner),
            )
            .child(
                ScrollView::new()
                    .width(Size::fill())
                    .height(Size::fill())
                    // Dragging inside the field selects text; it must not also
                    // drag the view around.
                    .drag_scrolling(false)
                    .child(
                        paragraph()
                            .holder(holder.read().clone())
                            // `visible_area`, not `area`: the caret, the
                            // highlights and `element_location` are all measured
                            // from the paragraph's area *minus its margin*, which
                            // is where the first glyph is painted. Taking `area`
                            // here offsets every press by the margin.
                            .on_sized(move |e: Event<SizedEventData>| area.set(e.visible_area))
                            .on_focus_press(on_text_focus_press)
                            .width(Size::fill())
                            .margin(Gaps::new_symmetric(GAP_VERT, GAP_HORI))
                            .font_family(self.font_family.clone())
                            .font_size(self.font_size)
                            .line_height(self.line_height)
                            // The cursor mode is left at `Fit` on purpose. It is
                            // the only one that measures the caret against the
                            // same box the text is painted into; `Expanded`
                            // measures against the area *including* the margin
                            // and drags caret and selection off by it.
                            .cursor_index(cursor_index)
                            .cursor_color(cursor_color)
                            .highlight_color(colors::selection_bg())
                            .color(if display_placeholder {
                                colors::fg_secondary()
                            } else {
                                colors::fg_primary()
                            })
                            .highlights(text_selection.map(|selection| vec![selection]))
                            .maybe(display_placeholder, |el| {
                                el.span(self.placeholder.as_ref().unwrap().to_string())
                            })
                            .maybe(!display_placeholder, |el| {
                                let editor = editable.editor().read();
                                if editor.has_preedit() {
                                    let (before, preedit, after) = editor.preedit_text_segments();
                                    el.span(before)
                                        .span(
                                            Span::new(preedit)
                                                .text_decoration(TextDecoration::Underline),
                                        )
                                        .span(after)
                                } else {
                                    el.span(text.clone())
                                }
                            }),
                    ),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_field_is_min_lines_tall() {
        assert_eq!(visible_lines("", 3, 16), 3);
        assert_eq!(visible_lines("SELECT 1", 3, 16), 3);
    }

    #[test]
    fn hard_breaks_grow_the_field_up_to_max_lines() {
        for breaks in 0..40 {
            let text = "x\n".repeat(breaks);
            let expected = (breaks + 1).clamp(3, 16);
            assert_eq!(visible_lines(&text, 3, 16), expected, "{breaks} breaks");
        }
    }

    #[test]
    fn a_trailing_newline_already_opens_its_row() {
        // The caret sits on the new row, so the row has to exist before anything
        // is typed into it.
        assert_eq!(visible_lines("a\nb\nc", 1, 16), 3);
        assert_eq!(visible_lines("a\nb\nc\n", 1, 16), 4);
    }

    #[test]
    fn soft_wrapped_lines_are_left_to_the_inner_scroll() {
        let long = "x".repeat(10_000);
        assert_eq!(visible_lines(&long, 3, 16), 3);
    }

    #[test]
    fn line_bounds_are_normalised_rather_than_panicking() {
        // `clamp` panics when min > max, and both are caller-supplied. A crossed
        // pair resolves in favour of the minimum, which is what `max_lines`
        // documents: it is never allowed below `min_lines`.
        assert_eq!(visible_lines("a\nb\nc\nd", 16, 3), 16);
        // And a field is always at least one row tall.
        assert_eq!(visible_lines("", 0, 0), 1);
        assert_eq!(visible_lines("a\nb", 0, 16), 2);
    }

    #[test]
    fn a_fixed_field_never_changes_height() {
        // What `TextArea::fixed_lines(5)` sets up.
        for breaks in 0..30 {
            let text = "x\n".repeat(breaks);
            assert_eq!(visible_lines(&text, 5, 5), 5, "{breaks} breaks");
        }
    }

    #[test]
    fn height_covers_every_row_plus_the_padding() {
        assert_eq!(field_height(3, 13., 1.5), 3. * 19.5 + GAP_VERT * 2.);
        // Growing by a row adds exactly one line box, which is what keeps the
        // grow-to-fit maths in step with the paragraph's own line height.
        let one_row = field_height(1, 14., 1.5);
        let two_rows = field_height(2, 14., 1.5);
        assert_eq!(two_rows - one_row, 14. * 1.5);
    }

    fn action(key: NamedKey, modifiers: Modifiers, binding: SubmitBinding) -> KeyAction {
        key_action(&Key::Named(key), modifiers, binding)
    }

    #[test]
    fn tab_is_left_alone_so_focus_can_move_on() {
        for binding in [
            SubmitBinding::ModifierEnter,
            SubmitBinding::ShiftEnter,
            SubmitBinding::Never,
        ] {
            assert_eq!(
                action(NamedKey::Tab, Modifiers::empty(), binding),
                KeyAction::Bubble
            );
        }
    }

    #[test]
    fn escape_always_drops_the_focus() {
        assert_eq!(
            action(
                NamedKey::Escape,
                Modifiers::empty(),
                SubmitBinding::ModifierEnter
            ),
            KeyAction::Unfocus
        );
    }

    #[test]
    fn modifier_enter_keeps_a_bare_enter_for_the_text() {
        let binding = SubmitBinding::ModifierEnter;
        assert_eq!(
            action(NamedKey::Enter, Modifiers::empty(), binding),
            KeyAction::Edit
        );
        assert_eq!(
            action(NamedKey::Enter, Modifiers::SHIFT, binding),
            KeyAction::Edit
        );
        assert_eq!(
            action(NamedKey::Enter, Modifiers::ctrl_or_meta(), binding),
            KeyAction::Submit
        );
    }

    #[test]
    fn shift_enter_swaps_the_two_around() {
        let binding = SubmitBinding::ShiftEnter;
        assert_eq!(
            action(NamedKey::Enter, Modifiers::empty(), binding),
            KeyAction::Submit
        );
        assert_eq!(
            action(NamedKey::Enter, Modifiers::SHIFT, binding),
            KeyAction::Edit
        );
    }

    #[test]
    fn never_leaves_enter_to_the_text() {
        let binding = SubmitBinding::Never;
        assert_eq!(
            action(NamedKey::Enter, Modifiers::empty(), binding),
            KeyAction::Edit
        );
        assert_eq!(
            action(NamedKey::Enter, Modifiers::ctrl_or_meta(), binding),
            KeyAction::Edit
        );
    }

    #[test]
    fn everything_else_goes_to_the_editor() {
        for binding in [
            SubmitBinding::ModifierEnter,
            SubmitBinding::ShiftEnter,
            SubmitBinding::Never,
        ] {
            assert_eq!(
                key_action(&Key::Character("a".into()), Modifiers::empty(), binding),
                KeyAction::Edit
            );
            assert_eq!(
                action(NamedKey::Backspace, Modifiers::empty(), binding),
                KeyAction::Edit
            );
            assert_eq!(
                action(NamedKey::ArrowDown, Modifiers::SHIFT, binding),
                KeyAction::Edit
            );
        }
    }
}
