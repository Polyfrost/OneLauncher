use freya::prelude::*;
use oneclient_auth::MinecraftAccount;
use oneclient_core::settings::skin::MinecraftSkin;

use crate::hooks::{use_dispatch, use_settings_snapshot};
use crate::view::app::settings::{section_header, settings_page, settings_row};
use crate::{try_accounts, use_current_account};

#[derive(PartialEq)]
pub struct SettingsSkinChanger;

impl Component for SettingsSkinChanger {
    fn render(&self) -> impl IntoElement {
        let profile = use_settings_snapshot().settings.global_game_settings;

        let dispatch = use_dispatch();

        settings_page()
            .child(skin_locker(account))
            .child(section_header("YOUR SKINS"))
            .into_element()
    }
}

fn skin(skin: MinecraftSkin) -> impl IntoElement {
    rect().horizontal().spacing(8.)
}

fn skin_locker(account: MinecraftAccount) -> impl IntoElement {
    rect().horizontal().spacing(8.).into_element();
}
