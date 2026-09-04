use std::collections::{HashMap, HashSet};

use freya::prelude::*;
use oneclient_common::search::{MatchScore, SearchQuery};
use oneclient_content::packages::{CachedPackageMeta, ContentType, ProviderId};
use oneclient_core::{BundleFileKind, BundleWithUpdateStatus, LinkedArtifactInfo};
use oneclient_db::models::OverrideType;

use crate::components::{CardLayout, PackageEntry};
use crate::hooks::{package_meta_batch, use_game_snapshot, use_package_meta_batch, use_view_state};

mod views;
use views::{ContentBox, ContentKind, EnabledFilter, HiddenFilter, SortMode, toolbar_bar};

const CARD_H: f32 = 84.;
const CARD_SPACING: f32 = 8.;
const CARD_GRID_H: f32 = 148.;
const GRID_GAP: f32 = 10.;
const GRID_MIN_W: f32 = 260.;
const LAZY_OVERSCAN: i64 = 2;

pub type PackageMetaMap = HashMap<(ProviderId, String), CachedPackageMeta>;

fn provider_project_ids(
    content: &[LinkedArtifactInfo],
    bundles: &[BundleWithUpdateStatus],
    content_type: ContentType,
    provider: ProviderId,
) -> Vec<String> {
    let mut ids = Vec::new();
    for bundle in bundles {
        for (file, _status) in &bundle.files {
            if file.content_type() != content_type {
                continue;
            }
            if let BundleFileKind::Managed {
                project_id,
                provider: file_provider,
                ..
            } = &file.kind
                && *file_provider == provider
            {
                ids.push(project_id.clone());
            }
        }
    }
    for info in content {
        if info.content_type != content_type || info.provider != Some(provider) {
            continue;
        }
        if let Some(project_id) = &info.project_id {
            ids.push(project_id.clone());
        }
    }
    ids
}

pub fn use_content_meta(
    content: &[LinkedArtifactInfo],
    bundles: &[BundleWithUpdateStatus],
    content_type: ContentType,
) -> PackageMetaMap {
    let mut out = PackageMetaMap::new();
    for provider in ProviderId::REMOTE_PROVIDERS.iter().copied() {
        let ids = provider_project_ids(content, bundles, content_type, provider);
        let query = use_package_meta_batch(provider, ids);
        for (project_id, meta) in package_meta_batch(&query) {
            out.insert((provider, project_id), meta);
        }
    }
    out
}

/// `stale` is the artifact hashes with a newer version bundle rows never carry it so the two update flows cannot both claim a package
/// Hidden bundle files get flagged rows [`HiddenFilter`] decides whether they show
pub fn bundle_packages(
    content: Vec<LinkedArtifactInfo>,
    bundles: &[BundleWithUpdateStatus],
    overrides: &HashMap<(String, String), String>,
    meta: &PackageMetaMap,
    stale: &HashSet<String>,
    content_type: ContentType,
) -> Vec<PackageEntry> {
    let mut by_project: HashMap<&str, &LinkedArtifactInfo> = HashMap::new();
    let mut by_hash: HashMap<&str, &LinkedArtifactInfo> = HashMap::new();
    for info in &content {
        if let Some(pid) = &info.project_id {
            by_project.insert(pid.as_str(), info);
        }
        by_hash.insert(info.hash.as_str(), info);
    }

    // Hidden is per-bundle so one bundle carrying a mod as a private dependency must not suppress a bundle that offers it openly
    let mut shown_elsewhere: HashSet<String> = HashSet::new();
    for bundle in bundles {
        for (file, _status) in &bundle.files {
            if !file.hidden && file.content_type() == content_type {
                shown_elsewhere.insert(file.kind.package_id());
            }
        }
    }

    let mut rows = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for bundle in bundles {
        let bundle_name = &bundle.archive.manifest.name;
        let category = bundle.archive.manifest.category.clone();
        for (file, _status) in &bundle.files {
            if file.content_type() != content_type {
                continue;
            }
            let pid = file.kind.package_id();
            // Let the bundle that lists it openly own the row whatever the iteration order
            if file.hidden && shown_elsewhere.contains(&pid) {
                continue;
            }
            if !seen.insert(pid.clone()) {
                continue;
            }

            let provider = match &file.kind {
                BundleFileKind::Managed { provider, .. } => *provider,
                BundleFileKind::External(_) => ProviderId::Local,
            };
            let installed_info = by_project
                .get(pid.as_str())
                .or_else(|| by_hash.get(pid.as_str()))
                .copied();
            let ov = overrides
                .get(&(bundle_name.clone(), pid.clone()))
                .map(String::as_str);
            let enabled = match installed_info {
                Some(info) => info.enabled,
                None => oneclient_core::effective_enabled(file, ov.and_then(OverrideType::parse)),
            };

            let categories = if category.is_empty() {
                Vec::new()
            } else {
                vec![category.clone()]
            };

            rows.push(make_row(
                pid,
                Some(bundle_name.clone()),
                provider,
                file.size,
                categories,
                enabled,
                file.enabled,
                installed_info,
                meta,
                file.display_name(),
                false,
                // Flagged rather than dropped `HiddenFilter` filters on the row and the seen id stops the loose-content pass resurrecting it as a local file
                file.hidden,
            ));
        }
    }

    for info in &content {
        let in_bundle = info.project_id.as_deref().is_some_and(|p| seen.contains(p))
            || seen.contains(&info.hash);
        if in_bundle {
            continue;
        }
        let provider = info.provider.unwrap_or(ProviderId::Local);
        let pid = info.project_id.clone().unwrap_or_else(|| info.hash.clone());
        let outdated = stale.contains(&info.hash);
        rows.push(make_row(
            pid,
            None,
            provider,
            0,
            Vec::new(),
            info.enabled,
            true,
            Some(info),
            meta,
            info.display_name
                .clone()
                .unwrap_or_else(|| info.file_name.clone()),
            outdated,
            false,
        ));
    }

    rows
}

