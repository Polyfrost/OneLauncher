use std::sync::Arc;

use clap::Subcommand;
use oneclient_common::paths;
use oneclient_core::LauncherState;
use oneclient_core::settings::LauncherSettings;
use serde::{Deserialize, Serialize};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

use super::Host;
use crate::Output;
use crate::ipc::{self, IpcCommand};
use crate::output::{accent, dim, good, heading};

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum TermsCommand {
    /// Read the terms of service and privacy policy
    Show,
    /// Accept the current terms
    Accept,
    /// Decline the terms, which turns off Polyfrost's online services
    Decline,
}

pub(crate) async fn save_settings(
    state: &LauncherState,
    change: impl FnOnce(&mut LauncherSettings),
) -> anyhow::Result<LauncherSettings> {
    let updated = {
        let mut settings = state.settings.write();
        change(&mut settings);
        settings.clone()
    };
    oneclient_core::settings::store::save_settings_and_apply(&state.services, &updated).await?;
    Ok(updated)
}

pub(super) async fn terms(
    state: &Arc<LauncherState>,
    command: TermsCommand,
    out: &Output,
) -> anyhow::Result<()> {
    match command {
        TermsCommand::Show => {
            let document = oneclient_core::fetch_terms(&state.services.requester).await?;
            let mut text = crate::markdown::render(&document.terms);
            if let Some(privacy) = &document.privacy {
                text.push_str(&format!(
                    "\n{}\n\n{}",
                    heading("Privacy policy"),
                    crate::markdown::render(privacy)
                ));
            }
            text.push_str(&format!("\n{}", dim(document.terms_url())));
            out.page(text);
            Ok(())
        }
        TermsCommand::Accept => {
            let document = oneclient_core::fetch_terms(&state.services.requester).await?;
            let was_declined = state.settings.read().declined_tos;
            save_settings(state, |settings| {
                settings.accepted_tos_version = document.version;
                settings.accepted_privacy_version = document.privacy_version.unwrap_or(0);
                settings.declined_tos = false;
            })
            .await?;
            out.line(format!("{} Accepted the terms", good("✓")));
            if was_declined {
                out.line(dim(
                    "OneClient reconnects to Polyfrost services the next time it starts",
                ));
            }
            Ok(())
        }
        TermsCommand::Decline => {
            oneclient_common::consent::decline();
            save_settings(state, |settings| {
                settings.declined_tos = true;
                settings.accepted_tos_version = 0;
                settings.accepted_privacy_version = 0;
            })
            .await?;
            out.line("Declined the terms, Polyfrost's online services are off");
            Ok(())
        }
    }
}

pub(super) async fn changelog(
    state: &Arc<LauncherState>,
    all: bool,
    out: &Output,
) -> anyhow::Result<()> {
    let entries = oneclient_core::fetch_changelog(&state.services.requester).await?;
    let Some(latest) = entries.first() else {
        out.line("No changelog yet");
        return Ok(());
    };
    let latest = latest.version.clone();

    let text = entries
        .iter()
        .take(if all { entries.len() } else { 1 })
        .map(|entry| {
            format!(
                "{}\n\n{}",
                heading(&entry.version),
                crate::markdown::render(&entry.body)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    out.page(text);

    save_settings(state, |settings| {
        settings.seen_changelog_version = Some(latest);
    })
    .await?;
    Ok(())
}

pub(super) fn info(out: &Output) -> anyhow::Result<()> {
    let system = System::new_with_specifics(
        RefreshKind::nothing()
            .with_memory(MemoryRefreshKind::nothing().with_ram())
            .with_cpu(CpuRefreshKind::nothing()),
    );
    let cpu = system
        .cpus()
        .first()
        .map(|cpu| cpu.brand().to_string())
        .unwrap_or_default();

    let lines = [
        format!(
            "OneClient v{} ({})",
            env!("CARGO_PKG_VERSION"),
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
        ),
        System::kernel_long_version(),
        cpu,
        format!("RAM Total of {} MB", system.total_memory() / 1024 / 1024),
        format!("Data folder {}", paths::data_dir()?.display()),
        format!("Config folder {}", paths::config_dir()?.display()),
    ];
    out.line(lines.join("\n"));
    Ok(())
}

pub(super) async fn update(host: Host, out: &Output) -> anyhow::Result<()> {
    if host == Host::Gui {
        ipc::send(IpcCommand::CheckUpdate);
        out.line("OneClient is checking for updates and will offer to install one");
        return Ok(());
    }

    let found = tokio::task::spawn_blocking(crate::update::check_for_update).await??;
    match found {
        Some(update) => {
            out.line(format!(
                "{} OneClient {} is out, you have {}",
                good("Update:"),
                update.version,
                env!("CARGO_PKG_VERSION")
            ));
            out.line(format!(
                "Open OneClient to install it, or download it from {}",
                accent(crate::update::RELEASES_URL)
            ));
        }
        None => out.line(format!(
            "OneClient {} is up to date",
            env!("CARGO_PKG_VERSION")
        )),
    }
    Ok(())
}
