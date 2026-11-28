use std::sync::LazyLock;
use std::time::Instant;

use freya::prelude::*;

use crate::i18n::{t, tf};
use crate::theme;
use crate::theme::{rounded, text_size};

pub fn border_all(width: f32) -> Border {
    Border::new()
        .fill(theme::colors::component_border())
        .width(BorderWidth {
            top: width,
            right: width,
            bottom: width,
            left: width,
        })
}

pub fn border_all_color(width: f32, color: Color) -> Border {
    Border::new().fill(color).width(BorderWidth {
        top: width,
        right: width,
        bottom: width,
        left: width,
    })
}

/// A tinted strip of explanation under a row; `accent` washes the background
pub fn note(message: String, accent: Color) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .spacing(10.)
        .padding(Gaps::new_symmetric(12., 14.))
        .corner_radius(rounded(10.))
        .background(accent.with_a(30))
        .child(
            label()
                .text(message)
                .font_size(text_size(12.))
                .max_lines(5)
                .width(Size::flex(1.0))
                .color(theme::colors::fg_primary()),
        )
        .into_element()
}

/// A captioned, read-only path in a boxed field
pub fn path_block(caption: &'static str, path: &std::path::Path) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(4.)
        .child(
            label()
                .text(caption)
                .font_size(text_size(11.))
                .color(theme::colors::fg_secondary()),
        )
        .child(
            rect()
                .width(Size::fill())
                .padding(Gaps::new_all(10.))
                .corner_radius(rounded(8.))
                .background(theme::colors::component_bg())
                .border(border_all_color(1., theme::colors::component_border()))
                .child(
                    label()
                        .text(path.display().to_string())
                        .font_size(text_size(12.))
                        .max_lines(3)
                        .width(Size::fill())
                        .color(theme::colors::fg_primary()),
                ),
        )
        .into_element()
}

pub fn fmt_date(ts: chrono::DateTime<chrono::Utc>) -> String {
    ts.format("%Y-%m-%d %H:%M").to_string()
}

pub fn fmt_local_datetime(ts: chrono::DateTime<chrono::FixedOffset>) -> String {
    static LOCALE: LazyLock<chrono::Locale> = LazyLock::new(|| {
        sys_locale::get_locale()
            .and_then(|code| chrono::Locale::try_from(code.replace('-', "_").as_str()).ok())
            .unwrap_or(chrono::Locale::en_US)
    });

    ts.with_timezone(&chrono::Local)
        .format_localized("%x %X", *LOCALE)
        .to_string()
}

/// Shortest band is a minute nothing re-renders on a timer so a seconds counter
/// would freeze at whatever it read when the row was drawn
pub fn relative_time(created_at: Instant) -> String {
    let secs = created_at.elapsed().as_secs();
    match secs {
        0..=59 => t("oneclient.ui.relative_time.just_now").to_string(),
        60..=3599 => tf("oneclient.ui.relative_time.minutes", &[&(secs / 60)]),
        3600..=86_399 => tf("oneclient.ui.relative_time.hours", &[&(secs / 3600)]),
        _ => tf("oneclient.ui.relative_time.days", &[&(secs / 86_400)]),
    }
}

pub fn relative_since(at: chrono::DateTime<chrono::Utc>) -> String {
    let elapsed = (chrono::Utc::now() - at).to_std().unwrap_or_default();
    relative_time(
        Instant::now()
            .checked_sub(elapsed)
            .unwrap_or_else(Instant::now),
    )
}

pub fn last_played_label(ts: Option<chrono::DateTime<chrono::Utc>>) -> String {
    let Some(ts) = ts else {
        return t("oneclient.ui.last_played.never").to_string();
    };
    last_played_days((chrono::Utc::now() - ts).num_days())
}

