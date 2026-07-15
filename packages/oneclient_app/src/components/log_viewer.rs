use std::sync::{Arc, LazyLock};

use freya::prelude::*;
use freya::text_edit::Clipboard;
use regex::Regex;

use crate::components::{Icon, IconType, ScrollArea, ScrollAreaCtx, corrected_scroll};
use crate::theme::{self, colors};

const LINE_H: f32 = 16.;
const BODY_PAD_TOP: f32 = 10.;
const BODY_PAD_BOTTOM: f32 = 20.;
const LINE_FONT_SIZE: f32 = 12.;
const NUM_GAP: f32 = 12.;
const CHAR_W: f32 = LINE_FONT_SIZE * 0.6;
const ROW_PAD_X: f32 = 10.;
const TAB_WIDTH: usize = 4;
const OVERSCAN: i64 = 4;

static LOG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\[((?:\S+ )?\d+:\d+:\d+(?:\.\d+)?)\] \[([^/\n\r]*)/(\w+)\]:? (?:\[(CHAT)\])?")
        .expect("valid log regex")
});

fn level_color(level: &str) -> Color {
    match level.to_ascii_uppercase().as_str() {
        "ERROR" | "FATAL" | "SEVERE" => colors::code_error(),
        "WARN" | "WARNING" => colors::code_warn(),
        "DEBUG" | "TRACE" => colors::code_debug(),
        _ => colors::code_info(),
    }
}

#[derive(Clone)]
struct ParsedLine {
    spans: Vec<(String, Color)>,
}

#[derive(Default)]
struct WidthCache {
    len: usize,
    last_ptr: usize,
    max: usize,
}

fn line_ptr(line: &Arc<str>) -> usize {
    Arc::as_ptr(line) as *const u8 as usize
}

impl WidthCache {
    fn max_cols(&mut self, lines: &Arc<Vec<Arc<str>>>) -> usize {
        let len = lines.len();
        let appended = len >= self.len
            && self.len > 0
            && lines.get(self.len - 1).map(line_ptr) == Some(self.last_ptr);

        let start = if appended {
            if len == self.len {
                return self.max;
            }
            self.len
        } else {
            self.max = 0;
            0
        };

        for line in &lines[start..] {
            self.max = self.max.max(visual_len(line));
        }
        self.len = len;
        self.last_ptr = lines.last().map(line_ptr).unwrap_or(0);
        self.max
    }
}

fn clean_line(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    let mut col = 0usize;
    while let Some(c) = chars.next() {
        match c {
            '§' => {
                chars.next();
            }
            '\t' => {
                let spaces = TAB_WIDTH - (col % TAB_WIDTH);
                for _ in 0..spaces {
                    out.push(' ');
                }
                col += spaces;
            }
            '\r' => {}
            _ => {
                out.push(c);
                col += 1;
            }
        }
    }
    out
}

fn visual_len(raw: &str) -> usize {
    let mut chars = raw.chars();
    let mut col = 0usize;
    while let Some(c) = chars.next() {
        match c {
            '§' => {
                chars.next();
            }
            '\t' => col += TAB_WIDTH - (col % TAB_WIDTH),
            '\r' => {}
            _ => col += 1,
        }
    }
    col
}

fn parse_line(raw: &str) -> ParsedLine {
    let cleaned = clean_line(raw);

    let Some(caps) = LOG_RE.captures(&cleaned) else {
        return ParsedLine {
            spans: vec![(cleaned, colors::fg_primary())],
        };
    };

    let matched = caps.get(0).map(|m| m.end()).unwrap_or(0);
    let time = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
    let thread = caps.get(2).map(|m| m.as_str()).unwrap_or_default();
    let level = caps.get(3).map(|m| m.as_str()).unwrap_or_default();
    let is_chat = caps.get(4).is_some();
    let lvl_color = level_color(level);

    let mut spans = vec![
        (format!("[{time}] "), colors::code_muted()),
        (format!("[{thread}/{level}]"), lvl_color),
    ];
    if is_chat {
        spans.push((" [CHAT]".to_string(), colors::code_chat()));
    }
    let msg_color = if is_chat {
        colors::code_chat()
    } else {
        colors::fg_primary()
    };
    spans.push((format!(" {}", &cleaned[matched..]), msg_color));

    ParsedLine { spans }
}

