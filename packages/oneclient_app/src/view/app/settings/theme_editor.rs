use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use freya::prelude::*;
use freya::router::RouterContext;

use super::{
    confirm_dialog, first_section_header, readable_secondary, section_header, settings_page,
    settings_row,
};
use crate::components::{
    Button, ColorPicker, Dropdown, Icon, IconType, Segment, SegmentedControl, TextInput, toggle,
};
use crate::hooks::{use_debounced, use_dispatch};
use crate::i18n::{t, tf};
use crate::routes::Route;
use crate::theme::colors::{self, Hex, Palette};
use crate::theme::text_size;
use crate::theme::{
    Theme, family_name, family_taken, family_variants, font_families, pixel_font_size_of,
    save_theme, variant_name,
};
use crate::view::app::settings::appearance_picker::{compose_theme, split_theme};

const BORDER_OPACITIES: [u8; 7] = [5, 10, 15, 20, 30, 45, 60];
const CORNERS: [(&str, f32); 4] = [
    ("oneclient.settings.theme_editor.corners.square", 0.),
    ("oneclient.settings.theme_editor.corners.subtle", 0.5),
    ("oneclient.settings.theme_editor.corners.default", 1.),
    ("oneclient.settings.theme_editor.corners.round", 1.5),
];

/// The family as the editor opened it
#[derive(Clone, PartialEq)]
struct EditorSeed {
    dark: Theme,
    light: Theme,
    family: String,
    radius: f32,
    pixel_icons: bool,
}

#[derive(Clone, PartialEq)]
struct EditorDraft {
    seed: EditorSeed,
    dark: Theme,
    light: Theme,
    name: String,
    radius: f32,
    pixel_icons: bool,
    editing_light: bool,
}

impl EditorDraft {
    fn dirty(&self) -> bool {
        self.seed.differs(
            &self.dark,
            &self.light,
            &family_name(&self.name),
            self.radius,
            self.pixel_icons,
        )
    }

    fn same_changes(&self, other: &Self) -> bool {
        self.seed == other.seed
            && self.dark == other.dark
            && self.light == other.light
            && self.name == other.name
            && self.radius == other.radius
            && self.pixel_icons == other.pixel_icons
    }
}

thread_local! {
    static EDITOR_DRAFTS: RefCell<HashMap<String, EditorDraft>> = RefCell::new(HashMap::new());
}

fn store_draft(id: &str, draft: EditorDraft) {
    EDITOR_DRAFTS.with_borrow_mut(|drafts| {
        if draft.dirty() {
            drafts.insert(id.to_string(), draft);
        } else {
            drafts.remove(id);
        }
    });
}

pub(super) fn clear_draft(id: &str) {
    EDITOR_DRAFTS.with_borrow_mut(|drafts| {
        drafts.remove(id);
    });
}

fn clear_saved_draft(id: &str, saved: &EditorDraft) -> bool {
    EDITOR_DRAFTS.with_borrow_mut(|drafts| {
        if drafts
            .get(id)
            .is_some_and(|draft| !draft.same_changes(saved))
        {
            return false;
        }
        drafts.remove(id);
        true
    })
}

impl EditorSeed {
    fn from_variants(variants: &[std::rc::Rc<Theme>]) -> Self {
        let dark = (*variants[0]).clone();
        let light = (*variants[variants.len() - 1]).clone();
        Self {
            family: family_name(&dark.name),
            radius: dark.radius_scale,
            pixel_icons: dark.pixel_icons,
            dark,
            light,
        }
    }

    /// Whether the drafts hold anything Discard would throw away
    fn differs(&self, dark: &Theme, light: &Theme, family: &str, radius: f32, pixel: bool) -> bool {
        *dark != self.dark
            || *light != self.light
            || family != self.family
            || radius != self.radius
            || pixel != self.pixel_icons
    }
}

/// Back to Appearance, where the editor is opened from, without stacking another copy of it
fn leave_editor() {
    let router = RouterContext::get();
    if router.can_go_back() {
        router.go_back();
    } else {
        let _ = router.replace(Route::SettingsAppearance {});
    }
}

