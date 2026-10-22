//! Only lists packages installed from the browser bundle content has its own flow

use std::collections::HashMap;

use freya::prelude::*;
use oneclient_common::{PackageUpdateMode, Patch};
use oneclient_content::packages::types::version_web_url;
use oneclient_content::packages::{CachedPackageMeta, ProviderId};
use oneclient_core::{BrowserPackageUpdate, ProfileUpdate};
use oneclient_db::models::ClusterId;

use crate::components::{
    Button, Dropdown, Icon, IconType, OverlayPopup, ScrollArea, Spinner, checkbox_labeled,
};
use crate::hooks::{
    package_meta_batch, project_web_urls, query_is_loading, use_dispatch, use_link_confirm,
    use_notifications_snapshot, use_package_meta_batch, use_project_web_urls,
};
use crate::notifications::PackageUpdateGroup;
use crate::theme::colors;
use crate::ui::border_all_color;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);
const DIALOG_W: f32 = 520.;
const DIALOG_H: f32 = 420.;
const CHOICE_DROPDOWN_W: f32 = 150.;

type MetaMap = HashMap<(ProviderId, String), CachedPackageMeta>;
/// CurseForge project pages by project id Modrinth links resolve from ids alone
struct PageMap {
    urls: HashMap<String, String>,
    loading: bool,
}

/// `Skip` is re-offered next launch `SkipVersion` persists the decline until a newer version
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum RowChoice {
    #[default]
    Update,
    Skip,
    SkipVersion,
}

impl RowChoice {
    const ALL: [Self; 3] = [Self::Update, Self::Skip, Self::SkipVersion];

    fn label(self) -> &'static str {
        match self {
            Self::Update => "Update",
            Self::Skip => "Skip for now",
            Self::SkipVersion => "Skip this version",
        }
    }

    fn options() -> Vec<String> {
        Self::ALL.iter().map(|c| c.label().to_string()).collect()
    }
}

type RowKey = (ClusterId, String);

fn row_key(update: &BrowserPackageUpdate) -> RowKey {
    (update.cluster_id, update.hash.clone())
}

#[derive(PartialEq)]
pub struct PackageUpdatePopup;

impl Component for PackageUpdatePopup {
    fn render(&self) -> impl IntoElement {
        let snapshot = use_notifications_snapshot();
        let dispatch = use_dispatch();

        // Absent entries default to Update so groups arriving mid-modal need no reconciliation
        let choices = use_state(HashMap::<RowKey, RowChoice>::new);
        let mut dont_show_again = use_state(|| false);

        let groups = snapshot.package_updates.clone();

        // Hooks run unconditionally so this must precede the early return below
        let mut meta = MetaMap::new();
        for provider in ProviderId::REMOTE_PROVIDERS.iter().copied() {
            let ids: Vec<String> = groups
                .iter()
                .flatten()
                .flat_map(|group| &group.packages)
                .filter(|update| update.provider == provider)
                .map(|update| update.project_id.clone())
                .collect();
            let query = use_package_meta_batch(provider, ids);
            for (project_id, cached) in package_meta_batch(&query) {
                meta.insert((provider, project_id), cached);
            }
        }

        let curseforge_ids: Vec<String> = groups
            .iter()
            .flatten()
            .flat_map(|group| &group.packages)
            .filter(|update| update.provider == ProviderId::CurseForge)
            .map(|update| update.project_id.clone())
            .collect();
        let pages_query = use_project_web_urls(ProviderId::CurseForge, curseforge_ids);
        let pages = PageMap {
            urls: project_web_urls(&pages_query),
            loading: query_is_loading(&pages_query),
        };

        let Some(groups) = groups.filter(|groups| !groups.is_empty()) else {
            // The popup stays mounted, so the tick must not survive into the next prompt
            dont_show_again.set_if_modified(false);
            return rect().into_element();
        };

        let close = dispatch.clone();

        OverlayPopup::new()
            .on_close(move |_| close.close_package_updates())
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(dialog(
                        &groups,
                        &meta,
                        &pages,
                        dispatch,
                        choices,
                        dont_show_again,
                    )),
            )
            .into_element()
    }
}