#[derive(PartialEq)]
pub struct LogViewer {
    title: String,
    lines: Arc<Vec<Arc<str>>>,
    streaming: bool,
    header: Option<Element>,
}

impl LogViewer {
    pub fn new(title: impl Into<String>, lines: impl Into<Arc<Vec<Arc<str>>>>) -> Self {
        Self {
            title: title.into(),
            lines: lines.into(),
            streaming: false,
            header: None,
        }
    }

    pub fn streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }

    pub fn header(mut self, header: impl IntoElement) -> Self {
        self.header = Some(header.into_element());
        self
    }
}

impl Component for LogViewer {
    fn render(&self) -> impl IntoElement {
        let streaming = self.streaming;
        let lines: Arc<Vec<Arc<str>>> = self.lines.clone();
        let count = lines.len();

        let content_h = count as f32 * LINE_H + BODY_PAD_TOP + BODY_PAD_BOTTOM;
        let width_cache =
            use_hook(|| std::rc::Rc::new(std::cell::RefCell::new(WidthCache::default())));

        let max_chars = width_cache.borrow_mut().max_cols(&lines);
        let max_digits = (count.max(1) as f32).log10().floor() as usize + 1;
        let num_col_w = max_digits as f32 * (LINE_FONT_SIZE * 0.62) + 6.;
        let content_w = ROW_PAD_X + num_col_w + NUM_GAP + max_chars as f32 * CHAR_W + ROW_PAD_X;

        let scroll = use_scroll_controller(ScrollConfig::default);
        let mut auto_scroll = use_state(|| streaming);

        let sel_anchor = use_state::<Option<(usize, usize)>>(|| None);
        let sel_focus = use_state::<Option<(usize, usize)>>(|| None);
        let selecting = use_state(|| false);

        let stick_bottom = streaming && *auto_scroll.read();

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .background(colors::page_elevated())
            .corner_radius(CornerRadius::new_all(12.))
            .maybe_child(
                (!self.title.is_empty() || streaming)
                    .then(|| toolbar(self.title.clone(), streaming, auto_scroll)),
            )
            .maybe_child(self.header.clone())
            .child(
                ScrollArea::new()
                    .scroll_controller(scroll)
                    .horizontal(content_w)
                    .stick_bottom(stick_bottom)
                    .on_user_scroll(move |_| {
                        if streaming {
                            auto_scroll.set_if_modified(false);
                        }
                    })
                    .content(move |ctx: ScrollAreaCtx| {
                        scroll_body(BodyArgs {
                            ctx,
                            scroll,
                            lines: lines.clone(),
                            content_h,
                            content_w,
                            num_col_w,
                            count,
                            sel_anchor,
                            sel_focus,
                            selecting,
                        })
                    }),
            )
    }
}

fn toolbar(title: String, streaming: bool, auto_scroll: State<bool>) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .content(Content::Flex)
        .padding(Gaps::new(12., 20., 12., 20.))
        .spacing(12.)
        .child(
            label()
                .text(title)
                .font_size(18.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        )
        .child(
            rect()
                .horizontal()
                .width(Size::flex(1.0))
                .main_align(Alignment::End)
                .cross_align(Alignment::Center)
                .maybe_child(streaming.then(|| auto_scroll_toggle(auto_scroll))),
        )
        .into_element()
}

fn auto_scroll_toggle(mut auto_scroll: State<bool>) -> impl IntoElement {
    let on = *auto_scroll.read();
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(6.)
        .padding(Gaps::new_symmetric(5., 10.))
        .corner_radius(CornerRadius::new_all(7.))
        .background(if on {
            colors::brand().with_a(40)
        } else {
            colors::component_bg()
        })
        .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
        .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
        .on_press(move |_| auto_scroll.toggle())
        .child(Icon::new(IconType::ChevronDown).size(13.).color(if on {
            colors::brand()
        } else {
            colors::fg_secondary()
        }))
        .child(label().text("Auto-scroll").font_size(11.).color(if on {
            colors::brand()
        } else {
            colors::fg_secondary()
        }))
        .into_element()
}

#[derive(Clone, Copy)]
struct PosGeom {
    count: usize,
    num_col_w: f32,
    viewport_top_px: f32,
    viewport_left_px: f32,
    corrected_x: f32,
    corrected_y: f32,
}

