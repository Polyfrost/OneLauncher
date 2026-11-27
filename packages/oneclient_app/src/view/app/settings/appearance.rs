use std::time::Duration;

use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::settings::LauncherSettings;

use super::settings_page;
use crate::components::{Button, ColorPicker, ContextMenu, Icon, IconType, toggle};
use crate::hooks::{Actions, use_debounced, use_dispatch, use_settings_snapshot};
use crate::i18n::{t, tf};
use crate::layout::SearchItem;
use crate::routes::Route;
use crate::theme::{
    colors, copy_family, delete_theme, duplicate_theme, export_theme, family_variants,
    import_theme, is_known, system_theme_id, text_size,
};
use crate::view::app::settings::appearance_picker::{
    compose_theme, split_theme, style_picker, theme_mode_picker,
};
use crate::view::app::settings::{
    confirm_dialog, first_section_header, readable_secondary, resettable, section_header,
    section_header_with_action, settings_row,
};

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

        // Same pickers as the onboarding Appearance step. The style cards add columns as the page
        // widens, so they keep their proportions
        settings_page()
            .child(super::focus_anchor(
                "appearance.theme",
                first_section_header(t("oneclient.settings.appearance.mode")),
            ))
            .child(theme_mode_picker(&settings.theme, &dispatch))
            .child(section_header_with_action(
                t("oneclient.settings.appearance.style"),
                false,
                theme_actions(&dispatch),
            ))
            .child(style_picker(&settings.theme, dense, true))
            .child(section_header(t(
                "oneclient.settings.appearance.section.effects",
            )))
            .child(accent_color_row(accent, defaults.accent_color.clone()))
            .child(dynamic_background_row(
                dynamic_bg,
                defaults.dynamic_background_enabled,
            ))
            .child(animations_row(animations_on, defaults.animations_enabled))
            .child(language_link())
            .into_element()
    }
}

/// Switches to the family named by its dark id, keeping the current Light/Dark/System mode
fn select_family(dispatch: &Actions, family: &str) {
    dispatch.edit_settings(|settings| {
        let (_, mode) = split_theme(&settings.theme);
        if let Some(id) = compose_theme(family, mode) {
            settings.theme = id;
        }
    });
}

/// "Looking to change your language?" with a link to the Language page
fn language_link() -> impl IntoElement {
    let mut hovered = use_state(|| false);
    let link = t("oneclient.settings.appearance.language_link");
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(6.)
        .padding(Gaps::new(12., 0., 0., 0.))
        .child(
            label()
                .text(t("oneclient.settings.appearance.language_hint"))
                .font_size(text_size(12.))
                .color(readable_secondary()),
        )
        .child(
            rect()
                .a11y_role(AccessibilityRole::Link)
                .a11y_focusable(true)
                .a11y_alt(link)
                .cursor(CursorIcon::Pointer)
                .on_pointer_enter(move |_| hovered.set_if_modified(true))
                .on_pointer_leave(move |_| hovered.set_if_modified(false))
                .on_press(|_| {
                    let _ = RouterContext::get().replace(Route::SettingsLanguage {});
                })
                .child(
                    label()
                        .text(link)
                        .font_size(text_size(12.))
                        .font_weight(FontWeight::MEDIUM)
                        // The link color the Markdown pages use, readable on every surface
                        .color(colors::code_info())
                        .maybe(*hovered.read(), |el| {
                            el.text_decoration(TextDecoration::Underline)
                        }),
                ),
        )
}

fn open_editor(id: String) {
    let _ = RouterContext::get().push(Route::SettingsThemeEditor { id });
}

/// New custom themes start as a copy of the built-in Polyfrost dark and light pair
fn create_theme(dispatch: &Actions) {
    let dispatch = dispatch.clone();
    // The window task below has no component context to find the router in
    let router = RouterContext::get();
    spawn_in_window(async move {
        match copy_family("dark", t("oneclient.settings.appearance.theme.new_name")).await {
            Ok(id) => {
                select_family(&dispatch, &id);
                let _ = router.push(Route::SettingsThemeEditor { id });
            }
            Err(err) => notify_failure(
                &dispatch,
                t("oneclient.settings.appearance.theme.create_failed"),
                err,
            ),
        }
    });
}

/// New theme and Import, side by side at the right of the Style header
fn theme_actions(dispatch: &Actions) -> Element {
    let create = dispatch.clone();
    let text = t("oneclient.settings.appearance.theme.new");
    rect()
        .horizontal()
        .spacing(8.)
        .child(
            Button::new()
                .secondary()
                .small()
                .alt(text)
                .on_press(move |_| create_theme(&create))
                .child(Icon::new(IconType::Plus).size(14.))
                .text(text),
        )
        .child(import_button(dispatch))
        .into_element()
}

