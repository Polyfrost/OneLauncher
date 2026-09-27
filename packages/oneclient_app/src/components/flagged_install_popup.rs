use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_content::packages::ResolvedAlternative;

use crate::components::{Button, Icon, IconType, OverlayPopup, ScrollArea, remote_icon};
use crate::hooks::{use_cached_image, use_dispatch, use_installs_snapshot};
use crate::routes::Route;
use crate::state::FlaggedInstallPrompt;
use crate::theme::colors;
use crate::ui::border_all_color;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);
const DIALOG_W: f32 = 460.;
const ROW_H: f32 = 48.;
const ICON_SIZE: f32 = 32.;
const ROW_GAP: f32 = 6.;
const VISIBLE_ROWS: usize = 3;
const LIST_MAX_H: f32 = VISIBLE_ROWS as f32 * ROW_H + (VISIBLE_ROWS - 1) as f32 * ROW_GAP;

#[derive(PartialEq)]
pub struct FlaggedInstallPopup;

impl Component for FlaggedInstallPopup {
    fn render(&self) -> impl IntoElement {
        let installs = use_installs_snapshot();
        let dispatch = use_dispatch();

        let Some(prompt) = installs.flagged.clone() else {
            return rect().into_element();
        };

        let close = dispatch.clone();

        OverlayPopup::new()
            .on_close(move |_| close.dismiss_flagged_install())
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(dialog(prompt, dispatch)),
            )
            .into_element()
    }
}

fn dialog(prompt: FlaggedInstallPrompt, dispatch: crate::Actions) -> impl IntoElement {
    let description = if prompt.alternatives.is_empty() {
        format!(
            "{} is flagged as a problematic mod. Installing it is not recommended.",
            prompt.name
        )
    } else {
        format!(
            "{} is flagged as a problematic mod. Installing it is not recommended, consider one of these alternatives instead.",
            prompt.name
        )
    };

    let mut body = rect()
        .vertical()
        .width(Size::px(DIALOG_W))
        .max_width(Size::window_percent(90.))
        .spacing(14.)
        .padding(Gaps::new_all(20.))
        .corner_radius(CornerRadius::new_all(14.))
        .background(CARD_BG)
        .border(border_all_color(1., colors::component_border()))
        .child(
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(10.)
                .child(
                    Icon::new(IconType::AlertTriangle)
                        .size(20.)
                        .color(colors::code_warn()),
                )
                .child(
                    label()
                        .text("Flagged mod")
                        .font_size(16.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                ),
        )
        .child(
            label()
                .text(description)
                .font_size(12.)
                .width(Size::fill())
                .color(colors::fg_secondary()),
        );

    let rows: Vec<Element> = prompt
        .alternatives
        .iter()
        .map(|alternative| {
            AlternativeRow {
                alternative: alternative.clone(),
                cluster_id: prompt.cluster_id,
            }
            .into_element()
        })
        .collect();

    if rows.len() > VISIBLE_ROWS {
        body = body.child(
            ScrollArea::new()
                .width(Size::fill())
                .height(Size::px(LIST_MAX_H))
                .spacing(ROW_GAP)
                .scrollbar_gutter(true)
                .children(rows),
        );
    } else if !rows.is_empty() {
        body = body.child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(ROW_GAP)
                .children(rows),
        );
    }

    let cancel = dispatch.clone();
    let anyway = dispatch;

    body.child(
        rect()
            .horizontal()
            .width(Size::fill())
            .main_align(Alignment::End)
            .spacing(8.)
            .child(
                Button::new()
                    .secondary()
                    .on_press(move |_| cancel.dismiss_flagged_install())
                    .text("Cancel"),
            )
            .child(
                Button::new()
                    .danger()
                    .on_press(move |_| anyway.install_flagged_anyway(prompt.clone()))
                    .text("Install anyway"),
            ),
    )
}

#[derive(PartialEq)]
struct AlternativeRow {
    alternative: ResolvedAlternative,
    cluster_id: i64,
}

impl Component for AlternativeRow {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let icon_query = use_cached_image(self.alternative.icon_url.clone(), 256);
        let icon = remote_icon(self.alternative.icon_url.as_deref(), &icon_query, ICON_SIZE);
        alternative_row(self.alternative.clone(), self.cluster_id, icon, dispatch)
    }
}

fn alternative_row(
    alternative: ResolvedAlternative,
    cluster_id: i64,
    icon: Element,
    dispatch: crate::Actions,
) -> impl IntoElement {
    let ResolvedAlternative {
        provider,
        project_id,
        name,
        version_number,
        ..
    } = alternative;

    rect()
        .horizontal()
        .content(Content::Flex)
        .width(Size::fill())
        .height(Size::px(ROW_H))
        .cross_align(Alignment::Center)
        .spacing(10.)
        .padding(Gaps::new_symmetric(0., 8.))
        .corner_radius(CornerRadius::new_all(8.))
        .background(colors::component_bg())
        .border(border_all_color(1., colors::component_border()))
        .child(icon)
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.))
                .spacing(2.)
                .child(
                    label()
                        .text(name)
                        .font_size(13.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .max_lines(1)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(version_number)
                        .font_size(11.)
                        .max_lines(1)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(
            Button::new()
                .primary()
                .small()
                .on_press(move |_| {
                    dispatch.dismiss_flagged_install();
                    let _ = RouterContext::get().push(Route::BrowserPackage {
                        cluster_id,
                        package_type: "mod".to_string(),
                        package_id: format!("{}:{}", provider as u8, project_id),
                    });
                })
                .child(Icon::new(IconType::ArrowRight).size(14.))
                .text("Go to page"),
        )
}
