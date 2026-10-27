use std::time::Duration;

use freya::prelude::*;
use freya::router::RouterContext;

use super::{section_header, settings_page, settings_row};
use crate::components::{
    Button, ColorPicker, Dropdown, IconType, Segment, SegmentedControl, TextInput,
};
use crate::hooks::{use_debounced, use_dispatch};
use crate::routes::Route;
use crate::theme::colors::{self, Hex, Palette};
use crate::theme::{Theme, font_families, save_theme};

const BORDER_OPACITIES: [u8; 7] = [5, 10, 15, 20, 30, 45, 60];
const CORNERS: [(&str, f32); 4] = [
    ("Square", 0.),
    ("Subtle", 0.5),
    ("Default", 1.),
    ("Round", 1.5),
];

#[derive(PartialEq)]
pub struct SettingsThemeEditor;

impl Component for SettingsThemeEditor {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let draft = use_state(|| {
            let mut theme = (*colors::active_theme()).clone();
            if theme.is_builtin() {
                theme.name = format!("{} (Custom)", theme.name);
            }
            theme
        });
        let name = use_state(|| draft.peek().name.clone());
        let light = use_state(|| draft.peek().colors.light);
        let radius = use_state(|| draft.peek().radius_scale);

        let edited = {
            let mut theme = draft.read().clone();
            theme.name = name.read().trim().to_string();
            theme.colors.light = *light.read();
            theme.radius_scale = *radius.read();
            theme
        };

        let settled = use_debounced(edited.clone(), Duration::from_millis(120));
        use_side_effect_with_deps(&settled.read().clone(), |theme| {
            colors::preview(Some(theme.clone()));
        });
        use_drop(|| colors::preview(None));

        let save = {
            let edited = edited.clone();
            move |_| match save_theme(edited.clone()) {
                Ok(id) => {
                    dispatch.edit_settings(|settings| settings.theme = id);
                    let _ = RouterContext::get().push(Route::SettingsAppearance {});
                }
                Err(err) => {
                    tracing::error!("Failed to save theme: {err}");
                    dispatch
                        .notify("Couldn't save the theme")
                        .body(err.to_string())
                        .error()
                        .toast_only()
                        .send();
                }
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
                "Base",
                "Whether the theme is dark or light. Changes which way hover and pressed shades go.",
                SegmentedControl::new(light)
                    .segment(Segment::new(false).label("Dark"))
                    .segment(Segment::new(true).label("Light")),
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
                draft,
                IconType::ParagraphWrap,
                "Font",
                "Used for all launcher text.",
                |theme| &mut theme.font,
            ))
            .child(font_row(
                draft,
                IconType::CodeSnippet02,
                "Code font",
                "Used for logs and code blocks.",
                |theme| &mut theme.mono_font,
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
                            .enabled(!edited.name.is_empty())
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
    let value = field(&mut draft.read().colors.clone()).color();
    settings_row(
        icon,
        title,
        description,
        ColorPicker::new(value, move |picked: Color| {
            let mut draft = draft;
            *field(&mut draft.write().colors) = Hex::from(picked);
        })
        .into_element(),
    )
}

fn border_color_row(draft: State<Theme>) -> impl IntoElement {
    let border = draft.read().colors.border.color();
    settings_row(
        IconType::Square,
        "Border color",
        "Outlines around cards, inputs and dialogs.",
        ColorPicker::new(colors::with_alpha(border, 255), move |picked: Color| {
            let mut draft = draft;
            let alpha = draft.peek().colors.border.color().a();
            draft.write().colors.border = Hex::from(colors::with_alpha(picked, alpha));
        })
        .into_element(),
    )
}

fn border_opacity_row(draft: State<Theme>) -> impl IntoElement {
    let border = draft.read().colors.border.color();
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
                let border = draft.peek().colors.border.color();
                draft.write().colors.border = Hex::from(colors::with_alpha(border, alpha));
            }),
    )
}

fn font_row(
    draft: State<Theme>,
    icon: IconType,
    title: &'static str,
    description: &'static str,
    field: fn(&mut Theme) -> &mut String,
) -> impl IntoElement {
    let families = font_families();
    let selected = field(&mut draft.read().clone()).clone();
    settings_row(
        icon,
        title,
        description,
        Dropdown::new(selected, families.clone())
            .width(Size::px(160.))
            .outlined()
            .on_select(move |index: usize| {
                let mut draft = draft;
                *field(&mut draft.write()) = families[index].clone();
            }),
    )
}
