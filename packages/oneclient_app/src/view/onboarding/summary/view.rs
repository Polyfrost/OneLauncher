use freya::prelude::*;
use freya::router::RouterContext;

use crate::components::{ART_PREVIEW_EDGE, Button, DynamicArt, Icon, IconType, ScrollArea};
use crate::hooks::ClusterBundles;
use crate::i18n::{t, tf};
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
    pub ready: bool,
    /// A run is in flight both buttons stay down until it settles
    pub finishing: bool,
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
        ready,
        finishing,
        failure,
    } = view;

    let version_rows: Vec<Element> = items
        .iter()
        .map(|cb| {
            let prefix = format!("{}|", cb.cluster.id);
            let count = selected.iter().filter(|k| k.starts_with(&prefix)).count();
            summary_line(
                &cb.cluster.mc_version,
                &if count == 1 {
                    tf("oneclient.onboarding.summary.packages.count.one", &[&count])
                } else {
                    tf(
                        "oneclient.onboarding.summary.packages.count.other",
                        &[&count],
                    )
                },
            )
        })
        .collect();

    let migration_section: Element = match migration {
        Some((source, folder, target)) => {
            let mut rows = vec![
                summary_line(t("oneclient.onboarding.summary.migration.from"), &source),
                summary_line(t("oneclient.onboarding.summary.migration.files"), &folder),
            ];
            if !target.is_empty() {
                rows.push(summary_line(
                    t("oneclient.onboarding.summary.migration.destination"),
                    &target,
                ));
            }
            summary_section(
                t("oneclient.onboarding.summary.migration.title"),
                None,
                rows,
            )
            .into_element()
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
            t("oneclient.onboarding.summary.title"),
            t("oneclient.onboarding.summary.subtitle"),
        ))
        .child(
            ScrollArea::new()
                .width(Size::fill())
                .height(Size::flex(1.0))
                .spacing(20.)
                .child(summary_section(
                    t("oneclient.onboarding.summary.general.title"),
                    None,
                    vec![
                        summary_line(t("oneclient.onboarding.summary.general.language"), language),
                        summary_line(
                            t("oneclient.onboarding.summary.general.account"),
                            &account_name,
                        ),
                    ],
                ))
                .child(migration_section)
                .child(summary_section(
                    t("oneclient.onboarding.summary.accessibility.title"),
                    None,
                    vec![
                        summary_line(
                            t("oneclient.onboarding.summary.accessibility.animations"),
                            if reduce_motion {
                                t("oneclient.common.off")
                            } else {
                                t("oneclient.common.on")
                            },
                        ),
                        summary_line(
                            t("oneclient.onboarding.summary.accessibility.parallax"),
                            if parallax {
                                t("oneclient.common.on")
                            } else {
                                t("oneclient.common.off")
                            },
                        ),
                    ],
                ))
                .child(summary_section(
                    t("oneclient.onboarding.summary.packages.title"),
                    Some(t("oneclient.onboarding.summary.packages.note")),
                    if version_rows.is_empty() {
                        vec![summary_line(
                            t("oneclient.onboarding.summary.packages.empty"),
                            "",
                        )]
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
                        .enabled(ready && !finishing)
                        .on_press(on_continue)
                        .text(t("oneclient.onboarding.summary.continue_anyway"))
                        .into_element()
                } else {
                    Button::new()
                        .secondary()
                        .width(Size::px(128.))
                        .enabled(!finishing)
                        .on_press(move |_| {
                            let _ = RouterContext::get().replace(Route::OnboardingPreferences {});
                        })
                        .text(t("oneclient.common.back"))
                        .into_element()
                })
                .child(
                    Button::new()
                        .primary()
                        .width(Size::px(160.))
                        .enabled(ready && !finishing)
                        .on_press(on_finish)
                        .text(if failed {
                            t("oneclient.common.try_again")
                        } else {
                            t("oneclient.onboarding.summary.finish")
                        })
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
                .text(t("oneclient.onboarding.summary.failure.title"))
                .font_size(14.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text(tf(
                    "oneclient.onboarding.summary.failure.description",
                    &[&reason.trim_end_matches('.')],
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
