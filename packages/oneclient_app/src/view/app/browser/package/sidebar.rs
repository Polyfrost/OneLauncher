use super::*;

use oneclient_content::packages::ProviderId;
use oneclient_content::packages::types::{ProjectDetail, ProjectMember};

use crate::Actions;
use crate::components::{Button, Icon, IconType};
use crate::theme::colors;
use crate::ui::border_all_color;

#[allow(clippy::too_many_arguments)]
pub(super) fn sidebar(
    project: Option<ProjectDetail>,
    latest_version: Option<String>,
    provider: ProviderId,
    cluster_id: i64,
    dispatch: Actions,
    confirm: State<Option<String>>,
    installed: Option<Installed>,
    installing: bool,
) -> impl IntoElement {
    let Some(project) = project else {
        return rect()
            .width(Size::px(SIDEBAR_W))
            .min_width(Size::px(SIDEBAR_W))
            .into_element();
    };

    let project_id = project.id.clone();
    // Nothing to do when this version is already there or while an install is running
    let have_latest = match (&installed, &latest_version) {
        (Some(installed), Some(latest)) => installed.is_version(latest),
        _ => false,
    };
    let can_install = latest_version.is_some() && !have_latest && !installing;

    rect()
        .vertical()
        .width(Size::px(SIDEBAR_W))
        .min_width(Size::px(SIDEBAR_W))
        .spacing(12.)
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .corner_radius(CornerRadius::new_all(12.))
                .overflow(Overflow::Clip)
                .background(PANEL_BG)
                .border(border_all_color(1., colors::component_border()))
                .child(
                    rect()
                        .width(Size::fill())
                        .overflow(Overflow::Clip)
                        .child(PackageBanner::new(project.icon_url.clone(), 110.))
                        .maybe_child(installed.as_ref().map(|installed| {
                            rect()
                                .position(Position::new_absolute().top(8.).left(8.))
                                .layer(Layer::Relative(7))
                                .child(installed_badge_overlay(installed.source))
                                .into_element()
                        })),
                )
                .child(
                    rect()
                        .vertical()
                        .width(Size::fill())
                        .padding(Gaps::new_all(12.))
                        .spacing(8.)
                        .child(
                            label()
                                .text(project.name.clone())
                                .font_size(18.)
                                .font_weight(FontWeight::BOLD)
                                .max_lines(2)
                                .color(colors::fg_primary()),
                        )
                        .child(ProviderTag {
                            text: format!("{} on {}", project.content_type, project.provider),
                            provider: project.provider,
                            url: provider_project_url(&project),
                            confirm,
                        })
                        .maybe(!project.summary.is_empty(), |el| {
                            el.child(
                                label()
                                    .text(project.summary.clone())
                                    .font_size(12.)
                                    .max_lines(4)
                                    .color(colors::fg_secondary()),
                            )
                        })
                        .child(
                            rect()
                                .horizontal()
                                .cross_align(Alignment::Center)
                                .spacing(4.)
                                .child(
                                    Icon::new(IconType::Download01)
                                        .size(12.)
                                        .color(colors::fg_secondary()),
                                )
                                .child(
                                    label()
                                        .text(format!(
                                            "{} downloads",
                                            abbreviate_number(project.downloads)
                                        ))
                                        .font_size(11.)
                                        .color(colors::fg_secondary()),
                                ),
                        ),
                ),
        )
        .child(
            Button::new()
                .primary()
                .width(Size::fill())
                .enabled(can_install)
                .on_press(move |_| {
                    if let Some(version_id) = latest_version.clone() {
                        dispatch.install_package(
                            cluster_id,
                            provider,
                            project_id.clone(),
                            version_id,
                        );
                    }
                })
                .child(Icon::new(IconType::Download01).size(14.))
                .text(if installing {
                    "Installing..."
                } else if have_latest {
                    "Latest installed"
                } else {
                    "Install latest"
                }),
        )
        .maybe(
            !project.members.is_empty() || !project.author.is_empty(),
            |el| el.child(authors_card(&project, confirm)),
        )
        .child(details_card(&project))
        .maybe(!project.links.is_empty(), |el| {
            el.child(links_card(&project.links, confirm))
        })
        .into_element()
}

