use std::time::Duration;

use freya::prelude::*;
use freya::router::RouterContext;

use super::{section_header, settings_page, settings_row};
use crate::components::{
    Button, ColorPicker, Dropdown, IconType, Segment, SegmentedControl, TextInput, toggle,
};
use crate::hooks::{use_debounced, use_dispatch};
use crate::i18n::t;
use crate::routes::Route;
use crate::theme::colors::{self, Hex, Palette};
use crate::theme::{
    Theme, family_name, family_taken, family_variants, font_families, pixel_font_size_of,
    save_theme, variant_name,
};
use crate::view::app::settings::appearance_picker::{compose_theme, split_theme};

const BORDER_OPACITIES: [u8; 7] = [5, 10, 15, 20, 30, 45, 60];
const CORNERS: [(&str, f32); 4] = [
    ("Square", 0.),
    ("Subtle", 0.5),
    ("Default", 1.),
    ("Round", 1.5),
];

/// Edits both variants of the family `id` belongs to; name, corners, fonts and icons are
/// shared, the colors are per variant
#[derive(PartialEq)]
pub struct SettingsThemeEditor {
    pub id: String,
}

impl Component for SettingsThemeEditor {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let variants = use_hook(|| family_variants(&self.id));
        let dark = use_state(|| (*variants[0]).clone());
        let light = use_state(|| (*variants[variants.len() - 1]).clone());
        let ids = [dark.peek().id.clone(), light.peek().id.clone()];
        let editing_light = use_state(|| colors::active_theme().colors.light);
        let draft = if *editing_light.read() { light } else { dark };
        let name = use_state(|| family_name(&dark.peek().name));
        let radius = use_state(|| dark.peek().radius_scale);
        let pixel_icons = use_state(|| dark.peek().pixel_icons);

        // Dark and Light are added per variant, so they are dropped from the typed name
        let family = family_name(&name.read());
        let finish = |theme: &Theme| {
            let mut theme = theme.clone();
            theme.name = variant_name(&family, theme.colors.light);
            theme.radius_scale = *radius.read();
            theme.pixel_icons = *pixel_icons.read();
            theme
        };
        let edited = finish(&draft.read());
        let both = [finish(&dark.read()), finish(&light.read())];
        let valid = !family.is_empty() && !family_taken(&family, &[&ids[0], &ids[1]]);

        let settled = use_debounced(edited.clone(), Duration::from_millis(120));
        use_side_effect_with_deps(&settled.read().clone(), |theme| {
            colors::preview(Some(theme.clone()));
        });
        use_drop(|| colors::preview(None));

        let mut saving = use_state(|| false);
        let save = {
            move |_| {
                let both = both.clone();
                let family = ids[0].clone();
                let dispatch = dispatch.clone();
                saving.set(true);
                spawn(async move {
                    let mut saved = Ok(String::new());
                    for theme in both {
                        saved = save_theme(theme).await;
                        if saved.is_err() {
                            break;
                        }
                    }
                    match saved {
                        Ok(_) => {
                            dispatch.edit_settings(|settings| {
                                let (_, mode) = split_theme(&settings.theme);
                                if let Some(id) = compose_theme(&family, mode) {
                                    settings.theme = id;
                                }
                            });
                            let _ = RouterContext::get().push(Route::SettingsAppearance {});
                        }
                        Err(err) => {
                            saving.set(false);
                            tracing::error!("Failed to save theme: {err:#}");
                            dispatch
                                .notify("Couldn't save the theme")
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
                "Name",
                "Saved as a JSON file in the launcher's themes folder.",
                rect()
                    .width(Size::px(220.))
                    .child(TextInput::new(name).placeholder("My theme")),
            ))
            .child(settings_row(
                IconType::Moon01,
                t("oneclient.settings.theme_editor.variant.title"),
                t("oneclient.settings.theme_editor.variant.description"),
                SegmentedControl::new(editing_light)
                    .segment(
                        Segment::new(false).label(t("oneclient.settings.appearance.theme.dark")),
                    )
                    .segment(
                        Segment::new(true).label(t("oneclient.settings.appearance.theme.light")),
                    ),
            ))
            .child(section_header("Colors"))
            .child(color_row(
                draft,
                IconType::LayoutTop,
                "Background",
                "Behind every page.",
                |palette| &mut palette.page,
            ))
            .child(color_row(
                draft,
                IconType::Copy01,
                "Surface",
                "Settings rows, dialogs and menus.",
                |palette| &mut palette.page_elevated,
            ))
            .child(color_row(
                draft,
                IconType::Square,
                "Cards",
                "Version, package and stat cards.",
                |palette| &mut palette.card,
            ))
            .child(color_row(
                draft,
                IconType::Sliders04,
                "Controls",
                "Inputs, dropdowns and secondary buttons.",
                |palette| &mut palette.component,
            ))
            .child(color_row(
                draft,
                IconType::ParagraphWrap,
                "Text",
                "Titles and body text.",
                |palette| &mut palette.text,
            ))
            .child(color_row(
                draft,
                IconType::MessageTextSquare01,
                "Secondary text",
                "Descriptions, hints and muted labels.",
                |palette| &mut palette.text_secondary,
            ))
            .child(color_row(
                draft,
                IconType::PaintPour,
                "Accent",
                "Primary buttons and highlights, unless you set an accent color.",
                |palette| &mut palette.accent,
            ))
            .child(section_header("Borders"))
            .child(border_color_row(draft))
            .child(border_opacity_row(draft))
            .child(settings_row(
                IconType::Square,
                "Corners",
                "How rounded cards, buttons and dialogs are.",
                SegmentedControl::new(radius).segments(
                    CORNERS
                        .iter()
                        .map(|(label, scale)| Segment::new(*scale).label(*label)),
                ),
            ))
            .child(section_header("Fonts"))
            .child(font_row(
                [dark, light],
                IconType::ParagraphWrap,
                "Font",
                "Used for all launcher text.",
                edited.font.clone(),
                |theme| &mut theme.font,
            ))
            .child(font_row(
                [dark, light],
                IconType::CodeSnippet02,
                "Code font",
                "Used for logs and code blocks.",
                edited.mono_font.clone(),
                |theme| &mut theme.mono_font,
            ))
            .child(settings_row(
                IconType::DotsGrid,
                "Pixel icons",
                "Use blocky pixel-art icons, like the Minecraft themes.",
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
                            .text("Discard"),
                    )
                    .child(
                        Button::new()
                            .primary()
                            .enabled(valid && !*saving.read())
                            .on_press(save)
                            .text("Save theme"),
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
        "Border color",
        "Outlines around cards, inputs and dialogs.",
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
        "Border opacity",
        "Hovered and pressed borders get stronger from here.",
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

/// Fonts are shared, so a pick is written to both variants
fn font_row(
    drafts: [State<Theme>; 2],
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
                for mut draft in drafts {
                    let mut theme = draft.write();
                    *field(&mut theme) = families[index].clone();
                    theme.pixel_font_size = pixel_font_size_of(&theme.font);
                }
            }),
    )
}
