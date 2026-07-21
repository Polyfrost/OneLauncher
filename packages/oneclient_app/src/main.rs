#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use freya::prelude::*;
use oneclient_app::{
    ConfirmLinkOverlay, CoreBridgeHandle, DataSync, LinkConfirmState, OneClientBridge, constants,
    router, theme, use_provide_link_confirm,
};
use tokio::runtime::Builder;

struct OneClientApp {
    bridge: OneClientBridge,
}

impl App for OneClientApp {
    fn render(&self) -> impl IntoElement {
        oneclient_app::use_provide_bridge(&self.bridge);

        let link_confirm = use_state(|| None::<String>);
        use_provide_link_confirm(LinkConfirmState(link_confirm));

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .child(DataSync)
            .child(ConfirmLinkOverlay)
            .child(router())
    }
}

fn main() {
    let mut builder = Builder::new_multi_thread();
    builder.enable_all().max_blocking_threads(16);

    // Debug builds emit unoptimized async code, which can cause stack overflows in some cases.
	// Default stack size is 2MB, so we'll increase it to 4MB for debug builds
    #[cfg(debug_assertions)]
    builder.thread_stack_size(4 * 1024 * 1024);

    let rt = builder.build().unwrap();
    let _tokio_guard = rt.enter();

    let settings = rt.block_on(oneclient_core::settings::store::load_settings(None));

    if settings.log_debug {
        oneclient_core::logger::init_debug()
    } else {
        oneclient_core::logger::init()
    }
    .expect("Failed to initialize logger");

    let _sentry_guard = oneclient_core::reporting::init(settings.crash_reporting);

    let (bridge, handle): (OneClientBridge, CoreBridgeHandle) = OneClientBridge::new();
    handle.spawn_runtime();

    #[cfg(target_os = "macos")]
    oneclient_app::platform::macos::loop_memory_collector();

    let window_config = WindowConfig::new_app(OneClientApp { bridge })
        .with_title(constants::WINDOW_TITLE)
        .with_app_id(constants::WINDOW_APP_ID)
        .with_icon(LaunchConfig::window_icon(include_bytes!(
            "../icons/128x128.png"
        )))
        .with_size(1200., 800.)
        .with_min_size(800., 600.)
        .with_transparency(true)
        .with_background(Color::TRANSPARENT);

    // macOS: keep the native frame (rounded corners + drop shadow) but hide the
    // titlebar and extend content into it. Other platforms stay borderless.
    #[cfg(target_os = "macos")]
    let window_config = window_config
        .with_decorations(true)
        .with_window_attributes(|attrs, _| {
            use freya::winit::platform::macos::WindowAttributesExtMacOS;
            attrs
                .with_titlebar_hidden(true)
                .with_title_hidden(true)
                .with_titlebar_transparent(true)
                .with_titlebar_buttons_hidden(true)
                .with_fullsize_content_view(true)
        });

    #[cfg(not(target_os = "macos"))]
    let window_config = window_config.with_decorations(false);

    let mut launch_config = LaunchConfig::new()
        .with_window(window_config)
        .with_gpu_resource_cache_limit(
            std::env::var("ONECLIENT_GPU_CACHE_MB")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(32 * 1024 * 1024),
        )
        .with_default_font(theme::DEFAULT_FONT);

    for (font, bytes) in theme::load_fonts() {
        launch_config = launch_config.with_font(font, bytes);
    }

    launch(launch_config);
}
