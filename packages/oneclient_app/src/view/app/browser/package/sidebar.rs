use super::*;

use oneclient_content::packages::ProviderId;
use oneclient_content::packages::types::{ProjectDetail, ProjectMember, VersionSummary};

use crate::components::{Button, Icon, IconType};
use crate::i18n::{t, tf};
use crate::utils::shorten;
use crate::theme::colors;
use crate::ui::border_all_color;

const BUTTON_NAME_CHARS: usize = 22;

pub(super) fn sidebar(
    project: Option<ProjectDetail>,
    latest_version: Option<VersionSummary>,
    installer: Installer,
    confirm: State<Option<String>>,
    installed: Option<Installed>,
    installing: bool,
    waiting: bool,
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
        (Some(installed), Some(latest)) => installed.is_version(&latest.version_id),
        _ => false,
    };
    let can_install = latest_version.is_some() && !have_latest && !installing && !waiting;
    let import_prompt = installer.import_prompt;
    let enable = installed
        .as_ref()
        .and_then(Installed::disabled_bundled)
        .and_then(|version| version.enable_action(installer.cluster_id));

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
                .background(colors::page_elevated())
                .border(border_all_color(1., colors::component_border()))
                .child(
                    rect()
                        .width(Size::fill())
                        .overflow(Overflow::Clip)
                        .child(PackageBanner::new(project.icon_url.clone(), 110.)),
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
                            text: tf(
                                "oneclient.browser.package.sidebar.provider",
                                &[&project.content_type, &project.provider],
                            ),
                            provider: project.provider,
                            url: project.web_url(),
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
                                        .text(tf(
                                            "oneclient.browser.package.sidebar.downloads",
                                            &[&abbreviate_number(project.downloads)],
                                        ))
                                        .font_size(11.)
                                        .color(colors::fg_secondary()),
                                ),
                        ),
                ),
        )
        .child(match enable {
            Some(action) => EnableButton {
                action,
                variant: EnableVariant::Sidebar,
            }
            .into_element(),
            None => Button::new()
                .primary()
                .width(Size::fill())
                .enabled(can_install)
                .on_press({
                    let installer = installer.clone();
                    move |_| {
                        if let Some(latest) = latest_version.clone() {
                            installer.install_latest(
                                project_id.clone(),
                                latest.version_id,
                                latest.version_number,
                            );
                        }
                    }
                })
                .child(Icon::new(IconType::Download01).size(14.))
                .text(if installing && installer.add_to_cluster {
                    t("oneclient.browser.package.sidebar.adding").to_string()
                } else if installing {
                    t("oneclient.browser.package.sidebar.installing").to_string()
                } else if installer.add_to_cluster {
                    match &installer.cluster_name {
                        Some(name) => tf(
                            "oneclient.browser.package.sidebar.add_to",
                            &[&shorten(name, BUTTON_NAME_CHARS, "\u{2026}")],
                        ),
                        None => t("oneclient.browser.package.sidebar.add_to_this").to_string(),
                    }
                } else if installer.modpack {
                    t("oneclient.browser.package.sidebar.install_ellipsis").to_string()
                } else if have_latest {
                    t("oneclient.browser.package.sidebar.latest_installed").to_string()
                } else {
                    t("oneclient.browser.package.sidebar.install_latest").to_string()
                })
                .into_element(),
        })
        .maybe_child(import_prompt.map(|mut prompt| {
            Button::new()
                .secondary()
                .width(Size::fill())
                .enabled(!installing && !waiting)
                .on_press(move |_| prompt.set(true))
                .child(Icon::new(IconType::Plus).size(14.))
                .text(t("oneclient.browser.package.sidebar.add_to_instance"))
                .into_element()
        }))
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
                    .on_press(move |_| confirm.set(Some(url.clone())))
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
        .background(colors::page_elevated())
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
                role: t("oneclient.browser.package.sidebar.author").to_string(),
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
    card(t("oneclient.browser.package.sidebar.authors"), rows)
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
                    .on_press(move |_| confirm.set(Some(url.clone())))
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
        rows.push(pill_detail(
            t("oneclient.browser.package.sidebar.versions"),
            &project.game_versions,
        ));
    }
    if !project.loaders.is_empty() {
        let loaders: Vec<String> = project.loaders.iter().map(|l| l.real_name()).collect();
        rows.push(pill_detail(
            t("oneclient.browser.package.sidebar.loaders"),
            &loaders,
        ));
    }
    if let Some(license) = &project.license {
        rows.push(detail_row(
            IconType::Key01,
            t("oneclient.browser.package.sidebar.license"),
            license,
        ));
    }
    rows.push(detail_row(
        IconType::Calendar,
        t("oneclient.browser.package.sidebar.created"),
        &project.created.format("%Y-%m-%d").to_string(),
    ));
    rows.push(detail_row(
        IconType::ClockRewind,
        t("oneclient.browser.package.sidebar.updated"),
        &project.updated.format("%Y-%m-%d").to_string(),
    ));
    card(t("oneclient.browser.package.sidebar.details"), rows)
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
    card_spaced(t("oneclient.browser.package.sidebar.links"), rows, 2.)
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
            .on_press(move |_| confirm.set(Some(url.clone())))
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
