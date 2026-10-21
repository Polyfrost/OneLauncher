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

#[cfg(target_os = "linux")]
fn open_target(target: &str) {
    use std::process::Stdio;

    let spawned = open::commands(target).into_iter().find_map(|mut command| {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()
    });

    match spawned {
        Some(mut child) => {
            std::thread::spawn(move || child.wait());
        }
        None => tracing::warn!("failed to open {target}"),
    }
}

#[cfg(not(target_os = "linux"))]
fn open_target(target: &str) {
    let target = target.to_owned();
    std::thread::spawn(move || {
        if let Err(err) = open::that_detached(&target) {
            tracing::warn!("failed to open {target}: {err}");
        }
    });
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

    #[cfg(target_os = "linux")]
    pub fn available() -> bool {
        let loaded = ["libayatana-appindicator3.so.1", "libappindicator3.so.1"]
            .into_iter()
            .any(|name| unsafe { libloading::Library::new(name) }.is_ok());
        if !loaded {
            tracing::warn!("tray icon disabled, appindicator failed to load");
        }
        loaded
    }

    #[cfg(not(target_os = "linux"))]
    pub fn available() -> bool {
        true
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

#[cfg(target_os = "windows")]
pub mod frame {
    use std::sync::atomic::{AtomicBool, Ordering};

    use freya::winit::platform::windows::{CornerPreference, WindowExtWindows};
    use freya::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use freya::winit::window::Window;
    use freya_winit::plugins::{FreyaPlugin, PluginEvent, PluginHandle};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows::Win32::Graphics::Dwm::DwmExtendFrameIntoClientArea;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
    };
    use windows::Win32::UI::Controls::MARGINS;
    use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::{
        DefWindowProcW, GWL_STYLE, GetWindowLongPtrW, HTBOTTOMRIGHT, HTLEFT, IsZoomed, MINMAXINFO,
        STYLESTRUCT, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
        SetWindowLongPtrW, SetWindowPos, WM_GETMINMAXINFO, WM_NCCALCSIZE, WM_NCDESTROY,
        WM_NCLBUTTONDOWN, WM_STYLECHANGING, WS_CAPTION, WS_THICKFRAME,
    };

    static RESIZING: AtomicBool = AtomicBool::new(false);

    pub struct FramePlugin;

    impl FreyaPlugin for FramePlugin {
        fn plugin_id(&self) -> &'static str {
            "oneclient-window-frame"
        }

        fn on_event(&mut self, event: &mut PluginEvent, _handle: PluginHandle) {
            let (window, driver, gpu) = match event {
                PluginEvent::WindowCreated {
                    window,
                    graphics_driver,
                    gpu_name,
                    ..
                }
                | PluginEvent::GraphicsDriverChanged {
                    window,
                    graphics_driver,
                    gpu_name,
                } => (*window, *graphics_driver, *gpu_name),
                _ => return,
            };

            if window.is_decorated() {
                return;
            }
            let Some(hwnd) = hwnd(window) else {
                return;
            };

            match gpu
                .filter(|gpu| driver == "OpenGL" && gpu.contains("Intel") && !gpu.contains("Arc"))
            {
                Some(gpu) => install(hwnd, gpu),
                None => decorate(window, hwnd),
            }
        }
    }

    fn hwnd(window: &Window) -> Option<HWND> {
        match window.window_handle().ok()?.as_raw() {
            RawWindowHandle::Win32(handle) => Some(HWND(handle.hwnd.get() as _)),
            _ => None,
        }
    }

    fn decorate(window: &Window, hwnd: HWND) {
        window.set_corner_preference(CornerPreference::Round);

        let margins = MARGINS {
            cxLeftWidth: 1,
            cxRightWidth: 1,
            cyTopHeight: 1,
            cyBottomHeight: 1,
        };
        if let Err(err) = unsafe { DwmExtendFrameIntoClientArea(hwnd, &margins) } {
            tracing::warn!("failed to extend the window frame for its shadow: {err}");
        }
    }

    fn install(hwnd: HWND, gpu: &str) {
        unsafe {
            if !SetWindowSubclass(hwnd, Some(subclass), 1, 0).as_bool() {
                tracing::warn!("failed to subclass the window for the {gpu} frame fix");
                return;
            }
            restyle(hwnd);
        }
        tracing::info!("dropped the window frame styles for {gpu} on OpenGL");
    }

    fn frame_styles(style: u32) -> u32 {
        if RESIZING.load(Ordering::Relaxed) {
            (style & !WS_CAPTION.0) | WS_THICKFRAME.0
        } else {
            style & !(WS_CAPTION.0 | WS_THICKFRAME.0)
        }
    }

    fn monitor_info(hwnd: HWND) -> Option<MONITORINFO> {
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        unsafe { GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut info) }
            .as_bool()
            .then_some(info)
    }

    unsafe fn restyle(hwnd: HWND) {
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
            SetWindowLongPtrW(hwnd, GWL_STYLE, frame_styles(style) as isize);

            let flags = SWP_FRAMECHANGED | SWP_NOZORDER | SWP_NOACTIVATE;
            let zoomed = IsZoomed(hwnd).as_bool();
            let result = match monitor_info(hwnd).filter(|_| zoomed) {
                Some(MONITORINFO { rcWork: work, .. }) => SetWindowPos(
                    hwnd,
                    None,
                    work.left,
                    work.top,
                    work.right - work.left,
                    work.bottom - work.top,
                    flags,
                ),
                None => SetWindowPos(hwnd, None, 0, 0, 0, 0, flags | SWP_NOMOVE | SWP_NOSIZE),
            };
            if let Err(err) = result {
                tracing::warn!("failed to apply the window frame styles: {err}");
            }
        }
    }

    unsafe extern "system" fn subclass(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        _data: usize,
    ) -> LRESULT {
        unsafe {
            match msg {
                WM_STYLECHANGING if wparam.0 as i32 == GWL_STYLE.0 => {
                    let change = &mut *(lparam.0 as *mut STYLESTRUCT);
                    change.styleNew = frame_styles(change.styleNew);
                    DefSubclassProc(hwnd, msg, wparam, lparam)
                }
                WM_NCCALCSIZE if !RESIZING.load(Ordering::Relaxed) => {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
                WM_NCLBUTTONDOWN if (HTLEFT..=HTBOTTOMRIGHT).contains(&(wparam.0 as u32)) => {
                    RESIZING.store(true, Ordering::Relaxed);
                    restyle(hwnd);
                    let result = DefSubclassProc(hwnd, msg, wparam, lparam);
                    RESIZING.store(false, Ordering::Relaxed);
                    restyle(hwnd);
                    result
                }
                WM_GETMINMAXINFO => {
                    let result = DefSubclassProc(hwnd, msg, wparam, lparam);
                    if let Some(info) = monitor_info(hwnd) {
                        let (work, monitor) = (info.rcWork, info.rcMonitor);
                        let limits = &mut *(lparam.0 as *mut MINMAXINFO);
                        limits.ptMaxPosition = POINT {
                            x: work.left - monitor.left,
                            y: work.top - monitor.top,
                        };
                        limits.ptMaxSize = POINT {
                            x: work.right - work.left,
                            y: work.bottom - work.top,
                        };
                    }
                    result
                }
                WM_NCDESTROY => {
                    let _ = RemoveWindowSubclass(hwnd, Some(subclass), 1);
                    DefSubclassProc(hwnd, msg, wparam, lparam)
                }
                _ => DefSubclassProc(hwnd, msg, wparam, lparam),
            }
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
