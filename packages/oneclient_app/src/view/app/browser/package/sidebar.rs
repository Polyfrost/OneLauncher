use super::*;

use oneclient_core::packages::types::{
    ProjectDetail, ProjectMember,
};
use oneclient_core::packages::ProviderId;

use crate::BridgeDispatch;
use crate::components::{Button, Icon, IconType};
use crate::theme::colors;
use crate::ui::border_all_color;


pub(super) fn sidebar(
    project: Option<ProjectDetail>,
    latest_version: Option<String>,
    provider: ProviderId,
    cluster_id: i64,
    dispatch: BridgeDispatch,
    confirm: State<Option<String>>,
) -> impl IntoElement {
    let Some(project) = project else {
        return rect()
            .width(Size::px(SIDEBAR_W))
            .min_width(Size::px(SIDEBAR_W))
            .into_element();
    };

    let project_id = project.id.clone();
    let can_install = latest_version.is_some();

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
                .child(PackageBanner::new(project.icon_url.clone(), 110.))
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
                        .child(
                            rect()
                                .horizontal()
                                .cross_align(Alignment::Center)
                                .spacing(4.)
                                .child(Icon::new(project.provider).size(12.))
                                .child(
                                    label()
                                        .text(format!(
                                            "{} on {}",
                                            project.content_type, project.provider
                                        ))
                                        .font_size(11.)
                                        .color(colors::fg_secondary()),
                                ),
                        )
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
                .text("Install latest"),
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

fn card(title: &str, rows: Vec<Element>) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
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

fn author_row(member: &ProjectMember, mut confirm: State<Option<String>>) -> Element {
    let url = member.url.clone();
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(8.)
        .corner_radius(CornerRadius::new_all(6.))
        .padding(Gaps::new_all(4.))
        .maybe(url.is_some(), |el| {
            let url = url.clone().unwrap_or_default();
            el.on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
                .on_press(move |_| confirm.set(Some(url.clone())))
        })
        .child(Thumbnail::new(member.avatar_url.clone(), 32.).radius(6.))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .child(
                    label()
                        .text(member.name.clone())
                        .font_size(12.)
                        .max_lines(1)
                        .color(colors::fg_primary()),
                )
                .maybe(!member.role.is_empty(), |el| {
                    el.child(
                        label()
                            .text(member.role.clone())
                            .font_size(10.)
                            .max_lines(1)
                            .color(colors::fg_secondary()),
                    )
                }),
        )
        .into_element()
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
    card("Links", rows)
}

fn link_row(label_text: String, url: String, mut confirm: State<Option<String>>) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(8.)
        .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
        .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
        .on_press(move |_| confirm.set(Some(url.clone())))
        .child(
            Icon::new(IconType::Link03)
                .size(14.)
                .color(colors::code_info()),
        )
        .child(
            label()
                .text(label_text)
                .font_size(12.)
                .color(colors::code_info()),
        )
        .into_element()
}