fn resolve_name(update: &BrowserPackageUpdate, meta: &MetaMap) -> String {
    meta.get(&(update.provider, update.project_id.clone()))
        .map(|cached| cached.name.clone())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| update.display_name.clone())
}

fn changelog_url(update: &BrowserPackageUpdate, pages: &PageMap) -> Option<String> {
    let project_url = pages.urls.get(&update.project_id).map(String::as_str);
    version_web_url(
        update.provider,
        &update.project_id,
        &update.latest_version_id,
        project_url,
    )
}

fn changelog_loading(update: &BrowserPackageUpdate, pages: &PageMap) -> bool {
    update.provider == ProviderId::CurseForge
        && pages.loading
        && !pages.urls.contains_key(&update.project_id)
}

fn dialog(
    groups: &[PackageUpdateGroup],
    meta: &MetaMap,
    pages: &PageMap,
    dispatch: crate::Actions,
    choices: State<HashMap<RowKey, RowChoice>>,
    dont_show_again: State<bool>,
) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::px(DIALOG_W))
        .height(Size::px(DIALOG_H))
        .max_width(Size::window_percent(95.))
        .overflow(Overflow::Clip)
        .corner_radius(CornerRadius::new_all(16.))
        .background(CARD_BG)
        .border(border_all_color(1., colors::component_border()))
        .shadow(Shadow::from((
            0.,
            18.,
            52.,
            0.,
            Color::from_argb(150, 0, 0, 0),
        )))
        .child(content(
            groups,
            meta,
            pages,
            dispatch,
            choices,
            dont_show_again,
        ))
}

