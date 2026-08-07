//! Only lists packages installed from the browser bundle content has its own flow

use std::collections::HashMap;

use freya::prelude::*;
use oneclient_content::packages::{CachedPackageMeta, ProviderId};
use oneclient_core::BrowserPackageUpdate;
use oneclient_db::models::ClusterId;

use crate::components::{Button, Dropdown, Icon, IconType, OverlayPopup, ScrollArea};
use crate::hooks::{
    package_meta_batch, use_dispatch, use_notifications_snapshot, use_package_meta_batch,
};
use crate::notifications::PackageUpdateGroup;
use crate::theme::colors;
use crate::ui::border_all_color;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);
const DIALOG_W: f32 = 520.;
const DIALOG_H: f32 = 420.;
const CHOICE_DROPDOWN_W: f32 = 150.;

type MetaMap = HashMap<(ProviderId, String), CachedPackageMeta>;

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

        let Some(groups) = groups.filter(|groups| !groups.is_empty()) else {
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
                    .child(dialog(&groups, &meta, dispatch, choices)),
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

fn dialog(
    groups: &[PackageUpdateGroup],
    meta: &MetaMap,
    dispatch: crate::Actions,
    choices: State<HashMap<RowKey, RowChoice>>,
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
        .child(content(groups, meta, dispatch, choices))
}

fn content(
    groups: &[PackageUpdateGroup],
    meta: &MetaMap,
    dispatch: crate::Actions,
    choices: State<HashMap<RowKey, RowChoice>>,
) -> impl IntoElement {
    let dismiss = dispatch.clone();
    let proceed_dispatch = dispatch.clone();
    let total: usize = groups.iter().map(|group| group.packages.len()).sum();

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
        .child(update_list(groups, meta, groups.len() > 1, choices))
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
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
                            let answers = choices.read();
                            for update in &all {
                                match answers
                                    .get(&row_key(update))
                                    .copied()
                                    .unwrap_or_default()
                                {
                                    RowChoice::Update => {
                                        proceed_dispatch.apply_package_update(update.clone());
                                    }
                                    RowChoice::Skip => {}
                                    RowChoice::SkipVersion => proceed_dispatch
                                        .skip_package_update(
                                            update.cluster_id,
                                            update.hash.clone(),
                                        ),
                                }
                            }
                            proceed_dispatch.close_package_updates();
                        })
                        .child(Icon::new(IconType::DownloadCloud02).size(15.))
                        .text("Proceed"),
                ),
        )
}

fn update_list(
    groups: &[PackageUpdateGroup],
    meta: &MetaMap,
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
                UpdateRow::new(resolve_name(update, meta), update.clone(), choice, choices)
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
    update: BrowserPackageUpdate,
    choice: RowChoice,
    choices: State<HashMap<RowKey, RowChoice>>,
    key: DiffKey,
}

impl UpdateRow {
    fn new(
        name: String,
        update: BrowserPackageUpdate,
        choice: RowChoice,
        choices: State<HashMap<RowKey, RowChoice>>,
    ) -> Self {
        Self {
            name,
            update,
            choice,
            choices,
            key: DiffKey::None,
        }
    }
}

impl PartialEq for UpdateRow {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.update == other.update && self.choice == other.choice
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
                    .child(
                        label()
                            .text(versions)
                            .font_size(11.)
                            .max_lines(1)
                            .width(Size::fill())
                            .color(colors::fg_secondary()),
                    ),
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
