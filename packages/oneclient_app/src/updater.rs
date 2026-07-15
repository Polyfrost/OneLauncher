use std::cell::Cell;

use cargo_packager_updater::{Config, Update, check_update};
use oneclient_core::LauncherState;
use oneclient_core::notification::{NotificationService, UserChoice};
use uuid::Uuid;

pub const UPDATER_PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDFGODk3MkMyMjg0MjFDMDUKUldRRkhFSW93bktKSHpkWjNEMXNzaDVINVpCTU8xSnhuK2RnV0dTZ2FkcFJWbG1zUkhGYTNjaUkK";

pub const UPDATER_ENDPOINT: &str =
    "https://github.com/Polyfrost/OneLauncher/releases/latest/download/latest.json";

const PROGRESS_STEP: u64 = 256 * 1024;

pub fn spawn_update_check(auto_install: bool) {
    tokio::spawn(async move {
        if let Err(err) = run_check(auto_install).await {
            tracing::warn!("update check failed: {err:#}");
        }
    });
}

async fn run_check(auto_install: bool) -> anyhow::Result<()> {
    let notifier = LauncherState::get()?.services.notifier.clone();

    // `check_update` performs a blocking HTTP request, so offload it to a thread pool.
    let Some(update) = tokio::task::spawn_blocking(check_for_update).await?? else {
        tracing::info!("no update available");
        return Ok(());
    };

    tracing::info!("update available: {}", update.version);

    if !auto_install {
        match notifier.prompt_update(&update.version).await? {
            UserChoice::Accept => {}
            _ => {
                tracing::info!("user declined update {}", update.version);
                return Ok(());
            }
        }
    }

    download_and_install(update, notifier).await
}

fn check_for_update() -> anyhow::Result<Option<Update>> {
    let current = env!("CARGO_PKG_VERSION").parse()?;
    let config = Config {
        endpoints: vec![UPDATER_ENDPOINT.parse()?],
        pubkey: UPDATER_PUBKEY.into(),
        ..Default::default()
    };

    Ok(check_update(current, config)?)
}

async fn download_and_install(update: Update, notifier: NotificationService) -> anyhow::Result<()> {
    let progress_id = Uuid::new_v4();
    let version = update.version.clone();
    let label = format!("Downloading OneClient {version}");

    notifier.send_progress(&progress_id, &label, 0, 0);

    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let downloaded = Cell::new(0u64);
        let last_sent = Cell::new(0u64);

        let bytes = update.download_extended(
            |chunk, total| {
                let now = downloaded.get() + chunk as u64;
                downloaded.set(now);
                let total = total.unwrap_or(0);

                if now == chunk as u64
                    || (total > 0 && now >= total)
                    || now - last_sent.get() >= PROGRESS_STEP
                {
                    last_sent.set(now);
                    notifier.send_progress(&progress_id, &label, now, total);
                }
            },
            || {},
        )?;

        let total = downloaded.get().max(1);
        notifier.send_progress(&progress_id, &label, total, total);

        update.install(bytes)?;

        notifier.send_info(
            "Update ready",
            &format!("OneClient {version} installed. Restart to apply."),
        );
        Ok(())
    })
    .await??;

    tracing::info!("update installed; restart to apply");
    Ok(())
}