fn import_button(dispatch: &Actions) -> Element {
    let dispatch = dispatch.clone();
    let text = t("oneclient.settings.appearance.theme.import");
    Button::new()
        .secondary()
        .small()
        .alt(text)
        .on_press(move |_| {
            let dispatch = dispatch.clone();
            spawn_in_window(async move {
                let Some(file) = rfd::AsyncFileDialog::new()
                    .set_title(t("oneclient.settings.appearance.theme.import_title"))
                    .add_filter("JSON", &["json"])
                    .pick_file()
                    .await
                else {
                    return;
                };
                match import_theme(file.path()).await {
                    Ok(id) => select_family(&dispatch, &id),
                    Err(err) => notify_failure(
                        &dispatch,
                        t("oneclient.settings.appearance.theme.import_failed"),
                        err,
                    ),
                }
            });
        })
        .child(Icon::new(IconType::FilePlus02).size(14.))
        .text(text)
        .into_element()
}

/// The kebab menu on a style card; custom families can also be edited and deleted
#[derive(PartialEq)]
pub(super) struct ThemeFileMenu {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) custom: bool,
    /// Owned by the card so a right-click on it opens this menu too
    pub(super) menu_at: State<Option<(f32, f32)>>,
}

impl Component for ThemeFileMenu {
    fn render(&self) -> impl IntoElement {
        let mut anchor = use_state(|| (0., 0.));
        let mut menu_at = self.menu_at;
        let mut deleting = use_state(|| false);
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
                    .secondary()
                    .icon()
                    .width(Size::px(26.))
                    .height(Size::px(26.))
                    .alt(alt.clone())
                    .tooltip(alt)
                    .on_press(move |_| menu_at.set(Some(*anchor.peek())))
                    .child(Icon::new(IconType::DotsVertical).size(14.)),
            )
            .maybe_child((*menu_at.read()).map(|(x, y)| {
                let mut menu = ContextMenu::new(x, y).title(self.name.clone());
                if self.custom {
                    let id = self.id.clone();
                    menu = menu.action(
                        IconType::Pencil01,
                        t("oneclient.settings.appearance.theme.edit"),
                        move |()| open_editor(id.clone()),
                    );
                }
                menu = theme_file_actions(menu, &self.id, &dispatch);
                if self.custom {
                    menu = menu.separator().danger_action(
                        IconType::Trash01,
                        t("oneclient.common.delete"),
                        move |()| deleting.set(true),
                    );
                }
                menu.on_close(move |()| menu_at.set(None))
            }))
            .maybe_child((*deleting.read()).then(|| {
                let id = self.id.clone();
                let dispatch = dispatch.clone();
                confirm_dialog(
                    tf(
                        "oneclient.settings.appearance.theme.delete_confirm.title",
                        &[&self.name],
                    ),
                    t("oneclient.settings.appearance.theme.delete_confirm.body"),
                    t("oneclient.common.delete"),
                    true,
                    (move |()| deleting.set(false)).into(),
                    (move |()| {
                        deleting.set(false);
                        delete_family(&id, &dispatch);
                    })
                    .into(),
                )
            }))
    }
}

fn delete_family(id: &str, dispatch: &Actions) {
    let id = id.to_owned();
    let dispatch = dispatch.clone();
    spawn_in_window(async move {
        // Capture the ids that name this family before deletion, then repair only
        // the current selection if it actually became invalid.
        let affected = family_ids(&id);
        match delete_theme(&id).await {
            Ok(()) => dispatch.edit_settings(|settings| {
                if theme_needs_fallback(&settings.theme, &affected, is_known) {
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
}

/// Every setting value that selects the family `id` belongs to
fn family_ids(id: &str) -> Vec<String> {
    let variants = family_variants(id);
    variants
        .iter()
        .map(|theme| theme.id.clone())
        .chain(variants.first().map(|dark| system_theme_id(&dark.id)))
        .collect()
}

fn theme_needs_fallback(
    current: &str,
    affected: &[String],
    known: impl FnOnce(&str) -> bool,
) -> bool {
    affected.iter().any(|affected| affected == current) && !known(current)
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
        keywords: &["theme", "dark", "light", "mode", "colors", "custom", "import"],
        route: Route::SettingsAppearance {},
    }
}

#[cfg(test)]
mod tests {
    use super::theme_needs_fallback;

    #[test]
    fn deleting_a_theme_repairs_only_the_current_invalid_selection() {
        let affected = [
            "custom-dark".to_string(),
            "custom-light".to_string(),
            "system:custom-dark".to_string(),
        ];

        let system = "system:custom-dark";
        assert!(theme_needs_fallback("custom-light", &affected, |_| false));
        assert!(theme_needs_fallback(system, &affected, |_| false));
        assert!(!theme_needs_fallback(system, &affected, |_| true));
        assert!(!theme_needs_fallback("system:other", &affected, |_| false));
    }
}
