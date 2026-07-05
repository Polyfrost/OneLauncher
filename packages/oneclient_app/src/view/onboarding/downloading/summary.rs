use super::*;

use freya::router::RouterContext;

use crate::components::{Button, DynamicArt, Icon, IconType, ScrollArea};
use crate::hooks::ClusterBundles;
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::utils::format_size;
use crate::view::onboarding::{onboarding_illustration, predownload_toggle_row, step_heading};

#[allow(clippy::too_many_arguments)]
pub(super) fn summary_view(
    items: &[ClusterBundles],
    selected: &std::collections::HashSet<String>,
    language: &str,
    reduce_motion: bool,
    parallax: bool,
    account_name: String,
    predownload: State<bool>,
    on_setup: impl FnMut(Event<PressEventData>) + 'static,
) -> impl IntoElement {
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

    let packages_note = if *predownload.read() {
        let est = rough_download_estimate(items, selected);
        format!(
            "Your selected content is downloaded now, during setup. About {} total.",
            format_size(est)
        )
    } else {
        "Content is downloaded the first time you launch each version.".to_string()
    };

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
                    Some(packages_note.as_str()),
                    if version_rows.is_empty() {
                        vec![summary_line("No versions", "")]
                    } else {
                        version_rows
                    },
                ))
                .child(predownload_toggle_row(predownload)),
        );

    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .content(Content::Flex)
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .height(Size::flex(1.0))
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
        )
        .child(
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
                        .on_press(move |_| {
                            let _ = RouterContext::get().replace(Route::OnboardingPreferences {});
                        })
                        .text("Back"),
                )
                .child(
                    Button::new()
                        .primary()
                        .width(Size::px(160.))
                        .on_press(on_setup)
                        .text("Setup")
                        .child(Icon::new(IconType::Download01).size(16.)),
                ),
        )
        .child(prefetch_art(items))
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
                .child(DynamicArt::for_cluster(&cb.cluster).max_edge(512))
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
