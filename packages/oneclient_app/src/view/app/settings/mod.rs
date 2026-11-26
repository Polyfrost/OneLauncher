mod accounts;
mod appearance;
pub(crate) mod appearance_picker;

mod apis;
mod changelog;
mod developer;
mod java;
mod language;
mod launcher;
mod minecraft;
mod storage;
mod theme_editor;

use freya::prelude::*;

pub use accounts::SettingsAccounts;
pub use apis::SettingsApis;
pub use appearance::SettingsAppearance;
pub use changelog::SettingsChangelog;
pub use developer::SettingsDeveloper;
pub use java::SettingsJava;
pub use language::SettingsLanguage;
pub use launcher::SettingsLauncher;
pub use minecraft::SettingsMinecraft;
pub use storage::SettingsStorage;
pub use theme_editor::SettingsThemeEditor;

use crate::{
    components::{Button, Icon, IconType, ScrollArea},
    i18n::t,
    theme::{colors, rounded, rounded_px, text_size},
    ui::border_all_color,
};

pub(crate) const PAGE_GAP: f32 = 6.;
const ROW_INSET: f32 = 16.;
const ROW_GAP: f32 = 16.;
const ROW_ICON: f32 = 24.;
const MIN_COPY_WIDTH: f32 = 160.;
/// Dropdowns and pickers on settings rows: 28% of the page, between these widths
const CONTROL_MIN_WIDTH: f32 = 160.;
const CONTROL_MAX_WIDTH: f32 = 220.;
const CONTROL_SHARE: f32 = 0.28;
/// WCAG AA for body text
const READABLE_CONTRAST: f32 = 4.5;
/// Disabled rows dim their title and control, never the copy that explains why
const DISABLED_OPACITY: f32 = 0.45;

fn control_width_for(pane_width: f32) -> f32 {
    (CONTROL_SHARE * pane_width).clamp(CONTROL_MIN_WIDTH, CONTROL_MAX_WIDTH)
}

/// Width for a dropdown, picker or field on a settings row, in Launcher and Instance Settings
pub(crate) fn settings_control_width() -> Size {
    Size::px(control_width_for(
        crate::layout::ClusterShell::workspace_pane_size().0,
    ))
}

fn channel_luminance(channel: u8) -> f32 {
    let c = f32::from(channel) / 255.;
    if c <= 0.039_28 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(color: Color) -> f32 {
    0.2126 * channel_luminance(color.r())
        + 0.7152 * channel_luminance(color.g())
        + 0.0722 * channel_luminance(color.b())
}

fn contrast(a: Color, b: Color) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn mix(from: Color, to: Color, amount: f32) -> Color {
    let channel =
        |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * amount).round() as u8;
    Color::from_rgb(
        channel(from.r(), to.r()),
        channel(from.g(), to.g()),
        channel(from.b(), to.b()),
    )
}

/// `secondary` moved toward `primary` just far enough to reach 4.5:1 on `surface`
fn readable_on(secondary: Color, primary: Color, surface: Color) -> Color {
    let mut amount = 0.;
    let mut ink = secondary;
    while contrast(ink, surface) < READABLE_CONTRAST && amount < 1. {
        amount = (amount + 0.05_f32).min(1.);
        ink = mix(secondary, primary, amount);
    }
    ink
}

/// Secondary text that stays readable on settings rows and the dialog surface (about #8B929F in
/// Polyfrost Dark). Themes whose secondary ink already passes keep it.
pub(crate) fn readable_secondary() -> Color {
    let secondary = colors::fg_secondary();
    let primary = colors::fg_primary();
    let on_card = readable_on(secondary, primary, colors::card());
    readable_on(on_card, primary, colors::page_elevated())
}

/// The fade over the cut-off row of a settings page or rail that scrolls
pub(crate) const SCROLL_FADE_HEIGHT: f32 = 24.;

