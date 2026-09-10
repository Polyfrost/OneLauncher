#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
#![recursion_limit = "256"]

use freya::prelude::*;
use freya::radio::use_init_radio_station;
use oneclient_app::ipc::{self, Claim};
use oneclient_app::state::{AppChannel, AppState, LauncherInit};
use oneclient_app::{
    Actions, ConfirmLinkOverlay, EventPump, LinkConfirmState, StartMaximizedState, cli, constants,
    events, platform, router, theme, use_provide_actions, use_provide_link_confirm,
    use_provide_start_maximized, microsoft_java,
};
use std::cell::Cell;
use tokio::runtime::Builder;

struct OneClientApp {
    needs_location: bool,
    start_maximized: bool,
    boot_launch: Cell<Option<String>>,
    ipc: Cell<Option<ipc::Listener>>,
}

impl App for OneClientApp {
    fn render(&self) -> impl IntoElement {
        let needs_location = self.needs_location;

        // Radio state is `!Send` so every writer runs on the UI thread `spawn_forever`
        let station = use_init_radio_station::<AppState, AppChannel>(move || AppState {
            launcher: LauncherInit {
                needs_location,
                ..LauncherInit::default()
            },
            ..AppState::default()
        });

        let boot_launch = self.boot_launch.take();
        let ipc_listener = self.ipc.take();

        let actions = use_hook(move || {
            let (signals_tx, signals_rx) = tokio::sync::mpsc::unbounded_channel();
            let (events_bus, events_rx) = oneclient_events::EventBus::channel();
            let actions = Actions::new(station, signals_tx, events_bus.clone());

            spawn_forever(
                EventPump {
                    events: events_rx,
                    signals: signals_rx,
                    station,
                }
                .run(),
            );

            if !needs_location {
                let startup = actions.clone();
                let rescue_bus = events_bus.clone();
                spawn_forever(async move {
                    match events::start_launcher(station, events_bus).await {
                        // Must follow startup `sync_bundles` needs the launcher handle and firing
                        // it early leaves `syncing_bundles` stuck disabling every launch button
                        Ok(()) => {
                            startup.sync_bundles();
                            microsoft_java::spawn_auto_install();
                        }
                        Err(err) => {
                            events::report_startup_failure(&station, &err);
                            oneclient_app::updater::spawn_update_check(false, rescue_bus);
                        }
                    }
                });
            }

            if let Some(folder) = boot_launch {
                actions.request_launch_by_folder(folder);
            }

            if let Some(listener) = ipc_listener {
                let served = actions.clone();
                spawn_forever(ipc::serve(listener, move |command| match command {
                    ipc::IpcCommand::Launch(folder) => {
                        platform::focus_window();
                        served.request_launch_by_folder(folder);
                    }
                    ipc::IpcCommand::Focus => platform::focus_window(),
                }));
            }

            actions
        });

        use_provide_actions(&actions);

        let link_confirm = use_state(|| None::<String>);
        use_provide_link_confirm(LinkConfirmState(link_confirm));

        use_provide_start_maximized(StartMaximizedState(self.start_maximized));

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .child(ConfirmLinkOverlay)
            .child(router())
    }
}