fn content(
    groups: &[PackageUpdateGroup],
    meta: &MetaMap,
    pages: &PageMap,
    dispatch: crate::Actions,
    choices: State<HashMap<RowKey, RowChoice>>,
    dont_show_again: State<bool>,
) -> impl IntoElement {
    let dismiss = dispatch.clone();
    let proceed_dispatch = dispatch.clone();
    let dsa_dispatch = dispatch.clone();
    let total: usize = groups.iter().map(|group| group.packages.len()).sum();
    let clusters: Vec<ClusterId> = groups.iter().map(|group| group.cluster_id).collect();

    let subtitle = match groups {
        [only] => format!(
            "{total} package{} in {} can be updated. Pick what happens to each, then Proceed.",
            if total == 1 { "" } else { "s" },
            only.cluster_name
        ),
        _ => format!(
            "{total} package{} across {} clusters can be updated. Pick what happens to each, then Proceed.",
            if total == 1 { "" } else { "s" },
            groups.len()
        ),
    };

    let all: Vec<BrowserPackageUpdate> = groups
        .iter()
        .flat_map(|group| group.packages.iter().cloned())
        .collect();

    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .content(Content::Flex)
        .padding(Gaps::new_all(22.))
        .spacing(14.)
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(3.)
                .child(
                    label()
                        .text("Updates available")
                        .font_size(17.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(subtitle)
                        .font_size(12.5)
                        .max_lines(3)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(update_list(groups, meta, pages, groups.len() > 1, choices))
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .cross_align(Alignment::Center)
                .main_align(Alignment::SpaceBetween)
                .spacing(8.)
                .child(
                    rect()
                        .direction(Direction::Horizontal)
                        .main_align(Alignment::Start)
                        .cross_align(Alignment::Center)
                        .spacing(6.)
                        .child(checkbox_labeled(dont_show_again, "Don't show again?")),
                )
                .child(
                    rect()
                        .width(Size::fill())
                        .direction(Direction::Horizontal)
                        .cross_align(Alignment::Center)
                        .main_align(Alignment::End)
                        .spacing(8.)
                        .child(
                            Button::new()
                                .ghost()
                                .on_press(move |_| dismiss.close_package_updates())
                                .text("Cancel"),
                        )
                        .child(
                            Button::new()
                                .primary()
                                .on_press(move |_| {
                                    if *dont_show_again.peek() {
                                        // A cluster override outranks the global one, so the prompt
                                        // is only silenced by writing the cluster that raised it
                                        for cluster_id in &clusters {
                                            dsa_dispatch.update_cluster_profile(
                                                *cluster_id,
                                                ProfileUpdate {
                                                    browser_update_mode: Patch::Set(
                                                        PackageUpdateMode::Skip,
                                                    ),
                                                    ..Default::default()
                                                },
                                            );
                                        }
                                    }

                                    let answers = choices.read();
                                    let mut chosen = Vec::new();
                                    for update in &all {
                                        match answers
                                            .get(&row_key(update))
                                            .copied()
                                            .unwrap_or_default()
                                        {
                                            RowChoice::Update => chosen.push(update.clone()),
                                            RowChoice::Skip => {}
                                            RowChoice::SkipVersion => proceed_dispatch
                                                .skip_package_update(
                                                    update.cluster_id,
                                                    update.hash.clone(),
                                                ),
                                        }
                                    }
                                    proceed_dispatch.proceed_package_updates(chosen);
                                })
                                .child(Icon::new(IconType::DownloadCloud02).size(15.))
                                .text("Proceed"),
                        ),
                ),
        )
}

fn update_list(
    groups: &[PackageUpdateGroup],
    meta: &MetaMap,
    pages: &PageMap,
    show_headers: bool,
    choices: State<HashMap<RowKey, RowChoice>>,
) -> impl IntoElement {
    let mut scroll = ScrollArea::new()
        .width(Size::fill())
        .height(Size::flex(1.0))
        .spacing(4.);

    for (index, group) in groups.iter().enumerate() {
        if show_headers {
            scroll = scroll.child(cluster_header(&group.cluster_name, index == 0));
        }

        for update in &group.packages {
            let key = format!("{}:{}", group.cluster_id, update.hash);
            let choice = choices
                .read()
                .get(&row_key(update))
                .copied()
                .unwrap_or_default();
            scroll = scroll.child(
                UpdateRow::new(
                    resolve_name(update, meta),
                    changelog_url(update, pages),
                    changelog_loading(update, pages),
                    update.clone(),
                    choice,
                    choices,
                )
                .key(key)
                .into_element(),
            );
        }
    }

    scroll.into_element()
}

fn cluster_header(name: &str, first: bool) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .padding(Gaps::new_symmetric(6., 8.))
        .margin(Gaps::new(if first { 0. } else { 8. }, 0., 3., 0.))
        .corner_radius(CornerRadius::new_all(8.))
        .background(colors::component_bg())
        .child(
            label()
                .text(name.to_string())
                .font_size(12.)
                .font_weight(FontWeight::SEMI_BOLD)
                .max_lines(1)
                .width(Size::fill())
                .color(colors::fg_primary()),
        )
}

struct UpdateRow {
    name: String,
    changelog_url: Option<String>,
    changelog_loading: bool,
    update: BrowserPackageUpdate,
    choice: RowChoice,
    choices: State<HashMap<RowKey, RowChoice>>,
    key: DiffKey,
}

impl UpdateRow {
    fn new(
        name: String,
        changelog_url: Option<String>,
        changelog_loading: bool,
        update: BrowserPackageUpdate,
        choice: RowChoice,
        choices: State<HashMap<RowKey, RowChoice>>,
    ) -> Self {
        Self {
            name,
            changelog_url,
            changelog_loading,
            update,
            choice,
            choices,
            key: DiffKey::None,
        }
    }
}

impl PartialEq for UpdateRow {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.changelog_url == other.changelog_url
            && self.changelog_loading == other.changelog_loading
            && self.update == other.update
            && self.choice == other.choice
    }
}

