use std::time::Duration;

use freya::prelude::*;
use freya::router::RouterContext;

use super::{section_header, settings_page, settings_row};
use crate::components::{
    Button, ColorPicker, Dropdown, IconType, Segment, SegmentedControl, TextInput, toggle,
};
use crate::hooks::{use_debounced, use_dispatch};
use crate::i18n::{t, tf};
use crate::routes::Route;
use crate::theme::colors::{self, Hex, Palette};
use crate::theme::{Theme, font_families, pixel_font_size_of, save_theme};

const BORDER_OPACITIES: [u8; 7] = [5, 10, 15, 20, 30, 45, 60];
const CORNERS: [(&str, f32); 4] = [
    ("oneclient.settings.theme_editor.corners.square", 0.),
    ("oneclient.settings.theme_editor.corners.subtle", 0.5),
    ("oneclient.common.default", 1.),
    ("oneclient.settings.theme_editor.corners.round", 1.5),
];

#[derive(PartialEq)]
pub struct SettingsThemeEditor;

impl Component for SettingsThemeEditor {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let draft = use_state(|| {
            let mut theme = (*colors::active_theme()).clone();
            if theme.is_builtin() {
                theme.name = tf(
                    "oneclient.settings.theme_editor.custom_name",
                    &[&theme.name],
                );
            }
            theme
        });
        let name = use_state(|| draft.peek().name.clone());
        let light = use_state(|| draft.peek().colors.light);
        let radius = use_state(|| draft.peek().radius_scale);
        let pixel_icons = use_state(|| draft.peek().pixel_icons);

        let edited = {
            let mut theme = draft.read().clone();
            theme.name = name.read().trim().to_string();
            theme.colors.light = *light.read();
            theme.radius_scale = *radius.read();
            theme.pixel_icons = *pixel_icons.read();
            theme
        };

        let settled = use_debounced(edited.clone(), Duration::from_millis(120));
        use_side_effect_with_deps(&settled.read().clone(), |theme| {
            colors::preview(Some(theme.clone()));
        });
        use_drop(|| colors::preview(None));

        let mut saving = use_state(|| false);
        let save = {
            let edited = edited.clone();
            move |_| {
                let theme = edited.clone();
                let dispatch = dispatch.clone();
                saving.set(true);
                spawn(async move {
                    match save_theme(theme).await {
                        Ok(id) => {
                            dispatch.edit_settings(|settings| settings.theme = id);
                            let _ = RouterContext::get().push(Route::SettingsAppearance {});
                        }
                        Err(err) => {
                            saving.set(false);
                            tracing::error!("Failed to save theme: {err:#}");
                            dispatch
                                .notify(t("oneclient.settings.theme_editor.save_failed"))
                                .body(format!("{err:#}"))
                                .error()
                                .toast_only()
                                .send();
                        }
                    }
                });
            }
        };

