pub fn no_window(command: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    #[cfg(not(windows))]
    let _ = command;
}

#[cfg(target_os = "linux")]
const APPIMAGE_ENV: [&str; 10] = [
    "LD_LIBRARY_PATH",
    "PATH",
    "XDG_DATA_DIRS",
    "GSETTINGS_SCHEMA_DIR",
    "PYTHONHOME",
    "PYTHONPATH",
    "PERLLIB",
    "QT_PLUGIN_PATH",
    "GST_PLUGIN_SYSTEM_PATH",
    "GST_PLUGIN_SYSTEM_PATH_1_0",
];

pub fn strip_appimage_env(command: &mut std::process::Command) {
    #[cfg(target_os = "linux")]
    {
        let Some(appdir) = std::env::var_os("APPDIR") else {
            return;
        };
        let appdir = appdir.to_string_lossy();

        for key in APPIMAGE_ENV {
            let Ok(value) = std::env::var(key) else {
                continue;
            };
            let kept = value
                .split(':')
                .filter(|entry| !entry.is_empty() && !entry.starts_with(&*appdir))
                .collect::<Vec<_>>();
            if kept.is_empty() {
                command.env_remove(key);
            } else {
                command.env(key, kept.join(":"));
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    let _ = command;
}
