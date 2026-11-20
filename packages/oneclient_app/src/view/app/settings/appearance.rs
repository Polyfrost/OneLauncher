use std::rc::Rc;
use std::time::Duration;

use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::settings::LauncherSettings;

use super::settings_page;
use crate::components::{
    Button, ColorPicker, ContextMenu, Icon, IconType, link_button, on_secondary, toggle,
};
use crate::hooks::{Actions, use_debounced, use_dispatch, use_settings_snapshot};
use crate::i18n::{t, tf};
use crate::layout::SearchItem;
use crate::routes::Route;
use crate::theme::{
    SYSTEM_THEME,
    colors::{self, Palette},
    delete_theme, duplicate_theme, export_theme, family_name, is_known, light_variant, system_pair,
    system_theme_id, text_size, theme_display_name, themes,
};
use crate::ui::border_all_color;
use crate::view::app::settings::appearance_picker::{
    RadioGroup, radio_focus_visible, section, style_picker, theme_mode_picker,
};
use crate::view::app::settings::{resettable, settings_row};

#[derive(PartialEq)]
pub struct SettingsAppearance;

impl Component for SettingsAppearance {
    fn render(&self) -> impl IntoElement {
        let settings = use_settings_snapshot().settings;
        let dense = Platform::get().current_window().root_size.read().width < 1080.;
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
            .child(
                // Same pickers as the onboarding Appearance step, capped so the previews keep
                // their proportions on the wide settings page
                rect()
                    .vertical()
                    .width(Size::fill())
                    .max_width(Size::px(640.))
                    .spacing(20.)
                    .padding(Gaps::new(0., 0., 12., 0.))
                    .child(section(
                        t("oneclient.settings.appearance.style"),
                        style_picker(&settings.theme, dense, true),
                    ))
                    .child(section(
                        t("oneclient.settings.appearance.mode"),
                        theme_mode_picker(&settings.theme, &dispatch),
                    )),
            )
            .maybe_child(custom_theme_cards(&settings.theme))
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
                        .text(t("oneclient.settings.appearance.language_hint"))
                        .font_size(text_size(12.))
                        .color(colors::fg_primary()),
                ),
            )
            .into_element()
    }
}

const FAMILY_WIDTH: f32 = 3. * 126. + 2. * 16.;