/// A settings page's scroll area. It reaches the dialog's right edge so its thumb sits in the
/// right inset, stays visible while the page overflows, and fades out the cut-off row.
pub(crate) fn settings_pane() -> ScrollArea {
    let pane = ScrollArea::new().width(Size::fill()).height(Size::fill());
    if colors::active_theme().liquid_glass {
        pane
    } else {
        pane.bottom_fade(colors::page_elevated())
            .bottom_fade_height(SCROLL_FADE_HEIGHT)
    }
}

pub fn settings_page() -> Rect {
    rect().vertical().width(Size::fill()).spacing(PAGE_GAP)
}

pub fn first_section_header(text: &'static str) -> impl IntoElement {
    section_label(text, 0.)
}

pub fn section_header(text: &'static str) -> impl IntoElement {
    section_label(text, 24. - PAGE_GAP)
}

fn section_label(text: &'static str, top: f32) -> Rect {
    rect().padding(Gaps::new(top, 0., 8. - PAGE_GAP, 0.)).child(
        label()
            .text(text.to_uppercase())
            .font_size(text_size(11.))
            .font_weight(FontWeight::SEMI_BOLD)
            .color(readable_secondary()),
    )
}

pub fn settings_row(
    icon: IconType,
    title: &'static str,
    description: impl Into<String>,
    trailing: impl Into<SettingsControl>,
) -> impl IntoElement {
    settings_row_inner(icon, title, description, trailing.into(), RowTone::Normal)
}

pub fn settings_row_disabled(
    icon: IconType,
    title: &'static str,
    description: impl Into<String>,
    trailing: impl IntoElement,
) -> impl IntoElement {
    settings_row_inner(icon, title, description, trailing.into(), RowTone::Disabled)
}

/// For destructive actions, tinted red like the other danger notices
pub fn settings_row_danger(
    icon: IconType,
    title: &'static str,
    description: impl Into<String>,
    trailing: impl IntoElement,
) -> impl IntoElement {
    settings_row_inner(icon, title, description, trailing.into(), RowTone::Danger)
}

#[derive(Clone, Copy, PartialEq)]
enum RowTone {
    Normal,
    Disabled,
    Danger,
}

fn settings_row_inner(
    icon: IconType,
    title: &'static str,
    description: impl Into<String>,
    trailing: SettingsControl,
    tone: RowTone,
) -> impl IntoElement {
    SettingsRow {
        icon,
        title,
        description: description.into(),
        trailing,
        tone,
    }
}

#[derive(PartialEq)]
struct SettingsRow {
    icon: IconType,
    title: &'static str,
    description: String,
    trailing: SettingsControl,
    tone: RowTone,
}

pub(crate) fn settings_row_surface(danger: bool) -> Rect {
    let theme = colors::active_theme();
    let tint = if danger {
        colors::danger().with_a(26)
    } else {
        colors::card()
    };
    let surface = if theme.liquid_glass && !danger {
        crate::theme::glass::content(rect(), colors::component_bg(), rounded_px(12.))
    } else {
        rect().corner_radius(rounded(12.)).background(tint)
    };
    surface
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(ROW_GAP)
        .min_height(Size::px(64.))
        .padding(Gaps::new_symmetric(14., ROW_INSET))
        .maybe(danger, |el| {
            el.border(border_all_color(1., colors::danger().with_a(128)))
        })
}

