const ALLOWED_URL_SCHEMES: [&str; 3] = ["http", "https", "mailto"];

pub fn focus_window() {
    use freya::prelude::{Platform, WinitPlatformExt};

    Platform::get().with_window(None, |win| {
        win.set_minimized(false);
        win.focus_window();
    });
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
}
