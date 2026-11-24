//! OS dark-mode preference.
//!
//! Freya derives `preferred_theme` from `winit::Window::theme()`, which does
//! not report the desktop color-scheme on Linux (X11 always returns `None`
//! and Wayland only exposes the app's own decoration theme). On Linux we read
//! the XDG Settings portal instead, via mundy's `color-scheme` preference.

#[cfg(target_os = "linux")]
use std::time::Duration;

use freya::prelude::PreferredTheme;
use tokio::sync::watch;

#[cfg(target_os = "linux")]
use mundy::{Interest, Preferences};
#[cfg(target_os = "linux")]
use std::sync::OnceLock;
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "linux")]
static DARK: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "linux")]
static CHANGED: OnceLock<watch::Sender<()>> = OnceLock::new();
#[cfg(target_os = "linux")]
static SUBSCRIPTION: OnceLock<mundy::Subscription> = OnceLock::new();

/// Starts watching the OS color-scheme. Call once on the main thread at boot.
pub fn start() {
    #[cfg(target_os = "linux")]
    {
        if SUBSCRIPTION.get().is_some() {
            return;
        }

        let (tx, _rx) = watch::channel(());
        let _ = CHANGED.set(tx);

        // The callback below also delivers the initial value, but doing a
        // blocking read here makes the very first frame use the right palette.
        if let Some(prefs) =
            Preferences::once_blocking(Interest::ColorScheme, Duration::from_millis(200))
        {
            DARK.store(prefs.color_scheme.is_dark(), Ordering::Relaxed);
        }

        let subscription = Preferences::subscribe(Interest::ColorScheme, |prefs| {
            let dark = prefs.color_scheme.is_dark();
            DARK.store(dark, Ordering::Relaxed);
            if let Some(tx) = CHANGED.get() {
                let _ = tx.send(());
            }
        });
        let _ = SUBSCRIPTION.set(subscription);
    }
}

/// Receiver that fires whenever the OS color-scheme changes.
pub fn subscribe() -> Option<watch::Receiver<()>> {
    #[cfg(target_os = "linux")]
    {
        CHANGED.get().map(watch::Sender::subscribe)
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

fn resolve(preferred: PreferredTheme, detected: bool) -> bool {
    if cfg!(target_os = "linux") {
        detected
    } else {
        preferred == PreferredTheme::Dark
    }
}

/// Whether a "System" theme should resolve to its dark variant right now.
pub fn system_dark(preferred: PreferredTheme) -> bool {
    #[cfg(target_os = "linux")]
    {
        resolve(preferred, DARK.load(Ordering::Relaxed))
    }
    #[cfg(not(target_os = "linux"))]
    {
        resolve(preferred, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_uses_the_detected_color_scheme() {
        if cfg!(target_os = "linux") {
            assert!(resolve(PreferredTheme::Light, true));
            assert!(!resolve(PreferredTheme::Dark, false));
        }
    }

    #[test]
    fn other_platforms_keep_freya_preferred_theme() {
        if !cfg!(target_os = "linux") {
            assert!(resolve(PreferredTheme::Dark, false));
            assert!(!resolve(PreferredTheme::Light, true));
        }
    }
}
