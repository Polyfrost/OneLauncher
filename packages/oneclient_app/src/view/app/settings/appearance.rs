use std::time::Duration;

use freya::prelude::*;
use oneclient_core::settings::{AppTheme, LauncherSettings};

use super::settings_page;
use crate::components::{ColorPicker, Icon, IconType, toggle};
use crate::hooks::{Actions, use_debounced, use_dispatch, use_settings_snapshot};
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::view::app::settings::{resettable, settings_row, settings_row_disabled};

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
        let settings = use_settings_snapshot().settings;
        let defaults = LauncherSettings::default();
        let dispatch = use_dispatch();

        let dynamic_bg = use_state({
            let v = settings.dynamic_background_enabled;
            move || v
        });

        let animations_on = use_state({
            let v = settings.animations_enabled;
            move || v
        });

        let mut first = use_state(|| true);
        let persist = dispatch.clone();
        use_side_effect(move || {
            let parallax = *dynamic_bg.read();
            let animations = *animations_on.read();
            if *first.peek() {
                first.set(false);
                return;
            }
            persist.edit_settings(|settings| {
                settings.dynamic_background_enabled = parallax;
                settings.animations_enabled = animations;
            });
        });

        let accent = use_state({
            let v = settings.accent_color.clone();
            move || v
        });
        let settled_accent = use_debounced(accent.read().clone(), Duration::from_millis(250));
        let mut accent_ready = use_state(|| false);
        let persist_accent = dispatch.clone();
        use_side_effect_with_deps(&settled_accent.read().clone(), move |value| {
            if !*accent_ready.peek() {
                accent_ready.set(true);
                return;
            }
            let value = value.clone();
            persist_accent.edit_settings(|settings| settings.accent_color = value);
        });

        settings_page()
            .child(theme_section(settings.theme, dispatch.clone()))
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .spacing(4.)
                    .padding(Gaps::new(8., 0., 0., 0.))
                    .child(accent_color_row(accent, defaults.accent_color.clone()))
                    .child(custom_theme_row())
                    .child(dynamic_background_row(
                        dynamic_bg,
                        defaults.dynamic_background_enabled,
                    ))
                    .child(animations_row(animations_on, defaults.animations_enabled)),
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

fn theme_section(selected: AppTheme, dispatch: Actions) -> impl IntoElement {
    let list = themes();
    let selected_index = AppTheme::ALL
        .iter()
        .position(|theme| *theme == selected)
        .unwrap_or_default();
    let current = list[selected_index];

    rect()
        .horizontal()
        .spacing(16.)
        .child(big_preview(current))
        .child(
            rect()
                .vertical()
                .spacing(16.)
                .child(rect().horizontal().spacing(16.).children(
                    (0..3).map(|i| theme_card(list[i], i, selected_index, dispatch.clone())),
                ))
                .child(rect().horizontal().spacing(16.).children(
                    (3..5).map(|i| theme_card(list[i], i, selected_index, dispatch.clone())),
                )),
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

fn theme_card(
    theme: ThemePreview,
    index: usize,
    selected_index: usize,
    dispatch: Actions,
) -> Element {
    let is_selected = selected_index == index;
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
        .cursor(CursorIcon::Pointer)
        .on_press(move |_| {
            dispatch.edit_settings(|settings| settings.theme = AppTheme::ALL[index]);
        })
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

fn accent_color_row(accent: State<Option<String>>, default: Option<String>) -> impl IntoElement {
    let shown = accent
        .read()
        .as_deref()
        .and_then(colors::parse_hex)
        .map_or_else(colors::brand, |(r, g, b)| Color::from_rgb(r, g, b));

    settings_row(
        IconType::PaintPour,
        "Accent color",
        "The main color used across the launcher. This doesn't edit your theme.",
        resettable(
            ColorPicker::new(shown, move |picked: Color| {
                let mut accent = accent;
                accent.set(Some(colors::format_hex(picked)));
            })
            .into_element(),
            accent,
            default,
        ),
    )
}

fn custom_theme_row() -> impl IntoElement {
    settings_row_disabled(
        IconType::Colors,
        "Custom theme",
        "Create, edit, and import launcher themes",
        Icon::new(IconType::ChevronRight).size(18.).into_element(),
    )
}

fn dynamic_background_row(enabled: State<bool>, default: bool) -> impl IntoElement {
    settings_row(
        IconType::Eye,
        "Parallax background",
        "Make the home screen background drift with your cursor. Turning it off keeps the background but disables the motion.",
        resettable(toggle(enabled), enabled, default),
    )
}

fn animations_row(animations_on: State<bool>, default: bool) -> impl IntoElement {
    settings_row(
        IconType::Play,
        "Animations",
        "Whether all launcher animations and transitions should be enabled.",
        resettable(toggle(animations_on), animations_on, default),
    )
}