/// User-made themes keep their own cards (with duplicate/export/delete) below the built-in
/// style and theme pickers; nothing is shown until one exists
fn custom_theme_cards(selected: &str) -> Option<Element> {
    if themes().iter().all(|theme| theme.is_builtin()) {
        return None;
    }

    let system_family = system_pair(selected).map(|(dark, _)| dark);

    let choice = |theme: &crate::theme::Theme| ThemeChoice {
        id: theme.id.clone(),
        name: if theme.colors.light {
            t("oneclient.settings.appearance.theme.light")
        } else {
            t("oneclient.settings.appearance.theme.dark")
        }
        .to_string(),
        full_name: theme.name.clone(),
        background: theme.colors.page.0.into(),
        preview: theme.colors,
        radius_scale: theme.radius_scale,
        selected: system_family.is_none() && theme.id == selected,
        concrete: true,
    };

    let mut families: Vec<(String, Vec<Rc<crate::theme::Theme>>)> = Vec::new();
    for theme in themes().into_iter().filter(|theme| !theme.is_builtin()) {
        let family = family_name(&theme.name);
        match families.iter_mut().find(|(name, _)| *name == family) {
            Some((_, members)) => members.push(theme),
            None => families.push((family, vec![theme])),
        }
    }

    let groups = families.into_iter().map(|(family, mut members)| {
        members.sort_by_key(|theme| theme.colors.light);
        let system = members.iter().find_map(|dark| {
            let light = light_variant(&dark.id)?;
            let light = members.iter().find(|theme| theme.id == light)?;
            let id = system_theme_id(&dark.id);
            Some(ThemeChoice {
                name: t("oneclient.settings.appearance.theme.system").to_string(),
                full_name: theme_display_name(&id),
                id,
                background: LinearGradient::new()
                    .angle(-90.)
                    .stop((dark.colors.page.0, 0.))
                    .stop((dark.colors.page.0, 50.))
                    .stop((light.colors.page.0, 50.))
                    .stop((light.colors.page.0, 100.))
                    .into(),
                selected: system_family.as_deref() == Some(dark.id.as_str()),
                concrete: false,
                ..choice(dark)
            })
        });

        CustomThemeFamily {
            name: family,
            options: system
                .into_iter()
                .chain(members.iter().map(|theme| choice(theme)))
                .collect(),
        }
        .into_element()
    });

    Some(
        rect()
            .vertical()
            .width(Size::fill())
            .spacing(10.)
            .padding(Gaps::new(0., 0., 12., 0.))
            .child(
                label()
                    .text(t("oneclient.settings.appearance.custom_themes"))
                    .font_size(text_size(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .color(colors::fg_primary()),
            )
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .content(Content::wrap_spacing(24.))
                    .spacing(24.)
                    .children(groups),
            )
            .into_element(),
    )
}

#[derive(Clone, PartialEq)]
struct ThemeChoice {
    id: String,
    name: String,
    full_name: String,
    background: Fill,
    preview: Palette,
    radius_scale: f32,
    selected: bool,
    concrete: bool,
}

#[derive(PartialEq)]
struct CustomThemeFamily {
    name: String,
    options: Rc<[ThemeChoice]>,
}

impl Component for CustomThemeFamily {
    fn render(&self) -> impl IntoElement {
        let radio = RadioGroup::use_options(self.options.iter().map(|_| true));
        let selected_index = self.options.iter().position(|choice| choice.selected);
        rect()
            .vertical()
            .spacing(10.)
            .width(Size::px(FAMILY_WIDTH))
            .child(
                label()
                    .text(self.name.clone())
                    .width(Size::fill())
                    .max_lines(1)
                    .text_overflow(TextOverflow::Ellipsis)
                    .font_size(text_size(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .color(colors::fg_primary()),
            )
            .child(
                rect()
                    .horizontal()
                    .spacing(16.)
                    .a11y_role(AccessibilityRole::RadioGroup)
                    .a11y_alt(self.name.clone())
                    .children(self.options.iter().enumerate().map(|(index, _)| {
                        ThemeCard {
                            index,
                            options: self.options.clone(),
                            radio: radio.clone(),
                            selected_index,
                        }
                        .into_element()
                    })),
            )
    }
}

#[derive(PartialEq)]
struct ThemeCard {
    index: usize,
    options: Rc<[ThemeChoice]>,
    radio: RadioGroup,
    selected_index: Option<usize>,
}

impl Component for ThemeCard {
    fn render(&self) -> impl IntoElement {
        let mut menu_at = use_state(|| None::<(f32, f32)>);
        let index = self.index;
        let radio = self.radio.clone();
        let pointer_radio = radio.clone();
        let a11y_id = radio.id(index);
        let focus = use_focus(a11y_id);
        let choice = &self.options[index];
        let selected = choice.selected;

        let options = self.options.clone();
        let dispatch = use_dispatch();
        let select = move |index: usize| {
            dispatch.edit_settings(|settings| settings.theme = options[index].id.clone());
        };
        let key_select = select.clone();
        let show_focus = radio_focus_visible(focus(), selected);
        let border_color = if show_focus {
            colors::fg_primary()
        } else if selected {
            colors::brand()
        } else {
            colors::component_border()
        };

        let card = rect()
            .vertical()
            .spacing(6.)
            .cursor(CursorIcon::Pointer)
            .a11y_alt(choice.full_name.clone())
            .on_press(move |_| pointer_radio.activate(index, &select))
            .maybe(choice.concrete, |el| {
                el.on_secondary_down(on_secondary(Some((move |at| menu_at.set(Some(at))).into())))
            })
            .child(
                rect()
                    .vertical()
                    .width(Size::px(126.))
                    .height(Size::px(78.))
                    .corner_radius(CornerRadius::new_all(8. * choice.radius_scale))
                    .background(choice.background.clone())
                    .border(
                        border_all_color(if show_focus { 2. } else { 1. }, border_color)
                            .alignment(BorderAlignment::Inner),
                    )
                    .padding(8.)
                    .spacing(20.)
                    .child(mock_line(
                        108.,
                        line_tint(choice.preview),
                        choice.radius_scale,
                    ))
                    .child(split_bar(choice.preview.accent.0, 96., choice.radius_scale)),
            )
            .child(
                label()
                    .text(choice.name.clone())
                    .width(Size::px(126.))
                    .max_lines(1)
                    .text_overflow(TextOverflow::Ellipsis)
                    .font_size(text_size(12.))
                    .font_weight(if selected {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::NORMAL
                    })
                    .color(if selected {
                        colors::fg_primary()
                    } else {
                        colors::fg_secondary()
                    }),
            );

        let card = radio.accessibility(card, index, self.selected_index);
        rect()
            .child(radio.keyboard(card, index, key_select))
            .maybe_child(choice.concrete.then(|| ThemeFileMenu {
                id: choice.id.clone(),
                name: choice.full_name.clone(),
                custom: Some(menu_at),
            }))
    }
}

#[derive(PartialEq)]
pub(super) struct ThemeFileMenu {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) custom: Option<State<Option<(f32, f32)>>>,
}

impl Component for ThemeFileMenu {
    fn render(&self) -> impl IntoElement {
        let mut anchor = use_state(|| (0., 0.));
        let internal_menu = use_state(|| None::<(f32, f32)>);
        let mut menu_at = self.custom.unwrap_or(internal_menu);
        let dispatch = use_dispatch();
        let alt = tf("oneclient.settings.appearance.theme.actions", &[&self.name]);

        rect()
            .position(Position::new_absolute().top(8.).right(8.))
            .layer(Layer::Relative(3))
            .on_sized(move |event: Event<SizedEventData>| {
                let frame = event.data().area;
                anchor.set_if_modified((frame.min_x(), frame.max_y()));
            })
            .child(
                Button::new()
                    .icon()
                    .width(Size::px(26.))
                    .height(Size::px(26.))
                    .alt(alt.clone())
                    .tooltip(alt)
                    .on_press(move |_| menu_at.set(Some(*anchor.peek())))
                    .child(Icon::new(IconType::DotsVertical).size(14.)),
            )
            .maybe_child((*menu_at.read()).map(|(x, y)| {
                let mut menu = theme_file_actions(
                    ContextMenu::new(x, y).title(self.name.clone()),
                    &self.id,
                    &dispatch,
                );
                if self.custom.is_some() {
                    menu = theme_delete_action(menu, &self.id, &dispatch);
                }
                menu.on_close(move |()| menu_at.set(None))
            }))
    }
}

fn theme_delete_action(menu: ContextMenu, id: &str, dispatch: &Actions) -> ContextMenu {
    let id = id.to_owned();
    let dispatch = dispatch.clone();
    menu.separator()
        .danger_action(IconType::Trash01, t("oneclient.common.delete"), move |()| {
            let id = id.clone();
            let dispatch = dispatch.clone();
            spawn_in_window(async move {
                // Capture the pairs that reference this file before deletion, then
                // repair only the current selection if it actually became invalid.
                let affected = affected_system_themes(&id);
                match delete_theme(&id).await {
                    Ok(()) => dispatch.edit_settings(|settings| {
                        if theme_needs_fallback(&settings.theme, &id, &affected, is_known) {
                            settings.theme = "dark".into();
                        }
                    }),
                    Err(err) => notify_failure(
                        &dispatch,
                        t("oneclient.settings.appearance.theme.delete_failed"),
                        err,
                    ),
                }
            });
        })
}

fn affected_system_themes(id: &str) -> Vec<String> {
    let mut affected = themes()
        .iter()
        .filter(|theme| {
            !theme.colors.light
                && (theme.id == id || light_variant(&theme.id).as_deref() == Some(id))
        })
        .map(|theme| system_theme_id(&theme.id))
        .collect::<Vec<_>>();
    if system_pair(SYSTEM_THEME).is_some_and(|(dark, light)| dark == id || light == id) {
        affected.push(SYSTEM_THEME.to_string());
    }
    affected
}

fn theme_needs_fallback(
    current: &str,
    removed: &str,
    affected_system: &[String],
    known: impl FnOnce(&str) -> bool,
) -> bool {
    (current == removed || affected_system.iter().any(|affected| affected == current))
        && !known(current)
}

fn theme_file_actions(menu: ContextMenu, id: &str, dispatch: &Actions) -> ContextMenu {
    let duplicate = {
        let id = id.to_owned();
        let dispatch = dispatch.clone();
        move |()| {
            let id = id.clone();
            let dispatch = dispatch.clone();
            spawn_in_window(async move {
                if let Err(err) = duplicate_theme(&id).await {
                    notify_failure(
                        &dispatch,
                        t("oneclient.settings.appearance.theme.duplicate_failed"),
                        err,
                    );
                }
            });
        }
    };
    let export = {
        let id = id.to_owned();
        let dispatch = dispatch.clone();
        move |()| {
            let id = id.clone();
            let dispatch = dispatch.clone();
            spawn_in_window(async move {
                let Some(file) = rfd::AsyncFileDialog::new()
                    .set_title(t("oneclient.settings.appearance.theme.export_title"))
                    .set_file_name(format!("{id}.json"))
                    .add_filter("JSON", &["json"])
                    .save_file()
                    .await
                else {
                    return;
                };
                match export_theme(&id, file.path()).await {
                    Ok(()) => dispatch
                        .notify(t("oneclient.settings.appearance.theme.exported"))
                        .info()
                        .toast_only()
                        .send(),
                    Err(err) => notify_failure(
                        &dispatch,
                        t("oneclient.settings.appearance.theme.export_failed"),
                        err,
                    ),
                }
            });
        }
    };

    menu.action(
        IconType::Copy01,
        t("oneclient.settings.appearance.theme.duplicate"),
        duplicate,
    )
    .action(
        IconType::Download01,
        t("oneclient.settings.appearance.theme.export"),
        export,
    )
}

fn notify_failure(dispatch: &Actions, title: &'static str, err: anyhow::Error) {
    tracing::error!("{title}: {err:#}");
    dispatch
        .notify(title)
        .body(format!("{err:#}"))
        .error()
        .toast_only()
        .send();
}

fn mock_line(width: f32, color: Color, radius_scale: f32) -> impl IntoElement {
    rect()
        .width(Size::px(width))
        .height(Size::px(10.))
        .corner_radius(CornerRadius::new_all(4. * radius_scale))
        .background(color)
        .into_element()
}

fn split_bar(accent: Color, width: f32, radius_scale: f32) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::px(width))
        .height(Size::px(16.))
        .corner_radius(CornerRadius::new_all(4. * radius_scale))
        .background(accent.with_a(128))
        .child(
            rect()
                .width(Size::px(width / 3.))
                .height(Size::fill())
                .corner_radius(CornerRadius::new_all(4. * radius_scale))
                .background(accent),
        )
        .into_element()
}

