//! Style (PolyGlass / Minecraft) and theme (System / Dark / Light) pickers, shared by the
//! onboarding Appearance step and Settings > Appearance so the two stay identical

use std::rc::Rc;

use freya::prelude::*;

use crate::AppAssets;
use crate::components::{Icon, IconType};
use crate::hooks::Actions;
use crate::i18n::t;
use crate::theme::{
    Theme, colors, counterpart, family_name, light_variant, resolve_theme, rounded, system_pair,
    system_theme_id, text_size, themes,
};
use crate::ui::border_all_color;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    System,
    Dark,
    Light,
}

/// Splits a theme id into the family (named by its dark variant) and the light/dark/system mode
pub(crate) fn split_theme(id: &str) -> (String, Mode) {
    if let Some((dark, _)) = system_pair(id) {
        return (dark, Mode::System);
    }
    let theme = resolve_theme(id);
    if theme.colors.light {
        let dark = counterpart(&theme.id).unwrap_or_else(|| theme.id.clone());
        (dark, Mode::Light)
    } else {
        (theme.id.clone(), Mode::Dark)
    }
}

fn compose_theme(dark: &str, mode: Mode) -> String {
    match mode {
        Mode::System => system_theme_id(dark),
        Mode::Dark => dark.to_string(),
        Mode::Light => light_variant(dark).unwrap_or_else(|| dark.to_string()),
    }
}

/// Built-in families that have both a dark and a light variant (PolyGlass, Minecraft)
fn style_families() -> Vec<Rc<Theme>> {
    themes()
        .into_iter()
        .filter(|theme| theme.is_builtin() && !theme.colors.light)
        .filter(|theme| light_variant(&theme.id).is_some())
        .collect()
}

pub(crate) fn section(title: &str, body: impl IntoElement) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(8.)
        .child(
            label()
                .text(title.to_string())
                .font_size(text_size(14.))
                .font_weight(FontWeight::MEDIUM)
                .color(colors::fg_primary()),
        )
        .child(body)
}

/// Unselected outlines and marks need more presence than the stock component border, which
/// all but disappears on the light themes
fn quiet_outline() -> Color {
    colors::fg_secondary().with_a(120)
}

/// The style cards for `theme_id`; `dense` trims the previews for short windows
pub(crate) fn style_picker(theme_id: &str, dense: bool, dispatch: &Actions) -> impl IntoElement {
    let (family, mode) = split_theme(theme_id);
    // System previews follow whichever variant the OS has the launcher showing right now
    let show_light = match mode {
        Mode::Light => true,
        Mode::Dark => false,
        Mode::System => colors::is_light(),
    };

    let cards = style_families().into_iter().map(|dark| {
        let preview = if show_light {
            light_variant(&dark.id)
                .map(|id| resolve_theme(&id))
                .unwrap_or_else(|| dark.clone())
        } else {
            dark.clone()
        };
        let selected = dark.id == family;
        let picked = dark.id.clone();
        let dispatch = dispatch.clone();
        style_card(&preview, selected, dense, move || {
            let picked = picked.clone();
            dispatch.edit_settings(|settings| {
                let (_, mode) = split_theme(&settings.theme);
                settings.theme = compose_theme(&picked, mode);
            });
        })
    });

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .spacing(16.)
        .children(cards)
}

