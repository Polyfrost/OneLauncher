use std::sync::atomic::{AtomicBool, Ordering};

use freya::prelude::*;
use freya::winit::dpi::{PhysicalPosition, PhysicalSize};

use crate::hooks::{use_dispatch, use_provide_station};
use crate::state::AppState;
use crate::theme::{self, colors};
use crate::view::single_window::{Claim, SingleWindow};

use super::panel::{PanelMode, SocialPanel};

const WINDOW_TITLE: &str = "Social — OneClient";

const WINDOW_WIDTH_PX: f64 = 940.;
const WINDOW_HEIGHT_PX: f64 = 720.;
const WINDOW_GAP_PX: f64 = 12.;

static CHAT_WINDOW: SingleWindow = SingleWindow::new();

static CHAT_FOCUSED: AtomicBool = AtomicBool::new(false);

pub fn open_chat_window() {
    let opening = match CHAT_WINDOW.claim() {
        Claim::Focus(id) => {
            Platform::get().focus_window(id);
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

fn set_chat_focus(focused: bool) {
    CHAT_FOCUSED.store(focused, Ordering::Relaxed);
}

pub fn is_chat_window_open() -> bool {
    CHAT_WINDOW.is_open()
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
    WindowConfig::new_app(ChatApp)
        .with_title(WINDOW_TITLE)
        .with_app_id(crate::constants::WINDOW_APP_ID)
        .with_icon(LaunchConfig::window_icon(include_bytes!(
            "../../../icons/128x128.png"
        )))
        .with_size(WINDOW_WIDTH_PX, WINDOW_HEIGHT_PX)
        .with_min_size(640., 400.)
        .with_decorations(true)
        .with_background(colors::page())
        .with_window_attributes(move |attrs, _| match position {
            Some(position) => attrs.with_position(position),
            None => attrs,
        })
        .with_window_handle(|window| CHAT_WINDOW.opened(window.id()))
        .with_on_close(|_, _| {
            CHAT_WINDOW.forget();
            CloseDecision::Close
        })
}

struct ChatApp;

impl App for ChatApp {
    fn render(&self) -> impl IntoElement {
        use_provide_station(AppState::default);
        let dispatch = use_dispatch();

        track_focus(&dispatch);

        use_drop({
            let dispatch = dispatch.clone();
            move || dispatch.dismiss_social_notice()
        });

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .background(colors::page())
            .color(colors::fg_primary())
            .font_family(theme::DEFAULT_FONT)
            .overflow(Overflow::Clip)
            .child(SocialPanel {
                mode: PanelMode::Window,
                expanded: true,
            })
    }
}

fn track_focus(dispatch: &crate::hooks::Actions) {
    let focused = *Platform::get().is_app_focused.read();
    let mut was_focused = use_state(|| focused);

    set_chat_focus(focused);

    if focused && !*was_focused.peek() {
        dispatch.sync_chat();
    }

    was_focused.set_if_modified(focused);
}
