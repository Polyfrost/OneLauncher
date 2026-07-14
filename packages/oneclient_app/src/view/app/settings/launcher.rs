use freya::prelude::*;

use crate::components::{IconType, link_button, toggle};
use crate::hooks::{use_dispatch, use_launcher, use_settings_snapshot};
use super::settings_page;
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

        let mut first = use_state(|| true);
        {
            let settings = settings.clone();
            use_side_effect(move || {
                let discord = *discord_rpc.read();
                if *first.peek() {
                    first.set(false);
                    return;
                }
                let mut next = settings.clone();
                next.discord_enabled = discord;
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