fn line_tint(theme: Palette) -> Color {
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
        t("oneclient.settings.appearance.accent_color.title"),
        t("oneclient.settings.appearance.accent_color.description"),
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
    settings_row(
        IconType::Colors,
        t("oneclient.settings.appearance.custom_theme.title"),
        t("oneclient.settings.appearance.custom_theme.description"),
        link_button().on_press(|_| {
            let _ = RouterContext::get().push(Route::SettingsThemeEditor {});
        }),
    )
}

fn dynamic_background_row(enabled: State<bool>, default: bool) -> impl IntoElement {
    settings_row(
        IconType::Eye,
        t("oneclient.settings.appearance.parallax_background.title"),
        t("oneclient.settings.appearance.parallax_background.description"),
        resettable(toggle(enabled), enabled, default),
    )
}

fn animations_row(animations_on: State<bool>, default: bool) -> impl IntoElement {
    settings_row(
        IconType::Play,
        t("oneclient.settings.appearance.animations.title"),
        t("oneclient.settings.appearance.animations.description"),
        resettable(toggle(animations_on), animations_on, default),
    )
}

inventory::submit! {
    SearchItem {
        id: "appearance.accent_color",
        icon: IconType::PaintPour,
        title: "oneclient.settings.appearance.accent_color.title",
        description: "oneclient.layout.search.appearance.accent_color.description",
        keywords: &["theme", "accent", "color", "brand"],
        route: Route::SettingsAppearance {},
    }
}