/// Why Save is unavailable for the typed family name, if it is
fn name_problem(family: &str, taken: bool) -> Option<String> {
    if family.is_empty() {
        Some(t("oneclient.settings.theme_editor.name.empty").to_string())
    } else if taken {
        Some(tf("oneclient.settings.theme_editor.name.taken", &[&family]))
    } else {
        None
    }
}

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
        let recovered =
            use_hook(|| EDITOR_DRAFTS.with_borrow(|drafts| drafts.get(&self.id).cloned()));
        // What the drafts are seeded from, so Discard only asks when something really changed
        let seed = use_hook(|| {
            recovered.as_ref().map_or_else(
                || EditorSeed::from_variants(&variants),
                |draft| draft.seed.clone(),
            )
        });
        let dark = use_state(|| {
            recovered
                .as_ref()
                .map_or_else(|| seed.dark.clone(), |d| d.dark.clone())
        });
        let light = use_state(|| {
            recovered
                .as_ref()
                .map_or_else(|| seed.light.clone(), |d| d.light.clone())
        });
        let ids = [dark.peek().id.clone(), light.peek().id.clone()];
        let editing_light = use_state(|| {
            recovered
                .as_ref()
                .map_or_else(|| colors::active_theme().colors.light, |d| d.editing_light)
        });
        let draft = if *editing_light.read() { light } else { dark };
        let name = use_state(|| {
            recovered
                .as_ref()
                .map_or_else(|| seed.family.clone(), |d| d.name.clone())
        });
        let radius = use_state(|| recovered.as_ref().map_or(seed.radius, |d| d.radius));
        let pixel_icons = use_state(|| {
            recovered
                .as_ref()
                .map_or(seed.pixel_icons, |d| d.pixel_icons)
        });

        let completed = use_hook(|| Rc::new(Cell::new(false)));
        let snapshot = EditorDraft {
            seed: seed.clone(),
            dark: dark.read().clone(),
            light: light.read().clone(),
            name: name.read().clone(),
            radius: *radius.read(),
            pixel_icons: *pixel_icons.read(),
            editing_light: *editing_light.read(),
        };
        if !completed.get() {
            store_draft(&self.id, snapshot.clone());
        }
        let draft_id = self.id.clone();
        let drop_seed = seed.clone();
        let drop_completed = completed.clone();
        use_drop(move || {
            if drop_completed.get() {
                clear_draft(&draft_id);
                return;
            }
            if let (
                Some(dark),
                Some(light),
                Some(name),
                Some(radius),
                Some(pixel_icons),
                Some(editing_light),
            ) = (
                dark.try_peek(),
                light.try_peek(),
                name.try_peek(),
                radius.try_peek(),
                pixel_icons.try_peek(),
                editing_light.try_peek(),
            ) {
                store_draft(
                    &draft_id,
                    EditorDraft {
                        seed: drop_seed,
                        dark: dark.clone(),
                        light: light.clone(),
                        name: name.clone(),
                        radius: *radius,
                        pixel_icons: *pixel_icons,
                        editing_light: *editing_light,
                    },
                );
            }
        });

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
        let problem = name_problem(
            &family,
            !family.is_empty() && family_taken(&family, &[&ids[0], &ids[1]]),
        );
        let valid = problem.is_none();
        let dirty = seed.differs(
            &dark.read(),
            &light.read(),
            &family,
            *radius.read(),
            *pixel_icons.read(),
        );
        let mut confirming_discard = use_state(|| false);

        let settled = use_debounced(edited.clone(), Duration::from_millis(120));
        use_side_effect_with_deps(&settled.read().clone(), |theme| {
            colors::preview(Some(theme.clone()));
        });
        use_drop(|| colors::preview(None));

        let mut saving = use_state(|| false);
        let discard_id = self.id.clone();
        let confirm_id = self.id.clone();
        let discard_completed = completed.clone();
        let save = {
            let completed = completed.clone();
            let draft_id = self.id.clone();
            move |_| {
                let both = both.clone();
                let family = ids[0].clone();
                let dispatch = dispatch.clone();
                let completed = completed.clone();
                let draft_id = draft_id.clone();
                let saved_draft = snapshot.clone();
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
                            completed.set(clear_saved_draft(&draft_id, &saved_draft));
                            dispatch.edit_settings(|settings| {
                                let (_, mode) = split_theme(&settings.theme);
                                if let Some(id) = compose_theme(&family, mode) {
                                    settings.theme = id;
                                }
                            });
                            leave_editor();
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
                t("oneclient.settings.theme_editor.name.title"),
                t("oneclient.settings.theme_editor.name.description"),
                rect().width(Size::px(220.)).child(
                    TextInput::new(name)
                        .placeholder(t("oneclient.settings.theme_editor.name.placeholder")),
                ),
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
            .child(first_section_header(t(
                "oneclient.settings.theme_editor.section.colors",
            )))
            .child(color_row(
                draft,
                IconType::LayoutTop,
                t("oneclient.settings.theme_editor.background.title"),
                t("oneclient.settings.theme_editor.background.description"),
                |palette| &mut palette.page,
            ))
            .child(color_row(
                draft,
                IconType::Copy01,
                t("oneclient.settings.theme_editor.surface.title"),
                t("oneclient.settings.theme_editor.surface.description"),
                |palette| &mut palette.page_elevated,
            ))
            .child(color_row(
                draft,
                IconType::Square,
                t("oneclient.settings.theme_editor.cards.title"),
                t("oneclient.settings.theme_editor.cards.description"),
                |palette| &mut palette.card,
            ))
            .child(color_row(
                draft,
                IconType::Sliders04,
                t("oneclient.settings.theme_editor.controls.title"),
                t("oneclient.settings.theme_editor.controls.description"),
                |palette| &mut palette.component,
            ))
            .child(color_row(
                draft,
                IconType::ParagraphWrap,
                t("oneclient.settings.theme_editor.text.title"),
                t("oneclient.settings.theme_editor.text.description"),
                |palette| &mut palette.text,
            ))
            .child(color_row(
                draft,
                IconType::MessageTextSquare01,
                t("oneclient.settings.theme_editor.secondary_text.title"),
                t("oneclient.settings.theme_editor.secondary_text.description"),
                |palette| &mut palette.text_secondary,
            ))
            .child(color_row(
                draft,
                IconType::PaintPour,
                t("oneclient.settings.theme_editor.accent.title"),
                t("oneclient.settings.theme_editor.accent.description"),
                |palette| &mut palette.accent,
            ))
            .child(section_header(t(
                "oneclient.settings.theme_editor.section.borders",
            )))
            .child(border_color_row(draft))
            .child(border_opacity_row(draft))
            .child(settings_row(
                IconType::Square,
                t("oneclient.settings.theme_editor.corners.title"),
                t("oneclient.settings.theme_editor.corners.description"),
                SegmentedControl::new(radius).segments(
                    CORNERS
                        .iter()
                        .map(|(label, scale)| Segment::new(*scale).label(t(label))),
                ),
            ))
            .child(section_header(t(
                "oneclient.settings.theme_editor.section.fonts",
            )))
            .child(font_row(
                [dark, light],
                IconType::ParagraphWrap,
                t("oneclient.settings.theme_editor.font.title"),
                t("oneclient.settings.theme_editor.font.description"),
                edited.font.clone(),
                |theme| &mut theme.font,
            ))
            .child(font_row(
                [dark, light],
                IconType::CodeSnippet02,
                t("oneclient.settings.theme_editor.code_font.title"),
                t("oneclient.settings.theme_editor.code_font.description"),
                edited.mono_font.clone(),
                |theme| &mut theme.mono_font,
            ))
            .child(settings_row(
                IconType::DotsGrid,
                t("oneclient.settings.theme_editor.pixel_icons.title"),
                t("oneclient.settings.theme_editor.pixel_icons.description"),
                toggle(pixel_icons),
            ))
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .spacing(8.)
                    .padding(Gaps::new(16., 0., 0., 0.))
                    // Save is disabled for a missing or taken name; say which
                    .child(
                        rect()
                            .horizontal()
                            .width(Size::flex(1.))
                            .cross_align(Alignment::Center)
                            .spacing(6.)
                            .maybe_child(problem.clone().map(|problem| {
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .content(Content::Flex)
                                    .cross_align(Alignment::Center)
                                    .spacing(6.)
                                    .child(
                                        Icon::new(IconType::InfoCircle)
                                            .size(14.)
                                            .color(readable_secondary()),
                                    )
                                    .child(
                                        label()
                                            .text(problem)
                                            .width(Size::flex(1.))
                                            .font_size(text_size(12.))
                                            .color(readable_secondary()),
                                    )
                                    .into_element()
                            })),
                    )
                    .child(
                        Button::new()
                            .ghost()
                            .on_press(move |_| {
                                if dirty {
                                    confirming_discard.set(true);
                                } else {
                                    discard_completed.set(true);
                                    clear_draft(&discard_id);
                                    leave_editor();
                                }
                            })
                            .text(t("oneclient.settings.theme_editor.discard")),
                    )
                    .child(
                        Button::new()
                            .primary()
                            .enabled(valid && !*saving.read())
                            .maybe(problem.is_some(), |button| {
                                button.tooltip(problem.clone().unwrap_or_default())
                            })
                            .on_press(save)
                            .text(t("oneclient.settings.theme_editor.save")),
                    ),
            )
            .maybe_child((*confirming_discard.read()).then(|| {
                confirm_dialog(
                    t("oneclient.settings.theme_editor.discard_confirm.title"),
                    t("oneclient.settings.theme_editor.discard_confirm.body"),
                    t("oneclient.settings.theme_editor.discard"),
                    true,
                    (move |()| confirming_discard.set(false)).into(),
                    (move |()| {
                        confirming_discard.set(false);
                        completed.set(true);
                        clear_draft(&confirm_id);
                        leave_editor();
                    })
                    .into(),
                )
            }))
            .into_element()
    }
}

