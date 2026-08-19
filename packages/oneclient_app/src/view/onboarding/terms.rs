use freya::prelude::*;
use freya::query::UseQuery;
use freya::router::RouterContext;

use crate::components::{Button, Icon, IconType, ScrollArea, Segment, SegmentedControl, toggle};
use crate::hooks::{
    TermsQuery, has_migration_data, terms_document, terms_error, terms_is_loading, use_dispatch,
    use_migration, use_settings_snapshot, use_terms,
};
use crate::platform::open_url;
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::view::onboarding::{onboarding_illustration, onboarding_page, step_heading};

#[derive(Clone, Copy, PartialEq)]
enum LegalTab {
    Terms,
    Privacy,
}

#[derive(PartialEq)]
pub struct OnboardingTerms;

impl Component for OnboardingTerms {
    fn render(&self) -> impl IntoElement {
        let settings = use_settings_snapshot().settings;
        let dispatch = use_dispatch();
        let query = use_terms();
        let migration_query = use_migration();

        let document = terms_document(&query);
        let error = terms_error(&query);
        let loading = terms_is_loading(&query);

        let accepted = use_state(|| false);
        let tab = use_state(|| LegalTab::Terms);
        let confirming_decline = use_state(|| false);

        let returning = settings.seen_onboarding;
        let next = if returning {
            Route::Home {}
        } else if has_migration_data(&migration_query) {
            Route::OnboardingMigration {}
        } else {
            Route::OnboardingLanguage {}
        };
        let back = (!returning).then_some(Route::OnboardingWelcome {});

        let terms_version = document.as_ref().map(|doc| doc.version).unwrap_or(1);
        let privacy_version = document
            .as_ref()
            .map(|doc| doc.privacy_version())
            .unwrap_or(1);
        let terms_url = document
            .as_ref()
            .map(|doc| doc.terms_url().to_string())
            .unwrap_or_else(|| oneclient_common::constants::TOS_URL.to_string());
        let privacy_url = document
            .as_ref()
            .map(|doc| doc.privacy_url().to_string())
            .unwrap_or_else(|| oneclient_common::constants::PRIVACY_URL.to_string());

        let privacy_body = document.as_ref().and_then(|doc| doc.privacy_body());
        let tabs = privacy_body.is_some().then(|| tab_switcher(tab));

        let body = if loading {
            loading_body()
        } else if let Some(document) = document.as_ref() {
            match (*tab.read(), privacy_body) {
                (LegalTab::Privacy, Some(privacy)) => markdown_body(privacy),
                _ => markdown_body(&document.terms),
            }
        } else {
            fallback_body(error.as_deref(), query)
        };

        let deciding = *confirming_decline.read();

        let content = rect()
            .vertical()
            .width(Size::fill())
            .spacing(16.)
            .child(step_heading(
                "Terms & Privacy",
                "Please read and accept these before continuing.",
            ))
            .maybe_child(tabs)
            .child(body)
            .child(link_row(terms_url, privacy_url))
            .child(if deciding {
                decline_warning()
            } else {
                accept_row(accepted)
            })
            .into_element();

        let nav = if deciding {
            decline_nav(
                move || {
                    let mut confirming = confirming_decline;
                    confirming.set(false);
                },
                move || {
                    dispatch.decline_tos();
                    let _ = RouterContext::get().replace(Route::Home {});
                },
            )
        } else {
            terms_nav(
                back,
                *accepted.read() && !loading,
                move || {
                    let mut confirming = confirming_decline;
                    confirming.set(true);
                },
                move || {
                    dispatch.accept_tos(terms_version, privacy_version);
                    let _ = RouterContext::get().replace(next.clone());
                },
            )
        };

        onboarding_page(onboarding_illustration(IconType::File02), content, nav)
    }
}

fn panel(child: impl IntoElement) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::px(220.))
        .content(Content::Flex)
        .corner_radius(CornerRadius::new_all(12.))
        .background(colors::page_elevated())
        .border(border_all_color(1., colors::component_border()))
        .child(child)
}

fn loading_body() -> Element {
    panel(
        rect()
            .width(Size::fill())
            .height(Size::fill())
            .center()
            .child(
                label()
                    .text("Loading the Terms of Service and Privacy Policy...")
                    .font_size(13.)
                    .color(colors::fg_secondary()),
            ),
    )
    .into_element()
}

fn tab_switcher(tab: State<LegalTab>) -> Element {
    SegmentedControl::new(tab)
        .segment(Segment::new(LegalTab::Terms).label("Terms"))
        .segment(Segment::new(LegalTab::Privacy).label("Privacy"))
        .into_element()
}

fn markdown_body(markdown: &str) -> Element {
    panel(
        ScrollArea::new()
            .width(Size::fill())
            .height(Size::flex(1.0))
            .padding(Gaps::new_all(16.))
            .child(
                MarkdownViewer::new(markdown.to_string())
                    .width(Size::fill())
                    .color(colors::fg_primary())
                    .color_link(colors::code_info())
                    .background_code(colors::component_bg())
                    .color_code(colors::fg_primary())
                    .background_blockquote(colors::component_bg())
                    .border_blockquote(colors::brand())
                    .background_divider(colors::component_border())
                    .heading_h1(20.)
                    .heading_h2(17.)
                    .heading_h3(15.)
                    .heading_h4(14.)
                    .heading_h5(13.)
                    .heading_h6(12.)
                    .paragraph_size(13.)
                    .code_font_size(12.),
            ),
    )
    .into_element()
}