/// Modrinth doesn't hand a url out so it's rebuilt from the slug CurseForge lists it as "Website"
fn provider_project_url(project: &ProjectDetail) -> Option<String> {
    match project.provider {
        ProviderId::Modrinth => Some(format!(
            "{}{}/{}",
            ProviderId::Modrinth.website(),
            project.content_type.modrinth_type(),
            project.slug
        )),
        ProviderId::CurseForge => project
            .links
            .iter()
            .find(|(label, _)| label == "Website")
            .map(|(_, url)| url.clone()),
        ProviderId::Local => None,
    }
}

/// Plain text when the provider has no page so a local package doesn't look clickable
#[derive(PartialEq)]
struct ProviderTag {
    text: String,
    provider: ProviderId,
    url: Option<String>,
    confirm: State<Option<String>>,
}

impl Component for ProviderTag {
    fn render(&self) -> impl IntoElement {
        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let focused = focus().is_focused();

        let url = self.url.clone();
        let interactive = url.is_some();
        let mut confirm = self.confirm;

        rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(4.)
            .child(Icon::new(self.provider).size(12.))
            .child(
                label()
                    .text(self.text.clone())
                    .font_size(11.)
                    .color(if interactive {
                        colors::code_info()
                    } else {
                        colors::fg_secondary()
                    }),
            )
            .maybe(interactive, |el| {
                let url = url.clone().unwrap_or_default();
                el.a11y_id(a11y_id)
                    .a11y_focusable(true)
                    .a11y_role(AccessibilityRole::Link)
                    .corner_radius(CornerRadius::new_all(6.))
                    .maybe(focused, |el| {
                        el.border(border_all_color(1., colors::brand()))
                    })
                    .cursor(CursorIcon::Pointer)
                    .on_all_press(move |_| confirm.set(Some(url.clone())))
            })
    }
}

fn card(title: &str, rows: Vec<Element>) -> impl IntoElement {
    card_spaced(title, rows, 10.)
}

/// `card` with a caller-chosen row gap one-line link lists read better tight
fn card_spaced(title: &str, rows: Vec<Element>, spacing: f32) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(spacing)
        .padding(Gaps::new_all(16.))
        .corner_radius(CornerRadius::new_all(12.))
        .background(PANEL_BG)
        .border(border_all_color(1., colors::component_border()))
        .child(
            label()
                .text(title.to_string())
                .font_size(13.)
                .font_weight(FontWeight::BOLD)
                .color(colors::fg_primary()),
        )
        .children(rows)
        .into_element()
}

fn authors_card(project: &ProjectDetail, confirm: State<Option<String>>) -> impl IntoElement {
    let rows: Vec<Element> = if project.members.is_empty() {
        vec![author_row(
            &ProjectMember {
                name: project.author.clone(),
                role: "Author".to_string(),
                url: None,
                avatar_url: None,
            },
            confirm,
        )]
    } else {
        project
            .members
            .iter()
            .map(|m| author_row(m, confirm))
            .collect()
    };
    card("Authors", rows)
}

fn author_row(member: &ProjectMember, confirm: State<Option<String>>) -> Element {
    AuthorRow {
        name: member.name.clone(),
        role: member.role.clone(),
        url: member.url.clone(),
        avatar_url: member.avatar_url.clone(),
        confirm,
    }
    .into_element()
}

#[derive(PartialEq)]
struct AuthorRow {
    name: String,
    role: String,
    url: Option<String>,
    avatar_url: Option<String>,
    confirm: State<Option<String>>,
}