#[allow(clippy::too_many_arguments)]
fn make_row(
    package_id: String,
    bundle_name: Option<String>,
    provider: ProviderId,
    size: u64,
    categories: Vec<String>,
    enabled: bool,
    manifest_default: bool,
    installed_info: Option<&LinkedArtifactInfo>,
    meta: &PackageMetaMap,
    fallback_name: String,
    update_available: bool,
    hidden: bool,
) -> PackageEntry {
    let m = meta.get(&(provider, package_id.clone()));
    let name = m
        .map(|p| p.name.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| fallback_name.clone());
    let file_name = installed_info
        .map(|i| i.file_name.clone())
        .unwrap_or(fallback_name);
    let author = m
        .map(|p| p.author.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_default();
    let description = m
        .map(|p| p.summary.clone())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            installed_info
                .and_then(|i| i.display_version.clone())
                .map(|v| format!("Version {v}"))
        })
        .unwrap_or_default();

    PackageEntry {
        package_id,
        bundle_name,
        provider,
        name,
        file_name,
        author,
        description,
        icon_url: m.and_then(|p| p.icon_url.clone()),
        size,
        categories,
        enabled,
        manifest_default,
        installed: installed_info.is_some(),
        hash: installed_info.map(|i| i.hash.clone()),
        update_available,
        hidden,
        seen_status: installed_info
            .map(|i| i.seen_status)
            .unwrap_or_default(),
    }
}

#[derive(Clone)]
pub(super) enum Tab {
    All,
    Category(String),
    Browser,
    Local,
}

impl Tab {
    pub(super) fn label(&self) -> String {
        match self {
            Tab::All => "All".to_string(),
            Tab::Category(c) => c.clone(),
            Tab::Browser => "Browser".to_string(),
            Tab::Local => "Local".to_string(),
        }
    }

    pub(super) fn matches(&self, p: &PackageEntry) -> bool {
        match self {
            Tab::All => true,
            Tab::Category(c) => p.categories.iter().any(|pc| pc == c),
            Tab::Browser => p.is_remote() && !p.in_bundle(),
            Tab::Local => !p.is_remote(),
        }
    }
}

fn query_score(p: &PackageEntry, query: &SearchQuery) -> Option<MatchScore> {
    query.best_score([p.name.as_str(), p.file_name.as_str()])
}

/// Search is scoped to the active tab a package must match the tab *and* the query
/// `Tab::All` searches the whole cluster
fn visible_packages(
    items: &[PackageEntry],
    tab: Option<&Tab>,
    query: &SearchQuery,
    show: EnabledFilter,
    hidden: HiddenFilter,
) -> Vec<PackageEntry> {
    items
        .iter()
        .filter(|p| tab.is_none_or(|t| t.matches(p)) && query_score(p, query).is_some())
        .filter(|p| show.keep(p) && hidden.keep(p))
        .cloned()
        .collect()
}

/// Stable so the toolbar's sort survives as the tie-break between equally good matches
fn rank_by_query(rows: &mut [PackageEntry], query: &SearchQuery) {
    rows.sort_by_key(|p| std::cmp::Reverse(query_score(p, query)));
}

pub fn bundle_categories(bundles: &[BundleWithUpdateStatus]) -> Vec<String> {
    let mut cats: Vec<String> = Vec::new();
    for bundle in bundles {
        let category = &bundle.archive.manifest.category;
        if !category.is_empty() && !cats.contains(category) {
            cats.push(category.clone());
        }
    }
    cats
}