fn pos_at_screen(
    lines: &[Arc<str>],
    geom: PosGeom,
    screen_x: f32,
    screen_y: f32,
) -> (usize, usize) {
    let region_y = screen_y - geom.viewport_top_px;
    let band_y = region_y - geom.corrected_y - BODY_PAD_TOP;
    let line = ((band_y / LINE_H).floor() as i64).clamp(0, (geom.count as i64 - 1).max(0)) as usize;

    let text_origin_x = ROW_PAD_X + geom.num_col_w + NUM_GAP;
    let local_x = screen_x - geom.viewport_left_px - geom.corrected_x - text_origin_x;
    let raw_col = (local_x / CHAR_W).round().max(0.) as usize;
    let len = lines.get(line).map(|l| visual_len(l)).unwrap_or(0);
    (line, raw_col.min(len))
}

struct BodyArgs {
    ctx: ScrollAreaCtx,
    scroll: ScrollController,
    lines: Arc<Vec<Arc<str>>>,
    content_h: f32,
    content_w: f32,
    num_col_w: f32,
    count: usize,
    sel_anchor: State<Option<(usize, usize)>>,
    sel_focus: State<Option<(usize, usize)>>,
    selecting: State<bool>,
}

fn scroll_body(args: BodyArgs) -> Element {
    let BodyArgs {
        ctx,
        mut scroll,
        lines,
        content_h,
        content_w,
        num_col_w,
        count,
        mut sel_anchor,
        mut sel_focus,
        mut selecting,
    } = args;

    let corrected_y = ctx.corrected_y;
    let corrected_x = ctx.corrected_x;
    let viewport_h = ctx.viewport_h;
    let viewport_top_px = ctx.viewport_top;
    let viewport_left_px = ctx.viewport_left;
    let body_w = content_w.max(ctx.viewport_w);

    let selection = match (*sel_anchor.read(), *sel_focus.read()) {
        (Some(a), Some(f)) => Some((a.min(f), a.max(f))),
        _ => None,
    };

    let geom = PosGeom {
        count,
        num_col_w,
        viewport_top_px,
        viewport_left_px,
        corrected_x,
        corrected_y,
    };

    let lines_down = lines.clone();
    let on_body_down = move |e: Event<PointerEventData>| {
        let pos = pos_at_screen(
            &lines_down,
            geom,
            e.global_location().x as f32,
            e.global_location().y as f32,
        );
        sel_anchor.set(Some(pos));
        sel_focus.set(Some(pos));
        selecting.set(true);
    };

    let lines_move = lines.clone();
    let on_global_move = move |e: Event<PointerEventData>| {
        if !*selecting.read() {
            return;
        }
        let cursor_y = e.global_location().y as f32;
        let pos = pos_at_screen(&lines_move, geom, e.global_location().x as f32, cursor_y);
        sel_focus.set(Some(pos));

        const EDGE: f32 = 24.;
        let region_y = cursor_y - viewport_top_px;
        let (_, cur_scroll) = scroll.into();
        let delta = if region_y < EDGE {
            (EDGE - region_y).min(EDGE)
        } else if region_y > viewport_h - EDGE {
            -((region_y - (viewport_h - EDGE)).min(EDGE))
        } else {
            0.
        };
        if delta != 0. {
            let target = cur_scroll as f32 + delta;
            let clamped = corrected_scroll(content_h, viewport_h, target) as i32;
            scroll.scroll_to_y(clamped);
        }
    };

    let on_global_release = move |_: Event<PointerEventData>| {
        if *selecting.read() {
            selecting.set(false);
        }
    };

    let lines_for_copy = lines.clone();
    let on_key_down = move |e: Event<KeyboardEventData>| {
        let copy = matches!(&e.key, Key::Character(c) if c.as_str().eq_ignore_ascii_case("c"))
            && (e.modifiers.contains(Modifiers::CONTROL) || e.modifiers.contains(Modifiers::META));
        if !copy {
            return;
        }
        if let (Some(a), Some(f)) = (*sel_anchor.read(), *sel_focus.read()) {
            let ((s_line, s_col), (e_line, e_col)) = (a.min(f), a.max(f));
            let mut parts: Vec<String> = Vec::with_capacity(e_line - s_line + 1);
            for line in s_line..=e_line {
                let Some(raw) = lines_for_copy.get(line) else {
                    continue;
                };
                let cleaned = clean_line(raw);
                let from = if line == s_line { s_col } else { 0 };
                let to = if line == e_line {
                    e_col
                } else {
                    cleaned.chars().count()
                };
                let slice: String = cleaned
                    .chars()
                    .skip(from)
                    .take(to.saturating_sub(from))
                    .collect();
                parts.push(slice);
            }
            let text = parts.join("\n");
            if let Err(err) = Clipboard::set(text) {
                tracing::warn!("clipboard copy failed: {err:?}");
            }
        }
    };

    rect()
        .width(Size::px(body_w))
        .height(Size::px(content_h))
        .on_pointer_down(on_body_down)
        .on_capture_global_pointer_move(on_global_move)
        .on_capture_global_pointer_press(on_global_release)
        .on_global_key_down(on_key_down)
        .child(log_body(
            &lines[..],
            content_h,
            corrected_y,
            viewport_h,
            body_w,
            num_col_w,
            selection,
        ))
        .into_element()
}

