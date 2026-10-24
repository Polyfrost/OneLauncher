use freya::prelude::*;

use crate::components::{Button, Icon, IconType, OverlayPopup, TextInput};
use crate::theme::colors;
use crate::ui::border_all_color;

const POPUP_WIDTH: f32 = 280.;
const POPUP_HEIGHT: f32 = 320.;
const POPUP_PADDING: f32 = 16.;
const SCREEN_MARGIN: f32 = 8.;
const TRIGGER_GAP: f32 = 6.;

const SV_HEIGHT: f32 = 160.;
const SV_HANDLE: f32 = 14.;
const HUE_HEIGHT: f32 = 16.;
const HUE_HANDLE: f32 = 12.;
const SWATCH_SIZE: f32 = 36.;

type Hsv = (f32, f32, f32);

#[derive(Clone, Copy, PartialEq, Default)]
enum Drag {
    #[default]
    None,
    Sv,
    Hue,
}

fn hsv_of(color: Color) -> Hsv {
    let hsv = color.to_hsv();
    (hsv.h, hsv.s, hsv.v)
}

fn same_rgb(a: Color, b: Color) -> bool {
    (a.r(), a.g(), a.b()) == (b.r(), b.g(), b.b())
}

fn digits(color: Color) -> String {
    colors::format_hex(color)
        .trim_start_matches('#')
        .to_string()
}

fn handle_shadow() -> Shadow {
    Shadow::from((0., 0., 3., 1., Color::from_argb(110, 0, 0, 0)))
}

#[derive(PartialEq)]
pub struct ColorPicker {
    value: Color,
    on_change: EventHandler<Color>,
}

impl ColorPicker {
    pub fn new(value: Color, on_change: impl Into<EventHandler<Color>>) -> Self {
        Self {
            value,
            on_change: on_change.into(),
        }
    }
}

