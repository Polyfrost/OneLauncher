use freya::prelude::*;
use freya::router::RouterContext;

use crate::components::{ART_PREVIEW_EDGE, Button, DynamicArt, Icon, IconType, ScrollArea};
use crate::hooks::ClusterBundles;
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::view::onboarding::{onboarding_illustration, onboarding_slide, step_heading};

pub(super) struct SummaryView<'a> {
    pub items: &'a [ClusterBundles],
    pub selected: &'a std::collections::HashSet<String>,
    pub language: &'a str,
    pub reduce_motion: bool,
    pub parallax: bool,
    pub account_name: String,
    pub migration: Option<(String, String, String)>,
    /// A run is in flight both buttons stay down until it settles
    pub finishing: bool,
    /// The import is dispatched on the first press so going back afterwards would apply to nothing
    pub attempted: bool,
    pub failure: Option<String>,
}

pub(super) fn summary_view(
    view: SummaryView<'_>,
    on_finish: impl FnMut(Event<PressEventData>) + 'static,
    on_continue: impl FnMut(Event<PressEventData>) + 'static,
) -> impl IntoElement {
    let SummaryView {
        items,
        selected,
        language,
        reduce_motion,
        parallax,
        account_name,
        migration,
        finishing,
        attempted,
        failure,
    } = view;

    let version_rows: Vec<Element> = items
        .iter()
        .map(|cb| {
            let prefix = format!("{}|", cb.cluster.id);
            let count = selected.iter().filter(|k| k.starts_with(&prefix)).count();
            summary_line(
                &cb.cluster.mc_version,
                &format!("{count} package{}", if count == 1 { "" } else { "s" }),
            )
        })
        .collect();

    let migration_section: Element = match migration {
        Some((source, folder, target)) => {
            let mut rows = vec![
                summary_line("From", &source),
                summary_line("Files", &folder),
            ];
            if !target.is_empty() {
                rows.push(summary_line("Destination", &target));
            }
            summary_section("Migration", None, rows).into_element()
        }
        None => rect().into_element(),
    };

    let failed = failure.is_some();

    let content = rect()
        .vertical()
        .width(Size::fill())
        .height(Size::flex(1.0))
        .content(Content::Flex)
        .spacing(20.)
        .child(step_heading(
            "You're all set",
            "Review your setup below, then finish.",
        ))
        .child(
            ScrollArea::new()
                .width(Size::fill())
                .height(Size::flex(1.0))
                .spacing(20.)
                .child(summary_section(
                    "General",
                    None,
                    vec![
                        summary_line("Language", language),
                        summary_line("Account", &account_name),
                    ],
                ))
                .child(migration_section)
                .child(summary_section(
                    "Accessibility",
                    None,
                    vec![
                        summary_line("Animations", if reduce_motion { "Off" } else { "On" }),
                        summary_line("Parallax background", if parallax { "On" } else { "Off" }),
                    ],
                ))
                .child(summary_section(
                    "Packages per version",
                    Some("Content is downloaded the first time you launch each version."),
                    if version_rows.is_empty() {
                        vec![summary_line("No versions", "")]
                    } else {
                        version_rows
                    },
                )),
        )
        .maybe_child(failure.map(|reason| failure_notice(&reason)));

    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .content(Content::Flex)
        .child(onboarding_slide(
            rect()
                .horizontal()
                .width(Size::fill())
                .height(Size::fill())
                .content(Content::Flex)
                .child(
                    rect()
                        .width(Size::flex(1.0))
                        .height(Size::fill())
                        .center()
                        .padding(Gaps::new_all(48.))
                        .child(onboarding_illustration(IconType::OnboardingComplete)),
                )
                .child(
                    rect()
                        .vertical()
                        .width(Size::flex(1.0))
                        .height(Size::fill())
                        .content(Content::Flex)
                        .padding(Gaps::new(48., 80., 24., 24.))
                        .child(content),
                ),
        ))
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .main_align(Alignment::End)
                .cross_align(Alignment::Center)
                .spacing(12.)
                .padding(Gaps::new(0., 40., 32., 40.))
                .child(if failed {
                    Button::new()
                        .secondary()
                        .width(Size::px(180.))
                        .enabled(!finishing)
                        .on_press(on_continue)
                        .text("Continue anyway")
                        .into_element()
                } else {
                    Button::new()
                        .secondary()
                        .width(Size::px(128.))
                        .enabled(!finishing && !attempted)
                        .on_press(move |_| {
                            let _ = RouterContext::get().replace(Route::OnboardingPreferences {});
                        })
                        .text("Back")
                        .into_element()
                })
                .child(
                    Button::new()
                        .primary()
                        .width(Size::px(160.))
                        .enabled(!finishing)
                        .on_press(on_finish)
                        .text(if failed { "Try again" } else { "Finish" })
                        .child(Icon::new(IconType::Check).size(16.)),
                ),
        )
        .child(prefetch_art(items))
}

fn failure_notice(reason: &str) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(4.)
        .padding(Gaps::new_all(14.))
        .corner_radius(CornerRadius::new_all(12.))
        .background(colors::danger().with_a(26))
        .border(border_all_color(1., colors::danger().with_a(128)))
        .child(
            label()
                .text("Your package choices could not be saved.")
                .font_size(14.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text(format!(
                    "{reason}. Continuing anyway keeps every mod the bundles ship by default."
                ))
                .font_size(11.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn prefetch_art(items: &[ClusterBundles]) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::px(1.))
        .overflow(Overflow::Clip)
        .opacity(0.)
        .interactive(false)
        .children(items.iter().map(|cb| {
            rect()
                .width(Size::px(1.))
                .height(Size::px(1.))
                .child(DynamicArt::for_cluster(&cb.cluster).max_edge(ART_PREVIEW_EDGE))
                .into_element()
        }))
        .into_element()
}

fn summary_section(title: &str, note: Option<&str>, rows: Vec<Element>) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(8.)
        .child(
            label()
                .text(title.to_string())
                .font_size(13.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_secondary()),
        )
        .maybe_child(note.map(|text| {
            label()
                .text(text.to_string())
                .font_size(11.)
                .color(colors::fg_secondary())
                .into_element()
        }))
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(2.)
                .corner_radius(CornerRadius::new_all(12.))
                .background(colors::page_elevated())
                .border(border_all_color(1., colors::component_border()))
                .padding(Gaps::new_symmetric(6., 4.))
                .children(rows),
        )
        .into_element()
}

fn summary_line(label_text: &str, value: &str) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .padding(Gaps::new_symmetric(10., 12.))
        .child(
            rect().width(Size::flex(1.0)).child(
                label()
                    .text(label_text.to_string())
                    .font_size(14.)
                    .color(colors::fg_primary()),
            ),
        )
        .child(
            label()
                .text(value.to_string())
                .font_size(14.)
                .font_weight(FontWeight::MEDIUM)
                .color(colors::fg_primary()),
        )
        .into_element()
}
