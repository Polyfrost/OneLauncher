use freya::prelude::*;
use oneclient_core::settings::{LaunchBehaviour, LauncherSettings};

use super::settings_page;
use crate::components::{Dropdown, IconType, link_button, toggle};
use crate::hooks::{use_dispatch, use_launcher, use_settings_snapshot};
use crate::platform;
use crate::view::app::settings::{section_header, settings_row};

#[derive(PartialEq)]
pub struct SettingsLauncher;

impl Component for SettingsLauncher {
    fn render(&self) -> impl IntoElement {
        let settings = use_settings_snapshot().settings;
        let data_dir = use_launcher().data_dir;
        let dispatch = use_dispatch();

        let discord_rpc = use_state({
            let v = settings.discord_enabled;
            move || v
        });

        let crash_reporting = use_state({
            let v = settings.crash_reporting;
            move || v
        });

        let mut first = use_state(|| true);
        {
            let settings = settings.clone();
            let dispatch = dispatch.clone();
            use_side_effect(move || {
                let discord = *discord_rpc.read();
                let crash = *crash_reporting.read();
                if *first.peek() {
                    first.set(false);
                    return;
                }
                let mut next = settings.clone();
                next.discord_enabled = discord;
                next.crash_reporting = crash;
                dispatch.set_settings(next);
            });
        }

        let folder = data_dir.clone();
        let open_folder = link_button().on_press(move |_| platform::open_url(&folder));

        settings_page()
            .child(section_header("GENERAL"))
            .child(settings_row(
                IconType::Eye,
                "Launcher Window",
                "What the window does while a game is running. The tray icon stays either way.",
                launch_behaviour_field(settings, dispatch),
            ))
            .child(settings_row(
                IconType::Link03,
                "Discord RPC",
                "Enable Discord Rich Presence.",
                toggle(discord_rpc),
            ))
            .child(settings_row(
                IconType::AlertTriangle,
                "Crash Reporting",
                "Send anonymous crash and error reports to help fix bugs. Applies on restart.",
                toggle(crash_reporting),
            ))
            .child(section_header("FOLDERS AND FILES"))
            .child(settings_row(
                IconType::Folder,
                "Launcher Folder",
                data_dir,
                open_folder,
            ))
            .into_element()
    }
}

/// Dispatched on its own rather than through the mirroring effect above: that
/// one debounces two toggles against each other, and a dropdown has no
/// intermediate states to debounce.
fn launch_behaviour_field(
    settings: LauncherSettings,
    dispatch: crate::Actions,
) -> impl IntoElement {
    let options: Vec<String> = LaunchBehaviour::ALL
        .iter()
        .map(|behaviour| behaviour.label().to_string())
        .collect();

    Dropdown::new(settings.launch_behaviour.label(), options)
        .width(Size::px(200.))
        .height(Size::px(34.))
        .on_select(move |idx: usize| {
            let Some(behaviour) = LaunchBehaviour::ALL.get(idx).copied() else {
                return;
            };
            let mut next = settings.clone();
            next.launch_behaviour = behaviour;
            dispatch.set_settings(next);
        })
}
