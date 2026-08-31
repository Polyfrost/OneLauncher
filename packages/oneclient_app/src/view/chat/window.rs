use std::sync::atomic::{AtomicBool, Ordering};

use freya::prelude::*;
use freya::winit::dpi::{PhysicalPosition, PhysicalSize};

use crate::components::secondary_window_controls;
use crate::hooks::use_provide_station;
use crate::theme::{self, colors};
use crate::view::single_window::{Claim, SingleWindow};

use super::page::ChatSurface;

const WINDOW_TITLE: &str = "OneClient — Messages";

const TITLEBAR_PADDING_PX: f32 = 12.;
const TITLEBAR_HEIGHT_PX: f32 = 60.;

const WINDOW_WIDTH_PX: f64 = 940.;
const WINDOW_HEIGHT_PX: f64 = 620.;
const WINDOW_GAP_PX: f64 = 12.;

static CHAT_WINDOW: SingleWindow = SingleWindow::new();

static CHAT_FOCUSED: AtomicBool = AtomicBool::new(false);

pub fn open_chat_window() {
    let opening = match CHAT_WINDOW.claim() {
        Claim::Focus(id) => {
            Platform::get().focus_window(Some(id));
            return;
        }
        Claim::Busy => return,
        Claim::Launch(guard) => guard,
    };

    let platform = Platform::get();
    let anchor = platform.post_callback(|id, ctx: &mut RendererContext| {
        let window = ctx.windows.get(&id)?.window();
        let monitor = window.current_monitor()?;

        Some(Anchor {
            position: window.outer_position().ok()?,
            size: window.outer_size(),
            scale_factor: window.scale_factor(),
            monitor_position: monitor.position(),
            monitor_size: monitor.size(),
        })
    });

    spawn_forever(async move {
        let _opening = opening;
        let beside = anchor.await.ok().flatten().map(beside_launcher);
        platform.launch_window(window_config(beside)).await;
    });
}

pub(super) fn set_chat_focus(focused: bool) {
    CHAT_FOCUSED.store(focused, Ordering::Relaxed);
}

pub fn is_chat_window_focused() -> bool {
    CHAT_WINDOW.is_open() && CHAT_FOCUSED.load(Ordering::Relaxed)
}

pub fn close_chat_window_in(ctx: &mut RendererContext<'_>) {
    CHAT_WINDOW.close_in(ctx);
}

struct Anchor {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    scale_factor: f64,
    monitor_position: PhysicalPosition<i32>,
    monitor_size: PhysicalSize<u32>,
}

fn beside_launcher(anchor: Anchor) -> PhysicalPosition<i32> {
    let scale = |value: f64| (value * anchor.scale_factor).round() as i32;

    let width = scale(WINDOW_WIDTH_PX);
    let height = scale(WINDOW_HEIGHT_PX);
    let gap = scale(WINDOW_GAP_PX);

    let launcher_right = anchor.position.x + anchor.size.width as i32;
    let monitor_right = anchor.monitor_position.x + anchor.monitor_size.width as i32;
    let monitor_bottom = anchor.monitor_position.y + anchor.monitor_size.height as i32;

    let to_the_right = launcher_right + gap;
    let to_the_left = anchor.position.x - gap - width;

    let x = if to_the_right + width <= monitor_right {
        to_the_right
    } else if to_the_left >= anchor.monitor_position.x {
        to_the_left
    } else {
        (monitor_right - width).max(anchor.monitor_position.x)
    };

    let y = anchor
        .position
        .y
        .min(monitor_bottom - height)
        .max(anchor.monitor_position.y);

    PhysicalPosition::new(x, y)
}

pub fn close_chat_window() {
    if let Some(id) = CHAT_WINDOW.take() {
        Platform::get().close_window(id);
    }
}

fn window_config(position: Option<PhysicalPosition<i32>>) -> WindowConfig {
    let config = WindowConfig::new_app(ChatApp)
        .with_title(WINDOW_TITLE)
        .with_app_id(crate::constants::WINDOW_APP_ID)
        .with_icon(LaunchConfig::window_icon(include_bytes!(
            "../../../icons/128x128.png"
        )))
        .with_size(WINDOW_WIDTH_PX, WINDOW_HEIGHT_PX)
        .with_min_size(560., 400.)
        .with_transparency(true)
        .with_background(Color::TRANSPARENT)
        .with_window_attributes(move |attrs, _| {
            let attrs = match position {
                Some(position) => attrs.with_position(position),
                None => attrs,
            };

            #[cfg(target_os = "macos")]
            let attrs = {
                use freya::winit::platform::macos::WindowAttributesExtMacOS;
                attrs
                    .with_title_hidden(true)
                    .with_titlebar_transparent(true)
                    .with_titlebar_buttons_hidden(true)
                    .with_fullsize_content_view(true)
            };

            attrs
        })
        .with_window_handle(|window| CHAT_WINDOW.opened(window.id()))
        .with_on_close(|_, _| {
            CHAT_WINDOW.forget();
            CloseDecision::Close
        });

    #[cfg(target_os = "macos")]
    let config = config.with_decorations(true);

    #[cfg(not(target_os = "macos"))]
    let config = config.with_decorations(false);

    config
}

struct ChatApp;

impl App for ChatApp {
    fn render(&self) -> impl IntoElement {
        use_provide_station();

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .background(colors::page())
            .color(colors::fg_primary())
            .font_family(theme::DEFAULT_FONT)
            .corner_radius(CornerRadius::new_all(crate::ui::use_window_corner()))
            .content(Content::Flex)
            .overflow(Overflow::Clip)
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .position(Position::new_absolute())
                    .interactive(false)
                    .child(crate::layout::gradient_overlay_radial()),
            )
            .child(titlebar())
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .overflow(Overflow::Clip)
                    .child(ChatSurface),
            )
    }
}

fn titlebar() -> impl IntoElement {
    crate::ui::glass_panel()
        .width(Size::fill())
        .height(Size::px(TITLEBAR_HEIGHT_PX))
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .height(Size::fill())
                .content(Content::Flex)
                .cross_align(Alignment::Center)
                .padding(Gaps::new_all(TITLEBAR_PADDING_PX))
                .child(
                    label()
                        .text("Messages")
                        .font_size(14.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary())
                        .max_lines(1),
                )
                .child(rect().width(Size::flex(1.0)).height(Size::fill()))
                .child(secondary_window_controls(close_chat_window)),
        )
        .child(
            rect()
                .window_drag()
                .layer(Layer::OverlayLevel(1))
                .width(Size::window_percent(100.))
                .height(Size::px(TITLEBAR_HEIGHT_PX))
                .position(Position::new_absolute().top(0.).left(0.).right(0.)),
        )
}