        settings_page()
            .child(settings_row(
                IconType::Pencil01,
                t("oneclient.settings.theme_editor.name"),
                t("oneclient.settings.theme_editor.name.description"),
                rect().width(Size::px(220.)).child(
                    TextInput::new(name)
                        .placeholder(t("oneclient.settings.theme_editor.name.placeholder")),
                ),
            ))
            .child(settings_row(
                IconType::Moon01,
                t("oneclient.settings.theme_editor.base"),
                t("oneclient.settings.theme_editor.base.description"),
                SegmentedControl::new(light)
                    .segment(
                        Segment::new(false).label(t("oneclient.settings.appearance.theme.dark")),
                    )
                    .segment(
                        Segment::new(true).label(t("oneclient.settings.appearance.theme.light")),
                    ),
            ))
            .child(section_header(t("oneclient.settings.theme_editor.colors")))
            .child(color_row(
                draft,
                IconType::LayoutTop,
                t("oneclient.settings.theme_editor.background"),
                t("oneclient.settings.theme_editor.background.description"),
                |palette| &mut palette.page,
            ))
            .child(color_row(
                draft,
                IconType::Copy01,
                t("oneclient.settings.theme_editor.surface"),
                t("oneclient.settings.theme_editor.surface.description"),
                |palette| &mut palette.page_elevated,
            ))
            .child(color_row(
                draft,
                IconType::Square,
                t("oneclient.settings.theme_editor.cards"),
                t("oneclient.settings.theme_editor.cards.description"),
                |palette| &mut palette.card,
            ))
            .child(color_row(
                draft,
                IconType::Sliders04,
                t("oneclient.settings.theme_editor.controls"),
                t("oneclient.settings.theme_editor.controls.description"),
                |palette| &mut palette.component,
            ))
            .child(color_row(
                draft,
                IconType::ParagraphWrap,
                t("oneclient.settings.theme_editor.text"),
                t("oneclient.settings.theme_editor.text.description"),
                |palette| &mut palette.text,
            ))
            .child(color_row(
                draft,
                IconType::MessageTextSquare01,
                t("oneclient.settings.theme_editor.secondary_text"),
                t("oneclient.settings.theme_editor.secondary_text.description"),
                |palette| &mut palette.text_secondary,
            ))
            .child(color_row(
                draft,
                IconType::PaintPour,
                t("oneclient.settings.theme_editor.accent"),
                t("oneclient.settings.theme_editor.accent.description"),
                |palette| &mut palette.accent,
            ))
            .child(section_header(t("oneclient.settings.theme_editor.borders")))
            .child(border_color_row(draft))
            .child(border_opacity_row(draft))
            .child(settings_row(
                IconType::Square,
                t("oneclient.settings.theme_editor.corners"),
                t("oneclient.settings.theme_editor.corners.description"),
                SegmentedControl::new(radius).segments(
                    CORNERS
                        .iter()
                        .map(|(label, scale)| Segment::new(*scale).label(t(label))),
                ),
            ))
            .child(section_header(t("oneclient.settings.theme_editor.fonts")))
            .child(font_row(
                draft,
                IconType::ParagraphWrap,
                t("oneclient.settings.theme_editor.font"),
                t("oneclient.settings.theme_editor.font.description"),
                edited.font.clone(),
                |theme| &mut theme.font,
            ))
            .child(font_row(
                draft,
                IconType::CodeSnippet02,
                t("oneclient.settings.theme_editor.code_font"),
                t("oneclient.settings.theme_editor.code_font.description"),
                edited.mono_font.clone(),
                |theme| &mut theme.mono_font,
            ))
            .child(settings_row(
                IconType::DotsGrid,
                t("oneclient.settings.theme_editor.pixel_icons"),
                t("oneclient.settings.theme_editor.pixel_icons.description"),
                toggle(pixel_icons),
            ))
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .main_align(Alignment::End)
                    .spacing(8.)
                    .padding(Gaps::new(16., 0., 0., 0.))
                    .child(
                        Button::new()
                            .ghost()
                            .on_press(|_| {
                                let _ = RouterContext::get().push(Route::SettingsAppearance {});
                            })
                            .text(t("oneclient.settings.theme_editor.discard")),
                    )
                    .child(
                        Button::new()
                            .primary()
                            .enabled(!edited.name.is_empty() && !*saving.read())
                            .on_press(save)
                            .text(t("oneclient.settings.theme_editor.save")),
                    ),
            )
            .into_element()
    }
}

fn color_row(
    draft: State<Theme>,
    icon: IconType,
    title: &'static str,
    description: &'static str,
    field: fn(&mut Palette) -> &mut Hex,
) -> impl IntoElement {
    let mut palette = draft.read().colors;
    let value = field(&mut palette).0;
    settings_row(
        icon,
        title,
        description,
        ColorPicker::new(value, move |picked: Color| {
            let mut draft = draft;
            *field(&mut draft.write().colors) = Hex(picked);
        })
        .into_element(),
    )
}

fn border_color_row(draft: State<Theme>) -> impl IntoElement {
    let border = draft.read().colors.border.0;
    settings_row(
        IconType::Square,
        t("oneclient.settings.theme_editor.border_color"),
        t("oneclient.settings.theme_editor.border_color.description"),
        ColorPicker::new(colors::with_alpha(border, 255), move |picked: Color| {
            let mut draft = draft;
            let alpha = draft.peek().colors.border.0.a();
            draft.write().colors.border = Hex(colors::with_alpha(picked, alpha));
        })
        .into_element(),
    )
}

fn border_opacity_row(draft: State<Theme>) -> impl IntoElement {
    let border = draft.read().colors.border.0;
    let percent = u32::from(border.a()) * 100 / 255;
    let selected = BORDER_OPACITIES
        .iter()
        .min_by_key(|option| u32::from(**option).abs_diff(percent))
        .copied()
        .unwrap_or_default();
    let options = BORDER_OPACITIES.iter().map(|o| format!("{o}%")).collect();

    settings_row(
        IconType::Eye,
        t("oneclient.settings.theme_editor.border_opacity"),
        t("oneclient.settings.theme_editor.border_opacity.description"),
        Dropdown::new(format!("{selected}%"), options)
            .outlined()
            .on_select(move |index: usize| {
                let mut draft = draft;
                let alpha = (u32::from(BORDER_OPACITIES[index]) * 255 / 100) as u8;
                let border = draft.peek().colors.border.0;
                draft.write().colors.border = Hex(colors::with_alpha(border, alpha));
            }),
    )
}

fn font_row(
    draft: State<Theme>,
    icon: IconType,
    title: &'static str,
    description: &'static str,
    selected: String,
    field: fn(&mut Theme) -> &mut String,
) -> impl IntoElement {
    let families = font_families();
    settings_row(
        icon,
        title,
        description,
        Dropdown::new(selected, families.clone())
            .width(Size::px(160.))
            .outlined()
            .on_select(move |index: usize| {
                let mut draft = draft;
                let mut theme = draft.write();
                *field(&mut theme) = families[index].clone();
                theme.pixel_font_size = pixel_font_size_of(&theme.font);
            }),
    )
}
