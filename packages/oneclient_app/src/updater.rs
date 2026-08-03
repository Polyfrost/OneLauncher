use std::cell::Cell;

use cargo_packager_updater::{Config, Update, check_update};
use oneclient_events::{Choice, EventBus, Persistence, Prompt};
use uuid::Uuid;

/// Choice id for the update prompt, so the overlay can recognise it among any
/// other pending prompt.
pub const UPDATE_CHOICE_INSTALL: &str = "update.install";

/// The only affirmative answer the update prompt offers; dismissing means "no".
enum UpdateAnswer {
    Install,
}

/// The update prompt, shared by the real check and the debug simulation so the
/// two cannot drift.
fn update_prompt(version: &str) -> Prompt<UpdateAnswer> {
    Prompt::new(
        "Update available",
        format!("OneClient {version} is ready to install. Download and install it now?"),
    )
    .option(
        Choice::primary(UPDATE_CHOICE_INSTALL, "Install"),
        UpdateAnswer::Install,
    )
    .dismiss("Not now")
}

pub const UPDATER_PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDFGODk3MkMyMjg0MjFDMDUKUldRRkhFSW93bktKSHpkWjNEMXNzaDVINVpCTU8xSnhuK2RnV0dTZ2FkcFJWbG1zUkhGYTNjaUkK";

pub const UPDATER_ENDPOINT: &str =
    "https://github.com/Polyfrost/OneLauncher/releases/latest/download/latest.json";

pub const RELEASES_URL: &str = "https://github.com/Polyfrost/OneLauncher/releases/latest";

const PROGRESS_STEP: u64 = 256 * 1024;

pub fn spawn_update_check(auto_install: bool) {
    tokio::spawn(async move {
        if let Err(err) = run_check(auto_install).await {
            tracing::warn!("update check failed: {err:#}");
        }
    });
}

/// Debug-only: drives the full auto-update UX (prompt, download progress, then the
/// "restart to apply" notification) against a fake release, without hitting the
/// network or touching disk. Mirrors [`run_check`] + [`download_and_install`] so the
/// debug page exercises the real notification path.
pub fn spawn_simulated_update() {
    tokio::spawn(async move {
        if let Err(err) = run_simulated_update().await {
            tracing::warn!("simulated update failed: {err:#}");
        }
    });
}

async fn run_simulated_update() -> anyhow::Result<()> {
    const FAKE_VERSION: &str = "9.9.9";
    const FAKE_TOTAL: u64 = 48 * 1024 * 1024; // arbitrary 48 MiB "download"

    let events = crate::launcher::state()?.services.events.clone();

    if events.ask(update_prompt(FAKE_VERSION)).await?.is_none() {
        tracing::info!("user declined simulated update");
        return Ok(());
    }

    let progress_id = Uuid::new_v4();
    let label = format!("Downloading OneClient {FAKE_VERSION}");

    let mut downloaded = 0u64;
    events.progress(progress_id, &label, downloaded, FAKE_TOTAL);
    while downloaded < FAKE_TOTAL {
        downloaded = (downloaded + PROGRESS_STEP * 8).min(FAKE_TOTAL);
        events.progress(progress_id, &label, downloaded, FAKE_TOTAL);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    events.finish_progress(
        progress_id,
        "Finished Downloading",
        format!("OneClient {FAKE_VERSION} is ready. Restart to apply."),
        Persistence::Persistent,
    );

    Ok(())
}

async fn run_check(auto_install: bool) -> anyhow::Result<()> {
    let events = crate::launcher::state()?.services.events.clone();

    // `check_update` performs a blocking HTTP request, so offload it to a thread pool.
    let Some(update) = tokio::task::spawn_blocking(check_for_update).await?? else {
        tracing::info!("no update available");
        return Ok(());
    };

    tracing::info!("update available: {}", update.version);

    // The bundled self-updater can only replace an AppImage in place: on Linux
    // cargo-packager-updater rejects every format except AppImage and locates the
    // target via the `APPIMAGE` env var. deb/rpm installs run from a package-managed
    // path (e.g. /usr/bin) we can't overwrite without root. Attempting an install
    // would fail with a permission error, or clobber the system binary if elevated.
    // Detect that case and point the user at the release page instead.
    if !can_self_update() {
        tracing::info!("install is not self-updatable (non-AppImage Linux); notifying only");
        events.notify("Update available")
            .body(format!(
                "OneClient {} is available. Download the latest package from {} to update.",
                update.version, RELEASES_URL
            ))
            // Asks the user to go and download a package by hand; useless if it
            // scrolls past while they are elsewhere.
            .persistent()
            .send();
        return Ok(());
    }

    if !auto_install
        && events.ask(update_prompt(&update.version)).await?.is_none() {
            tracing::info!("user declined update {}", update.version);
            return Ok(());
        }

    download_and_install(update, events).await
}

/// Whether the running install can be updated in place by the bundled updater.
///
/// Windows (NSIS) and macOS (.app) installs are always self-updatable. On Linux
/// only AppImage installs are: cargo-packager-updater replaces the file named by
/// the `APPIMAGE` env var and rejects every other format, so deb/rpm installs are
/// not: they run from a package-manager-owned path and never set `APPIMAGE`.
fn can_self_update() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::env::var_os("APPIMAGE").is_some()
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
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

async fn download_and_install(update: Update, events: EventBus) -> anyhow::Result<()> {
    let progress_id = Uuid::new_v4();
    let version = update.version.clone();
    let label = format!("Downloading OneClient {version}");

    events.progress(progress_id, &label, 0, 0);

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
                    events.progress(progress_id, &label, now, total);
                }
            },
            || {},
        )?;

        let total = downloaded.get().max(1);
        events.progress(progress_id, &label, total, total);

        update.install(bytes)?;

        // Convert the same download card into its finished state instead of emitting a
        // second notification, so the user sees one notification through to completion.
        events.finish_progress(
            progress_id,
            "Finished Downloading",
            format!("OneClient {version} is ready. Restart to apply."),
            // The update is only live after a restart the user has to perform,
            // so this has to outlive the five seconds it is on screen for.
            Persistence::Persistent,
        );
        Ok(())
    })
    .await??;

    tracing::info!("update installed; restart to apply");
    Ok(())
}
