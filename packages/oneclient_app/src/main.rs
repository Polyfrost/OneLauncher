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
    let rt = Builder::new_multi_thread()
        .enable_all()
        .max_blocking_threads(16)
        .build()
        .unwrap();
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

    let mut launch_config = LaunchConfig::new()
        .with_window(
            WindowConfig::new_app(OneClientApp { bridge })
                .with_title(constants::WINDOW_TITLE)
                .with_app_id(constants::APP_ID)
                .with_size(1200., 800.)
                .with_min_size(800., 600.)
                .with_decorations(false)
                .with_transparency(true)
                .with_background(Color::TRANSPARENT),
        )
        .with_gpu_resource_cache_limit(
            std::env::var("ONECLIENT_GPU_CACHE_MB")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(32),
        )
        .with_default_font(theme::DEFAULT_FONT);

    for (font, bytes) in theme::load_fonts() {
        launch_config = launch_config.with_font(font, bytes);
    }

    launch(launch_config);
}