fn fallback_body(error: Option<&str>, query: UseQuery<TermsQuery>) -> Element {
    panel(
        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .center()
            .spacing(10.)
            .padding(Gaps::new_all(20.))
            .child(
                Icon::new(IconType::AlertTriangle)
                    .size(22.)
                    .color(colors::code_warn()),
            )
            .child(
                label()
                    .text("Couldn't load the Terms of Service or Privacy Policy.")
                    .font_size(14.)
                    .font_weight(FontWeight::MEDIUM)
                    .color(colors::fg_primary()),
            )
            .child(
                label()
                    .text(
                        "Open them in your browser using the links below, or try again once \
                         you're back online.",
                    )
                    .font_size(12.)
                    .text_align(TextAlign::Center)
                    .color(colors::fg_secondary()),
            )
            .maybe_child(error.map(|err| {
                label()
                    .text(err.to_string())
                    .font_size(11.)
                    .max_lines(2)
                    .color(colors::fg_secondary().with_a(160))
            }))
            .child(
                Button::new()
                    .secondary()
                    .small()
                    .on_press(move |_| {
                        query.invalidate();
                    })
                    .text("Retry")
                    .child(Icon::new(IconType::RefreshCw01).size(14.)),
            ),
    )
    .into_element()
}

fn link_row(terms_url: String, privacy_url: String) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .spacing(10.)
        .child(external_link_button("Terms of Service", terms_url))
        .child(external_link_button("Privacy Policy", privacy_url))
        .into_element()
}

fn external_link_button(text: &'static str, url: String) -> impl IntoElement {
    Button::new()
        .secondary()
        .small()
        .on_press(move |_| open_url(&url))
        .text(text)
        .child(Icon::new(IconType::LinkExternal01).size(14.))
}

fn accept_row(accepted: State<bool>) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(16.)
        .content(Content::Flex)
        .padding(Gaps::new_symmetric(12., 16.))
        .corner_radius(CornerRadius::new_all(12.))
        .background(colors::page_elevated())
        .border(border_all_color(1., colors::component_border()))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(3.)
                .child(
                    label()
                        .text("I accept the Terms of Service and Privacy Policy")
                        .font_size(14.)
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text("Required for Poly+, downloads and updates.")
                        .font_size(11.)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(toggle(accepted))
        .into_element()
}

fn decline_warning() -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(8.)
        .padding(Gaps::new_symmetric(14., 16.))
        .corner_radius(CornerRadius::new_all(12.))
        .background(colors::page_elevated())
        .border(border_all_color(1., colors::danger()))
        .child(
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(8.)
                .child(
                    Icon::new(IconType::AlertTriangle)
                        .size(18.)
                        .color(colors::code_warn()),
                )
                .child(
                    label()
                        .text("Continue without accepting?")
                        .font_size(14.)
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_primary()),
                ),
        )
        .child(
            label()
                .text(
                    "OneClient stops contacting Polyfrost entirely: no Poly+, no crash reports, \
                     and no version, mod or bundle downloads. Instances you have already \
                     installed keep working, and signing in to Minecraft still works.",
                )
                .font_size(12.)
                .color(colors::fg_secondary()),
        )
        .child(
            label()
                .text(
                    "You can accept later under Settings > Launcher. That takes effect after a \
                     restart.",
                )
                .font_size(11.)
                .color(colors::fg_secondary().with_a(180)),
        )
        .into_element()
}

fn terms_nav(
    back: Option<Route>,
    next_enabled: bool,
    on_decline: impl FnMut() + 'static,
    on_next: impl FnMut() + 'static,
) -> Element {
    let mut on_decline = on_decline;
    let mut on_next = on_next;
    rect()
        .horizontal()
        .width(Size::fill())
        .main_align(Alignment::End)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .padding(Gaps::new(0., 40., 32., 40.))
        .maybe_child(back.map(|route| {
            Button::new()
                .secondary()
                .width(Size::px(128.))
                .on_press(move |_| {
                    let _ = RouterContext::get().replace(route.clone());
                })
                .text("Back")
                .into_element()
        }))
        .child(
            Button::new()
                .secondary()
                .width(Size::px(128.))
                .on_press(move |_| on_decline())
                .text("Decline"),
        )
        .child(
            Button::new()
                .primary()
                .width(Size::px(140.))
                .enabled(next_enabled)
                .on_press(move |_| on_next())
                .text("Next")
                .child(Icon::new(IconType::ArrowRight).size(16.)),
        )
        .into_element()
}

fn decline_nav(on_cancel: impl FnMut() + 'static, on_confirm: impl FnMut() + 'static) -> Element {
    let mut on_cancel = on_cancel;
    let mut on_confirm = on_confirm;

    rect()
        .horizontal()
        .width(Size::fill())
        .main_align(Alignment::End)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .padding(Gaps::new(0., 40., 32., 40.))
        .child(
            Button::new()
                .secondary()
                .width(Size::px(128.))
                .on_press(move |_| on_cancel())
                .text("Go back"),
        )
        .child(
            Button::new()
                .danger()
                .width(Size::px(220.))
                .on_press(move |_| on_confirm())
                .text("Decline and continue"),
        )
        .into_element()
}
