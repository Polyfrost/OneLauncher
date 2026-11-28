use std::time::Duration;

use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::settings::LauncherSettings;

use super::settings_page;
use crate::components::{
    Button, ColorPicker, ContextMenu, Icon, IconType, TooltipPlacement, toggle, use_tooltip_anchor,
};
use crate::hooks::{Actions, use_debounced, use_dispatch, use_settings_snapshot};
use crate::i18n::{t, tf};
use crate::layout::SearchItem;
use crate::routes::Route;
use crate::theme::{
    Theme, colors, copy_family, delete_theme, duplicate_theme, export_theme, family_variants,
    glass, import_theme, is_known, system_theme_id, text_size,
};
use crate::ui::border_all_color;
use crate::view::app::settings::appearance_picker::{
    compose_theme, section, section_with_action, split_theme, style_picker, theme_icon,
    theme_mode_picker,
};
use crate::view::app::settings::{
    confirm_dialog, readable_secondary, resettable, section_header, settings_row,
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
                rect()
                    .vertical()
                    .width(Size::fill())
                    .spacing(20.)
                    .padding(Gaps::new(0., 0., 12., 0.))
                    .child(section(
                        t("oneclient.settings.appearance.mode"),
                        theme_mode_picker(&settings.theme, &dispatch),
                    ))
                    .child(section_with_action(
                        t("oneclient.settings.appearance.style"),
                        Some(theme_actions(&dispatch)),
                        style_picker(&settings.theme, dense, true),
                    )),
            ))
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
    let a11y_id = use_a11y();
    let keyboard = use_focus(a11y_id)() == Focus::Keyboard;
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
                .a11y_id(a11y_id)
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
                        .maybe(*hovered.read() || keyboard, |el| {
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

pub(super) const KEBAB_SIZE: f32 = 22.;

/// The kebab itself, in the previewed theme
#[derive(PartialEq)]
struct ThemeFileButton {
    alt: String,
    preview: Theme,
    on_press: EventHandler<()>,
}

impl Component for ThemeFileButton {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let alt = self.alt.clone();
        let on_press = self.on_press.clone();
        let a11y_id = use_a11y();
        let keyboard = use_focus(a11y_id)() == Focus::Keyboard;
        let tooltip = use_tooltip_anchor(Some(alt.clone().into()), TooltipPlacement::Top);
        let preview = &self.preview;
        let palette = preview.colors;
        let radius = 6. * preview.radius_scale;
        let face = rect().corner_radius(CornerRadius::new_all(radius));
        let face = if preview.liquid_glass {
            glass::panel(face, preview, radius, palette.card.0)
        } else {
            face.background(palette.card.0)
                .border(border_all_color(1., palette.border.0))
        };
        let face = face
            .width(Size::px(KEBAB_SIZE))
            .height(Size::px(KEBAB_SIZE))
            .center()
            .maybe(*hovered.read(), |el| {
                el.background(palette.text.0.with_a(28))
            })
            .maybe(keyboard, |el| {
                el.border(border_all_color(1.5, palette.text.0))
            })
            .a11y_id(a11y_id)
            .a11y_role(AccessibilityRole::Button)
            .a11y_focusable(true)
            .a11y_alt(alt)
            .cursor(CursorIcon::Pointer)
            // The tooltip owns pointer-enter and pointer-leave on this rect
            .on_pointer_over(move |_| hovered.set_if_modified(true))
            .on_pointer_out(move |_| hovered.set_if_modified(false))
            .on_press(move |_| on_press.call(()))
            .child(theme_icon(preview, "dots-vertical", 14., palette.text.0));
        tooltip.attach(face)
    }
}

/// The kebab menu on a style card; custom families can also be edited and deleted
#[derive(PartialEq)]
pub(super) struct ThemeFileMenu {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) custom: bool,
    /// Owned by the card so a right-click on it opens this menu too
    pub(super) menu_at: State<Option<(f32, f32)>>,
    /// The theme the card previews; the button is drawn in it like the card's Play button,
    /// so every card keeps its own look whichever style is applied
    pub(super) preview: Theme,
    /// From the card's edges to the preview's content, so the button lines up with its panel
    pub(super) inset: f32,
}

