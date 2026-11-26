use freya::prelude::*;
use oneclient_content::packages::ContentType;
use oneclient_core::clusters::Cluster;

use crate::components::{Icon, IconType};
use crate::hooks::{
    bundle_overrides_map, bundles_with_status_items, cluster_content_items, query_is_loading,
    try_cluster_analytics, use_bundle_overrides, use_bundles_with_status, use_cluster,
    use_cluster_analytics, use_cluster_content,
};
use crate::i18n::t;
use crate::layout::{ClusterShell, cluster_content};
use crate::theme::{colors, rounded, text_size};
use crate::ui::{border_all_color, columns_for, last_played_label};

use super::cluster_not_found;
use super::package_manager::{PackageMetaMap, bundle_packages};

const FACT_MIN_W: f32 = 160.;
const FACT_MAX_COLS: usize = 3;
const FACT_GAP: f32 = 12.;
const SECTION_GAP: f32 = 16.;
/// The radius shared by rows, cards and panels across the workspace dialogs
const SURFACE_RADIUS: f32 = 12.;
/// The activity block's share of the pane height, and its limits
const ACTIVITY_SHARE: f32 = 0.28;
const ACTIVITY_MIN: f32 = 160.;
const ACTIVITY_MAX: f32 = 240.;

fn activity_height(pane_height: f32) -> f32 {
    (ACTIVITY_SHARE * pane_height).clamp(ACTIVITY_MIN, ACTIVITY_MAX)
}

/// Columns that spread `count` tiles over as few rows as fit, with the rows as even as possible
fn balanced_columns(count: usize, max_columns: usize) -> usize {
    let max_columns = max_columns.max(1);
    if count == 0 {
        return max_columns;
    }
    let rows = count.div_ceil(max_columns);
    count.div_ceil(rows)
}

#[derive(PartialEq)]
pub struct ClusterOverview {
    pub cluster_id: i64,
}

impl Component for ClusterOverview {
    fn render(&self) -> impl IntoElement {
        let cluster = use_cluster(self.cluster_id);
        let query = use_cluster_analytics(self.cluster_id);
        // The same queries the Mods page reads, so the count matches its list
        let mods = use_cluster_content(self.cluster_id, ContentType::Mod);
        let bundles = use_bundles_with_status(self.cluster_id);
        let overrides = use_bundle_overrides(self.cluster_id);
        let mut facts_width = use_state(|| 0f32);
        let (_, pane_height) = ClusterShell::workspace_pane_size();
        let padding = ClusterShell::workspace_padding();
        let Some(cluster) = cluster else {
            return cluster_content().child(cluster_not_found()).into_element();
        };
        let analytics = try_cluster_analytics(&query);
        let activity = activity_height(pane_height);
        let body = match &analytics {
            None => activity_notice(
                IconType::ClockRewind,
                t("oneclient.cluster.overview.loading"),
                None,
                activity,
            ),
            Some(analytics) if analytics.playtime.session_count == 0 => activity_notice(
                IconType::BarChartSquare02,
                t("oneclient.cluster.overview.no_history"),
                Some(t("oneclient.cluster.overview.start_history")),
                activity,
            ),
            Some(analytics) => crate::view::app::analytics_body(analytics),
        };

        let mod_count = (!cluster.lacks_mod_loader()).then(|| {
            if query_is_loading(&mods) || query_is_loading(&bundles) {
                return "\u{2026}".to_string();
            }
            let rows = bundle_packages(
                cluster_content_items(&mods),
                &bundles_with_status_items(&bundles),
                &bundle_overrides_map(&overrides),
                &PackageMetaMap::new(),
                &Default::default(),
                ContentType::Mod,
            );
            rows.iter()
                .filter(|p| p.opted_in && !p.deleted && !p.hidden && p.enabled)
                .count()
                .to_string()
        });
        let facts: Vec<Element> = instance_facts(&cluster, mod_count)
            .into_iter()
            .map(|(name, value)| fact_tile(name, value))
            .collect();
        let cols = balanced_columns(
            facts.len(),
            columns_for(*facts_width.read(), FACT_MIN_W, FACT_MAX_COLS, FACT_GAP),
        );
        // Every row fills the width; a shorter last row stretches its tiles instead of leaving a gap
        let fact_rows = rect()
            .vertical()
            .width(Size::fill())
            .spacing(FACT_GAP)
            .on_sized(move |event: Event<SizedEventData>| {
                let width = event.area.width();
                if (width - *facts_width.peek()).abs() > 0.5 {
                    facts_width.set(width);
                }
            })
            .children(facts.chunks(cols).map(|row| {
                rect()
                    .horizontal()
                    .content(Content::Flex)
                    .width(Size::fill())
                    .spacing(FACT_GAP)
                    .children(row.iter().cloned())
                    .into_element()
            }));

        cluster_content()
            .child(
                crate::view::app::settings::settings_pane()
                    .padding(Gaps::new(0., padding, 0., 0.))
                    .child(
                        rect()
                            .vertical()
                            .width(Size::fill())
                            .spacing(SECTION_GAP)
                            .child(fact_rows)
                            .child(body),
                    ),
            )
            .into_element()
    }
}