/// `hidden` applies here too so a category of only hidden dependencies offers no tab
fn build_tabs(categories: &[String], items: &[PackageEntry], hidden: HiddenFilter) -> Vec<Tab> {
    let mut cats: Vec<String> = categories.to_vec();
    for item in items.iter().filter(|p| hidden.keep(p)) {
        for c in &item.categories {
            if !cats.contains(c) {
                cats.push(c.clone());
            }
        }
    }

    // Category tabs are hidden when empty All + Browser + Local are always shown
    let mut tabs: Vec<Tab> = vec![Tab::All];
    tabs.extend(
        cats.into_iter()
            .map(Tab::Category)
            .filter(|t| items.iter().any(|p| hidden.keep(p) && t.matches(p))),
    );

    tabs.push(Tab::Browser);
    tabs.push(Tab::Local);
    tabs
}

#[derive(PartialEq)]
pub struct PackageManager {
    title: &'static str,
    noun_plural: &'static str,
    package_type: &'static str,
    content_type: ContentType,
    cluster_id: i64,
    items: Vec<PackageEntry>,
    categories: Vec<String>,
}

impl PackageManager {
    pub fn new(
        title: &'static str,
        noun_plural: &'static str,
        package_type: &'static str,
        content_type: ContentType,
        cluster_id: i64,
        items: Vec<PackageEntry>,
        categories: Vec<String>,
    ) -> Self {
        Self {
            title,
            noun_plural,
            package_type,
            content_type,
            cluster_id,
            items,
            categories,
        }
    }
}

impl Component for PackageManager {
    fn render(&self) -> impl IntoElement {
        let items = self.items.clone();
        let noun_plural = self.noun_plural;
        let package_type = self.package_type;
        let cluster_id = self.cluster_id;
        let content_type = self.content_type;

        // Cleared once on mount so the rows already rendered keep their badges for this visit
        use_hook(|| {
            spawn_forever(async move {
                let Ok(state) = crate::launcher::state() else {
                    return;
                };
                match oneclient_content::packages::PackageStore::retire_seen_badges(
                    &state.services.content(),
                )
                .await
                {
                    Ok(cleared) if cleared > 0 => {
                        tracing::debug!(cleared, "retired package badges after the list was viewed");
                    }
                    Ok(_) => {}
                    Err(err) => tracing::warn!(%err, "failed to retire package badges"),
                }
            });
        });

        let session_live = use_game_snapshot().is_active(cluster_id);
        let active = use_state(|| 0usize);

        let search = use_state(String::new);
        let enabled_filter = use_state(|| EnabledFilter::All);
        let hidden_filter = use_state(|| HiddenFilter::Hide);
        let view = use_view_state("cluster.packages");
        let sort = view.sort;
        let layout = view.layout;
        let grid_columns = view.columns;
        let query = SearchQuery::new(&search.read());
        let sort_mode = sort
            .read()
            .as_deref()
            .and_then(SortMode::from_key)
            .unwrap_or(SortMode::NameAsc);

        let show = *enabled_filter.read();
        let hidden = *hidden_filter.read();
        let card_layout = CardLayout::from(*layout.read());

        let tabs = build_tabs(&self.categories, &items, hidden);
        let active_idx = (*active.read()).min(tabs.len().saturating_sub(1));
        let tab_filter = tabs.get(active_idx);

        let mut filtered = visible_packages(&items, tab_filter, &query, show, hidden);
        sort_mode.sort(&mut filtered);
        // Relevance has to win while searching a fuzzy typo match would otherwise outrank the exact one
        if !query.is_empty() {
            rank_by_query(&mut filtered, &query);
        }

        // Coming up empty during a search is about the query not the tab so it gets its own empty state
        let content_kind = if filtered.is_empty() && !query.is_empty() {
            ContentKind::NoMatches {
                scope: match tab_filter {
                    Some(Tab::All) | None => None,
                    Some(tab) => Some(tab.label()),
                },
            }
        } else {
            match tab_filter {
                Some(Tab::Browser) => ContentKind::Browser,
                Some(Tab::Local) => ContentKind::Local,
                _ => ContentKind::Other,
            }
        };

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .child(toolbar_bar(
                &tabs,
                active_idx,
                active,
                search,
                sort,
                sort_mode,
                enabled_filter,
                hidden_filter,
                layout,
                grid_columns,
                cluster_id,
                package_type,
            ))
            .maybe_child(session_live.then(|| views::running_notice(noun_plural, content_type)))
            .child(ContentBox::new(
                filtered,
                noun_plural,
                package_type,
                content_type,
                cluster_id,
                content_kind,
                card_layout,
                *grid_columns.read(),
            ))
    }
}
