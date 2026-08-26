use freya::prelude::*;
use oneclient_java::{JavaRuntime, JavaVendor};

use super::settings_page;
use crate::components::{Button, Icon, IconType, JavaInstallManager, ScrollArea};
use crate::hooks::{Actions, java_runtimes, use_dispatch, use_java_runtimes};
use crate::invalidate_java_queries;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::view::app::settings::section_header;

#[derive(PartialEq)]
pub struct SettingsJava;

impl Component for SettingsJava {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let runtimes_query = use_java_runtimes();
        let runtimes = java_runtimes(&runtimes_query);
        let mut show_manager = use_state(|| false);

        fn invalidate_runtimes(dispatch: Actions) {
            spawn(async move {
                invalidate_java_queries().await;
                dispatch
                    .notify("Java runtimes refreshed")
                    .body("The installed runtime list is up to date")
                    .info()
                    .send();
            });
        }

        let refresh_dispatch = dispatch.clone();

        let mut shell = settings_page()
            .child(section_header("ADD RUNTIME"))
            .child(AddRow { show_manager }.into_element())
            .child(
                rect()
                    .width(Size::Fill)
                    .direction(Direction::Horizontal)
                    .main_align(Alignment::SpaceBetween)
                    .cross_align(Alignment::Center)
                    .child(section_header("INSTALLED RUNTIMES"))
                    .child(
                        Button::new()
                            .secondary()
                            .small()
                            .enabled(false) // disabled for now
                            .on_press(move |_| {
                                invalidate_runtimes(refresh_dispatch.clone());
                            })
                            .child(label().text("Refresh"))
                    )
            )
            .child(runtimes_table(runtimes));

        if *show_manager.read() {
            shell = shell.child(
                JavaInstallManager::new()
                    .on_install(move |(vendor, major): (JavaVendor, u32)| {
                        dispatch.install_java_runtime(vendor, major);
                        show_manager.set(false);
                    })
                    .on_close(move |()| show_manager.set(false))
                    .into_element(),
            );
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

        // so the scrollarea becomes bigger when horizontal scroll bar is visible (then it won't obscure the file path)
        let mut viewport_w = use_state(|| 0f32);
        let content_w = path_content_width(&runtime.absolute_path);
        let measured_w = *viewport_w.read();
        let has_scrollbar = measured_w <= 0. || measured_w < content_w;
        let path_height = if has_scrollbar { 28. } else { 18. };

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
                    .height(Size::px(path_height))
                    .overflow(Overflow::Clip)
                    .on_sized(move |e: Event<SizedEventData>| {
                        let w = e.area.width();
                        if (*viewport_w.read() - w).abs() > 0.5 {
                            viewport_w.set(w);
                        }
                    })
                    .child(
                        ScrollArea::new()
                            .horizontal(content_w)
                            .width(Size::fill())
                            .height(Size::px(path_height))
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

fn remove_button(dispatch: Actions, path: String) -> impl IntoElement {
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