/// What the instance already knows about itself, so the page is useful before any play history
fn instance_facts(cluster: &Cluster, mod_count: Option<String>) -> Vec<(&'static str, String)> {
    let loader = if cluster.mc_loader.is_modded() {
        match cluster.mc_loader_version.as_deref() {
            Some(version) => format!("{} {version}", cluster.mc_loader),
            None => cluster.mc_loader.to_string(),
        }
    } else {
        t("oneclient.cluster.overview.facts.vanilla").to_string()
    };
    let created = cluster.created_at.map_or_else(
        || t("oneclient.cluster.overview.facts.unknown").to_string(),
        |created| {
            created
                .with_timezone(&chrono::Local)
                .format("%Y-%m-%d")
                .to_string()
        },
    );
    let folder = if cluster.uses_dedicated_dir() {
        t("oneclient.cluster.overview.facts.folder.dedicated")
    } else {
        t("oneclient.cluster.overview.facts.folder.shared")
    };

    let mut facts = vec![
        (
            t("oneclient.cluster.overview.facts.version"),
            cluster.mc_version.clone(),
        ),
        (t("oneclient.cluster.overview.facts.loader"), loader),
    ];
    if let Some(count) = mod_count {
        facts.push((t("oneclient.cluster.overview.facts.mods"), count));
    }
    facts.extend([
        (
            t("oneclient.cluster.overview.facts.last_played"),
            last_played_label(cluster.last_played),
        ),
        (t("oneclient.cluster.overview.facts.created"), created),
        (
            t("oneclient.cluster.overview.facts.folder"),
            folder.to_string(),
        ),
    ]);
    facts
}

fn fact_tile(name: &'static str, value: String) -> Element {
    rect()
        .vertical()
        .width(Size::flex(1.0))
        .spacing(4.)
        .padding(Gaps::new_symmetric(12., 14.))
        .corner_radius(rounded(SURFACE_RADIUS))
        .background(colors::card())
        .border(border_all_color(1., colors::component_border()).alignment(BorderAlignment::Inner))
        .child(
            label()
                .text(name)
                .font_size(text_size(12.))
                .max_lines(1)
                .color(crate::view::app::settings::readable_secondary()),
        )
        .child(
            label()
                .text(value)
                .font_size(text_size(14.))
                .font_weight(FontWeight::SEMI_BOLD)
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .width(Size::fill())
                .color(colors::fg_primary()),
        )
        .into_element()
}

/// The message in place of play history, centered in a block sized to a share of the pane
fn activity_notice(icon: IconType, title: &str, hint: Option<&str>, height: f32) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::px(height))
        .center()
        .spacing(6.)
        .padding(Gaps::new_all(24.))
        .background(colors::card())
        .corner_radius(rounded(SURFACE_RADIUS))
        .border(border_all_color(1., colors::component_border()).alignment(BorderAlignment::Inner))
        .child(
            rect()
                .padding(Gaps::new(0., 0., 4., 0.))
                .child(Icon::new(icon).size(28.).color(colors::fg_secondary())),
        )
        .child(
            label()
                .text(title.to_string())
                .width(Size::fill())
                .text_align(TextAlign::Center)
                .font_size(text_size(14.))
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        )
        .maybe_child(hint.map(|hint| {
            label()
                .text(hint.to_string())
                .width(Size::fill())
                .text_align(TextAlign::Center)
                .font_size(text_size(12.))
                .color(crate::view::app::settings::readable_secondary())
        }))
        .into_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_activity_block_takes_28_percent_of_the_pane_between_160_and_240() {
        assert_eq!(activity_height(371.), 160.);
        assert!((activity_height(725.) - 203.).abs() < 0.01);
        assert_eq!(activity_height(859.), 240.);
    }

    #[test]
    fn fact_tiles_spread_evenly_over_the_fewest_rows() {
        assert_eq!(balanced_columns(6, 3), 3);
        assert_eq!(balanced_columns(6, 2), 2);
        assert_eq!(balanced_columns(5, 3), 3);
        assert_eq!(balanced_columns(4, 3), 2);
        assert_eq!(balanced_columns(6, 5), 3);
        assert_eq!(balanced_columns(6, 1), 1);
    }
}
