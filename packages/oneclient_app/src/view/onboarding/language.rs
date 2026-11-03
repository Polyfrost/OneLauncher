use freya::prelude::*;

use crate::components::{IconType, ScrollArea};
use crate::hooks::{
    Actions, has_migration_data, use_dispatch, use_migration, use_onboarding_selection,
    use_settings_snapshot,
};
use crate::i18n::{t, tf};
use crate::routes::Route;
use crate::theme::colors;
use crate::translations;
use crate::ui::border_all_color;
use crate::view::onboarding::{
    onboarding_illustration, onboarding_nav, onboarding_page, step_heading,
};

const LIST_MAX_H: f32 = 360.;

#[derive(PartialEq)]
pub struct OnboardingLanguage;

impl Component for OnboardingLanguage {
    fn render(&self) -> impl IntoElement {
        let current = use_settings_snapshot().settings.language;
        let dispatch = use_dispatch();
        let language = use_onboarding_selection().language;
        let migration_query = use_migration();
        let back = if has_migration_data(&migration_query) {
            Route::OnboardingMigration {}
        } else {
            Route::OnboardingTerms {}
        };

        let rows: Vec<Element> = translations::choices()
            .into_iter()
            .map(|code| {
                let active = code == current;
                language_row(code, active, dispatch.clone(), language)
            })
            .collect();

        let content = rect()
            .vertical()
            .width(Size::fill())
            .spacing(16.)
            .child(step_heading(
                "OneClient",
                t("oneclient.onboarding.welcome.subtitle"),
            ))
            .child(
                label()
                    .text(t("oneclient.onboarding.language.choose"))
                    .font_size(18.)
                    .font_weight(FontWeight::MEDIUM)
                    .color(colors::fg_primary()),
            )
            .child(
                ScrollArea::new()
                    .width(Size::fill())
                    .max_height(Size::px(LIST_MAX_H))
                    .show_scrollbar(true)
                    .spacing(4.)
                    .children(rows),
            )
            .into_element();

        onboarding_page(
            onboarding_illustration(IconType::OnboardingLanguage),
            content,
            onboarding_nav(Some(back), Route::OnboardingAccount {}, true),
        )
    }
}

fn language_row(
    code: Option<String>,
    active: bool,
    dispatch: Actions,
    mut chosen: State<String>,
) -> Element {
    let (name, _) = translations::names(code.as_deref());
    let coverage = translations::coverage(code.as_deref());

    let background = if active {
        colors::brand()
    } else {
        Color::TRANSPARENT
    };

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .padding(Gaps::new_symmetric(14., 20.))
        .corner_radius(CornerRadius::new_all(10.))
        .background(background)
        .maybe(active, |el| {
            el.border(border_all_color(1., colors::brand()))
        })
        .a11y_role(AccessibilityRole::Button)
        .cursor(CursorIcon::Pointer)
        .on_press({
            let name = name.clone();
            move |_| {
                let code = code.clone();
                dispatch.edit_settings(|settings| settings.language = code);
                chosen.set(name.clone());
            }
        })
        .child(
            rect().width(Size::flex(1.0)).child(
                label()
                    .text(name)
                    .font_size(16.)
                    .font_weight(FontWeight::MEDIUM)
                    .color(colors::fg_primary()),
            ),
        )
        .maybe_child(coverage.map(|percentage| {
            label()
                .text(tf(
                    "oneclient.onboarding.language.localized",
                    &[&percentage],
                ))
                .font_size(13.)
                .color(if active {
                    colors::fg_primary()
                } else {
                    colors::fg_secondary()
                })
                .into_element()
        }))
        .into_element()
}
