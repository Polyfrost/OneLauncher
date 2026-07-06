
#[cfg(not(target_os = "windows"))]
pub fn open_url(url: &str) {
    let result = {
        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("xdg-open").arg(url).spawn()
        }
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open").arg(url).spawn()
        }
    };

    if let Err(err) = result {
        tracing::warn!("failed to open url {url}: {err}");
    }
}

#[cfg(target_os = "windows")]
pub fn open_url(url: &str) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;

    // SW_SHOWNORMAL
    const SW_SHOWNORMAL: i32 = 1;

    fn wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    let operation = wide("open");
    let file = wide(url);

    let ret = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };

    if (ret as isize) <= 32 {
        tracing::warn!("failed to open url {url}: ShellExecuteW returned {}", ret as isize);
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
                fn malloc_zone_pressure_relief(
                    zone: *mut core::ffi::c_void,
                    goal: usize,
                ) -> usize;
            }
            malloc_zone_pressure_relief(core::ptr::null_mut(), 0);
        }
    }
}