impl Component for AuthorRow {
    fn render(&self) -> impl IntoElement {
        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let focused = focus().is_focused();

        let url = self.url.clone();
        let interactive = url.is_some();
        let mut confirm = self.confirm;

        rect()
            .horizontal()
            .width(Size::fill())
            .cross_align(Alignment::Center)
            .spacing(8.)
            .corner_radius(CornerRadius::new_all(6.))
            .padding(Gaps::new_all(4.))
            .maybe(interactive, |el| {
                let url = url.clone().unwrap_or_default();
                el.a11y_id(a11y_id)
                    .a11y_focusable(true)
                    .a11y_role(AccessibilityRole::Button)
                    .cursor(CursorIcon::Pointer)
                    .on_all_press(move |_| confirm.set(Some(url.clone())))
            })
            .maybe(interactive && focused, |el| {
                el.border(border_all_color(1., colors::brand()))
            })
            .child(Thumbnail::new(self.avatar_url.clone(), 32.).radius(6.))
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .child(
                        label()
                            .text(self.name.clone())
                            .font_size(12.)
                            .max_lines(1)
                            .color(colors::fg_primary()),
                    )
                    .maybe(!self.role.is_empty(), |el| {
                        el.child(
                            label()
                                .text(self.role.clone())
                                .font_size(10.)
                                .max_lines(1)
                                .color(colors::fg_secondary()),
                        )
                    }),
            )
    }
}

fn details_card(project: &ProjectDetail) -> impl IntoElement {
    let mut rows: Vec<Element> = Vec::new();
    if !project.game_versions.is_empty() {
        rows.push(pill_detail("Versions", &project.game_versions));
    }
    if !project.loaders.is_empty() {
        let loaders: Vec<String> = project.loaders.iter().map(|l| l.to_string()).collect();
        rows.push(pill_detail("Loaders", &loaders));
    }
    if let Some(license) = &project.license {
        rows.push(detail_row(IconType::Key01, "License", license));
    }
    rows.push(detail_row(
        IconType::Calendar,
        "Created",
        &project.created.format("%Y-%m-%d").to_string(),
    ));
    rows.push(detail_row(
        IconType::ClockRewind,
        "Updated",
        &project.updated.format("%Y-%m-%d").to_string(),
    ));
    card("Details", rows)
}

fn detail_row(icon: IconType, key: &str, value: &str) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .spacing(8.)
        .content(Content::Flex)
        .child(
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(8.)
                .child(Icon::new(icon).size(14.).color(colors::fg_secondary()))
                .child(
                    label()
                        .text(key.to_string())
                        .font_size(12.)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(
            rect()
                .width(Size::flex(1.0))
                .main_align(Alignment::End)
                .child(
                    label()
                        .text(value.to_string())
                        .font_size(12.)
                        .max_lines(3)
                        .width(Size::fill())
                        .color(colors::fg_primary()),
                ),
        )
        .into_element()
}

fn links_card(links: &[(String, String)], confirm: State<Option<String>>) -> impl IntoElement {
    let rows: Vec<Element> = links
        .iter()
        .map(|(lbl, url)| link_row(lbl.clone(), url.clone(), confirm))
        .collect();
    card_spaced("Links", rows, 2.)
}

fn link_row(label_text: String, url: String, confirm: State<Option<String>>) -> Element {
    LinkRow {
        label_text,
        url,
        confirm,
    }
    .into_element()
}

#[derive(PartialEq)]
struct LinkRow {
    label_text: String,
    url: String,
    confirm: State<Option<String>>,
}

impl Component for LinkRow {
    fn render(&self) -> impl IntoElement {
        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let focused = focus().is_focused();

        let url = self.url.clone();
        let mut confirm = self.confirm;

        rect()
            .horizontal()
            .width(Size::fill())
            .cross_align(Alignment::Center)
            .spacing(8.)
            .corner_radius(CornerRadius::new_all(6.))
            .padding(Gaps::new_symmetric(2., 4.))
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Link)
            .maybe(focused, |el| {
                el.border(border_all_color(1., colors::brand()))
            })
            .cursor(CursorIcon::Pointer)
            .on_all_press(move |_| confirm.set(Some(url.clone())))
            .child(
                Icon::new(IconType::Link03)
                    .size(14.)
                    .color(colors::code_info()),
            )
            .child(
                label()
                    .text(self.label_text.clone())
                    .font_size(12.)
                    .color(colors::code_info()),
            )
    }
}