impl Component for ThemeFileMenu {
    fn render(&self) -> impl IntoElement {
        let mut anchor = use_state(|| (0., 0.));
        let mut menu_at = self.menu_at;
        let mut deleting = use_state(|| false);
        let dispatch = use_dispatch();
        let alt = tf("oneclient.settings.appearance.theme.actions", &[&self.name]);
        let face = ThemeFileButton {
            alt,
            preview: self.preview.clone(),
            on_press: (move |()| menu_at.set(Some(*anchor.peek()))).into(),
        };

        rect()
            .position(Position::new_absolute().top(self.inset).right(self.inset))
            .layer(Layer::Relative(3))
            .on_sized(move |event: Event<SizedEventData>| {
                let frame = event.data().area;
                anchor.set_if_modified((frame.min_x(), frame.max_y()));
            })
            .child(face)
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
            Ok(()) => {
                for id in &affected {
                    super::theme_editor::clear_draft(id);
                }
                dispatch.edit_settings(|settings| {
                    if theme_needs_fallback(&settings.theme, &affected, is_known) {
                        settings.theme = "dark".into();
                    }
                });
            }
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

#[cfg(test)]
mod kebab_tests {
    use super::*;
    use freya_testing::TestingRunner;
    use std::time::Duration;

    #[tokio::test]
    async fn the_kebab_wears_its_cards_theme_tints_on_hover_and_opens_the_menu() {
        type Fixture = (State<Option<(f32, f32)>>, State<bool>);

        fn app() -> impl IntoElement {
            crate::hooks::use_provide_station(|| {
                let mut app = crate::state::AppState::default();
                app.settings.settings.animations_enabled = false;
                app
            });
            crate::components::use_provide_tooltips();
            let (menu_at, minecraft) = use_consume::<Fixture>();
            // The active theme changes; the previewed one doesn't
            colors::apply(
                if minecraft() {
                    "minecraft-dark"
                } else {
                    "dark"
                },
                None,
            );
            rect()
                .width(Size::px(200.))
                .height(Size::px(200.))
                .child(ThemeFileButton {
                    alt: format!("Options {}", minecraft()),
                    preview: (*crate::theme::resolve_theme("dark")).clone(),
                    on_press: (move |()| {
                        let mut menu_at = menu_at;
                        menu_at.set(Some((0., 0.)));
                    })
                    .into(),
                })
        }
        let (mut runner, (menu_at, mut minecraft)) = TestingRunner::new(
            app,
            (200., 200.).into(),
            |runner| {
                runner.provide_root_context(|| -> Fixture {
                    (State::create(None), State::create(false))
                })
            },
            1.,
        );
        let face = |runner: &TestingRunner| {
            runner
                .find(|node, element| {
                    let a11y = element.accessibility();
                    (a11y.builder.role() == AccessibilityRole::Button).then(|| {
                        let style = element.style();
                        (
                            style.background.clone(),
                            style.corner_radius,
                            node.layout().area.center(),
                        )
                    })
                })
                .unwrap()
        };
        runner.poll_n(Duration::ZERO, 4);
        let (idle, radius, center) = face(&runner);
        runner.run_in(|| minecraft.set(true));
        runner.poll_n(Duration::ZERO, 4);
        assert_eq!((face(&runner).0, face(&runner).1), (idle.clone(), radius));

        let at = (f64::from(center.x), f64::from(center.y));
        runner.move_cursor(at);
        runner.poll_n(Duration::ZERO, 4);
        assert_ne!(face(&runner).0, idle);
        runner.move_cursor((20., 180.));
        runner.poll_n(Duration::ZERO, 4);
        assert_eq!(face(&runner).0, idle);
        runner.click_cursor(at);
        runner.run_in(|| assert!(menu_at.peek().is_some()));
        colors::apply("dark", None);
    }
}