fn last_played_days(days: i64) -> String {
    match days {
        ..=0 => t("oneclient.ui.last_played.today").to_string(),
        1 => t("oneclient.ui.last_played.yesterday").to_string(),
        d @ 2..=6 => tf("oneclient.ui.last_played.days", &[&d]),
        d @ 7..=13 => tf("oneclient.ui.last_played.weeks.one", &[&(d / 7)]),
        d @ 14..=29 => tf("oneclient.ui.last_played.weeks.other", &[&(d / 7)]),
        d @ 30..=59 => tf("oneclient.ui.last_played.months.one", &[&(d / 30)]),
        d @ 60..=364 => tf(
            "oneclient.ui.last_played.months.other",
            &[&(d / 30).min(11)],
        ),
        d @ 365..=729 => tf("oneclient.ui.last_played.years.one", &[&(d / 365)]),
        d => tf("oneclient.ui.last_played.years.other", &[&(d / 365)]),
    }
}

/// Gap the clamped menu keeps from the window edges
pub const EDGE_MARGIN: f32 = 8.;

pub fn clamp_to_window(x: f32, y: f32, width: f32, height: f32) -> (f32, f32) {
    let window = window_logical_size();
    let clamp = |pos: f32, len: f32, limit: f32| {
        pos.clamp(EDGE_MARGIN, (limit - len - EDGE_MARGIN).max(EDGE_MARGIN))
    };

    (
        clamp(x, width, window.width),
        clamp(y, height, window.height),
    )
}

/// Returns the `Rect` not an `Element` so callers can inset or round it
pub fn divider() -> Rect {
    rect()
        .width(Size::fill())
        .height(Size::px(1.))
        .background(theme::colors::component_border())
}

/// An element's edge for [`edge_line`]
#[derive(Clone, Copy)]
pub enum Edge {
    Top,
    Right,
    Bottom,
}

/// A 1 px line along one edge of the parent, given the parent's own `padding`. Unlike a one-sided
/// border it paints nothing on the other sides, and it takes no part in the parent's layout.
///
/// Absolute children resolve against the parent's content box before it wraps its children, so
/// `Bottom` and `Right` need a parent whose height and width aren't auto; see [`underlined`].
pub fn edge_line(edge: Edge, color: Color, padding: Gaps) -> Rect {
    // Negative margins stretch the line over the padding and, unlike a size function, follow the
    // window's scale factor
    let across = Gaps::new(0., -padding.right(), 0., -padding.left());
    let down = Gaps::new(-padding.top(), 0., -padding.bottom(), 0.);

    let (position, width, height, margin) = match edge {
        Edge::Top => (
            Position::new_absolute().top(-padding.top()),
            Size::percent(100.),
            Size::px(1.),
            across,
        ),
        Edge::Right => (
            Position::new_absolute().right(-padding.right()),
            Size::px(1.),
            Size::percent(100.),
            down,
        ),
        Edge::Bottom => (
            Position::new_absolute().bottom(-padding.bottom()),
            Size::percent(100.),
            Size::px(1.),
            across,
        ),
    };

    rect()
        .position(position)
        .width(width)
        .height(height)
        .margin(margin)
        .interactive(false)
        .background(color)
}

/// Wraps a full-width row of any height with a line along the inside of its bottom edge
pub fn underlined(row: impl IntoElement, color: Color) -> Rect {
    rect().vertical().width(Size::fill()).child(row).child(
        // Empty, so the line hangs up into the row's last pixel
        rect()
            .width(Size::fill())
            .height(Size::px(0.))
            .child(edge_line(Edge::Bottom, color, Gaps::default())),
    )
}

pub fn centered_note(text: &str) -> Element {
    rect()
        .width(Size::fill())
        .height(Size::px(240.))
        .center()
        .child(
            label()
                .text(text.to_string())
                .font_size(text_size(14.))
                .color(theme::colors::fg_secondary()),
        )
        .into_element()
}

pub fn grid_columns_for_width(width: f32, max_col: f32, gap: f32) -> usize {
    if width <= 0. {
        return 1;
    }

    (((width + gap) / (max_col + gap)).ceil() as usize).max(1)
}

