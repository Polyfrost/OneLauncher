use freya::prelude::*;

use super::{section_header, settings_page};
use crate::components::{Button, Icon, IconType};
use crate::i18n::t;
use crate::theme::{colors, rounded, text_size};
use crate::ui::border_all_color;

const ISSUES_URL: &str = "https://github.com/RemixClient/RemixClient/issues/new/choose";

#[derive(PartialEq)]
pub struct SettingsHelp;

impl Component for SettingsHelp {
    fn render(&self) -> impl IntoElement {
        settings_page()
            .child(section_header(t("oneclient.settings.help.section.start")))
            .child(feature(
                IconType::Users01,
                t("oneclient.settings.help.offline.title"),
                t("oneclient.settings.help.offline.body"),
            ))
            .child(feature(
                IconType::Folder,
                t("oneclient.settings.help.portable.title"),
                t("oneclient.settings.help.portable.body"),
            ))
            .child(feature(
                IconType::Database01,
                t("oneclient.settings.help.backups.title"),
                t("oneclient.settings.help.backups.body"),
            ))
            .child(feature(
                IconType::SearchMd,
                t("oneclient.settings.help.mods.title"),
                t("oneclient.settings.help.mods.body"),
            ))
            .child(feature(
                IconType::CodeSnippet02,
                t("oneclient.settings.help.java.title"),
                t("oneclient.settings.help.java.body"),
            ))
            .child(feature(
                IconType::AlertTriangle,
                t("oneclient.settings.help.crashes.title"),
                t("oneclient.settings.help.crashes.body"),
            ))
            .child(feature(
                IconType::Globe01,
                t("oneclient.settings.help.privacy.title"),
                t("oneclient.settings.help.privacy.body"),
            ))
            .child(section_header(t("oneclient.settings.help.section.report")))
            .child(report_card())
            .into_element()
    }
}

fn feature(icon: IconType, title: &'static str, body: &'static str) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .spacing(12.)
        .padding(Gaps::new_all(14.))
        .corner_radius(rounded(10.))
        .background(colors::page_elevated())
        .border(border_all_color(1., colors::component_border()))
        .child(Icon::new(icon).size(20.).color(colors::brand()))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(3.)
                .child(
                    label()
                        .text(title)
                        .font_size(text_size(14.))
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(body)
                        .font_size(text_size(12.))
                        .color(colors::fg_secondary()),
                ),
        )
        .into_element()
}

fn report_card() -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(12.)
        .padding(Gaps::new_all(16.))
        .corner_radius(rounded(12.))
        .background(colors::page_elevated())
        .border(border_all_color(1., colors::component_border()))
        .child(
            label()
                .text(t("oneclient.settings.help.report.title"))
                .font_size(text_size(15.))
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text(t("oneclient.settings.help.report.body"))
                .font_size(text_size(12.))
                .color(colors::fg_secondary()),
        )
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .spacing(8.)
                .content(Content::wrap_spacing(8.))
                .child(
                    Button::new()
                        .primary()
                        .on_press(|_| crate::platform::open_url(ISSUES_URL))
                        .child(Icon::new(IconType::Github).size(16.))
                        .text(t("oneclient.settings.help.report.github")),
                )
                .child(
                    Button::new()
                        .secondary()
                        .on_press(|_| {
                            if let Ok(dir) = oneclient_common::paths::logs_dir() {
                                crate::platform::open_path(&dir.to_string_lossy());
                            }
                        })
                        .child(Icon::new(IconType::Folder).size(16.))
                        .text(t("oneclient.settings.help.report.logs")),
                ),
        )
        .child(
            label()
                .text(t("oneclient.settings.help.report.note"))
                .font_size(text_size(11.))
                .color(colors::fg_secondary()),
        )
        .into_element()
}
