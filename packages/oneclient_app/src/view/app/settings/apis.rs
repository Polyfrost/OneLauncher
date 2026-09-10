use freya::prelude::*;

use super::settings_page;
use crate::components::{IconType, TextInput};
use crate::hooks::{ResetNotice, use_dispatch, use_game_active, use_settings_snapshot};
use crate::view::app::settings::{reset_row, section_header, settings_row, take_notice};

fn normalize(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

const RESET_NOTICE: ResetNotice = ResetNotice {
    title: "APIs reset",
    body: "Your keys are cleared and both endpoints are back to the ones OneClient ships with.",
};

#[derive(PartialEq)]
pub struct SettingsApis;

impl Component for SettingsApis {
    fn render(&self) -> impl IntoElement {
        let settings = use_settings_snapshot().settings;
        let dispatch = use_dispatch();

        let mut modrinth_key = use_state({
            let v = settings.modrinth_api_key.clone().unwrap_or_default();
            move || v
        });
        let mut curseforge_key = use_state({
            let v = settings.curseforge_api_key.clone().unwrap_or_default();
            move || v
        });
        let mut custom_api_endpoint = use_state({
            let v = settings.custom_api_endpoint.clone().unwrap_or_default();
            move || v
        });
        let mut custom_meta_url_base = use_state({
            let v = settings.custom_meta_url_base.clone().unwrap_or_default();
            move || v
        });

        let mut first = use_state(|| true);
        let mut announce = use_state(|| false);
        {
            let dispatch = dispatch.clone();
            use_side_effect(move || {
                let modrinth = modrinth_key.read().clone();
                let curseforge = curseforge_key.read().clone();
                let endpoint = custom_api_endpoint.read().clone();
                let meta_url_base = custom_meta_url_base.read().clone();
                if *first.peek() {
                    first.set(false);
                    return;
                }
                dispatch.edit_settings_notifying(take_notice(announce, RESET_NOTICE), |settings| {
                    settings.modrinth_api_key = normalize(&modrinth);
                    settings.curseforge_api_key = normalize(&curseforge);
                    settings.custom_api_endpoint = normalize(&endpoint);
                    settings.custom_meta_url_base = normalize(&meta_url_base);
                });
            });
        }

        let game_active = use_game_active();
        let confirming_reset = use_state(|| false);
        let reset = move |()| {
            announce.set(true);
            modrinth_key.set(String::new());
            curseforge_key.set(String::new());
            custom_api_endpoint.set(String::new());
            custom_meta_url_base.set(String::new());
        };

        settings_page()
            .child(section_header("PROVIDERS"))
            .child(settings_row(
                IconType::Key01,
                "Modrinth",
                "Personal access token used for Modrinth requests.",
                TextInput::new(modrinth_key)
                    .placeholder("Optional")
                    .width(Size::px(220.)),
            ))
            .child(settings_row(
                IconType::Key01,
                "CurseForge",
                "API key used for CurseForge requests.",
                TextInput::new(curseforge_key)
                    .placeholder("Default")
                    .width(Size::px(220.)),
            ))
            .child(section_header("ADVANCED"))
            .child(settings_row(
                IconType::Globe01,
                "Custom API Endpoint",
                "Override the default OneClient backend endpoint.",
                TextInput::new(custom_api_endpoint)
                    .placeholder("Default")
                    .width(Size::px(220.)),
            ))
            .child(settings_row(
                IconType::Globe01,
                "Custom Meta URL Base",
                "Override the default bundles and versions data host.",
                TextInput::new(custom_meta_url_base)
                    .placeholder("Default")
                    .width(Size::px(220.)),
            ))
            .child(reset_row(
                confirming_reset,
                game_active,
                "Clear the keys on this page and go back to the endpoints OneClient ships with.",
                "Your Modrinth and CurseForge keys are cleared, and the custom API endpoint \
                 and meta URL base go back to the ones OneClient ships with. Requests start \
                 using the built-in defaults straight away.",
                reset.into(),
            ))
            .into_element()
    }
}
