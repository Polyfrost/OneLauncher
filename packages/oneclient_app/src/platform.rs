//! The OS-facing edges of the app: the clipboard, the browser, the launcher
//! window itself, and the background icon that outlives it.

use std::collections::HashSet;

use freya::prelude::*;
use freya::radio::RadioStation;
use freya::router::RouterContext;
use oneclient_core::settings::LaunchBehaviour;

use crate::routes::Route;
use crate::state::{AppChannel, AppState};

pub fn open_url(url: &str) {
    if let Err(err) = open::that_detached(url) {
        tracing::warn!("failed to open url {url}: {err}");
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

/// Puts the launcher window back in front of the user.
///
/// Also un-minimizes: a window hidden while minimized comes back minimized, and
/// the user asked to see it, not to be handed a Dock icon.
pub fn show_window(platform: &Platform) {
    platform.with_window(None, |window| {
        // Before the window: an Accessory app has no business activating, and
        // AppKit will not give it key focus while it is one.
        #[cfg(target_os = "macos")]
        macos::set_menu_bar_only(false);

        window.set_minimized(false);
        window.set_visible(true);
        window.focus_window();
    });
}

/// Takes the launcher window off the screen without closing it.
pub fn hide_window(platform: &Platform) {
    platform.with_window(None, |window| {
        window.set_visible(false);

        #[cfg(target_os = "macos")]
        macos::set_menu_bar_only(true);
    });
}

/// Winds the launcher down and stops the event loop.
///
/// The only way out of the process. With a tray registered freya keeps the
/// event loop alive after the last window goes, so closing the window no longer
/// ends anything, and `std::process::exit` would take the Discord socket and
/// the database pool down mid-sentence.
pub fn quit(platform: &Platform) {
    let platform = platform.clone();
    spawn_forever(shutdown_and_exit(platform));
}

async fn shutdown_and_exit(platform: Platform) {
    if let Ok(state) = crate::launcher::state() {
        oneclient_core::shutdown(&state).await;
    }
    // The acknowledgement is dropped rather than awaited: the callback it would
    // answer from is the one that ends the loop, so nothing is left to deliver it.
    drop(platform.post_callback(|_, ctx| ctx.exit()));
}

/// Applies the "what the window does while a game runs" setting.
///
/// `previous` is what this returned last time. The window is only touched when
/// the live cluster changes, so the log lines arriving several times a second
/// during a session cannot re-hide a window the user just brought back by hand.
///
/// `started_here` is the set of clusters this launcher session actually
/// started. A game adopted at startup went straight to `Running` without ever
/// being launched from here, and hiding the window the user has only just
/// opened is not what "hide while playing" asks for.
pub fn sync_game_presence(
    platform: &Platform,
    station: &RadioStation<AppState, AppChannel>,
    started_here: &HashSet<i64>,
    previous: Option<i64>,
) -> Option<i64> {
    // The tray's log entry follows any live game, adopted or not; only the
    // window behaviour cares who started it.
    let (current, any) = {
        let state = station.peek();
        (
            state.game.running_cluster_where(|id| started_here.contains(&id)),
            state.game.running_cluster(),
        )
    };

    tray::set_game_running(any.is_some());

    if current == previous {
        return previous;
    }

    // Read at the transition rather than cached: the setting is a dropdown the
    // user can change while a game is running, and what matters is what it says
    // when the game starts or stops.
    let behaviour = crate::launcher::state()
        .map(|state| state.settings.read().launch_behaviour)
        .unwrap_or_default();

    match window_action(behaviour, current.is_some()) {
        WindowAction::Nothing => {}
        WindowAction::Hide => hide_window(platform),
        WindowAction::Show => show_window(platform),
        WindowAction::Quit => quit(platform),
    }

    current
}

/// What the window does about a game having started or stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowAction {
    Nothing,
    Hide,
    Show,
    Quit,
}

/// The setting, as a decision. Pure, and separate from the work it describes:
/// this is the whole of what the three options mean, and none of it needs a
/// window or a launcher to be true.
fn window_action(behaviour: LaunchBehaviour, playing: bool) -> WindowAction {
    match (behaviour, playing) {
        (LaunchBehaviour::KeepVisible, _) => WindowAction::Nothing,
        (LaunchBehaviour::HideWhilePlaying, true) => WindowAction::Hide,
        (LaunchBehaviour::HideWhilePlaying, false) => WindowAction::Show,
        (LaunchBehaviour::CloseLauncher, true) => WindowAction::Quit,
        // Nothing is left to close, and nothing to bring back: the launcher
        // that started this game is gone.
        (LaunchBehaviour::CloseLauncher, false) => WindowAction::Nothing,
    }
}

/// Answers the tray's menu from inside the UI.
///
/// The tray runs on the renderer thread — its own GTK thread on Linux — so it
/// can only post intent; the router and the app state are reachable from here
/// and nowhere else. Called from the root layout, which lives inside the router
/// and never unmounts.
pub fn use_tray_bridge() {
    let dispatch = crate::hooks::use_dispatch();

    use_hook(move || {
        let Some(mut inbox) = tray::take_inbox() else {
            return;
        };
        let platform = Platform::get();
        let router = RouterContext::get();

        spawn_forever(async move {
            while let Some(command) = inbox.recv().await {
                match command {
                    tray::TrayCommand::ShowWindow => show_window(&platform),
                    tray::TrayCommand::ShowLogs => {
                        show_window(&platform);
                        // The entry is disabled while nothing is playing, but a
                        // game can exit between the menu opening and the click.
                        if let Some(cluster_id) = dispatch.running_cluster() {
                            let _ = router.push(Route::ProcessLogs { cluster_id });
                        }
                    }
                    tray::TrayCommand::Quit => shutdown_and_exit(platform.clone()).await,
                }
            }
        });
    });
}

/// The background icon: a menu-bar item on macOS, a system tray icon on Windows
/// and Linux.
///
/// Freya owns the `tray-icon` integration (it creates the icon on the main
/// thread during `resumed`, which macOS insists on, and gives Linux its own GTK
/// thread), so this is only the menu and what its entries mean.
pub mod tray {
    use std::cell::RefCell;
    use std::sync::{Mutex, OnceLock};

    use freya::prelude::{LaunchConfig, RendererContext};
    use freya::tray::TrayEvent;
    use freya::tray::menu::{Menu, MenuItem, PredefinedMenuItem};
    use freya::tray::{TrayIcon, TrayIconBuilder};
    use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

    use crate::constants;

    const SHOW_ID: &str = "oneclient.show";
    const LOGS_ID: &str = "oneclient.logs";
    const QUIT_ID: &str = "oneclient.quit";

    /// What the tray asks the UI to do on its behalf.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TrayCommand {
        ShowWindow,
        ShowLogs,
        Quit,
    }

    static COMMANDS: OnceLock<UnboundedSender<TrayCommand>> = OnceLock::new();
    static INBOX: Mutex<Option<UnboundedReceiver<TrayCommand>>> = Mutex::new(None);

    thread_local! {
        /// Parked here rather than in a global because muda's handles are
        /// `Rc`-backed and belong to whichever thread built the tray.
        static LOGS_ITEM: RefCell<Option<MenuItem>> = const { RefCell::new(None) };
    }

    fn sender() -> &'static UnboundedSender<TrayCommand> {
        COMMANDS.get_or_init(|| {
            let (tx, rx) = unbounded_channel();
            *INBOX.lock().unwrap() = Some(rx);
            tx
        })
    }

    pub fn send(command: TrayCommand) {
        if sender().send(command).is_err() {
            tracing::debug!("no tray bridge listening; dropping {command:?}");
        }
    }

    /// Takes the receiving end, once. [`None`] on any later call.
    pub(super) fn take_inbox() -> Option<UnboundedReceiver<TrayCommand>> {
        sender();
        INBOX.lock().unwrap().take()
    }

    /// Mirrors "a game is running" onto the "Show logs" entry.
    ///
    /// A no-op on Linux, where freya parks the tray on its own GTK thread and
    /// the handle is not ours to touch from here — the entry stays enabled
    /// there and answers with whatever is playing when it is clicked.
    pub(super) fn set_game_running(running: bool) {
        LOGS_ITEM.with_borrow(|item| {
            if let Some(item) = item {
                item.set_enabled(running);
            }
        });
    }

    /// Builds the icon and its menu. Freya calls this once, on the thread that
    /// owns the tray.
    pub fn build() -> TrayIcon {
        let show = MenuItem::with_id(SHOW_ID, "Show window", true, None);
        // Nothing is playing this early, and [`set_game_running`] takes it from
        // here — except on Linux, where the handle never comes back within
        // reach, so the entry starts live rather than being stuck off forever.
        let logs = MenuItem::with_id(LOGS_ID, "Show logs", cfg!(target_os = "linux"), None);
        let quit = MenuItem::with_id(QUIT_ID, "Quit launcher", true, None);

        let menu = Menu::with_items(&[
            &show,
            &logs,
            &PredefinedMenuItem::separator(),
            &quit,
        ])
        .expect("failed to build the tray menu");

        LOGS_ITEM.with_borrow_mut(|slot| *slot = Some(logs));

        TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip(constants::WINDOW_TITLE)
            .with_icon(LaunchConfig::tray_icon(include_bytes!(
                "../icons/32x32.png"
            )))
            // Windows expects a left click to open the app and only a right
            // click to open the menu; macOS and Linux want the menu either way.
            .with_menu_on_left_click(!cfg!(target_os = "windows"))
            .build()
            .expect("failed to create the tray icon")
    }

    /// Turns a tray interaction into a [`TrayCommand`].
    ///
    /// The [`RendererContext`] could show the window from here, but everything
    /// goes through the bridge instead so that "show the window" means the same
    /// thing however it was asked for.
    pub fn handle(event: TrayEvent, _ctx: RendererContext<'_>) {
        match event {
            TrayEvent::Menu(menu) => match menu.id.0.as_str() {
                SHOW_ID => send(TrayCommand::ShowWindow),
                LOGS_ID => send(TrayCommand::ShowLogs),
                QUIT_ID => send(TrayCommand::Quit),
                other => tracing::debug!("unhandled tray menu entry {other}"),
            },

            #[cfg(target_os = "windows")]
            TrayEvent::Icon(freya::tray::TrayIconEvent::Click {
                button: freya::tray::MouseButton::Left,
                button_state: freya::tray::MouseButtonState::Up,
                ..
            }) => send(TrayCommand::ShowWindow),

            TrayEvent::Icon(_) => {}
        }
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

    /// Swaps the app between a Dock app and a menu-bar-only one.
    ///
    /// A Regular app whose only window is hidden keeps a Dock icon that clicking
    /// does nothing with, because winit surfaces no reopen event to answer it —
    /// so while the window is away the app becomes an Accessory, which is what
    /// the menu-bar item is for.
    ///
    /// winit 0.30 only lets the policy be chosen when the event loop is built,
    /// so this goes through the Objective-C runtime directly, the same way
    /// [`release_unused_memory`] reaches into libmalloc. Main thread only.
    pub fn set_menu_bar_only(menu_bar_only: bool) {
        // NSApplicationActivationPolicyRegular / NSApplicationActivationPolicyAccessory.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_leaves_the_window_alone() {
        assert_eq!(
            window_action(LaunchBehaviour::default(), true),
            WindowAction::Nothing,
        );
        assert_eq!(
            window_action(LaunchBehaviour::default(), false),
            WindowAction::Nothing,
        );
    }

    #[test]
    fn hiding_is_undone_when_the_game_stops() {
        assert_eq!(
            window_action(LaunchBehaviour::HideWhilePlaying, true),
            WindowAction::Hide,
        );
        assert_eq!(
            window_action(LaunchBehaviour::HideWhilePlaying, false),
            WindowAction::Show,
        );
    }

    #[test]
    fn closing_is_not_undone_when_the_game_stops() {
        assert_eq!(
            window_action(LaunchBehaviour::CloseLauncher, true),
            WindowAction::Quit,
        );
        assert_eq!(
            window_action(LaunchBehaviour::CloseLauncher, false),
            WindowAction::Nothing,
        );
    }
}
