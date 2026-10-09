use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_content::packages::ResolvedAlternative;

use crate::components::{
    Button, Icon, IconType, Markdown, MarkdownStyle, OverlayPopup, remote_icon,
};
use crate::hooks::{use_cached_image, use_dispatch, use_installs_snapshot};
use crate::routes::Route;
use crate::state::FlaggedInstallPrompt;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::view::app::browser::encode_package_id;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);
const DIALOG_W: f32 = 460.;
const ROW_H: f32 = 48.;
const ICON_SIZE: f32 = 32.;
const ROW_GAP: f32 = 6.;
const SECTION_GAP: f32 = 14.;
const CONTENT_MAX_H: f32 = 400.;
const DIALOG_PADDING: f32 = 20.;

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
        .height(Size::Inner)
        .max_width(Size::window_percent(90.))
        .max_height(Size::window_percent(85.))
        .overflow(Overflow::Clip)
        .spacing(SECTION_GAP)
        .padding(Gaps::new_all(DIALOG_PADDING))
        .corner_radius(CornerRadius::new_all(14.))
        .background(CARD_BG)
        .border(border_all_color(1., colors::component_border()))
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(SECTION_GAP)
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
                ),
        );

    if prompt.explanation.is_some() || !prompt.alternatives.is_empty() {
        body = body.child(flagged_content(&prompt));
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

fn flagged_content(prompt: &FlaggedInstallPrompt) -> impl IntoElement {
    let mut content = rect().vertical().width(Size::fill()).spacing(SECTION_GAP);

    if let Some(markdown) = &prompt.explanation {
        content = content.child(explanation_panel(markdown.clone()));
    }

    if !prompt.alternatives.is_empty() {
        content = content.child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(ROW_GAP)
                .children(prompt.alternatives.iter().map(|alternative| {
                    AlternativeRow {
                        alternative: alternative.clone(),
                        cluster_id: prompt.cluster_id,
                        mc_version: prompt.mc_version.clone(),
                    }
                    .into_element()
                })),
        );
    }

    ScrollView::new()
        .width(Size::fill())
        .height(Size::Inner)
        .max_height(Size::px(CONTENT_MAX_H))
        .child(content)
}

#[derive(PartialEq)]
struct AlternativeRow {
    alternative: ResolvedAlternative,
    cluster_id: i64,
    mc_version: String,
}

impl Component for AlternativeRow {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let icon_query = use_cached_image(self.alternative.icon_url.clone(), 256);
        let icon = remote_icon(self.alternative.icon_url.as_deref(), &icon_query, ICON_SIZE);
        let version = self
            .alternative
            .version_number
            .clone()
            .unwrap_or_else(|| format!("No version for {}", self.mc_version));
        alternative_row(
            self.alternative.clone(),
            version,
            self.cluster_id,
            icon,
            dispatch,
        )
    }
}

fn alternative_row(
    alternative: ResolvedAlternative,
    version: String,
    cluster_id: i64,
    icon: Element,
    dispatch: crate::Actions,
) -> impl IntoElement {
    let ResolvedAlternative {
        provider,
        project_id,
        name,
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
                        .text(version)
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
                        package_id: encode_package_id(provider, &project_id),
                    });
                })
                .child(Icon::new(IconType::ArrowRight).size(14.))
                .text("Go to page"),
        )
}

pub(crate) fn explanation_panel(markdown: String) -> impl IntoElement {
    rect()
        .width(Size::fill())
        .padding(Gaps::new_all(12.))
        .corner_radius(CornerRadius::new_all(8.))
        .background(colors::component_bg())
        .border(border_all_color(1., colors::component_border()))
        .child(
            Markdown::new(markdown)
                .width(Size::fill())
                .style(MarkdownStyle {
                    color: colors::fg_primary(),
                    color_link: colors::code_info(),
                    color_code: colors::fg_primary(),
                    background_code: CARD_BG,
                    background_blockquote: CARD_BG,
                    border_blockquote: colors::component_border(),
                    background_divider: colors::component_border(),
                    headings: [16., 14., 13., 12., 12., 12.],
                    paragraph_size: 12.,
                    code_font_size: 11.,
                    ..MarkdownStyle::default()
                }),
        )
}
