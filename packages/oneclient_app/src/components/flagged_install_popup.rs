use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_content::packages::ResolvedAlternative;

use crate::components::{
    Button, Icon, IconType, Markdown, MarkdownStyle, OverlayPopup, ScrollArea, remote_icon,
};
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
const SECTION_GAP: f32 = 14.;
const CONTENT_MAX_H: f32 = 400.;
const CONTENT_MIN_H: f32 = 120.;
const DIALOG_PADDING: f32 = 20.;
const WINDOW_MARGIN: f32 = 16.;

#[derive(PartialEq)]
pub struct FlaggedInstallPopup;

impl Component for FlaggedInstallPopup {
    fn render(&self) -> impl IntoElement {
        let installs = use_installs_snapshot();
        let dispatch = use_dispatch();

        let Some(prompt) = installs.flagged.clone() else {
            return rect().into_element();
        };

        OverlayPopup::new()
            .on_close(move |_| dispatch.dismiss_flagged_install())
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(FlaggedDialog { prompt }),
            )
            .into_element()
    }
}

fn content_max_height(window_h: f32, header_h: f32, footer_h: f32) -> f32 {
    let room = window_h
        - WINDOW_MARGIN * 2.
        - DIALOG_PADDING * 2.
        - SECTION_GAP * 2.
        - header_h
        - footer_h;
    room.min(CONTENT_MAX_H).max(CONTENT_MIN_H)
}

fn measure_into(mut target: State<f32>) -> impl FnMut(Event<SizedEventData>) {
    move |e: Event<SizedEventData>| {
        let measured = e.area.height();
        if (*target.peek() - measured).abs() > 0.5 {
            target.set(measured);
        }
    }
}

#[derive(PartialEq)]
struct FlaggedDialog {
    prompt: FlaggedInstallPrompt,
}

impl Component for FlaggedDialog {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let header_h = use_state(|| 0f32);
        let footer_h = use_state(|| 0f32);

        let platform = Platform::get();
        let scale = (*platform.scale_factor.read() as f32).max(f32::EPSILON);
        let window_h = platform.root_size.read().height / scale;
        let max_content = content_max_height(window_h, *header_h.read(), *footer_h.read());

        dialog(
            self.prompt.clone(),
            dispatch,
            max_content,
            header_h,
            footer_h,
        )
    }
}

fn dialog(
    prompt: FlaggedInstallPrompt,
    dispatch: crate::Actions,
    max_content: f32,
    header_h: State<f32>,
    footer_h: State<f32>,
) -> impl IntoElement {
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
                .on_sized(measure_into(header_h))
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
        body = body.child(FlaggedContent {
            explanation: prompt.explanation.clone(),
            alternatives: prompt.alternatives.clone(),
            cluster_id: prompt.cluster_id,
            mc_version: prompt.mc_version.clone(),
            max_height: max_content,
        });
    }

    let cancel = dispatch.clone();
    let anyway = dispatch;

    body.child(
        rect()
            .horizontal()
            .width(Size::fill())
            .main_align(Alignment::End)
            .spacing(8.)
            .on_sized(measure_into(footer_h))
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
struct FlaggedContent {
    explanation: Option<String>,
    alternatives: Vec<ResolvedAlternative>,
    cluster_id: i64,
    mc_version: String,
    max_height: f32,
}

impl Component for FlaggedContent {
    fn render(&self) -> impl IntoElement {
        let content_h = use_state(|| 0f32);
        let height = (*content_h.read()).min(self.max_height);

        let mut content = rect()
            .vertical()
            .width(Size::fill())
            .spacing(SECTION_GAP)
            .on_sized(measure_into(content_h));

        if let Some(markdown) = &self.explanation {
            content = content.child(explanation_panel(markdown.clone()));
        }

        if !self.alternatives.is_empty() {
            content = content.child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .spacing(ROW_GAP)
                    .children(self.alternatives.iter().map(|alternative| {
                        AlternativeRow {
                            alternative: alternative.clone(),
                            cluster_id: self.cluster_id,
                            mc_version: self.mc_version.clone(),
                        }
                        .into_element()
                    })),
            );
        }

        ScrollArea::new()
            .width(Size::fill())
            .height(Size::px(height))
            .scrollbar_gutter(true)
            .child(content)
    }
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
                        package_id: format!("{}:{}", provider as u8, project_id),
                    });
                })
                .child(Icon::new(IconType::ArrowRight).size(14.))
                .text("Go to page"),
        )
}

fn explanation_panel(markdown: String) -> impl IntoElement {
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

#[cfg(test)]
mod tests {
    use freya_testing::prelude::*;

    use super::*;

    const MARKER_W: f32 = 17.;

    fn content_height(markdown: &'static str) -> f32 {
        let app = move || {
            rect()
                .vertical()
                .width(Size::px(420.))
                .height(Size::px(600.))
                .child(FlaggedContent {
                    explanation: Some(markdown.to_string()),
                    alternatives: Vec::new(),
                    cluster_id: 0,
                    mc_version: String::new(),
                    max_height: CONTENT_MAX_H,
                })
                .child(rect().width(Size::px(MARKER_W)).height(Size::px(10.)))
        };

        let mut test = launch_test(app);
        for _ in 0..4 {
            test.sync_and_update();
        }

        test.find(|node, _| {
            let area = node.layout().area;
            (area.width() == MARKER_W).then(|| area.min_y())
        })
        .expect("marker node")
    }

    #[test]
    fn short_content_fits_its_text() {
        let height = content_height("This mod steals session tokens.");

        assert!(height > 20., "content collapsed to {height}");
        assert!(height < 80., "content padded out to {height}");
    }

    #[test]
    fn content_grows_past_the_old_explanation_cap() {
        let medium = "A line of explanation.\n\n".repeat(9);
        let height = content_height(Box::leak(medium.into_boxed_str()));

        assert!(height > 200., "content stopped at {height}");
        assert!(height < CONTENT_MAX_H, "content capped early at {height}");
    }

    #[test]
    fn tall_window_keeps_the_400px_cap() {
        assert_eq!(content_max_height(1080., 60., 36.), CONTENT_MAX_H);
    }

    #[test]
    fn short_window_shrinks_the_content_to_fit() {
        let height = content_max_height(500., 60., 36.);
        let dialog_h = height + 60. + 36. + SECTION_GAP * 2. + DIALOG_PADDING * 2.;

        assert!(height < CONTENT_MAX_H);
        assert!(dialog_h <= 500. - WINDOW_MARGIN * 2. + 0.01);
    }

    #[test]
    fn tiny_window_keeps_a_usable_minimum() {
        assert_eq!(content_max_height(250., 60., 36.), CONTENT_MIN_H);
    }

    #[test]
    fn long_content_is_capped_and_scrolls() {
        let long = "A line of explanation.\n\n".repeat(60);
        let height = content_height(Box::leak(long.into_boxed_str()));

        assert_eq!(height, CONTENT_MAX_H);
    }
}
