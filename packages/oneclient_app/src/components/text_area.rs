use std::borrow::Cow;

use freya::prelude::*;
use freya::text_edit::{EditableConfig, EditableEvent, EditorLine, TextEditor, use_editable};

use crate::theme::{self, colors};

const GAP_VERT: f32 = 10.0;
const GAP_HORI: f32 = 12.0;
/// Line height as a multiple of the font size. The grow-to-fit maths below
/// depends on it, so the paragraph has to be drawn with the same number.
const LINE_HEIGHT: f32 = 1.5;
/// Height the field keeps while empty, and the one it stops growing at and
/// starts scrolling instead, both in lines.
const MIN_LINES: usize = 3;
const MAX_LINES: usize = 16;

/// Multi-line sibling of [`TextInput`](super::TextInput).
///
/// Freya's `Input` cannot be talked into doing this: it clamps its paragraph to
/// a single line, scrolls horizontally, and spends Enter on submitting. So this
/// drives `use_editable` directly, which is what `Input` itself does one layer
/// down.
///
/// Enter inserts a newline and Ctrl/Cmd+Enter submits — the usual console
/// bargain, because a bare Enter fires half-written text far too easily.
#[derive(Clone, PartialEq)]
pub struct TextArea {
    value: Writable<String>,
    placeholder: Option<Cow<'static, str>>,
    on_submit: Option<EventHandler<String>>,
    font_family: Cow<'static, str>,
    font_size: f32,
    key: DiffKey,
}

impl TextArea {
    pub fn new(value: impl Into<Writable<String>>) -> Self {
        Self {
            value: value.into(),
            placeholder: None,
            on_submit: None,
            font_family: Cow::Borrowed(theme::DEFAULT_FONT),
            font_size: 14.,
            key: DiffKey::None,
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<Cow<'static, str>>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Called with the whole text on Ctrl/Cmd+Enter.
    pub fn on_submit(mut self, on_submit: impl Into<EventHandler<String>>) -> Self {
        self.on_submit = Some(on_submit.into());
        self
    }

    /// Draws the text in the bundled JetBrains Mono, for code-ish content.
    pub fn monospace(mut self) -> Self {
        self.font_family = Cow::Borrowed(theme::MONO_FONT);
        self
    }

    pub fn font_size(mut self, font_size: f32) -> Self {
        self.font_size = font_size;
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
        let mut editable = use_editable(|| self.value.read().to_string(), EditableConfig::new);
        let mut value = self.value.clone();

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
        let on_key_down = move |e: Event<KeyboardEventData>| {
            // Tab is the only way out of the field with the keyboard, so it is
            // left to move the focus instead of being inserted.
            if e.key == Key::Named(NamedKey::Tab) {
                return;
            }

            let key = e.key.clone();
            let modifiers = e.modifiers;

            match &key {
                Key::Named(NamedKey::Escape) | Key::Named(NamedKey::Shift) => {}
                _ => {
                    e.stop_propagation();
                    e.prevent_default();
                }
            }

            match &key {
                // Enter belongs to the text here, so submitting takes a modifier.
                Key::Named(NamedKey::Enter) if modifiers.contains(Modifiers::ctrl_or_meta()) => {
                    if let Some(on_submit) = &on_submit {
                        on_submit.call(editable.editor().peek().committed_text());
                    }
                }
                Key::Named(NamedKey::Escape) => {
                    a11y_id.request_unfocus();
                    Cursor::set(CursorIcon::default());
                }
                _ => {
                    editable.process_event(EditableEvent::KeyDown {
                        key: &key,
                        modifiers,
                    });
                    *value.write() = editable.editor().read().committed_text();
                }
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

        let focused = focus().is_focused();
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
        // Counting hard breaks rather than `lines()` so a trailing newline
        // already opens up the row it puts the caret on. Wrapped long lines are
        // not counted, which is what the inner scroll is there for.
        let rows = (text.bytes().filter(|b| *b == b'\n').count() + 1).clamp(MIN_LINES, MAX_LINES);
        let height = rows as f32 * (self.font_size * LINE_HEIGHT) + GAP_VERT * 2.;

        rect()
            .a11y_id(a11y_id)
            .a11y_focusable(true)
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
            .width(Size::fill())
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
                            .on_sized(move |e: Event<SizedEventData>| area.set(e.area))
                            .on_focus_press(on_text_focus_press)
                            .width(Size::fill())
                            .margin(Gaps::new_symmetric(GAP_VERT, GAP_HORI))
                            .font_family(self.font_family.clone())
                            .font_size(self.font_size)
                            .line_height(LINE_HEIGHT)
                            // The paragraph is taller than its viewport once the
                            // text scrolls, so the caret has to be placed
                            // against the full area and not the visible slice.
                            .cursor_mode(CursorMode::Expanded)
                            .cursor_index(cursor_index)
                            .cursor_color(colors::fg_primary())
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
