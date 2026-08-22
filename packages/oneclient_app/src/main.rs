#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
#![recursion_limit = "256"]

use freya::prelude::*;
use freya::radio::use_init_radio_station;
use oneclient_app::state::{AppChannel, AppState};
use oneclient_app::{
    Actions, ConfirmLinkOverlay, EventPump, LinkConfirmState, constants, events, router, theme,
    use_provide_actions, use_provide_link_confirm,
};
use tokio::runtime::Builder;

struct OneClientApp;

impl App for OneClientApp {
    fn render(&self) -> impl IntoElement {
        // Radio state is `!Send` so every writer runs on the UI thread `spawn_forever`
        let station = use_init_radio_station::<AppState, AppChannel>(AppState::default);

        let actions = use_hook(move || {
            let (signals_tx, signals_rx) = tokio::sync::mpsc::unbounded_channel();
            let (events_bus, events_rx) = oneclient_events::EventBus::channel();
            let actions = Actions::new(station, signals_tx, events_bus.clone());

            // Started first so nothing emitted during startup waits on a consumer
            spawn_forever(
                EventPump {
                    events: events_rx,
                    signals: signals_rx,
                    station,
                }
                .run(),
            );

            let startup = actions.clone();
            spawn_forever(async move {
                match events::start_launcher(station, events_bus).await {
                    // Must follow startup `sync_bundles` needs the launcher handle and firing
                    // it early leaves `syncing_bundles` stuck disabling every launch button
                    Ok(()) => startup.sync_bundles(),
                    Err(err) => events::report_startup_failure(&station, &err),
                }
            });

            actions
        });

        use_provide_actions(&actions);

        let link_confirm = use_state(|| None::<String>);
        use_provide_link_confirm(LinkConfirmState(link_confirm));

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .child(ConfirmLinkOverlay)
            .child(router())
    }
}

fn main() {
    let mut builder = Builder::new_multi_thread();
    builder.enable_all().max_blocking_threads(64);

    // Debug builds emit unoptimized async code that can overflow the default 2MB stack
    #[cfg(debug_assertions)]
    builder.thread_stack_size(3 * 1024 * 1024);

    let rt = builder.build().unwrap();
    let _tokio_guard = rt.enter();

    let settings = rt.block_on(oneclient_core::settings::store::load_settings(None));

    oneclient_common::consent::init(settings.declined_tos);

    if settings.log_debug {
        oneclient_core::logger::init_debug()
    } else {
        oneclient_core::logger::init()
    }
    .expect("Failed to initialize logger");

    let _sentry_guard = oneclient_core::reporting::init(settings.crash_reporting);


    #[cfg(target_os = "macos")]
    oneclient_app::platform::macos::loop_memory_collector();

    let window_config = WindowConfig::new_app(OneClientApp)
        .with_title(constants::WINDOW_TITLE)
        .with_app_id(constants::WINDOW_APP_ID)
        .with_icon(LaunchConfig::window_icon(include_bytes!(
            "../icons/128x128.png"
        )))
        .with_size(1200., 800.)
        .with_min_size(800., 600.)
        .with_transparency(true)
        .with_background(Color::TRANSPARENT);

    #[cfg(target_os = "macos")]
    let window_config = window_config
        .with_decorations(true)
        .with_window_attributes(|attrs, _| {
            use freya::winit::platform::macos::WindowAttributesExtMacOS;
            attrs
                .with_title_hidden(true)
                .with_titlebar_transparent(true)
                .with_titlebar_buttons_hidden(true)
                .with_fullsize_content_view(true)
				.with_has_shadow(true)
        });

    #[cfg(not(target_os = "macos"))]
    let window_config = window_config.with_decorations(false);

    let mut launch_config = LaunchConfig::new()
        .with_window(window_config)
        .with_gpu_resource_cache_limit(
            std::env::var("ONECLIENT_GPU_CACHE_BYTES")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(512 * 1024 * 1024),
        )
        .with_default_font(theme::DEFAULT_FONT);

    for (font, bytes) in theme::load_fonts() {
        launch_config = launch_config.with_font(font, bytes);
    }

    launch(launch_config);
}