pub fn columns_for(width: f32, min_cell: f32, max: usize, gap: f32) -> usize {
    let max = max.max(1);
    if width <= 0. {
        return max;
    }

    (((width + gap) / (min_cell + gap)).floor() as usize).clamp(1, max)
}

pub fn fixed_grid(cards: Vec<Element>, columns: usize, card_height: f32, gap: f32) -> Element {
    let columns = columns.max(1);
    let mut root = rect().vertical().width(Size::fill()).spacing(gap);

    for (index, chunk) in cards.chunks(columns).enumerate() {
        let mut row = rect()
            .key(index)
            .horizontal()
            .width(Size::fill())
            .height(Size::px(card_height))
            .content(Content::Flex)
            .spacing(gap);

        for card in chunk {
            row = row.child(
                rect()
                    .width(Size::flex(1.0))
                    .height(Size::fill())
                    .child(card.clone()),
            );
        }
        for _ in chunk.len()..columns {
            row = row.child(rect().width(Size::flex(1.0)).height(Size::fill()));
        }

        root = root.child(row.into_element());
    }

    root.into_element()
}

/// Short final rows are padded with empty flex cells so tiles keep the column width
pub fn flow_grid(items: Vec<Element>, cols: usize, mut width: State<f32>, gap: f32) -> Element {
    let cols = cols.max(1);

    let mut root = rect().vertical().width(Size::fill()).spacing(gap);
    let mut iter = items.into_iter();
    let mut remaining = true;
    while remaining {
        let mut row = rect()
            .horizontal()
            .width(Size::fill())
            .spacing(gap)
            .content(Content::Flex);
        let mut filled = 0;
        for _ in 0..cols {
            if let Some(item) = iter.next() {
                row = row.child(item);
                filled += 1;
            } else {
                row = row.child(rect().width(Size::flex(1.0)));
            }
        }
        if filled == 0 {
            break;
        }
        remaining = filled == cols;
        root = root.child(row.into_element());
    }

    root.on_sized(move |event: Event<SizedEventData>| {
        let w = event.data().area.width();
        if (w - *width.peek()).abs() > 0.5 {
            *width.write() = w;
        }
    })
    .into_element()
}

pub fn entrance_motion_layer(
    slide_x: f32,
    slide_y: f32,
    opacity: f32,
    child: impl IntoElement,
) -> impl IntoElement {
    rect()
        .width(Size::fill())
        .height(Size::fill())
        .overflow(Overflow::Clip)
        .child(
            rect()
                .width(Size::fill())
                .height(Size::fill())
                .position(Position::new_absolute().top(slide_y).left(slide_x))
                .opacity(opacity)
                .child(child),
        )
        .into_element()
}

pub trait ImageFallbackExt: Sized {
    fn fallback(self, placeholder: impl IntoElement) -> Self;
}

impl ImageFallbackExt for ImageViewer {
    fn fallback(self, placeholder: impl IntoElement) -> Self {
        let placeholder = placeholder.into_element();
        self.error_renderer(move |_: String| placeholder.clone())
    }
}

pub fn window_logical_size() -> Size2D {
    *Platform::get().current_window().root_size.peek()
}

pub(crate) fn is_text_entry(role: AccessibilityRole) -> bool {
    matches!(
        role,
        AccessibilityRole::TextInput
            | AccessibilityRole::MultilineTextInput
            | AccessibilityRole::SearchInput
            | AccessibilityRole::PasswordInput
    )
}

#[cfg(test)]
mod last_played_tests {
    use super::last_played_days;

    #[test]
    fn long_ago_switches_from_months_to_years() {
        assert_eq!(last_played_days(364), "11 months ago");
        assert_eq!(last_played_days(365), "1 year ago");
        assert_eq!(last_played_days(729), "1 year ago");
        assert_eq!(last_played_days(730), "2 years ago");
    }
}

