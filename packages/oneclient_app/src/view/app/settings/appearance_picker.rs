//! Style and theme (System / Dark / Light) pickers, shared by the
//! onboarding Appearance step and Settings > Appearance so the two stay identical

use std::{cell::RefCell, rc::Rc};

use accesskit::Toggled;
use freya::prelude::*;

use super::appearance::ThemeFileMenu;
use crate::AppAssets;
use crate::components::{Icon, IconType};
use crate::hooks::{Actions, use_dispatch};
use crate::i18n::t;
use crate::theme::{
    Theme, colors, counterpart, family_name, glass, light_variant, resolve_theme, rounded,
    rounded_px, system_pair, system_theme_id, text_size, themes,
};
use crate::ui::border_all_color;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    System,
    Dark,
    Light,
}

#[derive(Clone, PartialEq)]
pub(super) struct RadioGroup {
    options: Rc<[(AccessibilityId, bool)]>,
}

impl RadioGroup {
    pub(super) fn use_options(available: impl IntoIterator<Item = bool>) -> Self {
        let ids = use_hook(|| Rc::new(RefCell::new(Vec::new())));
        let mut ids = ids.borrow_mut();
        let options = available
            .into_iter()
            .enumerate()
            .map(|(index, available)| {
                if index == ids.len() {
                    ids.push(AccessibilityId::new_unique());
                }
                (ids[index], available)
            })
            .collect();
        Self { options }
    }

    pub(super) fn id(&self, index: usize) -> AccessibilityId {
        self.options[index].0
    }

    pub(super) fn accessibility(
        &self,
        element: Rect,
        index: usize,
        selected: Option<usize>,
    ) -> Rect {
        let available = self.options[index].1;
        let tab_stop = selected
            .filter(|&i| self.options[i].1)
            .or_else(|| self.options.iter().position(|&(_, available)| available));
        element
            .a11y_id(self.id(index))
            .a11y_role(AccessibilityRole::RadioButton)
            .a11y_focusable(available)
            .a11y_builder(|node| {
                node.set_toggled(Toggled::from(selected == Some(index)));
                if !available {
                    node.set_disabled();
                }
            })
            // Freya includes only the member whose ID matches the group in Tab
            // traversal, but assistive technology can still focus any enabled radio.
            .map(tab_stop, |element, stop| {
                element.a11y_member_of(self.id(stop))
            })
    }

    pub(super) fn activate(&self, index: usize, select: impl FnOnce(usize)) {
        if self.options[index].1 {
            self.id(index).request_focus();
            select(index);
        }
    }

    /// Apply after `on_press` because Freya stores one handler per event.
    pub(super) fn keyboard(
        self,
        element: Rect,
        index: usize,
        select: impl Fn(usize) + 'static,
    ) -> Rect {
        element.on_key_down(move |event: Event<KeyboardEventData>| {
            if event.is_press_event() {
                self.activate(index, &select);
                return;
            }
            if !event.modifiers.is_empty() {
                return;
            }
            let forward = match event.key {
                Key::Named(NamedKey::ArrowRight | NamedKey::ArrowDown) => true,
                Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowUp) => false,
                _ => return,
            };
            event.prevent_default();
            event.stop_propagation();
            if let Some(next) =
                next_radio(index, self.options.len(), forward, |i| self.options[i].1)
            {
                self.activate(next, &select);
            }
        })
    }
}

fn next_radio(
    index: usize,
    count: usize,
    forward: bool,
    available: impl Fn(usize) -> bool,
) -> Option<usize> {
    (1..count)
        .map(|step| {
            if forward {
                (index + step) % count
            } else {
                (index + count - step) % count
            }
        })
        .find(|&next| available(next))
}