inventory::submit! {
    SearchItem {
        id: "appearance.custom_theme",
        icon: IconType::Colors,
        title: "oneclient.settings.appearance.custom_theme.title",
        description: "oneclient.layout.search.appearance.custom_theme.description",
        keywords: &["theme", "custom", "import", "export"],
        route: Route::SettingsAppearance {},
    }
}

inventory::submit! {
    SearchItem {
        id: "appearance.parallax_background",
        icon: IconType::Eye,
        title: "oneclient.settings.appearance.parallax_background.title",
        description: "oneclient.layout.search.appearance.parallax_background.description",
        keywords: &["background", "parallax", "motion"],
        route: Route::SettingsAppearance {},
    }
}

inventory::submit! {
    SearchItem {
        id: "appearance.animations",
        icon: IconType::Play,
        title: "oneclient.settings.appearance.animations.title",
        description: "oneclient.layout.search.appearance.animations.description",
        keywords: &["reduce motion", "animations", "effects"],
        route: Route::SettingsAppearance {},
    }
}

inventory::submit! {
    SearchItem {
        id: "appearance.theme",
        icon: IconType::Colors,
        title: "oneclient.onboarding.summary.accessibility.theme",
        description: "oneclient.layout.search.appearance.theme.description",
        keywords: &["theme", "dark", "light", "mode", "colors"],
        route: Route::SettingsAppearance {},
    }
}

#[cfg(test)]
mod tests {
    use super::theme_needs_fallback;

    #[test]
    fn deleting_a_theme_repairs_only_the_current_invalid_selection() {
        let removed = "custom-light";
        let system = "system:custom-dark";
        let affected = [system.to_string()];

        assert!(theme_needs_fallback(removed, removed, &affected, |_| false));
        assert!(theme_needs_fallback(system, removed, &affected, |_| false));
        assert!(!theme_needs_fallback(system, removed, &affected, |_| true));
        assert!(!theme_needs_fallback(
            "system:other-dark",
            removed,
            &affected,
            |_| false,
        ));
    }
}
