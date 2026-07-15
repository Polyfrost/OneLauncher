use freya::prelude::*;
use freya::router::RouterContext;

use super::settings_page;
use crate::Route;
use crate::components::{Button, IconType, link_button, toggle};
use crate::hooks::{use_browser_compat, use_dispatch, use_settings_snapshot};
use crate::view::app::settings::{section_header, settings_row};

#[derive(PartialEq)]
pub struct SettingsDeveloper;

impl Component for SettingsDeveloper {
    fn render(&self) -> impl IntoElement {
        let settings = use_settings_snapshot().settings;
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
            let mut next = settings.clone();
            next.log_debug = enabled;
            dispatch.set_settings(next);
        });

        let show_dev = use_state(|| false);
        let browser_compat = use_browser_compat();

        settings_page()
            .child(section_header("BROWSER"))
            .child(settings_row(
                IconType::SearchMd,
                "Compatible content only",
                "Filter the content browser to the active cluster's version and loader.",
                toggle(browser_compat),
            ))
            .child(settings_row(
                IconType::InfoCircle,
                "Open Debug Info",
                "View runtime and environment details.",
                Button::new(),
            ))
            .child(section_header("DEV TOOLS"))
            .child(settings_row(
                IconType::Terminal,
                "Log Debug Info",
                "WARNING! This requires a restart to apply. Logs out debug info.",
                toggle(log_debug),
            ))
            .child(settings_row(
                IconType::CodeSnippet02,
                "Show Dev stuff",
                "Enable the dev tools and shows the debug page.",
                toggle(show_dev),
            ))
            .child(settings_row(
                IconType::CodeSnippet02,
                "Debug Page",
                "View the debug page.",
                link_button().on_press(move |_| {
                    let _ = RouterContext::get().push(Route::Debug {});
                }),
            ))
            .into_element()
    }
}
