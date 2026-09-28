use std::collections::HashSet;

use freya::prelude::{Platform, WinitPlatformExt, spawn_forever};
use freya::radio::RadioStation;
use oneclient_core::settings::LaunchBehaviour;

use crate::state::{AppChannel, AppState};

const ALLOWED_URL_SCHEMES: [&str; 3] = ["http", "https", "mailto"];

pub fn focus_window() {
    Platform::get().with_window(Platform::window_id(), |win| {
        #[cfg(target_os = "macos")]
        macos::set_menu_bar_only(false);

        win.set_visible(true);
        win.set_minimized(false);
        win.focus_window();
    });
}

pub fn hide_window() {
    Platform::get().with_window(Platform::window_id(), |win| {
        #[cfg(target_os = "macos")]
        if !tray::is_active() {
            win.set_minimized(true);
            return;
        }

        win.set_visible(false);

        #[cfg(target_os = "macos")]
        macos::set_menu_bar_only(true);
    });
}

pub fn close() {
    let background =
        crate::launcher::state().is_ok_and(|state| state.settings.read().run_in_background);
    if background { hide_window() } else { quit() }
}

pub fn quit() {
    let platform = Platform::get();
    spawn_forever(async move {
        if let Ok(state) = crate::launcher::state() {
            oneclient_core::shutdown(&state).await;
        }
        drop(platform.post_callback(|_, ctx| ctx.exit()));
    });
}

pub fn follow_game(
    station: &RadioStation<AppState, AppChannel>,
    launched: &HashSet<i64>,
    previous: Option<i64>,
) -> Option<i64> {
    let (current, any, behaviour) = {
        let state = station.peek();
        (
            state
                .game
                .running_clusters()
                .filter(|id| launched.contains(id))
                .min(),
            state.game.running_clusters().next().is_some(),
            state.settings.settings.launch_behaviour,
        )
    };

    tray::set_game_running(any);

    if current == previous {
        return previous;
    }

    match (behaviour, current.is_some()) {
        (LaunchBehaviour::HideWhilePlaying, true) => hide_window(),
        (LaunchBehaviour::HideWhilePlaying, false) => focus_window(),
        (LaunchBehaviour::CloseLauncher, true) => quit(),
        _ => {}
    }

    current
}

pub fn open_url(url: &str) {
    if !has_allowed_scheme(url) {
        tracing::warn!("refused to open url with a disallowed scheme: {url}");
        return;
    }

    open_target(url);
}

/// For paths the launcher itself produced, which carry no url scheme.
pub fn open_path(path: &str) {
    open_target(path);
}

fn has_allowed_scheme(url: &str) -> bool {
    let Some((scheme, _)) = url.split_once(':') else {
        return false;
    };

    ALLOWED_URL_SCHEMES
        .iter()
        .any(|allowed| scheme.eq_ignore_ascii_case(allowed))
}

fn open_target(target: &str) {
    if let Err(err) = open::that_detached(target) {
        tracing::warn!("failed to open {target}: {err}");
    }
}

pub fn copy_image_to_clipboard(path: std::path::PathBuf) {
    std::thread::spawn(move || {
        let img = match image::open(&path) {
            Ok(img) => img.into_rgba8(),
            Err(err) => {
                tracing::warn!("failed to decode {} for clipboard: {err}", path.display());
                return;
            }
        };
        let (width, height) = (img.width() as usize, img.height() as usize);
        let data = arboard::ImageData {
            width,
            height,
            bytes: std::borrow::Cow::Owned(img.into_raw()),
        };
        match arboard::Clipboard::new() {
            Ok(mut clip) => {
                if let Err(err) = clip.set_image(data) {
                    tracing::warn!("failed to copy image to clipboard: {err}");
                }
            }
            Err(err) => tracing::warn!("failed to open clipboard: {err}"),
        }
    });
}

pub mod tray {
    use std::cell::RefCell;
    use std::sync::atomic::{AtomicBool, Ordering};

    use freya::prelude::{LaunchConfig, RendererContext};
    use freya::tray::menu::{Menu, MenuItem, PredefinedMenuItem};
    use freya::tray::{TrayEvent, TrayIcon, TrayIconBuilder};

    use crate::constants;
    use crate::ipc::{self, IpcCommand};

    const OPEN_ID: &str = "oneclient.open";
    const PLAY_ID: &str = "oneclient.play";
    const STOP_ID: &str = "oneclient.stop";
    const LOGS_ID: &str = "oneclient.logs";
    const QUIT_ID: &str = "oneclient.quit";

    #[cfg(target_os = "macos")]
    const ICON: &[u8] = include_bytes!("../icons/tray.png");
    #[cfg(not(target_os = "macos"))]
    const ICON: &[u8] = include_bytes!("../icons/64x64.png");

