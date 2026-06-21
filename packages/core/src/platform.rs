//! Platform-specific startup workarounds.
//!
//! These run before the async runtime is started and before any GUI/webview
//! initialisation, so they must stay dependency-free and side-effect-light.

/// Apply platform workarounds that must take effect before the webview starts.
///
/// On Linux this disables webkit2gtk's DMABUF renderer on Wayland sessions that
/// use the NVIDIA proprietary driver. There, the DMABUF path hands the
/// compositor buffers it rejects and GTK aborts with "Error 71 (Protocol
/// error)" before the window can open, so the app never starts. Disabling the
/// DMABUF renderer avoids the crash at a small webview-compositing cost, so it
/// is only applied for the affected configuration and never clobbers a value
/// the user set themselves.
///
/// See <https://github.com/Polyfrost/OneLauncher/issues/496>.
#[cfg(target_os = "linux")]
pub fn apply_startup_workarounds() {
	// Respect an explicit user override either way.
	if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_some() {
		return;
	}

	let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some()
		|| std::env::var_os("XDG_SESSION_TYPE").is_some_and(|t| t.eq_ignore_ascii_case("wayland"));

	let nvidia = std::path::Path::new("/dev/nvidia0").exists()
		|| std::path::Path::new("/proc/driver/nvidia/version").exists();

	if wayland && nvidia {
		// SAFETY: called as the first statement of `main`, before the async
		// runtime is built and before any other thread is spawned, so nothing
		// else can be touching the process environment concurrently.
		unsafe {
			std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
		}
	}
}

/// No-op on non-Linux targets.
#[cfg(not(target_os = "linux"))]
pub fn apply_startup_workarounds() {}
