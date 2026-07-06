//! In-app self-update via [`cargo-packager-updater`].

use cargo_packager_updater::{Config, check_update};

pub const UPDATER_PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDFGODk3MkMyMjg0MjFDMDUKUldRRkhFSW93bktKSHpkWjNEMXNzaDVINVpCTU8xSnhuK2RnV0dTZ2FkcFJWbG1zUkhGYTNjaUkK";

pub const UPDATER_ENDPOINT: &str =
    "https://github.com/Polyfrost/OneLauncher/releases/latest/download/latest.json";

pub fn spawn_update_check(auto_install: bool) {
    std::thread::Builder::new()
        .name("oneclient-updater".into())
        .spawn(move || {
            if let Err(err) = run_check(auto_install) {
                tracing::warn!("update check failed: {err}");
            }
        })
        .ok();
}

fn run_check(auto_install: bool) -> anyhow::Result<()> {
    let current = env!("CARGO_PKG_VERSION").parse()?;
    let config = Config {
        endpoints: vec![UPDATER_ENDPOINT.parse()?],
        pubkey: UPDATER_PUBKEY.into(),
        ..Default::default()
    };

    match check_update(current, config)? {
        Some(update) => {
            tracing::info!("update available: {}", update.version);
            if auto_install {
                tracing::info!("downloading and installing update…");
                update.download_and_install()?;
                tracing::info!("update installed; restart to apply");
            }
        }
        None => tracing::info!("no update available"),
    }

    Ok(())
}