/// Handlers read the live setting at press time instead of capturing this render's picks: a
/// press can reach a closure from an earlier render, which would apply a stale combination
pub(crate) fn theme_mode_picker(theme_id: &str, dispatch: &Actions) -> impl IntoElement {
    let (_, mode) = split_theme(theme_id);
    let segments = [
        (
            Mode::System,
            t("oneclient.settings.appearance.theme.system"),
        ),
        (Mode::Dark, t("oneclient.settings.appearance.theme.dark")),
        (Mode::Light, t("oneclient.settings.appearance.theme.light")),
    ]
    .into_iter()
    .map(|(option, text)| {
        let dispatch = dispatch.clone();
        let active = option == mode;
        rect()
            .width(Size::flex(1.))
            .height(Size::px(34.))
            .center()
            .corner_radius(rounded(7.))
            .background(if active {
                colors::brand()
            } else {
                Color::TRANSPARENT
            })
            .cursor(CursorIcon::Pointer)
            .a11y_role(AccessibilityRole::Button)
            .on_press(move |_| {
                dispatch.edit_settings(|settings| {
                    let (family, _) = split_theme(&settings.theme);
                    settings.theme = compose_theme(&family, option);
                });
            })
            .child(
                label()
                    .text(text.to_string())
                    .font_size(text_size(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .color(if active {
                        colors::on_brand()
                    } else {
                        colors::fg_primary().with_a(190)
                    }),
            )
            .into_element()
    });

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .spacing(4.)
        .padding(4.)
        .corner_radius(rounded(10.))
        .background(colors::component_bg())
        .border(border_all_color(1., quiet_outline()))
        .children(segments)
}

const CARD_RING: f32 = 2.;
const PREVIEW_HEIGHT: f32 = 104.;
const PREVIEW_HEIGHT_DENSE: f32 = 64.;
/// Fixed so the two fonts can't give the cards different heights and shift the page
const FOOTER_HEIGHT: f32 = 62.;
const FOOTER_HEIGHT_DENSE: f32 = 40.;

/// A card per style: a miniature launcher drawn in that style's own colors, font, corners and
/// icons, above its name, a one-line description and a selection mark
fn style_card(
    theme: &Theme,
    selected: bool,
    dense: bool,
    mut on_press: impl FnMut() + 'static,
) -> Element {
    let pixel = theme.pixel_font_size.is_some_and(|step| step > 0.);
    let blurb = if pixel {
        t("oneclient.onboarding.preferences.style.minecraft")
    } else {
        t("oneclient.onboarding.preferences.style.polyglass")
    };

    let footer = rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::px(if dense {
            FOOTER_HEIGHT_DENSE
        } else {
            FOOTER_HEIGHT
        }))
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(10.)
        .padding(Gaps::new_symmetric(0., 14.))
        .background(if selected {
            colors::brand().with_a(40)
        } else {
            Color::TRANSPARENT
        })
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.))
                .spacing(3.)
                .child(
                    label()
                        .text(family_name(&theme.name))
                        .max_lines(1)
                        .font_size(text_size(14.))
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                )
                .maybe_child((!dense).then(|| {
                    label()
                        .text(blurb.to_string())
                        .width(Size::fill())
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis)
                        .font_size(text_size(12.))
                        .color(colors::fg_secondary())
                        .into_element()
                })),
        )
        .child(selection_mark(selected));

    // Children paint over a border, so the ring sits outside a clipped inner body
    rect()
        .width(Size::flex(1.))
        .padding(CARD_RING)
        .corner_radius(rounded(12.))
        .border(border_all_color(
            if selected { CARD_RING } else { 1. },
            if selected {
                colors::brand()
            } else {
                quiet_outline()
            },
        ))
        .cursor(CursorIcon::Pointer)
        .a11y_role(AccessibilityRole::Button)
        .on_press(move |_| on_press())
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .corner_radius(rounded(12. - CARD_RING))
                .overflow(Overflow::Clip)
                .background(colors::card())
                .child(miniature(theme, pixel, dense))
                .child(
                    rect()
                        .width(Size::fill())
                        .height(Size::px(1.))
                        .background(colors::component_border()),
                )
                .child(footer),
        )
        .into_element()
}

fn selection_mark(selected: bool) -> Element {
    let mark = rect()
        .width(Size::px(20.))
        .height(Size::px(20.))
        .center()
        .corner_radius(rounded(10.));
    if selected {
        mark.background(colors::brand())
            .child(
                Icon::new(IconType::Check)
                    .size(14.)
                    .color(colors::on_brand()),
            )
            .into_element()
    } else {
        mark.border(border_all_color(1.5, quiet_outline()))
            .into_element()
    }
}