impl Component for ColorPicker {
    fn render(&self) -> impl IntoElement {
        let mut open = use_state(|| false);
        let mut trigger_area = use_state(Area::default);
        let mut sv_area = use_state(Area::default);
        let mut hue_area = use_state(Area::default);
        let mut drag = use_state(Drag::default);
        let mut hsv = use_state(|| hsv_of(self.value));
        let mut hex = use_state(|| digits(self.value));
        let mut seen = use_state(|| self.value);

        if *seen.peek() != self.value {
            seen.set(self.value);
            let (h, s, v) = *hsv.peek();
            if !same_rgb(Color::from_hsv(h, s, v), self.value) {
                hsv.set(hsv_of(self.value));
                hex.set(digits(self.value));
            }
        }
        let (h, s, v) = hsv();
        let current = Color::from_hsv(h, s, v);
        let on_current = colors::on_color(self.value);

        let mut emit = {
            let on_change = self.on_change.clone();
            move |next: Hsv| {
                hsv.set(next);
                let color = Color::from_hsv(next.0, next.1, next.2);
                hex.set(digits(color));
                on_change.call(color);
            }
        };

        let pick_sv = {
            let mut emit = emit.clone();
            move |point: CursorPoint| {
                let area = sv_area.read().to_f64();
                let s = ((point.x - area.min_x()) / area.width()).clamp(0., 1.) as f32;
                let y = ((point.y - area.min_y()) / area.height()).clamp(0., 1.) as f32;
                let hue = hsv.peek().0;
                emit((hue, s, 1. - y));
            }
        };

        let pick_hue = {
            let mut emit = emit.clone();
            move |point: CursorPoint| {
                let area = hue_area.read().to_f64();
                let x = ((point.x - area.min_x()) / area.width()).clamp(0., 1.) as f32;
                let (_, s, v) = *hsv.peek();
                emit((x * 360., s, v));
            }
        };

        let trigger = rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(12.)
            .padding(Gaps::new_symmetric(8., 16.))
            .corner_radius(CornerRadius::new_all(8.))
            .background(self.value)
            .border(border_all_color(1., colors::component_border()))
            .cursor(CursorIcon::Pointer)
            .a11y_role(AccessibilityRole::Button)
            .on_sized(move |e: Event<SizedEventData>| trigger_area.set_if_modified(e.area))
            .on_press(move |_| open.toggle())
            .child(Icon::new(IconType::Colors).size(18.).color(on_current))
            .child(
                label()
                    .text(colors::format_hex(self.value))
                    .font_size(12.)
                    .color(on_current),
            );

        let popup = open().then(|| {
            let window = *Platform::get().root_size.peek();
            let anchor = trigger_area();
            let left = (anchor.max_x() as f32 - POPUP_WIDTH)
                .min(window.width - POPUP_WIDTH - SCREEN_MARGIN)
                .max(SCREEN_MARGIN);
            let below = anchor.max_y() as f32 + TRIGGER_GAP;
            let top = if below + POPUP_HEIGHT > window.height - SCREEN_MARGIN {
                (anchor.min_y() as f32 - TRIGGER_GAP - POPUP_HEIGHT).max(SCREEN_MARGIN)
            } else {
                below
            };

            let track_width = POPUP_WIDTH - 2. * POPUP_PADDING;
            let sv_handle_x = s * track_width - SV_HANDLE / 2.;
            let sv_handle_y = (1. - v) * SV_HEIGHT - SV_HANDLE / 2.;
            let hue_handle_x = h / 360. * track_width - HUE_HANDLE / 2.;

            let header = rect()
                .horizontal()
                .width(Size::fill())
                .main_align(Alignment::SpaceBetween)
                .cross_align(Alignment::Center)
                .child(
                    label()
                        .text("Color Picker")
                        .font_size(14.)
                        .color(colors::fg_primary()),
                )
                .child(
                    Button::new()
                        .ghost()
                        .icon()
                        .alt("Close")
                        .on_press(move |_| open.set(false))
                        .child(Icon::new(IconType::XClose).size(16.)),
                );

            let sv_pane = rect()
                .width(Size::fill())
                .height(Size::px(SV_HEIGHT))
                .on_sized(move |e: Event<SizedEventData>| sv_area.set_if_modified(e.area))
                .on_pointer_down({
                    let mut pick_sv = pick_sv.clone();
                    move |e: Event<PointerEventData>| {
                        if !e.data().is_primary() {
                            return;
                        }
                        drag.set(Drag::Sv);
                        pick_sv(e.global_location());
                        e.stop_propagation();
                        e.prevent_default();
                    }
                })
                .child(
                    rect()
                        .width(Size::fill())
                        .height(Size::fill())
                        .corner_radius(CornerRadius::new_all(8.))
                        .overflow(Overflow::Clip)
                        .background(
                            LinearGradient::new()
                                .angle(-90.)
                                .stop((Color::WHITE, 0.))
                                .stop((Color::from_hsv(h, 1., 1.), 100.)),
                        )
                        .child(
                            rect()
                                .position(Position::new_absolute())
                                .width(Size::fill())
                                .height(Size::fill())
                                .background(
                                    LinearGradient::new()
                                        .angle(0.)
                                        .stop((Color::from_argb(0, 0, 0, 0), 0.))
                                        .stop((Color::BLACK, 100.)),
                                ),
                        ),
                )
                .child(
                    rect()
                        .position(Position::new_absolute().left(sv_handle_x).top(sv_handle_y))
                        .layer(Layer::Relative(3))
                        .interactive(false)
                        .width(Size::px(SV_HANDLE))
                        .height(Size::px(SV_HANDLE))
                        .corner_radius(CornerRadius::new_all(SV_HANDLE))
                        .background(current)
                        .border(
                            Border::new()
                                .fill(Color::WHITE)
                                .width(2.)
                                .alignment(BorderAlignment::Inner),
                        )
                        .shadow(handle_shadow()),
                );

            let hue_bar = rect()
                .width(Size::fill())
                .height(Size::px(HUE_HEIGHT))
                .on_sized(move |e: Event<SizedEventData>| hue_area.set_if_modified(e.area))
                .on_pointer_down({
                    let mut pick_hue = pick_hue.clone();
                    move |e: Event<PointerEventData>| {
                        if !e.data().is_primary() {
                            return;
                        }
                        drag.set(Drag::Hue);
                        pick_hue(e.global_location());
                        e.stop_propagation();
                        e.prevent_default();
                    }
                })
                .child(
                    rect()
                        .width(Size::fill())
                        .height(Size::fill())
                        .corner_radius(CornerRadius::new_all(8.))
                        .background(
                            LinearGradient::new()
                                .angle(-90.)
                                .stop(((255, 0, 0), 0.))
                                .stop(((255, 255, 0), 16.67))
                                .stop(((0, 255, 0), 33.33))
                                .stop(((0, 255, 255), 50.))
                                .stop(((0, 0, 255), 66.67))
                                .stop(((255, 0, 255), 83.33))
                                .stop(((255, 0, 0), 100.)),
                        ),
                )
                .child(
                    rect()
                        .position(
                            Position::new_absolute()
                                .left(hue_handle_x)
                                .top((HUE_HEIGHT - HUE_HANDLE) / 2.),
                        )
                        .layer(Layer::Relative(3))
                        .interactive(false)
                        .width(Size::px(HUE_HANDLE))
                        .height(Size::px(HUE_HANDLE))
                        .corner_radius(CornerRadius::new_all(HUE_HANDLE))
                        .background(Color::WHITE)
                        .shadow(handle_shadow()),
                );

            let hex_row = rect()
                .horizontal()
                .width(Size::fill())
                .cross_align(Alignment::Center)
                .spacing(12.)
                .child(
                    rect()
                        .width(Size::px(SWATCH_SIZE))
                        .height(Size::px(SWATCH_SIZE))
                        .corner_radius(CornerRadius::new_all(8.))
                        .background(current)
                        .border(border_all_color(1., colors::component_border())),
                )
                .child(
                    TextInput::new(hex)
                        .width(Size::flex(1.))
                        .leading(
                            label()
                                .text("#")
                                .font_size(14.)
                                .color(colors::fg_secondary())
                                .into_element(),
                        )
                        .on_submit(move |text: String| {
                            if let Some((r, g, b)) = colors::parse_hex(&text) {
                                let color = Color::from_rgb(r, g, b);
                                let mut next = hsv_of(color);
                                if next.1 == 0. || next.2 == 0. {
                                    next.0 = hsv.peek().0;
                                }
                                emit(next);
                            } else {
                                hex.set(digits(Color::from_hsv(h, s, v)));
                            }
                        }),
                );

            OverlayPopup::new()
                .backdrop(false)
                .position(Position::new_global().left(left).top(top))
                .on_close(move |_| open.set(false))
                .child(
                    rect()
                        .width(Size::px(POPUP_WIDTH))
                        .vertical()
                        .spacing(14.)
                        .padding(POPUP_PADDING)
                        .corner_radius(CornerRadius::new_all(16.))
                        .background(colors::page_elevated())
                        .border(border_all_color(1., colors::component_border()))
                        .shadow(Shadow::from((
                            0.,
                            8.,
                            32.,
                            0.,
                            Color::from_argb(120, 0, 0, 0),
                        )))
                        .on_global_pointer_move(move |e: Event<PointerEventData>| match drag() {
                            Drag::Sv => pick_sv.clone()(e.global_location()),
                            Drag::Hue => pick_hue.clone()(e.global_location()),
                            Drag::None => {}
                        })
                        .on_global_pointer_up(move |_| drag.set_if_modified(Drag::None))
                        .child(header)
                        .child(sv_pane)
                        .child(hue_bar)
                        .child(hex_row),
                )
        });

        rect().child(trigger).maybe_child(popup)
    }
}