pub(super) fn radio_focus_visible(focus: Focus, selected: bool) -> bool {
    focus == Focus::Keyboard || (focus.is_focused() && !selected)
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

fn compose_theme(family: &str, mode: Mode) -> Option<String> {
    id_for_mode(
        family,
        resolve_theme(family).colors.light,
        light_variant(family),
        mode,
    )
}

fn id_for_mode(
    family: &str,
    light_only: bool,
    light_variant: Option<String>,
    mode: Mode,
) -> Option<String> {
    match mode {
        Mode::System => light_variant.map(|_| system_theme_id(family)),
        Mode::Dark => (!light_only).then(|| family.to_string()),
        Mode::Light if light_only => Some(family.to_string()),
        Mode::Light => light_variant,
    }
}

/// Built-in families that have both a dark and a light variant.
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
///
/// Settings also exposes the file actions for each displayed preview.
pub(crate) fn style_picker(theme_id: &str, dense: bool, manage: bool) -> impl IntoElement {
    let (family, mode) = split_theme(theme_id);
    // System previews follow whichever variant the OS has the launcher showing right now
    let show_light = match mode {
        Mode::Light => true,
        Mode::Dark => false,
        Mode::System => colors::is_light(),
    };

    let styles = style_families();
    let selected_index = styles.iter().position(|theme| theme.id == family);
    let families: Rc<[String]> = styles.iter().map(|theme| theme.id.clone()).collect();
    let radio = RadioGroup::use_options(styles.iter().map(|_| true));
    let cards = styles.into_iter().enumerate().map(|(index, dark)| {
        let preview = show_light
            .then(|| light_variant(&dark.id))
            .flatten()
            .unwrap_or_else(|| dark.id.clone());
        StyleCard {
            selected_index,
            index,
            families: families.clone(),
            radio: radio.clone(),
            preview,
            dense,
            manage,
        }
        .into_element()
    });

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .a11y_role(AccessibilityRole::RadioGroup)
        .a11y_alt(t("oneclient.settings.appearance.style"))
        .spacing(if dense { 10. } else { 12. })
        .children(cards)
}

/// Handlers read the live setting at press time instead of capturing this render's picks: a
/// press can reach a closure from an earlier render, which would apply a stale combination
pub(crate) fn theme_mode_picker(theme_id: &str, dispatch: &Actions) -> impl IntoElement {
    const MODES: [Mode; 3] = [Mode::System, Mode::Dark, Mode::Light];
    const HEIGHT: f32 = 34.;
    let (family, mode) = split_theme(theme_id);
    let choices = MODES.map(|option| compose_theme(&family, option).is_some());
    let radio = RadioGroup::use_options(choices);
    let can_switch = choices.iter().filter(|&&available| available).count() > 1;
    let theme = colors::active_theme();
    let glass = theme.liquid_glass;
    let track = glass::use_pill_track(3);
    let selected = MODES.iter().position(|&option| option == mode);
    let indicator = glass.then(|| track.indicator(selected));
    let select = {
        let dispatch = dispatch.clone();
        move |index: usize| {
            dispatch.edit_settings(|settings| {
                let (family, _) = split_theme(&settings.theme);
                if let Some(id) = compose_theme(&family, MODES[index]) {
                    settings.theme = id;
                }
            });
        }
    };

    let segments = [
        t("oneclient.settings.appearance.theme.system"),
        t("oneclient.settings.appearance.theme.dark"),
        t("oneclient.settings.appearance.theme.light"),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, text)| {
        let select = select.clone();
        let key_select = select.clone();
        let radio = radio.clone();
        let pointer_radio = radio.clone();
        let available = choices[index];
        let active = selected == Some(index);
        let lit = if glass {
            track.highlights(index, active)
        } else {
            active
        };
        let a11y_id = radio.id(index);
        let focus = use_focus(a11y_id);
        let segment = rect()
            .width(Size::flex(1.))
            .height(Size::px(HEIGHT))
            .center()
            .corner_radius(if glass {
                CornerRadius::new_all(HEIGHT / 2.)
            } else {
                rounded(7.)
            });
        radio
            .accessibility(segment, index, selected)
            .maybe(active && !glass, |el| el.background(colors::brand()))
            .a11y_alt(text)
            .maybe(radio_focus_visible(focus(), active), |el| {
                let stroke = if active && !glass {
                    colors::on_brand()
                } else {
                    colors::fg_primary()
                };
                el.border(border_all_color(2., stroke).alignment(BorderAlignment::Inner))
            })
            .maybe(available, |el| {
                radio.keyboard(
                    el.cursor(CursorIcon::Pointer)
                        .on_press(move |event: Event<PressEventData>| {
                            if !glass
                                || !can_switch
                                || !matches!(
                                    event.data(),
                                    PressEventData::Mouse(_) | PressEventData::Touch(_)
                                )
                            {
                                pointer_radio.activate(index, &select);
                            }
                        }),
                    index,
                    key_select,
                )
            })
            .maybe(glass, |el| track.option(el, index, available && can_switch))
            .child(
                label()
                    .text(text.to_string())
                    .font_size(text_size(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .color(if !available {
                        colors::fg_secondary().with_a(138)
                    } else if active && !glass {
                        colors::on_brand()
                    } else if lit {
                        colors::fg_primary()
                    } else {
                        colors::fg_primary().with_a(190)
                    }),
            )
            .into_element()
    });

    rect()
        .background(if glass {
            colors::component_bg().with_a(148)
        } else {
            colors::component_bg()
        })
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .a11y_role(AccessibilityRole::RadioGroup)
        .a11y_alt(t("oneclient.settings.appearance.mode"))
        .spacing(4.)
        .padding(4.)
        .corner_radius(if glass {
            CornerRadius::new_all(21.)
        } else {
            rounded(10.)
        })
        .border(border_all_color(1., quiet_outline()))
        .maybe(glass && can_switch, |el| {
            let radio = radio.clone();
            let select = select.clone();
            track.events(el.overflow(Overflow::Clip), move |index| {
                radio.activate(index, &select);
            })
        })
        .maybe_child(indicator)
        .children(segments)
}

const CARD_RING: f32 = 2.;
const PREVIEW_HEIGHT: f32 = 104.;
const PREVIEW_HEIGHT_DENSE: f32 = 64.;
/// Fixed so the fonts can't give the cards different heights and shift the page; tall enough
/// for the name over a two-line blurb with air above and below
const FOOTER_HEIGHT: f32 = 76.;
const FOOTER_HEIGHT_DENSE: f32 = 40.;
const FOOTER_PAD_X: f32 = 12.;

/// A card per style: a miniature launcher drawn in that style's own colors, font, corners and
/// icons, above its name, a short description and a selection mark
#[derive(PartialEq)]
struct StyleCard {
    index: usize,
    /// The dark id for each displayed style family.
    families: Rc<[String]>,
    radio: RadioGroup,
    /// Variant the miniature is drawn with
    preview: String,
    selected_index: Option<usize>,
    dense: bool,
    manage: bool,
}

impl Component for StyleCard {
    fn render(&self) -> impl IntoElement {
        let theme = resolve_theme(&self.preview);
        let theme: &Theme = &theme;
        let (selected, dense) = (self.selected_index == Some(self.index), self.dense);
        let dispatch = use_dispatch();
        let index = self.index;
        let radio = self.radio.clone();
        let pointer_radio = radio.clone();
        let a11y_id = radio.id(index);
        let focus = use_focus(a11y_id);
        let show_focus = radio_focus_visible(focus(), selected);
        let ringed = selected || show_focus;

        let pixel = theme.pixel_font_size.is_some_and(|step| step > 0.);
        let blurb = if pixel {
            t("oneclient.onboarding.preferences.style.minecraft")
        } else if theme.liquid_glass {
            t("oneclient.onboarding.preferences.style.polyglass")
        } else {
            t("oneclient.onboarding.preferences.style.polyfrost")
        };
        let name = family_name(&theme.name);

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
            .padding(Gaps::new_symmetric(0., FOOTER_PAD_X))
            .background(if selected {
                colors::brand().with_a(40)
            } else {
                Color::TRANSPARENT
            })
            .child(
                // Sized by its text and centred by the row, a fixed height here let a wrapped
                // blurb spill onto the card edge
                rect()
                    .vertical()
                    .width(Size::flex(1.))
                    .overflow(Overflow::Clip)
                    .spacing(2.)
                    .child(
                        label()
                            .text(name.clone())
                            .width(Size::fill())
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .font_size(text_size(13.))
                            .font_weight(FontWeight::SEMI_BOLD)
                            .color(colors::fg_primary()),
                    )
                    .maybe_child((!dense).then(|| {
                        label()
                            .text(blurb.to_string())
                            .width(Size::fill())
                            .max_lines(2)
                            .line_height(1.3)
                            .text_overflow(TextOverflow::Ellipsis)
                            .font_size(text_size(11.))
                            .color(colors::fg_secondary())
                            .into_element()
                    })),
            )
            .child(selection_mark(selected));

        // Children paint over a border, so the rim sits outside a clipped inner body. The body
        // always starts right where the rim ends: a thin rim is inset by a margin instead of
        // being left floating a pixel off the body, which read as a doubled outline
        let rim = if ringed { CARD_RING } else { 1. };
        let outer_radius = rounded_px(12.);
        let families = self.families.clone();
        let select = move |picked: usize| {
            dispatch.edit_settings(|settings| {
                let (_, mode) = split_theme(&settings.theme);
                if let Some(id) = compose_theme(&families[picked], mode) {
                    settings.theme = id;
                }
            });
        };
        let key_select = select.clone();

        let card = rect()
            .width(Size::fill())
            .margin(Gaps::new_all(CARD_RING - rim))
            .padding(rim)
            .corner_radius(CornerRadius::new_all(outer_radius - (CARD_RING - rim)))
            .border(
                border_all_color(
                    rim,
                    if show_focus {
                        colors::fg_primary()
                    } else if selected {
                        colors::brand()
                    } else {
                        quiet_outline()
                    },
                )
                .alignment(BorderAlignment::Inner),
            )
            .cursor(CursorIcon::Pointer)
            .a11y_alt(name)
            .on_press(move |_| pointer_radio.activate(index, &select))
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .corner_radius(CornerRadius::new_all((outer_radius - CARD_RING).max(0.)))
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
            );

        let card = radio.accessibility(card, index, self.selected_index);
        rect()
            .width(Size::flex(1.))
            .child(radio.keyboard(card, index, key_select))
            .maybe_child(self.manage.then(|| ThemeFileMenu {
                id: theme.id.clone(),
                name: theme.name.clone(),
                custom: None,
            }))
    }
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
    // The pixel font draws smaller at the same point size.
    let (title_size, body_size, meta_size) = if pixel {
        (14., 13., 11.)
    } else {
        (13., 12., 10.)
    };

    let header_ink = palette.text.0;
    let top_bar = rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(6.)
        .child(theme_icon(theme, "icon-logo", 13., header_ink))
        .child(
            text("OneClient", title_size, header_ink).font_weight(if pixel {
                FontWeight::NORMAL
            } else {
                FontWeight::BOLD
            }),
        );

    let instance =
        glass::panel(
            rect().corner_radius(radius(8.)),
            theme,
            8. * theme.radius_scale,
            palette.card.0,
        )
        .horizontal()
        .width(Size::fill())
        .height(Size::flex(1.))
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(6.)
        // Keeps the labels and the Play button off the panel's rounded edge
        .padding(Gaps::new_symmetric(0., if dense { 7. } else { 9. }))
        .maybe(!theme.liquid_glass, |el| {
            el.border(border_all_color(1., palette.border.0))
        })
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.))
                .overflow(Overflow::Clip)
                .spacing(if pixel { 1. } else { 2. })
                .child(text("Survival", body_size, palette.text.0))
                .maybe_child((!dense).then(|| {
                    text("26.1 Fabric", meta_size, palette.text_secondary.0).into_element()
                })),
        )
        .child(play_button(theme, pixel, dense));

    rect()
        .background(palette.page.0)
        .width(Size::fill())
        .height(Size::px(if dense {
            PREVIEW_HEIGHT_DENSE
        } else {
            PREVIEW_HEIGHT
        }))
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .height(Size::fill())
                .content(Content::Flex)
                .padding(if dense { 7. } else { 9. })
                .spacing(if dense { 6. } else { 8. })
                .child(top_bar)
                .child(instance),
        )
        .into_element()
}

