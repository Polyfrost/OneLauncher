use std::rc::Rc;
use std::time::Duration;

use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::settings::LauncherSettings;

use super::settings_page;
use crate::components::{
    ColorPicker, ContextMenu, IconType, kebab_button, link_button, on_secondary, toggle,
};
use crate::hooks::{Actions, use_debounced, use_dispatch, use_settings_snapshot};
use crate::i18n::t;
use crate::routes::Route;
use crate::theme::{
    colors::{self, Palette},
    delete_theme, duplicate_theme, export_theme, family_name, light_variant, system_aware,
    system_pair, system_theme_id, text_size, theme_display_name, themes,
};
use crate::ui::border_all_color;
use crate::view::app::settings::{resettable, settings_row};

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
            .child(theme_cards(&settings.theme))
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

pub(crate) fn theme_cards(selected: &str) -> impl IntoElement {
    let system_family = system_pair(selected).map(|(dark, _)| dark);

    let card = |theme: &crate::theme::Theme| ThemeCard {
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
        menu: true,
        deletable: !theme.is_builtin(),
        selected_theme: selected.to_string(),
    };

    let mut families: Vec<(String, Vec<Rc<crate::theme::Theme>>)> = Vec::new();
    for theme in themes() {
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
            Some(ThemeCard {
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
                menu: false,
                deletable: false,
                ..card(dark)
            })
        });

        rect()
            .vertical()
            .spacing(10.)
            .width(Size::px(FAMILY_WIDTH))
            .child(
                label()
                    .text(family)
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
                    .maybe_child(system.map(IntoElement::into_element))
                    .children(members.iter().map(|theme| card(theme).into_element())),
            )
            .into_element()
    });

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::wrap_spacing(24.))
        .spacing(24.)
        .children(groups)
        .into_element()
}

#[derive(Clone, PartialEq)]
struct ThemeCard {
    id: String,
    name: String,
    full_name: String,
    background: Fill,
    preview: Palette,
    radius_scale: f32,
    selected: bool,
    menu: bool,
    deletable: bool,
    selected_theme: String,
}

impl Component for ThemeCard {
    fn render(&self) -> impl IntoElement {
        let mut menu_at = use_state(|| None::<(f32, f32)>);

        let id = self.id.clone();
        let dispatch = use_dispatch();
        let border_color = if self.selected {
            colors::brand()
        } else {
            colors::component_border()
        };

        let open_menu: EventHandler<(f32, f32)> = (move |at| menu_at.set(Some(at))).into();
        let menu = menu_at.read().filter(|_| self.menu).map(|(x, y)| {
            let duplicate = {
                let id = self.id.clone();
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
                let id = self.id.clone();
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

            let mut menu = ContextMenu::new(x, y)
                .title(self.full_name.clone())
                .action(
                    IconType::Copy01,
                    t("oneclient.settings.appearance.theme.duplicate"),
                    duplicate,
                )
                .action(
                    IconType::Download01,
                    t("oneclient.settings.appearance.theme.export"),
                    export,
                );
            if self.deletable {
                let id = self.id.clone();
                let selected = self.selected_theme.clone();
                let dispatch = dispatch.clone();
                menu = menu.separator().danger_action(
                    IconType::Trash01,
                    t("oneclient.common.delete"),
                    move |()| {
                        let id = id.clone();
                        let selected = selected.clone();
                        let dispatch = dispatch.clone();
                        spawn_in_window(async move {
                            let in_use = [true, false]
                                .into_iter()
                                .any(|dark| system_aware(&selected, dark) == id);
                            match delete_theme(&id).await {
                                Ok(()) if in_use => dispatch
                                    .edit_settings(|settings| settings.theme = "dark".into()),
                                Ok(()) => {}
                                Err(err) => notify_failure(
                                    &dispatch,
                                    t("oneclient.settings.appearance.theme.delete_failed"),
                                    err,
                                ),
                            }
                        });
                    },
                );
            }
            menu.on_close(move |()| menu_at.set(None)).into_element()
        });

        rect()
            .vertical()
            .spacing(6.)
            .cursor(CursorIcon::Pointer)
            .on_press(move |_| {
                dispatch.edit_settings(|settings| settings.theme = id.clone());
            })
            .maybe(self.menu, |el| {
                el.on_secondary_down(on_secondary(Some(open_menu.clone())))
            })
            .maybe_child(menu)
            .child(
                rect()
                    .vertical()
                    .width(Size::px(126.))
                    .height(Size::px(78.))
                    .corner_radius(CornerRadius::new_all(8. * self.radius_scale))
                    .background(self.background.clone())
                    .border(border_all_color(1., border_color))
                    .padding(8.)
                    .spacing(20.)
                    .child(mock_line(108., line_tint(self.preview), self.radius_scale))
                    .child(split_bar(self.preview.accent.0, 96., self.radius_scale))
                    .maybe_child(self.menu.then(|| {
                        rect()
                            .position(Position::new_absolute().top(-4.).right(-4.))
                            .layer(Layer::Relative(3))
                            .corner_radius(CornerRadius::new_all(6. * self.radius_scale))
                            .background(colors::card())
                            .child(kebab_button(open_menu.clone()))
                            .into_element()
                    })),
            )
            .child(
                label()
                    .text(self.name.clone())
                    .width(Size::px(126.))
                    .max_lines(1)
                    .text_overflow(TextOverflow::Ellipsis)
                    .font_size(text_size(12.))
                    .font_weight(if self.selected {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::NORMAL
                    })
                    .color(if self.selected {
                        colors::fg_primary()
                    } else {
                        colors::fg_secondary()
                    }),
            )
    }
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