/// One of the launcher's own icons, picked from the previewed theme's icon set rather than
/// the active one so each card shows its own style
fn theme_icon(theme: &Theme, name: &str, size: f32, color: Color) -> Element {
    let pixel_path = format!("icons/pixel/{name}.svg");
    let path = if theme.pixel_icons && AppAssets::get(&pixel_path).is_some() {
        pixel_path
    } else {
        format!("icons/{name}.svg")
    };
    let bytes = AppAssets::get_bytes(&path).unwrap_or_default();
    SvgViewer::new((path, bytes))
        .show_loader(false)
        .width(Size::px(size))
        .height(Size::px(size))
        .color(color)
        .into_element()
}

/// Drawn with the previewed theme rather than the active one, so both cards stay honest
/// whichever style is currently applied
fn miniature(theme: &Theme, pixel: bool, dense: bool) -> Element {
    let palette = theme.colors;
    let radius = |r: f32| CornerRadius::new_all(r * theme.radius_scale);
    let font = theme.font.clone();
    let text = move |value: &str, size: f32, color: Color| {
        label()
            .text(value.to_string())
            .font_family(font.clone())
            .font_size(size)
            .max_lines(1)
            .text_overflow(TextOverflow::Ellipsis)
            .color(color)
    };
    // The pixel font draws visibly smaller at the same size, so it runs a step larger to
    // read the same as PolyGlass
    let (title_size, body_size, meta_size) = if pixel {
        (14., 13., 11.)
    } else {
        (13., 12., 10.)
    };

    let top_bar = rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(6.)
        .child(theme_icon(theme, "icon-logo", 13., palette.text.0))
        .child(
            text("OneClient", title_size, palette.text.0).font_weight(if pixel {
                FontWeight::NORMAL
            } else {
                FontWeight::BOLD
            }),
        );

    let instance = rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::flex(1.))
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(10.)
        .padding(if dense {
            Gaps::new(3., 4., 3., 10.)
        } else {
            Gaps::new_symmetric(0., 10.)
        })
        .corner_radius(radius(8.))
        .background(palette.card.0)
        .border(border_all_color(1., palette.border.0))
        .maybe_child((!dense).then(|| {
            rect()
                .width(Size::px(30.))
                .height(Size::px(30.))
                .center()
                .corner_radius(radius(6.))
                .background(palette.accent.0.with_a(60))
                .child(theme_icon(theme, "cube-01", 16., palette.accent.0))
                .into_element()
        }))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.))
                .spacing(3.)
                .child(text("Survival", body_size, palette.text.0))
                .maybe_child((!dense).then(|| {
                    text("26.1 Fabric", meta_size, palette.text_secondary.0).into_element()
                })),
        )
        .child(play_button(theme, pixel, dense));

    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::px(if dense {
            PREVIEW_HEIGHT_DENSE
        } else {
            PREVIEW_HEIGHT
        }))
        .content(Content::Flex)
        .padding(if dense { 10. } else { 12. })
        .spacing(if dense { 6. } else { 8. })
        .background(palette.page.0)
        .child(top_bar)
        .child(instance)
        .into_element()
}

/// Same accent button the launcher draws; the theme's corner scale squares it off for Minecraft
fn play_button(theme: &Theme, pixel: bool, dense: bool) -> Element {
    rect()
        .width(Size::px(if dense { 50. } else { 56. }))
        .height(Size::px(if dense { 20. } else { 26. }))
        .center()
        .corner_radius(CornerRadius::new_all(7. * theme.radius_scale))
        .background(theme.colors.accent.0)
        .child(
            label()
                .text("Play")
                .font_family(theme.font.clone())
                .font_size(if pixel { 12. } else { 11. })
                .font_weight(if pixel {
                    FontWeight::NORMAL
                } else {
                    FontWeight::SEMI_BOLD
                })
                .color(Color::WHITE),
        )
        .into_element()
}
