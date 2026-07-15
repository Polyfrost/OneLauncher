use freya::prelude::*;
use oneclient_core::java::{AvailableJava, JavaRuntime, JavaVendor};

use super::settings_page;
use crate::components::{Button, Dropdown, Icon, IconType, OverlayPopup, ScrollArea};
use crate::hooks::{
    BridgeDispatch, java_runtimes, provider_versions, use_dispatch, use_java_runtimes,
    use_provider_versions,
};
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::view::app::settings::section_header;

fn providers() -> Vec<(JavaVendor, &'static str)> {
    vec![
        (JavaVendor::Zulu, "Azul Zulu"),
        (JavaVendor::Adoptium, "Eclipse Temurin"),
        (JavaVendor::Corretto, "Amazon Corretto"),
        (JavaVendor::Liberica, "BellSoft Liberica"),
    ]
}

#[derive(PartialEq)]
pub struct SettingsJava;

impl Component for SettingsJava {
    fn render(&self) -> impl IntoElement {
        let runtimes_query = use_java_runtimes();
        let runtimes = java_runtimes(&runtimes_query);
        let show_manager = use_state(|| false);

        let mut shell = settings_page()
            .child(section_header("ADD RUNTIME"))
            .child(AddRow { show_manager }.into_element())
            .child(section_header("INSTALLED RUNTIMES"))
            .child(runtimes_table(runtimes));

        if *show_manager.read() {
            shell = shell.child(InstallManagerPopup { show_manager }.into_element());
        }

        shell.into_element()
    }
}

#[derive(PartialEq)]
struct AddRow {
    show_manager: State<bool>,
}

impl Component for AddRow {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let mut show_manager = self.show_manager;

        let pick = move |_| {
            let dispatch = dispatch.clone();
            spawn(async move {
                if let Some(handle) = rfd::AsyncFileDialog::new()
                    .set_title("Select a Java installation folder")
                    .pick_folder()
                    .await
                {
                    dispatch.add_custom_java_runtime(handle.path().to_path_buf());
                }
            });
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .spacing(10.)
            .child(
                Button::new()
                    .primary()
                    .on_press(move |_| show_manager.set(true))
                    .child(Icon::new(IconType::Download01).size(14.))
                    .text("Install Manager"),
            )
            .child(
                Button::new()
                    .secondary()
                    .on_press(pick)
                    .child(Icon::new(IconType::Folder).size(14.))
                    .text("Add from folder"),
            )
    }
}

fn runtimes_table(runtimes: Vec<JavaRuntime>) -> impl IntoElement {
    if runtimes.is_empty() {
        return rect()
            .width(Size::fill())
            .padding(Gaps::new_symmetric(16., 16.))
            .corner_radius(CornerRadius::new_all(12.))
            .background(colors::page_elevated())
            .child(
                label()
                    .text("No Java runtimes installed yet.")
                    .font_size(12.)
                    .color(colors::fg_secondary()),
            )
            .into_element();
    }

    let mut table = rect()
        .vertical()
        .width(Size::fill())
        .corner_radius(CornerRadius::new_all(12.))
        .background(colors::page_elevated())
        .border(border_all_color(1., colors::component_border()))
        .overflow(Overflow::Clip)
        .child(table_header());

    let count = runtimes.len();
    for (idx, runtime) in runtimes.into_iter().enumerate() {
        table = table.child(
            RuntimeRow {
                runtime,
                last: idx + 1 == count,
            }
            .into_element(),
        );
    }

    table.into_element()
}

fn table_header() -> impl IntoElement {
    fn head(text: &'static str, width: Size) -> impl IntoElement {
        rect()
            .width(width)
            .child(
                label()
                    .text(text)
                    .font_size(11.)
                    .font_weight(FontWeight::MEDIUM)
                    .color(colors::fg_secondary()),
            )
            .into_element()
    }

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .padding(Gaps::new_symmetric(8., 14.))
        .background(colors::component_bg())
        .child(head("VENDOR", Size::px(130.)))
        .child(head("VERSION", Size::px(90.)))
        .child(
            rect().width(Size::flex(1.0)).child(
                label()
                    .text("PATH")
                    .font_size(11.)
                    .font_weight(FontWeight::MEDIUM)
                    .color(colors::fg_secondary()),
            ),
        )
        .child(rect().width(Size::px(34.)))
        .into_element()
}

