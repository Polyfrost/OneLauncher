use freya::prelude::*;
use oneclient_content::modpacks::{
    ModpackRelease, ModpackUpdateStatus, check_modpack_update, cluster_modpack,
};

use crate::components::{Button, IconType};
use crate::hooks::{use_dispatch, use_installs_snapshot};
use crate::launcher::{self, off_ui};
use crate::view::app::settings::settings_row;

#[derive(Clone, PartialEq)]
enum UpdateCheck {
    Idle,
    Checking,
    UpToDate,
    Available { version_id: String, label: String },
    Unpublished,
    Failed(String),
}

fn load_release(cluster_id: i64, mut current: State<Option<ModpackRelease>>) {
    spawn(async move {
        let Ok(state) = launcher::state() else { return };
        let content = state.services.content();
        let found = off_ui(async move { cluster_modpack(cluster_id, &content).await }).await;
        match found {
            Ok(release) => current.set(release),
            Err(err) => tracing::debug!(cluster_id, error = %err, "could not identify the modpack"),
        }
    });
}

fn run_check(cluster_id: i64, mut check: State<UpdateCheck>) {
    check.set(UpdateCheck::Checking);
    spawn(async move {
        let Ok(state) = launcher::state() else {
            check.set(UpdateCheck::Idle);
            return;
        };
        let content = state.services.content();
        let found = off_ui(async move { check_modpack_update(cluster_id, &content).await }).await;

        check.set(match found {
            Ok(ModpackUpdateStatus::Unpublished) => UpdateCheck::Unpublished,
            Ok(ModpackUpdateStatus::UpToDate) => UpdateCheck::UpToDate,
            Ok(ModpackUpdateStatus::Available(latest)) => UpdateCheck::Available {
                version_id: latest.version_id,
                label: latest.name,
            },
            Err(err) => UpdateCheck::Failed(err.to_string()),
        });
    });
}

fn update_description(current: Option<&ModpackRelease>, check: &UpdateCheck) -> String {
    let installed = current.map(|release| format!("Installed: {}. ", release.name));
    let status = match check {
        UpdateCheck::Idle => "Check Modrinth or CurseForge for a newer version of this pack.".to_string(),
        UpdateCheck::Checking => "Checking for a newer version...".to_string(),
        UpdateCheck::UpToDate => "This is the newest version for this Minecraft version.".to_string(),
        UpdateCheck::Available { label, .. } => {
            format!("{label} is available. Mods you removed stay removed and configs you edited are kept.")
        }
        UpdateCheck::Unpublished => {
            "This pack is not published on Modrinth or CurseForge, so it cannot be updated from here."
                .to_string()
        }
        UpdateCheck::Failed(err) => format!("Could not check for updates: {err}"),
    };
    format!("{}{status}", installed.unwrap_or_default())
}

#[derive(PartialEq)]
pub struct ModpackUpdateRow {
    pub cluster_id: i64,
}

impl Component for ModpackUpdateRow {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let dispatch = use_dispatch();
        let check = use_state(|| UpdateCheck::Idle);
        let current = use_state(|| None::<ModpackRelease>);
        let busy = use_installs_snapshot().is_modpack_job(cluster_id);

        use_hook(move || load_release(cluster_id, current));

        let mut was_busy = use_state(|| busy);
        if *was_busy.peek() != busy {
            was_busy.set(busy);
            if !busy {
                let mut check = check;
                check.set(UpdateCheck::Idle);
                load_release(cluster_id, current);
            }
        }

        let state = check.read().clone();
        let description = update_description(current.read().as_ref(), &state);

        let button = match (&state, busy) {
            (_, true) => Button::new()
                .small()
                .secondary()
                .enabled(false)
                .text("Working..."),
            (UpdateCheck::Available { version_id, .. }, false) => {
                let version_id = version_id.clone();
                Button::new()
                    .small()
                    .primary()
                    .on_press(move |_| dispatch.update_modpack(cluster_id, version_id.clone()))
                    .text("Update")
            }
            (UpdateCheck::Checking, false) => Button::new()
                .small()
                .secondary()
                .enabled(false)
                .text("Checking..."),
            (UpdateCheck::Unpublished, false) => Button::new()
                .small()
                .secondary()
                .enabled(false)
                .text("Check for updates"),
            (_, false) => Button::new()
                .small()
                .secondary()
                .on_press(move |_| run_check(cluster_id, check))
                .text("Check for updates"),
        };

        settings_row(
            IconType::DownloadCloud02,
            "Modpack Updates",
            description,
            button,
        )
    }
}

#[derive(PartialEq)]
pub struct ModpackRepairRow {
    pub cluster_id: i64,
}

impl Component for ModpackRepairRow {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let dispatch = use_dispatch();
        let busy = use_installs_snapshot().is_modpack_job(cluster_id);

        let button = Button::new()
            .small()
            .secondary()
            .enabled(!busy)
            .maybe(!busy, |el| {
                el.on_press(move |_| dispatch.repair_modpack(cluster_id))
            })
            .text(if busy { "Working..." } else { "Reinstall" });

        settings_row(
            IconType::RefreshCw01,
            "Reinstall Missing Files",
            "Download anything from the pack that is missing and reopen the manual downloads for \
             CurseForge files. Files you removed stay removed.",
            button,
        )
    }
}