impl KeyExt for UpdateRow {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for UpdateRow {
    fn render(&self) -> impl IntoElement {
        let update = self.update.clone();
        let choice = self.choice;
        let mut choices = self.choices;
        let key = row_key(&update);
        let mut confirm = use_link_confirm();
        let changelog_url = self.changelog_url.clone();
        let changelog_icon = if self.changelog_loading {
            Spinner::new(15.).into_element()
        } else {
            Icon::new(IconType::File02).size(15.).into_element()
        };

        let versions = match (
            update.installed_version_name.is_empty(),
            update.latest_version_name.is_empty(),
        ) {
            (false, false) => format!(
                "{} to {}",
                update.installed_version_name, update.latest_version_name
            ),
            (true, false) => format!("New version {}", update.latest_version_name),
            _ => "A newer version is available".to_string(),
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .cross_align(Alignment::Center)
            .spacing(10.)
            .padding(Gaps::new_symmetric(8., 10.))
            .corner_radius(CornerRadius::new_all(8.))
            .background(colors::component_bg())
            .content(Content::Flex)
            .child(
                Icon::new(IconType::RefreshCw01)
                    .size(14.)
                    .color(colors::brand()),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(2.)
                    .child(
                        label()
                            .text(self.name.clone())
                            .font_size(12.5)
                            .font_weight(FontWeight::MEDIUM)
                            .max_lines(1)
                            .width(Size::fill())
                            .color(colors::fg_primary()),
                    )
                    .child(VersionsLabel { text: versions }),
            )
            .child(
                // Stays in place while CurseForge's project page is still loading so rows don't shift
                Button::new()
                    .ghost()
                    .icon()
                    .tooltip("View changelog")
                    .enabled(changelog_url.is_some())
                    .on_press(move |_| {
                        if let Some(url) = &changelog_url {
                            confirm.set(Some(url.clone()));
                        }
                    })
                    .child(changelog_icon),
            )
            .child(
                Dropdown::new(choice.label(), RowChoice::options())
                    .width(Size::px(CHOICE_DROPDOWN_W))
                    .height(Size::px(26.))
                    .on_select(move |idx: usize| {
                        if let Some(choice) = RowChoice::ALL.get(idx) {
                            choices.write().insert(key.clone(), *choice);
                        }
                    }),
            )
    }
}

/// Wide enough that the versions line is laid out at its natural width before clipping
const VERSIONS_TRACK_PX: f32 = 4096.;

/// Cuts from the start so the version being updated to stays readable
#[derive(PartialEq)]
struct VersionsLabel {
    text: String,
}

impl Component for VersionsLabel {
    fn render(&self) -> impl IntoElement {
        let mut box_width = use_state(|| 0f32);
        let mut text_width = use_state(|| 0f32);
        let mut clip_width = use_state(|| 0f32);

        // Decided against the whole row so showing the ellipsis can't flip it back
        let overflowing = *text_width.read() > *box_width.read() + 0.5;
        let offset = if overflowing {
            (*clip_width.read() - *text_width.read()).min(0.)
        } else {
            0.
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .overflow(Overflow::Clip)
            .on_sized(move |event: Event<SizedEventData>| {
                let width = event.data().area.width();
                if (width - *box_width.peek()).abs() > 0.5 {
                    box_width.set(width);
                }
            })
            .maybe(overflowing, |el| el.child(versions_text("…".to_string())))
            .child(
                rect()
                    .width(Size::flex(1.0))
                    .overflow(Overflow::Clip)
                    .on_sized(move |event: Event<SizedEventData>| {
                        let width = event.data().area.width();
                        if (width - *clip_width.peek()).abs() > 0.5 {
                            clip_width.set(width);
                        }
                    })
                    .child(
                        rect().width(Size::px(VERSIONS_TRACK_PX)).child(
                            rect()
                                .offset_x(offset)
                                .on_sized(move |event: Event<SizedEventData>| {
                                    let width = event.data().area.width();
                                    if (width - *text_width.peek()).abs() > 0.5 {
                                        text_width.set(width);
                                    }
                                })
                                .child(versions_text(self.text.clone())),
                        ),
                    ),
            )
    }
}

fn versions_text(text: String) -> impl IntoElement {
    label()
        .text(text)
        .font_size(11.)
        .max_lines(1)
        .color(colors::fg_secondary())
}