/// Same accent button the launcher draws; the theme's corner scale squares it off for Minecraft
fn play_button(theme: &Theme, pixel: bool, dense: bool) -> Element {
    let radius = 7. * theme.radius_scale;
    rect()
        .corner_radius(CornerRadius::new_all(radius))
        .background(theme.colors.accent.0)
        .width(Size::px(42.))
        .height(Size::px(if dense { 20. } else { 26. }))
        .center()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpaired_themes_only_offer_their_installed_mode() {
        for (light_only, own_mode) in [(false, Mode::Dark), (true, Mode::Light)] {
            for mode in [Mode::Dark, Mode::Light, Mode::System] {
                assert_eq!(
                    id_for_mode("custom", light_only, None, mode),
                    (mode == own_mode).then(|| "custom".to_string()),
                );
            }
        }
    }

    #[test]
    fn paired_themes_offer_each_mode() {
        for (mode, id) in [
            (Mode::Dark, "custom-dark"),
            (Mode::Light, "custom-light"),
            (Mode::System, "system:custom-dark"),
        ] {
            assert_eq!(
                id_for_mode("custom-dark", false, Some("custom-light".into()), mode).as_deref(),
                Some(id),
            );
        }
    }

    #[test]
    fn arrows_wrap_and_skip_unavailable_radio_options() {
        let choices = [true, false, true, false];
        let available = |i: usize| choices[i];
        assert_eq!(next_radio(0, choices.len(), true, available), Some(2));
        assert_eq!(next_radio(0, choices.len(), false, available), Some(2));
        assert_eq!(next_radio(2, choices.len(), true, available), Some(0));
        assert_eq!(next_radio(2, choices.len(), false, available), Some(0));
        assert_eq!(next_radio(0, 3, true, |i| i == 0), None);
    }

    #[test]
    fn radio_keyboard_focus_roves_and_preserves_activation() {
        use std::time::Duration;

        use accesskit::{Action, ActionRequest, TreeId};
        use freya_testing::TestingRunner;

        type Fixture = (
            State<Option<usize>>,
            State<Vec<AccessibilityId>>,
            State<bool>,
        );

        fn app() -> impl IntoElement {
            crate::hooks::use_provide_station(|| {
                let mut app = crate::state::AppState::default();
                app.settings.settings.animations_enabled = false;
                app
            });
            let (selected, ids, glass) = use_consume::<Fixture>();
            let radio = RadioGroup::use_options([true, false, true]);
            let drag_radio = radio.clone();
            let track = glass::use_pill_track(3);
            let kebab = use_a11y();
            use_hook(|| {
                let mut ids = ids;
                ids.set((0..3).map(|i| radio.id(i)).chain([kebab]).collect());
            });
            let select = move |index| {
                let mut selected = selected;
                selected.set(Some(index));
            };
            let checked = selected();
            let glass = glass();
            rect()
                .horizontal()
                .on_global_key_down(|event: Event<KeyboardEventData>| {
                    assert_ne!(event.key, Key::Named(NamedKey::ArrowDown));
                })
                .child(
                    rect()
                        .horizontal()
                        .a11y_role(AccessibilityRole::RadioGroup)
                        .maybe(glass, |el| {
                            track.events(el, move |index| drag_radio.activate(index, select))
                        })
                        .children((0..3).map(|index| {
                            let radio_button = radio
                                .accessibility(
                                    rect().width(Size::px(32.)).height(Size::px(24.)),
                                    index,
                                    checked,
                                )
                                .maybe(glass, |el| track.option(el, index, index != 1));
                            if index == 1 {
                                return radio_button;
                            }
                            let pointer_radio = radio.clone();
                            radio.clone().keyboard(
                                radio_button.on_press(move |event: Event<PressEventData>| {
                                    if !glass
                                        || matches!(event.data(), PressEventData::Accessibility(_))
                                    {
                                        pointer_radio.activate(index, select);
                                    }
                                }),
                                index,
                                select,
                            )
                        })),
                )
                .child(
                    rect()
                        .width(Size::px(28.))
                        .height(Size::px(24.))
                        .a11y_id(kebab)
                        .a11y_role(AccessibilityRole::Button)
                        .a11y_alt("Theme options")
                        .a11y_focusable(true),
                )
        }

        let (mut runner, (selected, ids, mut glass)) = TestingRunner::new(
            app,
            (240., 160.).into(),
            |runner| {
                runner.provide_root_context(|| -> Fixture {
                    (
                        State::create(None),
                        State::create(vec![]),
                        State::create(false),
                    )
                })
            },
            1.,
        );
        runner.poll_n(Duration::ZERO, 3);
        let focus_next = |runner: &mut TestingRunner| {
            runner.run_in(|| {
                Platform::get()
                    .current_window()
                    .send(UserEvent::FocusAccessibilityNode(
                        AccessibilityFocusStrategy::Forward(
                            AccessibilityFocusMovement::OutsideGroup,
                        ),
                    ));
            });
            runner.sync_and_update();
        };
        let assert_focus = |runner: &TestingRunner, index: usize| {
            runner.run_in(|| assert!(ids.peek()[index].is_focused()));
        };
        let press = |runner: &mut TestingRunner, key| {
            runner.press_key(key);
            runner.poll_n(Duration::ZERO, 3);
        };
        let request_at = |runner: &mut TestingRunner, action, index: usize| {
            let target_node = runner.run_in(|| ids.peek()[index]);
            let accepted = runner.send_accessibility_action(ActionRequest {
                action,
                target_node,
                target_tree: TreeId::ROOT,
                data: None,
            });
            runner.poll_n(Duration::ZERO, 3);
            accepted
        };

        focus_next(&mut runner);
        assert_focus(&runner, 0);
        press(&mut runner, Key::Named(NamedKey::Enter));
        runner.run_in(|| assert_eq!(*selected.peek(), Some(0)));

        press(&mut runner, Key::Named(NamedKey::ArrowRight));
        assert_focus(&runner, 2);
        runner.run_in(|| assert_eq!(*selected.peek(), Some(2)));
        focus_next(&mut runner);
        assert_focus(&runner, 3);
        focus_next(&mut runner);
        assert_focus(&runner, 2);

        press(&mut runner, Key::Named(NamedKey::ArrowDown));
        assert_focus(&runner, 0);
        runner.run_in(|| assert_eq!(*selected.peek(), Some(0)));

        runner.click_cursor((80., 12.));
        runner.poll_n(Duration::ZERO, 3);
        assert_focus(&runner, 2);
        runner.run_in(|| assert_eq!(*selected.peek(), Some(2)));
        press(&mut runner, Key::Named(NamedKey::ArrowLeft));
        assert_focus(&runner, 0);
        runner.run_in(|| assert_eq!(*selected.peek(), Some(0)));

        assert!(!request_at(&mut runner, Action::Focus, 1));
        assert!(request_at(&mut runner, Action::Focus, 2));
        assert_focus(&runner, 2);
        runner.run_in(|| assert_eq!(*selected.peek(), Some(0)));
        press(&mut runner, Key::Character(" ".into()));
        runner.run_in(|| assert_eq!(*selected.peek(), Some(2)));
        assert!(request_at(&mut runner, Action::Click, 0));
        assert_focus(&runner, 0);
        runner.run_in(|| assert_eq!(*selected.peek(), Some(0)));

        runner.run_in(|| glass.set(true));
        runner.poll_n(Duration::ZERO, 3);
        runner.press_cursor((16., 12.));
        runner.move_cursor((80., 12.));
        runner.release_cursor((80., 12.));
        runner.poll_n(Duration::ZERO, 3);
        assert_focus(&runner, 2);
        runner.run_in(|| assert_eq!(*selected.peek(), Some(2)));
    }
}
