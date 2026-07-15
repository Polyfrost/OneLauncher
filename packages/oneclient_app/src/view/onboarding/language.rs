use freya::prelude::*;

use crate::components::IconType;
use crate::hooks::{has_migration_data, use_migration, use_onboarding_selection};
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::view::onboarding::{
    onboarding_illustration, onboarding_nav, onboarding_page, step_heading,
};

struct Language {
    name: &'static str,
    percentage: u32,
}

const LANGUAGES: [Language; 1] = [Language {
    name: "English",
    percentage: 100,
}];

#[derive(PartialEq)]
pub struct OnboardingLanguage;

impl Component for OnboardingLanguage {
    fn render(&self) -> impl IntoElement {
        let selected = use_state(|| 0usize);
        let language = use_onboarding_selection().language;
        let migration_query = use_migration();
        let back = if has_migration_data(&migration_query) {
            Route::OnboardingMigration {}
        } else {
            Route::OnboardingTerms {}
        };

        let rows: Vec<Element> = LANGUAGES
            .iter()
            .enumerate()
            .map(|(index, lang)| language_row(lang, index == *selected.read(), selected, language))
            .collect();

        let content = rect()
            .vertical()
            .width(Size::fill())
            .spacing(16.)
            .child(step_heading(
                "OneClient",
                "Let's get you all set-up with the most advanced client.",
            ))
            .child(
                label()
                    .text("Choose your preferred language")
                    .font_size(18.)
                    .font_weight(FontWeight::MEDIUM)
                    .color(colors::fg_primary()),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
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
    language: &Language,
    active: bool,
    mut selected: State<usize>,
    mut chosen: State<String>,
) -> Element {
    let index = LANGUAGES
        .iter()
        .position(|l| l.name == language.name)
        .unwrap_or(0);
    let name = language.name;

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
        .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
        .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
        .on_press(move |_| {
            selected.set(index);
            chosen.set(name.to_string());
        })
        .child(
            rect().width(Size::flex(1.0)).child(
                label()
                    .text(language.name)
                    .font_size(16.)
                    .font_weight(FontWeight::MEDIUM)
                    .color(colors::fg_primary()),
            ),
        )
        .child(
            label()
                .text(format!("{}% localized", language.percentage))
                .font_size(13.)
                .color(if active {
                    colors::fg_primary()
                } else {
                    colors::fg_secondary()
                }),
        )
        .into_element()
}
