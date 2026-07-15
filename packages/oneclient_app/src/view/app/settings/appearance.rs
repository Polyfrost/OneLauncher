use freya::prelude::*;

use super::settings_page;
use crate::components::{Icon, IconType, toggle};
use crate::hooks::{use_dispatch, use_settings_snapshot};
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::view::app::settings::settings_row;

#[derive(Clone, Copy)]
struct ThemePreview {
    background: Color,
    accent: Color,
    light: bool,
}

fn themes() -> [ThemePreview; 5] {
    [
        ThemePreview {
            background: Color::from_rgb(17, 23, 28),
            accent: Color::from_rgb(43, 75, 255),
            light: false,
        },
        ThemePreview {
            background: Color::from_rgb(0, 0, 0),
            accent: Color::from_rgb(111, 143, 255),
            light: false,
        },
        ThemePreview {
            background: Color::from_rgb(40, 42, 54),
            accent: Color::from_rgb(121, 112, 169),
            light: false,
        },
        ThemePreview {
            background: Color::from_rgb(244, 250, 255),
            accent: Color::from_rgb(57, 87, 255),
            light: true,
        },
        ThemePreview {
            background: Color::from_rgb(255, 255, 255),
            accent: Color::from_rgb(192, 195, 210),
            light: true,
        },
    ]
}

#[derive(PartialEq)]
pub struct SettingsAppearance;

impl Component for SettingsAppearance {
    fn render(&self) -> impl IntoElement {
        let selected_theme = use_state(|| 0usize);
        let animations_on = use_state(|| true);

        let settings = use_settings_snapshot().settings;
        let dispatch = use_dispatch();

        let dynamic_bg = use_state({
            let v = settings.dynamic_background_enabled;
            move || v
        });

        let mut first = use_state(|| true);
        use_side_effect(move || {
            let enabled = *dynamic_bg.read();
            if *first.peek() {
                first.set(false);
                return;
            }
            let mut next = settings.clone();
            next.dynamic_background_enabled = enabled;
            dispatch.set_settings(next);
        });

        settings_page()
            .child(theme_section(selected_theme))
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .spacing(4.)
                    .padding(Gaps::new(8., 0., 0., 0.))
                    .child(accent_color_row())
                    .child(custom_theme_row())
                    .child(dynamic_background_row(dynamic_bg))
                    .child(animations_row(animations_on)),
            )
            .child(
                rect().padding(Gaps::new(8., 0., 0., 0.)).child(
                    label()
                        .text("Looking to change your language?")
                        .font_size(12.)
                        .color(colors::fg_primary()),
                ),
            )
            .into_element()
    }
}

fn theme_section(selected: State<usize>) -> impl IntoElement {
    let list = themes();
    let current = list[(*selected.read()).min(list.len() - 1)];

    rect()
        .horizontal()
        .spacing(16.)
        .child(big_preview(current))
        .child(
            rect()
                .vertical()
                .spacing(16.)
                .child(
                    rect()
                        .horizontal()
                        .spacing(16.)
                        .children((0..3).map(|i| theme_card(list[i], i, selected))),
                )
                .child(
                    rect()
                        .horizontal()
                        .spacing(16.)
                        .children((3..5).map(|i| theme_card(list[i], i, selected))),
                ),
        )
        .into_element()
}

fn big_preview(theme: ThemePreview) -> impl IntoElement {
    rect()
        .width(Size::px(296.))
        .height(Size::px(183.))
        .corner_radius(CornerRadius::new_all(16.))
        .background(colors::page())
        .border(border_all_color(3., Color::from_argb(26, 255, 255, 255)))
        .center()
        .child(
            rect()
                .vertical()
                .width(Size::px(168.))
                .height(Size::px(123.))
                .corner_radius(CornerRadius::new_all(8.))
                .background(theme.background)
                .border(border_all_color(1., colors::component_border()))
                .padding(20.)
                .spacing(20.)
                .child(mock_line(108., theme.accent))
                .child(mock_line(84., line_tint(theme)))
                .child(split_bar(theme.accent, 96.)),
        )
        .into_element()
}

fn theme_card(theme: ThemePreview, index: usize, mut selected: State<usize>) -> Element {
    let is_selected = *selected.read() == index;
    let border_color = if is_selected {
        colors::brand()
    } else {
        colors::component_border()
    };

    rect()
        .vertical()
        .width(Size::px(126.))
        .height(Size::px(78.))
        .corner_radius(CornerRadius::new_all(8.))
        .background(theme.background)
        .border(border_all_color(1., border_color))
        .padding(8.)
        .spacing(20.)
        .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
        .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
        .on_press(move |_| *selected.write() = index)
        .child(mock_line(108., line_tint(theme)))
        .child(split_bar(theme.accent, 96.))
        .into_element()
}

fn mock_line(width: f32, color: Color) -> impl IntoElement {
    rect()
        .width(Size::px(width))
        .height(Size::px(10.))
        .corner_radius(CornerRadius::new_all(4.))
        .background(color)
        .into_element()
}

fn split_bar(accent: Color, width: f32) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::px(width))
        .height(Size::px(16.))
        .corner_radius(CornerRadius::new_all(4.))
        .background(accent.with_a(128))
        .child(
            rect()
                .width(Size::px(width / 3.))
                .height(Size::fill())
                .corner_radius(CornerRadius::new_all(4.))
                .background(accent),
        )
        .into_element()
}

fn line_tint(theme: ThemePreview) -> Color {
    if theme.light {
        Color::from_argb(38, 0, 0, 0)
    } else {
        Color::from_argb(46, 255, 255, 255)
    }
}

fn accent_color_row() -> impl IntoElement {
    settings_row(
        IconType::PaintPour,
        "Accent color",
        "The main color used across the launcher. This doesn't edit your theme.",
        rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(12.)
            .padding(Gaps::new_symmetric(8., 16.))
            .corner_radius(CornerRadius::new_all(8.))
            .background(colors::brand())
            .child(Icon::new(IconType::Colors).size(18.))
            .child(
                label()
                    .text("#2B4BFF")
                    .font_size(12.)
                    .color(colors::fg_primary()),
            )
            .into_element(),
    )
}

fn custom_theme_row() -> impl IntoElement {
    settings_row(
        IconType::Colors,
        "Custom theme",
        "Create, edit, and import launcher themes",
        Icon::new(IconType::ChevronRight).size(18.).into_element(),
    )
}

fn dynamic_background_row(enabled: State<bool>) -> impl IntoElement {
    settings_row(
        IconType::Eye,
        "Parallax background",
        "Make the home screen background drift with your cursor. Turning it off keeps the background but disables the motion.",
        toggle(enabled),
    )
}

fn animations_row(animations_on: State<bool>) -> impl IntoElement {
    settings_row(
        IconType::Play,
        "Animations",
        "Toggle all animations in the launcher. May cause buggy behavior.",
        toggle(animations_on),
    )
}
