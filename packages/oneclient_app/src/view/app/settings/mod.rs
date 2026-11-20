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
    components::{Button, Icon, IconType},
    i18n::t,
    theme::{colors, rounded, text_size},
    ui::border_all_color,
};

/// Bottom inset so the last row scrolls clear of the shell's clip edge instead of ending on it
pub fn settings_page() -> Rect {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(4.)
        .padding(Gaps::new(0., 0., 24., 0.))
}

pub fn section_header(text: &'static str) -> impl IntoElement {
    rect()
        .padding(Gaps::new(16., 0., 8., 2.))
        .child(
            label()
                .text(text)
                .font_size(text_size(13.))
                .font_weight(FontWeight::MEDIUM)
                .color(colors::fg_secondary()),
        )
        .into_element()
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
    let description = description.into();
    let danger = tone == RowTone::Danger;
    let tint = if danger {
        colors::danger().with_a(26)
    } else {
        colors::page_elevated()
    };
    let row = if colors::active_theme().liquid_glass && !danger {
        crate::theme::glass::content(rect(), tint, 16.)
    } else {
        rect().corner_radius(rounded(12.)).background(tint)
    };
    row.maybe(tone == RowTone::Disabled, |el| el.opacity(0.4))
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(16.)
        .padding(Gaps::new_symmetric(12., 16.))
        .maybe(danger, |el| {
            el.border(border_all_color(1., colors::danger().with_a(128)))
        })
        .child(Icon::new(icon).maybe(danger, |icon| icon.color(colors::danger())))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(2.)
                .child(
                    rect()
                        .horizontal()
                        .cross_align(Alignment::Center)
                        .spacing(6.)
                        .child(
                            label()
                                .text(title)
                                .font_size(text_size(16.))
                                .font_weight(FontWeight::MEDIUM)
                                .color(colors::fg_primary()),
                        )
                        .maybe_child(trailing.reset),
                )
                .child(
                    label()
                        .text(description)
                        .font_size(text_size(12.))
                        .color(colors::fg_secondary()),
                ),
        )
        .child(trailing.control)
        .into_element()
}

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
