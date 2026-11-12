use cargo_packager_updater::{Config, Update, WindowsConfig, check_update};

pub const UPDATER_PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDFGODk3MkMyMjg0MjFDMDUKUldRRkhFSW93bktKSHpkWjNEMXNzaDVINVpCTU8xSnhuK2RnV0dTZ2FkcFJWbG1zUkhGYTNjaUkK";
pub const UPDATER_ENDPOINT: &str =
    "https://github.com/Polyfrost/OneLauncher/releases/latest/download/latest.json";
pub const RELEASES_URL: &str = "https://github.com/Polyfrost/OneLauncher/releases/latest";

pub fn can_self_update() -> bool {
    if cfg!(debug_assertions) {
        return false;
    }

    if std::env::var_os("ONECLIENT_DISABLE_AUTOUPDATE")
        .is_some_and(|val| val.eq_ignore_ascii_case("1"))
    {
        return false;
    }

    #[cfg(target_os = "linux")]
    {
        std::env::var_os("APPIMAGE").is_some()
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

pub fn check_for_update() -> anyhow::Result<Option<Update>> {
    let current = env!("CARGO_PKG_VERSION").parse()?;
    let config = Config {
        endpoints: vec![UPDATER_ENDPOINT.parse()?],
        pubkey: UPDATER_PUBKEY.into(),
        // The NSIS installer recreates the desktop and start menu shortcuts on every passive
        // install unless told not to, which brings back a shortcut the user deleted
        windows: Some(WindowsConfig {
            installer_args: Some(vec!["/NS".into()]),
            ..Default::default()
        }),
        ..Default::default()
    };

    Ok(check_update(current, config)?)
}