/// The picker shows and picks the opaque color; a translucent field (the Cards fill of the
/// glass themes, for one) keeps its alpha, as the border row does
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
        ColorPicker::new(colors::with_alpha(value, 255), move |picked: Color| {
            let mut draft = draft;
            let mut palette = draft.peek().colors;
            let alpha = field(&mut palette).0.a();
            *field(&mut draft.write().colors) = Hex(keep_alpha(picked, alpha));
        })
        .into_element(),
    )
}

fn keep_alpha(picked: Color, alpha: u8) -> Color {
    colors::with_alpha(picked, alpha)
}

fn border_color_row(draft: State<Theme>) -> impl IntoElement {
    let border = draft.read().colors.border.0;
    settings_row(
        IconType::Square,
        t("oneclient.settings.theme_editor.border_color.title"),
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
        t("oneclient.settings.theme_editor.border_opacity.title"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picked_color_keeps_the_fields_alpha() {
        let picked = Color::from_rgb(0x12, 0x34, 0x56);
        let kept = keep_alpha(picked, 128);
        assert_eq!(
            (kept.r(), kept.g(), kept.b(), kept.a()),
            (0x12, 0x34, 0x56, 128)
        );
        assert_eq!(keep_alpha(picked, 255).a(), 255);
    }

    #[test]
    fn discard_only_asks_once_something_changed() {
        let dark = crate::theme::resolve_theme("dark");
        let light = crate::theme::resolve_theme("light");
        let seed = EditorSeed::from_variants(&[dark.clone(), light.clone()]);
        let family = seed.family.clone();
        assert!(!seed.differs(&dark, &light, &family, seed.radius, seed.pixel_icons));
        assert!(seed.differs(&dark, &light, "Reef", seed.radius, seed.pixel_icons));
        assert!(seed.differs(&dark, &light, &family, 0.5, seed.pixel_icons));
        let mut tinted = (*dark).clone();
        tinted.colors.card = Hex(Color::from_rgb(1, 2, 3));
        assert!(seed.differs(&tinted, &light, &family, seed.radius, seed.pixel_icons));
    }

    #[test]
    fn drafts_survive_navigation_and_save_does_not_erase_newer_edits() {
        let dark = crate::theme::resolve_theme("dark");
        let light = crate::theme::resolve_theme("light");
        let seed = EditorSeed::from_variants(&[dark.clone(), light.clone()]);
        let mut draft = EditorDraft {
            dark: (*dark).clone(),
            light: (*light).clone(),
            name: "Reef".into(),
            radius: seed.radius,
            pixel_icons: seed.pixel_icons,
            editing_light: false,
            seed,
        };
        store_draft("test-draft", draft.clone());
        assert!(EDITOR_DRAFTS.with_borrow(|drafts| drafts.get("test-draft") == Some(&draft)));
        let saved = draft.clone();
        draft.name = "Reef Two".into();
        store_draft("test-draft", draft.clone());
        assert!(!clear_saved_draft("test-draft", &saved));
        assert!(EDITOR_DRAFTS.with_borrow(|drafts| drafts.get("test-draft") == Some(&draft)));
        assert!(clear_saved_draft("test-draft", &draft));
        assert!(!EDITOR_DRAFTS.with_borrow(|drafts| drafts.contains_key("test-draft")));
        draft.name = draft.seed.family.clone();
        store_draft("test-draft", draft);
        assert!(!EDITOR_DRAFTS.with_borrow(|drafts| drafts.contains_key("test-draft")));
    }

    #[test]
    fn save_explains_a_missing_or_taken_name() {
        assert!(name_problem("", false).is_some());
        let taken = name_problem("Polyfrost", true).unwrap();
        assert!(taken.contains("Polyfrost"), "{taken}");
        assert_eq!(name_problem("Reef", false), None);
    }
}