#[derive(PartialEq)]
struct RuntimeRow {
    runtime: JavaRuntime,
    last: bool,
}

impl Component for RuntimeRow {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let runtime = &self.runtime;
        let path = runtime.absolute_path.clone();

        fn cell(text: String, width: Size) -> impl IntoElement {
            rect()
                .width(width)
                .child(
                    label()
                        .text(text)
                        .font_size(13.)
                        .color(colors::fg_primary()),
                )
                .into_element()
        }

        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(12.)
            .padding(Gaps::new_symmetric(8., 14.))
            .maybe(!self.last, |el| {
                el.border(
                    Border::new()
                        .width(BorderWidth {
                            bottom: 1.,
                            ..Default::default()
                        })
                        .fill(colors::component_border()),
                )
            })
            .child(cell(
                format!("{} {}", runtime.vendor, runtime.major),
                Size::px(130.),
            ))
            .child(cell(runtime.version.clone(), Size::px(90.)))
            .child(
                rect()
                    .width(Size::flex(1.0))
                    .overflow(Overflow::Clip)
                    .child(
                        ScrollArea::new()
                            .horizontal(path_content_width(&runtime.absolute_path))
                            .width(Size::fill())
                            .height(Size::px(18.))
                            .show_scrollbar(false)
                            .child(
                                label()
                                    .text(runtime.absolute_path.clone())
                                    .font_size(12.)
                                    .max_lines(1)
                                    .color(colors::fg_secondary())
                                    .into_element(),
                            ),
                    ),
            )
            .child(remove_button(dispatch, path))
    }
}

fn path_content_width(path: &str) -> f32 {
    (path.chars().count() as f32 * 7.0).max(1.0)
}

fn remove_button(dispatch: BridgeDispatch, path: String) -> impl IntoElement {
    Button::new()
        .ghost()
        .small()
        .on_press(move |_| dispatch.remove_java_runtime(path.clone()))
        .child(
            Icon::new(IconType::Trash01)
                .size(14.)
                .color(colors::danger()),
        )
        .into_element()
}

#[derive(PartialEq)]
struct InstallManagerPopup {
    show_manager: State<bool>,
}

impl Component for InstallManagerPopup {
    fn render(&self) -> impl IntoElement {
        let mut show_manager = self.show_manager;
        let providers = providers();
        let mut selected = use_state(|| 0usize);

        let idx = (*selected.read()).min(providers.len() - 1);
        let (vendor, _) = providers[idx].clone();

        let versions_query = use_provider_versions(vendor.clone());
        let (versions, loading) = provider_versions(&versions_query);

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
                area = area.child(
                    VersionRow {
                        vendor: vendor.clone(),
                        available,
                        show_manager,
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

        OverlayPopup::new()
            .on_close(move |()| show_manager.set(false))
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
                                            .on_press(move |_| show_manager.set(false))
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
    show_manager: State<bool>,
}

impl Component for VersionRow {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let mut show_manager = self.show_manager;
        let vendor = self.vendor.clone();
        let major = self.available.major;
        let version = self.available.package.name.clone();

        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(12.)
            .padding(Gaps::new_symmetric(10., 12.))
            .corner_radius(CornerRadius::new_all(10.))
            .background(colors::component_bg())
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(2.)
                    .child(
                        label()
                            .text(format!("Java {major}"))
                            .font_size(14.)
                            .font_weight(FontWeight::MEDIUM)
                            .color(colors::fg_primary()),
                    )
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
                        dispatch.install_java_runtime(vendor.clone(), major);
                        show_manager.set(false);
                    })
                    .child(Icon::new(IconType::Download01).size(13.))
                    .text("Install"),
            )
    }
}