    static ACTIVE: AtomicBool = AtomicBool::new(false);

    thread_local! {
        static GAME_ITEMS: RefCell<Vec<MenuItem>> = const { RefCell::new(Vec::new()) };
    }

    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(super) fn is_active() -> bool {
        ACTIVE.load(Ordering::Relaxed)
    }

    pub(super) fn set_game_running(running: bool) {
        GAME_ITEMS.with_borrow(|items| {
            for item in items {
                item.set_enabled(running);
            }
        });
    }

    pub fn build() -> TrayIcon {
        let live_on_linux = cfg!(target_os = "linux");
        let stop = MenuItem::with_id(STOP_ID, "Stop game", live_on_linux, None);
        let logs = MenuItem::with_id(LOGS_ID, "Show logs", live_on_linux, None);

        let menu = Menu::with_items(&[
            &MenuItem::with_id(OPEN_ID, "Open OneClient", true, None),
            &MenuItem::with_id(PLAY_ID, "Play last version", true, None),
            &PredefinedMenuItem::separator(),
            &stop,
            &logs,
            &PredefinedMenuItem::separator(),
            &MenuItem::with_id(QUIT_ID, "Quit OneClient", true, None),
        ])
        .expect("failed to build the tray menu");

        GAME_ITEMS.with_borrow_mut(|items| *items = vec![stop, logs]);
        ACTIVE.store(true, Ordering::Relaxed);

        TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip(constants::WINDOW_TITLE)
            .with_icon(LaunchConfig::tray_icon(ICON))
            .with_icon_as_template(cfg!(target_os = "macos"))
            .with_menu_on_left_click(!cfg!(target_os = "windows"))
            .build()
            .expect("failed to create the tray icon")
    }

    pub fn handle(event: TrayEvent, _ctx: RendererContext<'_>) {
        let command = match event {
            TrayEvent::Menu(menu) => match menu.id.0.as_str() {
                OPEN_ID => IpcCommand::Focus,
                PLAY_ID => return play_last(),
                STOP_ID => IpcCommand::Stop,
                LOGS_ID => IpcCommand::Logs,
                QUIT_ID => IpcCommand::Quit,
                _ => return,
            },
            #[cfg(target_os = "windows")]
            TrayEvent::Icon(freya::tray::TrayIconEvent::Click {
                button: freya::tray::MouseButton::Left,
                button_state: freya::tray::MouseButtonState::Up,
                ..
            }) => IpcCommand::Focus,
            TrayEvent::Icon(_) => return,
        };
        ipc::send(command);
    }

    fn play_last() {
        tokio::spawn(async {
            let Ok(state) = crate::launcher::state() else {
                return;
            };
            let Ok(clusters) = state.clusters.list().await else {
                return;
            };
            if let Some(cluster) = crate::utils::sort_clusters_for_home(clusters)
                .into_iter()
                .next()
            {
                ipc::send(IpcCommand::Launch(cluster.folder_name));
            }
        });
    }
}

#[cfg(target_os = "macos")]
pub mod macos {
    use std::time::Duration;

    pub fn loop_memory_collector() {
        tokio::spawn(async {
            tokio::time::sleep(Duration::from_secs(8)).await;

            loop {
                release_unused_memory();
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });
    }

    fn release_unused_memory() {
        unsafe {
            unsafe extern "C" {
                fn malloc_zone_pressure_relief(zone: *mut core::ffi::c_void, goal: usize) -> usize;
            }
            malloc_zone_pressure_relief(core::ptr::null_mut(), 0);
        }
    }

    pub fn set_menu_bar_only(menu_bar_only: bool) {
        const REGULAR: isize = 0;
        const ACCESSORY: isize = 1;

        type Id = *mut core::ffi::c_void;

        unsafe {
            unsafe extern "C" {
                fn objc_getClass(name: *const core::ffi::c_char) -> Id;
                fn sel_registerName(name: *const core::ffi::c_char) -> Id;
                fn objc_msgSend();
            }

            let class = objc_getClass(c"NSApplication".as_ptr());
            if class.is_null() {
                return;
            }

            let send: unsafe extern "C" fn() = objc_msgSend;
            let shared: unsafe extern "C" fn(Id, Id) -> Id = core::mem::transmute(send);
            let app = shared(class, sel_registerName(c"sharedApplication".as_ptr()));
            if app.is_null() {
                return;
            }

            let set_policy: unsafe extern "C" fn(Id, Id, isize) -> bool =
                core::mem::transmute(send);
            set_policy(
                app,
                sel_registerName(c"setActivationPolicy:".as_ptr()),
                if menu_bar_only { ACCESSORY } else { REGULAR },
            );
        }
    }
}