impl Component for SettingsRow {
    fn render(&self) -> impl IntoElement {
        let mut width = use_state(|| None::<f32>);
        let mut control_width = use_state(|| 0.);
        // Measure the control itself so switches can stay inline when wide pickers
        // and translated status messages need a second line.
        let stacked = width().is_some_and(|width| {
            width - 2. * ROW_INSET - ROW_ICON - 2. * ROW_GAP - control_width() < MIN_COPY_WIDTH
        });
        let danger = self.tone == RowTone::Danger;
        let theme = colors::active_theme();
        let description_ink = if theme.id == "dark" || theme.id == "minecraft-dark" {
            colors::fg_primary().with_a(185)
        } else {
            colors::fg_secondary()
        };
        let content_opacity = if self.tone == RowTone::Disabled {
            DISABLED_OPACITY
        } else {
            1.
        };

        settings_row_surface(danger)
            .cross_align(if stacked {
                Alignment::Start
            } else {
                Alignment::Center
            })
            .on_sized(move |event: Event<SizedEventData>| {
                width.set_if_modified(Some(event.area.width()));
            })
            .child(
                rect().opacity(content_opacity).child(
                    Icon::new(self.icon)
                        .size(ROW_ICON)
                        .maybe(danger, |icon| icon.color(colors::danger())),
                ),
            )
            .child(
                rect()
                    .width(Size::flex(1.))
                    .content(Content::Flex)
                    .maybe(stacked, |el| el.vertical().spacing(8.))
                    .maybe(!stacked, |el| {
                        el.horizontal()
                            .spacing(ROW_GAP)
                            .cross_align(Alignment::Center)
                    })
                    .child(
                        rect()
                            .vertical()
                            .width(if stacked {
                                Size::fill()
                            } else {
                                Size::flex(1.)
                            })
                            .spacing(2.)
                            .child(
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .opacity(content_opacity)
                                    .cross_align(Alignment::Center)
                                    .spacing(6.)
                                    .child(
                                        label()
                                            .max_width(if self.trailing.reset.is_some() {
                                                Size::percent(90.)
                                            } else {
                                                Size::fill()
                                            })
                                            .text(self.title)
                                            .font_size(text_size(16.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .color(colors::fg_primary()),
                                    )
                                    .maybe_child(self.trailing.reset.clone()),
                            )
                            .child(
                                label()
                                    .text(self.description.clone())
                                    .font_size(text_size(12.))
                                    .color(description_ink),
                            ),
                    )
                    .child(
                        rect()
                            .max_width(Size::fill())
                            .opacity(content_opacity)
                            .on_sized(move |event: Event<SizedEventData>| {
                                control_width.set_if_modified(event.area.width());
                            })
                            .child(self.trailing.control.clone()),
                    ),
            )
    }
}

#[derive(PartialEq)]
pub struct SettingsControl {
    control: Element,
    reset: Option<Element>,
}

impl<T: IntoElement> From<T> for SettingsControl {
    fn from(control: T) -> Self {
        Self {
            control: control.into_element(),
            reset: None,
        }
    }
}

impl SettingsControl {
    pub(crate) fn with_reset(control: impl IntoElement, reset: Option<Element>) -> Self {
        Self {
            control: control.into_element(),
            reset,
        }
    }
}

pub(crate) fn title_reset(label: &'static str, on_reset: EventHandler<()>) -> Element {
    Button::new()
        .ghost()
        .icon()
        .width(Size::px(22.))
        .height(Size::px(20.))
        .alt(label)
        .tooltip(label)
        .on_press(move |_| on_reset.call(()))
        .child(
            Icon::new(IconType::RefreshCcw02)
                .size(13.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

#[derive(PartialEq)]
struct ResetButton<T: Clone + PartialEq + 'static> {
    values: Vec<(State<T>, T)>,
}

impl<T: Clone + PartialEq + 'static> Component for ResetButton<T> {
    fn render(&self) -> impl IntoElement {
        let at_default = self
            .values
            .iter()
            .all(|(value, default)| *value.read() == *default);

        if at_default {
            return rect().into_element();
        }

        let values = self.values.clone();
        title_reset(
            t("oneclient.settings.reset.to_default"),
            (move |()| {
                for (value, default) in &values {
                    let mut value = *value;
                    value.set_if_modified(default.clone());
                }
            })
            .into(),
        )
    }
}

pub fn resettable<T: Clone + PartialEq + 'static>(
    control: impl IntoElement,
    value: State<T>,
    default: T,
) -> SettingsControl {
    resettable_all(control, vec![(value, default)])
}

pub fn resettable_all<T: Clone + PartialEq + 'static>(
    control: impl IntoElement,
    values: Vec<(State<T>, T)>,
) -> SettingsControl {
    SettingsControl::with_reset(control, Some(ResetButton { values }.into_element()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use freya_testing::TestingRunner;
    use std::time::Duration;

    #[test]
    fn settings_controls_take_28_percent_of_the_page_between_160_and_220() {
        // 800x600, 1024x700 and 1280x800 dialogs
        assert_eq!(control_width_for(524.), 160.);
        assert!((control_width_for(727.04) - 203.57).abs() < 0.01);
        assert_eq!(control_width_for(947.9), 220.);
    }

    #[test]
    fn secondary_text_is_lifted_only_until_it_is_readable() {
        let card = Color::from_rgb(0x1A, 0x22, 0x29);
        let primary = Color::from_rgb(0xD5, 0xDB, 0xFF);
        let dark = readable_on(Color::from_rgb(0x75, 0x78, 0x83), primary, card);
        assert!(contrast(dark, card) >= READABLE_CONTRAST);
        // close to #8B929F, not all the way to the primary ink
        assert!(dark.r() < 0x95 && dark.r() > 0x80, "{dark:?}");

        let light_card = Color::from_rgb(0xE3, 0xEA, 0xFB);
        let light = Color::from_rgb(0x4E, 0x56, 0x61);
        assert_eq!(readable_on(light, Color::BLACK, light_card), light);
    }

    #[test]
    fn wide_settings_controls_move_below_the_description_when_space_runs_out() {
        for (width, controls, stacked) in
            [(440., 300., true), (720., 300., false), (440., 84., false)]
        {
            let (mut test, ()) = TestingRunner::new(
                move || {
                    settings_row(
                        IconType::Copy01,
                        "Migrate",
                        "Copy browser content to or from another instance.",
                        rect()
                            .width(Size::px(controls))
                            .height(Size::px(28.))
                            .a11y_alt("Migration controls"),
                    )
                },
                (width, 200.).into(),
                |_| (),
                1.,
            );
            test.poll_n(Duration::ZERO, 5);
            let area = |name| {
                test.find(|node, element| {
                    let mut access = element.accessibility().builder.clone();
                    element.finish_accessibility(&mut access);
                    (access.label() == Some(name) || access.value() == Some(name))
                        .then(|| node.layout().area)
                })
                .unwrap()
            };
            let title = area("Migrate");
            let control = area("Migration controls");
            assert!(title.height() < 32., "title should fit on one line");
            assert!(control.max_x() <= width - ROW_INSET + 0.5);
            if stacked {
                assert!(control.min_y() > title.max_y());
                assert_eq!(control.min_x(), title.min_x());
            } else {
                assert!(control.min_y() < title.max_y());
                assert!(control.min_x() > title.max_x());
            }
        }
    }

    #[test]
    fn reset_mounts_for_a_modified_value_and_disappears_after_reset() {
        let (mut test, mut setting) = TestingRunner::new(
            || {
                let setting = use_consume::<State<bool>>();
                settings_row(
                    IconType::Settings01,
                    "Example setting",
                    "Description",
                    resettable(rect().width(Size::px(40.)), setting, false),
                )
            },
            (600., 120.).into(),
            |test| test.provide_root_context(|| State::create(false)),
            1.,
        );
        let reset = |test: &TestingRunner| {
            test.find(|node, element| {
                (element.accessibility().builder.label() == Some("Reset to default"))
                    .then(|| node.layout().area)
            })
        };
        test.sync_and_update();
        assert!(reset(&test).is_none());

        setting.set(true);
        test.sync_and_update();
        let button = reset(&test).expect("a modified setting should show Reset");
        assert!(button.min_x() < 280.);
        test.click_cursor((
            f64::from(button.min_x() + button.width() * 0.5),
            f64::from(button.min_y() + button.height() * 0.5),
        ));
        test.sync_and_update();
        assert!(!*setting.read());
        assert!(reset(&test).is_none());
    }
}
