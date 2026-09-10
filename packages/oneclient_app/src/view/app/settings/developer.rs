use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::settings::LauncherSettings;

use super::settings_page;
use crate::Route;
use crate::components::{IconType, link_button, toggle};
use crate::hooks::{
    BROWSER_COMPAT_DEFAULT, ResetNotice, use_browser_compat, use_dispatch, use_game_active,
    use_settings_snapshot,
};
use crate::view::app::settings::{reset_row, section_header, settings_row, take_notice};
use crate::view::console::open_log_console;

const RESET_NOTICE: ResetNotice = ResetNotice {
    title: "Developer options reset",
    body: "Content filtering is back on and debug logging is off. Restart OneClient to \
           finish applying the logging change.",
};

#[derive(PartialEq)]
pub struct SettingsDeveloper;

impl Component for SettingsDeveloper {
    fn render(&self) -> impl IntoElement {
        let settings = use_settings_snapshot().settings;
        let dispatch = use_dispatch();

        let mut log_debug = use_state({
            let v = settings.log_debug;
            move || v
        });

        let mut first = use_state(|| true);
        let mut announce = use_state(|| false);
        {
            let dispatch = dispatch.clone();
            use_side_effect(move || {
                let enabled = *log_debug.read();
                if *first.peek() {
                    first.set(false);
                    return;
                }
                dispatch.edit_settings_notifying(
                    take_notice(announce, RESET_NOTICE),
                    |settings| settings.log_debug = enabled,
                );
            });
        }

        let mut browser_compat = use_browser_compat();

        let game_active = use_game_active();
        let confirming_reset = use_state(|| false);
        let reset = move |()| {
            announce.set(true);
            browser_compat.set(BROWSER_COMPAT_DEFAULT);
            log_debug.set(LauncherSettings::default().log_debug);
        };

        settings_page()
            .child(section_header("BROWSER"))
            .child(settings_row(
                IconType::SearchMd,
                "Compatible content only",
                "Filter the content browser to the active cluster's version and loader.",
                toggle(browser_compat),
            ))
            .child(section_header("DEV TOOLS"))
            .child(settings_row(
                IconType::Sliders04,
                "Log Debug Info",
                "WARNING! This requires a restart to apply. Logs out debug info.",
                toggle(log_debug),
            ))
            .child(settings_row(
                IconType::Terminal,
                "Log Console",
                "Stream the launcher's logs in a separate window. Nothing is captured while it is closed.",
                link_button().on_press(move |_| open_log_console()),
            ))
            .child(settings_row(
                IconType::CodeSnippet02,
                "Debug Page",
                "View the debug page.",
                link_button().on_press(move |_| {
                    let _ = RouterContext::get().push(Route::Debug {});
                }),
            ))
            .child(reset_row(
                confirming_reset,
                game_active,
                "Put the options on this page back to their defaults.",
                "Compatible content only goes back on and debug logging goes back off. \
                Debug logging takes effect the next time OneClient starts.",
                reset.into(),
            ))
            .into_element()
    }
}