#[cfg(test)]
mod edge_line_tests {
    use super::*;
    use freya_testing::TestingRunner;

    const ROW: Color = Color::from_rgb(1, 0, 0);
    const CONTENT: Color = Color::from_rgb(2, 0, 0);
    const LINE: Color = Color::from_rgb(3, 0, 0);

    #[derive(Clone, Copy, PartialEq)]
    enum Case {
        Plain,
        Top,
        Underlined,
        FixedPlain,
        FixedBottom,
        FixedRight,
    }

    fn app() -> impl IntoElement {
        let case = *use_consume::<State<Case>>().read();
        let padding = Gaps::new(16., 24., 10., 14.);
        let block = |width: Size, height: f32| {
            rect()
                .width(width)
                .height(Size::px(height))
                .background(CONTENT)
        };
        let fixed = matches!(
            case,
            Case::FixedPlain | Case::FixedBottom | Case::FixedRight
        );
        let edge = match case {
            Case::Top => Some(Edge::Top),
            Case::FixedBottom => Some(Edge::Bottom),
            Case::FixedRight => Some(Edge::Right),
            _ => None,
        };
        let row = rect()
            .horizontal()
            .width(Size::fill())
            .height(if fixed { Size::px(60.) } else { Size::auto() })
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(12.)
            .padding(padding)
            .background(ROW)
            .maybe_child(edge.map(|edge| edge_line(edge, LINE, padding)))
            .child(block(Size::px(30.), 20.))
            .child(block(Size::flex(1.), 10.))
            .child(block(Size::px(40.), 30.));
        let row = if case == Case::Underlined {
            underlined(row, LINE).into_element()
        } else {
            row.into_element()
        };

        rect()
            .vertical()
            .width(Size::fill())
            .child(row)
            .child(block(Size::fill(), 20.))
    }

    fn areas(case: Case, scale: f64) -> [Vec<Area>; 3] {
        let (mut runner, _) = TestingRunner::new(
            app,
            (600., 300.).into(),
            |runner| runner.provide_root_context(|| State::create(case)),
            scale,
        );
        runner.sync_and_update();
        [ROW, CONTENT, LINE].map(|color| {
            let fill: Fill = color.into();
            runner.find_many(|node, element| {
                (element.style().background == fill).then(|| node.layout().visible_area())
            })
        })
    }

    fn assert_area(actual: &Area, x: f32, y: f32, width: f32, height: f32) {
        let expected = [x, y, width, height];
        let actual = [
            actual.min_x(),
            actual.min_y(),
            actual.width(),
            actual.height(),
        ];
        assert!(
            actual
                .iter()
                .zip(expected)
                .all(|(a, e)| (a - e).abs() < 0.01),
            "{actual:?} is not {expected:?}"
        );
    }

    #[test]
    fn a_line_covers_its_edge_and_moves_nothing() {
        for scale in [1., 1.5] {
            let px = scale as f32;
            for (case, plain) in [
                (Case::Top, Case::Plain),
                (Case::Underlined, Case::Plain),
                (Case::FixedBottom, Case::FixedPlain),
                (Case::FixedRight, Case::FixedPlain),
            ] {
                let [rows, content, lines] = areas(case, scale);
                let [plain_rows, plain_content, none] = areas(plain, scale);
                assert!(none.is_empty());
                assert_eq!(rows, plain_rows);
                assert_eq!(content.len(), 4);
                assert_eq!(content, plain_content);
                assert_eq!(lines.len(), 1);

                let (row, line) = (rows[0], &lines[0]);
                let (x, y) = (row.min_x(), row.min_y());
                let (w, h) = (row.width(), row.height());
                match case {
                    Case::Top => assert_area(line, x, y, w, px),
                    Case::FixedRight => assert_area(line, row.max_x() - px, y, px, h),
                    _ => assert_area(line, x, row.max_y() - px, w, px),
                }
            }
        }
    }
}