fn main() {
    let cli = cli::parse();

    let mut builder = Builder::new_multi_thread();
    builder.enable_all().max_blocking_threads(64);

    // Debug builds emit unoptimized async code that can overflow the default 2MB stack
    #[cfg(debug_assertions)]
    builder.thread_stack_size(3 * 1024 * 1024);

    let rt = builder.build().unwrap();
    let _tokio_guard = rt.enter();

    // no settings file is the sign of a fresh install, but not proof of one
    let never_set_up = oneclient_common::paths::settings_file()
        .map(|path| !path.exists())
        .unwrap_or(false);
    let has_database = oneclient_common::paths::database_file()
        .map(|path| path.exists())
        .unwrap_or(false);
    let was_damaged = oneclient_common::paths::damaged_settings_file()
        .map(|path| path.exists())
        .unwrap_or(false);
    let needs_location = never_set_up && !has_database && !was_damaged;

    let mut unprotected = None;
    let ipc = match rt.block_on(ipc::claim(&cli)) {
        Claim::Forwarded => return,
        Claim::Primary(listener) => Some(listener),
        Claim::Solo(reason) => {
            unprotected = Some(reason);
            None
        }
    };

    let settings = rt.block_on(oneclient_core::settings::store::load_settings(None));

    oneclient_common::consent::init(settings.declined_tos);

    let abandoned_move = settings
        .data_dir
        .clone()
        .filter(|dir| oneclient_core::relocate::incomplete(dir));

    if let Some(dir) = settings.data_dir.clone()
        && abandoned_move.is_none()
    {
        oneclient_common::paths::set_data_dir(dir);
    }

    if settings.log_debug {
        oneclient_core::logger::init_debug()
    } else {
        oneclient_core::logger::init()
    }
    .expect("Failed to initialize logger");

    if let Some(reason) = unprotected {
        tracing::warn!("no single-instance endpoint, a second launcher can start: {reason}");
    }

    match oneclient_app::shortcut::launcher_exe()
        .and_then(|exe| oneclient_app::protocol::register(&exe))
    {
        Ok(()) => {}
        Err(err) => tracing::warn!(
            "could not register the {}:// handler: {err:#}",
            oneclient_app::protocol::SCHEME
        ),
    }

    if let Some(dir) = abandoned_move {
        tracing::error!(
            path = %dir.display(),
            "a move of the game data folder never finished; staying on the old folder"
        );
    }

    let _sentry_guard = oneclient_core::reporting::init(settings.crash_reporting);


    #[cfg(target_os = "macos")]
    oneclient_app::platform::macos::loop_memory_collector();

    let start_maximized = settings.start_maximized;

    let window_config = WindowConfig::new_app(OneClientApp {
        needs_location,
        start_maximized,
        boot_launch: Cell::new(cli.launch),
        ipc: Cell::new(ipc),
    })
        .with_title(constants::WINDOW_TITLE)
        .with_app_id(constants::WINDOW_APP_ID)
        .with_icon(LaunchConfig::window_icon(include_bytes!(
            "../icons/128x128.png"
        )))
        .with_size(1200., 800.)
        .with_min_size(800., 600.)
        .with_transparency(true)
        .with_background(Color::TRANSPARENT)
        // A half-copied data folder is unrecoverable, so the window refuses to
        // close while one is being moved; the move screen says as much
        .with_on_close(|_, _| {
            if oneclient_core::relocate::in_progress() {
                tracing::warn!("close request ignored, the data folder is still being moved");
                CloseDecision::KeepOpen
            } else {
                CloseDecision::Close
            }
        });

    #[cfg(target_os = "macos")]
    let window_config = window_config
        .with_decorations(true)
        .with_window_attributes(move |attrs, _| {
            use freya::winit::platform::macos::WindowAttributesExtMacOS;
            attrs
                .with_title_hidden(true)
                .with_titlebar_transparent(true)
                .with_titlebar_buttons_hidden(true)
                .with_fullsize_content_view(true)
				.with_has_shadow(true)
                .with_maximized(start_maximized)
        });

    #[cfg(not(target_os = "macos"))]
    let window_config = window_config
        .with_decorations(false)
        .with_window_attributes(move |attrs, _| attrs.with_maximized(start_maximized));

    let mut launch_config = LaunchConfig::new()
        .with_window(window_config)
        .with_gpu_resource_cache_limit(
            std::env::var("ONECLIENT_GPU_CACHE_BYTES")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(96 * 1024 * 1024),
        )
        .with_default_font(theme::DEFAULT_FONT);

    for (font, bytes) in theme::load_fonts() {
        launch_config = launch_config.with_font(font, bytes);
    }

    launch(launch_config);
}
