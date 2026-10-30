use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::settings::LauncherSettings;

use super::settings_page;
use crate::Route;
use crate::components::{IconType, link_button, toggle};
use crate::hooks::{
    BROWSER_COMPAT_DEFAULT, use_browser_compat, use_dispatch, use_settings_snapshot,
};
use crate::i18n::t;
use crate::view::app::settings::{resettable, section_header, settings_row};
use crate::view::console::open_log_console;

#[derive(PartialEq)]
pub struct SettingsDeveloper;

impl Component for SettingsDeveloper {
    fn render(&self) -> impl IntoElement {
        let settings = use_settings_snapshot().settings;
        let defaults = LauncherSettings::default();
        let dispatch = use_dispatch();

        let log_debug = use_state({
            let v = settings.log_debug;
            move || v
        });

        let mut first = use_state(|| true);
        use_side_effect(move || {
            let enabled = *log_debug.read();
            if *first.peek() {
                first.set(false);
                return;
            }
            dispatch.edit_settings(|settings| settings.log_debug = enabled);
        });

        let browser_compat = use_browser_compat();

        settings_page()
            .child(section_header(t(
                "oneclient.settings.developer.section.browser",
            )))
            .child(settings_row(
                IconType::SearchMd,
                t("oneclient.settings.developer.compat_only.title"),
                t("oneclient.settings.developer.compat_only.description"),
                resettable(
                    toggle(browser_compat),
                    browser_compat,
                    BROWSER_COMPAT_DEFAULT,
                ),
            ))
            .child(section_header(t(
                "oneclient.settings.developer.section.dev_tools",
            )))
            .child(settings_row(
                IconType::Sliders04,
                t("oneclient.settings.developer.log_debug.title"),
                t("oneclient.settings.developer.log_debug.description"),
                resettable(toggle(log_debug), log_debug, defaults.log_debug),
            ))
            .child(settings_row(
                IconType::Terminal,
                t("oneclient.settings.developer.log_console.title"),
                t("oneclient.settings.developer.log_console.description"),
                link_button().on_press(move |_| open_log_console()),
            ))
            .child(settings_row(
                IconType::CodeSnippet02,
                t("oneclient.settings.developer.debug_page.title"),
                t("oneclient.settings.developer.debug_page.description"),
                link_button().on_press(move |_| {
                    let _ = RouterContext::get().push(Route::Debug {});
                }),
            ))
            .into_element()
    }
}
