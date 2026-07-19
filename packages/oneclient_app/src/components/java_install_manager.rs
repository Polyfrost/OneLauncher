use freya::prelude::*;
use oneclient_core::java::{AvailableJava, JavaVendor};

use crate::components::{Button, Dropdown, Icon, IconType, OverlayPopup, ScrollArea};
use crate::hooks::{provider_versions, use_provider_versions};
use crate::theme::colors;
use crate::ui::border_all_color;

fn providers() -> Vec<(JavaVendor, &'static str)> {
    vec![
        (JavaVendor::Zulu, "Azul Zulu"),
        (JavaVendor::Adoptium, "Eclipse Temurin"),
        (JavaVendor::Corretto, "Amazon Corretto"),
        (JavaVendor::Liberica, "BellSoft Liberica"),
    ]
}

/// Reusable "Install Java" picker. Lists downloadable runtimes per provider and
/// calls `on_install` with the chosen `(vendor, major)`. When `suggested` is set,
/// the matching major is pulled to the top and highlighted.
#[derive(PartialEq)]
pub struct JavaInstallManager {
    suggested: Option<u32>,
    on_install: Option<EventHandler<(JavaVendor, u32)>>,
    on_close: Option<EventHandler<()>>,
}

impl Default for JavaInstallManager {
    fn default() -> Self {
        Self::new()
    }
}

impl JavaInstallManager {
    pub fn new() -> Self {
        Self {
            suggested: None,
            on_install: None,
            on_close: None,
        }
    }

    pub fn suggested(mut self, major: Option<u32>) -> Self {
        self.suggested = major;
        self
    }

    pub fn on_install(mut self, handler: impl Into<EventHandler<(JavaVendor, u32)>>) -> Self {
        self.on_install = Some(handler.into());
        self
    }

    pub fn on_close(mut self, handler: impl Into<EventHandler<()>>) -> Self {
        self.on_close = Some(handler.into());
        self
    }
}

impl Component for JavaInstallManager {
    fn render(&self) -> impl IntoElement {
        let providers = providers();
        let mut selected = use_state(|| 0usize);
        let suggested = self.suggested;

        let idx = (*selected.read()).min(providers.len() - 1);
        let (vendor, _) = providers[idx].clone();

        let versions_query = use_provider_versions(vendor.clone());
        let (mut versions, loading) = provider_versions(&versions_query);

        // Pull the suggested major to the top so it reads as the default choice.
        if let Some(major) = suggested {
            versions.sort_by_key(|v| v.major != major);
        }

        let provider_labels: Vec<String> =
            providers.iter().map(|(_, name)| name.to_string()).collect();
        let current_label = provider_labels[idx].clone();

        let inner: Element = if loading {
            status_text("Loading available versions...")
        } else if versions.is_empty() {
            status_text("No versions available from this provider.")
        } else {
            let mut area = ScrollArea::new()
                .width(Size::fill())
                .height(Size::fill())
                .spacing(8.);
            for available in versions {
                let highlight = suggested == Some(available.major);
                area = area.child(
                    VersionRow {
                        vendor: vendor.clone(),
                        available,
                        suggested: highlight,
                        on_install: self.on_install.clone(),
                    }
                    .into_element(),
                );
            }
            area.into_element()
        };

        let list = rect()
            .width(Size::fill())
            .height(Size::px(280.))
            .child(inner)
            .into_element();

        let close_scrim = self.on_close.clone();
        let close_button = self.on_close.clone();

        OverlayPopup::new()
            .on_close(move |()| {
                if let Some(handler) = &close_scrim {
                    handler.call(());
                }
            })
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(440.))
                            .max_width(Size::window_percent(90.))
                            .spacing(16.)
                            .padding(Gaps::new_all(20.))
                            .corner_radius(CornerRadius::new_all(16.))
                            .background(colors::page_elevated())
                            .border(border_all_color(1., colors::component_border()))
                            .child(
                                label()
                                    .text("Install Java")
                                    .font_size(18.)
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .color(colors::fg_primary()),
                            )
                            .child(
                                rect()
                                    .vertical()
                                    .width(Size::fill())
                                    .spacing(6.)
                                    .child(
                                        label()
                                            .text("Provider")
                                            .font_size(11.)
                                            .font_weight(FontWeight::MEDIUM)
                                            .color(colors::fg_secondary()),
                                    )
                                    .child(
                                        Dropdown::new(current_label, provider_labels)
                                            .width(Size::fill())
                                            .height(Size::px(34.))
                                            .on_select(move |i: usize| selected.set(i)),
                                    ),
                            )
                            .child(list)
                            .child(
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .main_align(Alignment::End)
                                    .child(
                                        Button::new()
                                            .ghost()
                                            .on_press(move |_| {
                                                if let Some(handler) = &close_button {
                                                    handler.call(());
                                                }
                                            })
                                            .text("Close"),
                                    ),
                            ),
                    ),
            )
            .into_element()
    }
}

fn status_text(text: &'static str) -> Element {
    rect()
        .width(Size::fill())
        .padding(Gaps::new_symmetric(16., 4.))
        .child(
            label()
                .text(text)
                .font_size(13.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

#[derive(PartialEq)]
struct VersionRow {
    vendor: JavaVendor,
    available: AvailableJava,
    suggested: bool,
    on_install: Option<EventHandler<(JavaVendor, u32)>>,
}

impl Component for VersionRow {
    fn render(&self) -> impl IntoElement {
        let vendor = self.vendor.clone();
        let major = self.available.major;
        let version = self.available.package.name.clone();
        let on_install = self.on_install.clone();

        let mut title_row = rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(8.)
            .child(
                label()
                    .text(format!("Java {major}"))
                    .font_size(14.)
                    .font_weight(FontWeight::MEDIUM)
                    .color(colors::fg_primary()),
            );

        if self.suggested {
            title_row = title_row.child(
                rect()
                    .padding(Gaps::new_symmetric(2., 8.))
                    .corner_radius(CornerRadius::new_all(6.))
                    .background(colors::brand())
                    .child(
                        label()
                            .text("Suggested")
                            .font_size(10.)
                            .font_weight(FontWeight::SEMI_BOLD)
                            .color(colors::fg_primary()),
                    ),
            );
        }

        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(12.)
            .padding(Gaps::new_symmetric(10., 12.))
            .corner_radius(CornerRadius::new_all(10.))
            .background(colors::component_bg())
            .maybe(self.suggested, |el| {
                el.border(border_all_color(1., colors::brand()))
            })
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(2.)
                    .child(title_row)
                    .child(
                        label()
                            .text(version)
                            .font_size(11.)
                            .max_lines(1)
                            .color(colors::fg_secondary()),
                    ),
            )
            .child(
                Button::new()
                    .primary()
                    .small()
                    .on_press(move |_| {
                        if let Some(handler) = &on_install {
                            handler.call((vendor.clone(), major));
                        }
                    })
                    .child(Icon::new(IconType::Download01).size(13.))
                    .text("Install"),
            )
    }
}
