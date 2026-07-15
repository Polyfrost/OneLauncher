use freya::prelude::*;

use super::settings_page;
use crate::components::{IconType, link_button, toggle};
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
