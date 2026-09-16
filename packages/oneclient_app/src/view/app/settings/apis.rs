use freya::prelude::*;

use super::settings_page;
use crate::components::{IconType, TextInput};
use crate::hooks::{use_dispatch, use_settings_snapshot};
use crate::view::app::settings::{resettable, section_header, settings_row};

fn normalize(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[derive(PartialEq)]
pub struct SettingsApis;

impl Component for SettingsApis {
    fn render(&self) -> impl IntoElement {
        let settings = use_settings_snapshot().settings;
        let dispatch = use_dispatch();

        let modrinth_key = use_state({
            let v = settings.modrinth_api_key.clone().unwrap_or_default();
            move || v
        });
        let curseforge_key = use_state({
            let v = settings.curseforge_api_key.clone().unwrap_or_default();
            move || v
        });
        let custom_api_endpoint = use_state({
            let v = settings.custom_api_endpoint.clone().unwrap_or_default();
            move || v
        });
        let custom_meta_url_base = use_state({
            let v = settings.custom_meta_url_base.clone().unwrap_or_default();
            move || v
        });

        let mut first = use_state(|| true);
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
                dispatch.edit_settings(|settings| {
                    settings.modrinth_api_key = normalize(&modrinth);
                    settings.curseforge_api_key = normalize(&curseforge);
                    settings.custom_api_endpoint = normalize(&endpoint);
                    settings.custom_meta_url_base = normalize(&meta_url_base);
                });
            });
        }

        settings_page()
            .child(section_header("PROVIDERS"))
            .child(settings_row(
                IconType::Key01,
                "Modrinth",
                "Personal access token used for Modrinth requests.",
                resettable(
                    TextInput::new(modrinth_key)
                        .placeholder("Optional")
                        .width(Size::px(220.)),
                    modrinth_key,
                    String::new(),
                ),
            ))
            .child(settings_row(
                IconType::Key01,
                "CurseForge",
                "API key used for CurseForge requests.",
                resettable(
                    TextInput::new(curseforge_key)
                        .placeholder("Default")
                        .width(Size::px(220.)),
                    curseforge_key,
                    String::new(),
                ),
            ))
            .child(section_header("ADVANCED"))
            .child(settings_row(
                IconType::Globe01,
                "Custom API Endpoint",
                "Override the default OneClient backend endpoint.",
                resettable(
                    TextInput::new(custom_api_endpoint)
                        .placeholder("Default")
                        .width(Size::px(220.)),
                    custom_api_endpoint,
                    String::new(),
                ),
            ))
            .child(settings_row(
                IconType::Globe01,
                "Custom Meta URL Base",
                "Override the default bundles and versions data host.",
                resettable(
                    TextInput::new(custom_meta_url_base)
                        .placeholder("Default")
                        .width(Size::px(220.)),
                    custom_meta_url_base,
                    String::new(),
                ),
            ))
            .into_element()
    }
}