fn log_body(
    lines: &[Arc<str>],
    content_h: f32,
    corrected_y: f32,
    viewport_h: f32,
    body_w: f32,
    num_col_w: f32,
    selection: Option<((usize, usize), (usize, usize))>,
) -> impl IntoElement {
    let count = lines.len();
    let scrolled = -corrected_y;
    let first = (((scrolled - BODY_PAD_TOP) / LINE_H).floor() as i64 - OVERSCAN).max(0) as usize;
    let visible = (viewport_h / LINE_H).ceil() as usize + OVERSCAN as usize * 2 + 1;
    let last = (first + visible).min(count);

    let mut body = rect().width(Size::px(body_w)).height(Size::px(content_h));
    for (i, raw) in lines.iter().enumerate().take(last).skip(first) {
        let y = BODY_PAD_TOP + i as f32 * LINE_H;
        let sel_cols = selection.and_then(|((sl, sc), (el, ec))| {
            if i < sl || i > el {
                return None;
            }
            let len = visual_len(raw);
            let c0 = if i == sl { sc } else { 0 };
            let c1 = if i == el { ec } else { len };
            (c1 > c0).then_some((c0, c1))
        });
        let parsed = parse_line(raw);
        body = body.child(log_line_row(i, &parsed, y, body_w, num_col_w, sel_cols));
    }
    body
}

fn log_line_row(
    index: usize,
    parsed: &ParsedLine,
    y: f32,
    body_w: f32,
    num_col_w: f32,
    sel_cols: Option<(usize, usize)>,
) -> impl IntoElement {
    let mut message = paragraph()
        .max_lines(1)
        .width(Size::fill())
        .height(Size::px(LINE_H))
        .font_family(theme::MONO_FONT)
        .font_size(LINE_FONT_SIZE);
    for (text, color) in &parsed.spans {
        message = message.span(Span::new(text.clone()).color(*color));
    }

    let highlight = sel_cols.map(|(c0, c1)| {
        rect()
            .position(
                Position::new_absolute()
                    .top(0.)
                    .left(num_col_w + NUM_GAP + c0 as f32 * CHAR_W),
            )
            .width(Size::px((c1 - c0) as f32 * CHAR_W))
            .height(Size::px(LINE_H))
            .corner_radius(CornerRadius::new_all(2.))
            .background(colors::selection_bg())
            .into_element()
    });

    rect()
        .horizontal()
        .position(Position::new_absolute().top(y).left(ROW_PAD_X))
        .width(Size::px((body_w - ROW_PAD_X * 2.).max(0.)))
        .height(Size::px(LINE_H))
        .cross_align(Alignment::Center)
        .spacing(NUM_GAP)
        .maybe_child(highlight)
        .child(
            rect().width(Size::px(num_col_w)).child(
                label()
                    .text(format!("{}", index + 1))
                    .font_family(theme::MONO_FONT)
                    .font_size(LINE_FONT_SIZE)
                    .text_align(TextAlign::Right)
                    .width(Size::fill())
                    .color(colors::code_muted()),
            ),
        )
        .child(message)
}
