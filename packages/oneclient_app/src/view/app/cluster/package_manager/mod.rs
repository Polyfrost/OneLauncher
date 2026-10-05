use std::collections::{HashMap, HashSet};

use freya::prelude::*;
use oneclient_common::search::{MatchScore, SearchQuery};
use oneclient_content::packages::{CachedPackageMeta, ContentType, ProviderId};
use oneclient_core::{
    BundleFileKind, BundleFileType, BundleWithUpdateStatus, FileUpdateStatus, LinkedArtifactInfo,
};
use oneclient_db::models::OverrideType;

use crate::components::{
    CARD_GRID_H, CardLayout, GRID_GAP, GRID_MIN_W, PackageEntry, disable_warning_body,
    toggle_action,
};
use crate::hooks::{
    ClusterAction, EssentialGuardKind, PendingEssential, disable_warnings, package_meta_batch,
    use_cluster_mutation, use_disable_warnings, use_essential_guard, use_game_snapshot,
    use_package_meta_batch, use_selection, use_view_state,
};

use super::folder_list::confirm_dialog;

mod views;
use views::{
    AdvancedSection, Bulk, ContentBox, ContentKind, EnabledFilter, HiddenFilter, SortMode,
    toolbar_bar,
};
pub(super) use views::{empty_hint, empty_shell, empty_title, notice_bar};

const CARD_H: f32 = 84.;
pub(super) const CARD_SPACING: f32 = 8.;
pub(super) const GRID_MAX_COLS: usize = 5;

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

fn local_project_ids(
    content: &[LinkedArtifactInfo],
    bundles: &[BundleWithUpdateStatus],
    content_type: ContentType,
) -> Vec<String> {
    let bundled = bundles
        .iter()
        .flat_map(|bundle| &bundle.files)
        .filter(|(file, _status)| file.content_type() == content_type)
        .filter(|(file, _status)| matches!(file.kind, BundleFileKind::External { .. }))
        .map(|(file, _status)| file.kind.metadata_id());
    content
        .iter()
        .filter(|info| info.content_type == content_type && info.provider.is_none())
        .map(|info| info.hash.clone())
        .chain(bundled)
        .collect()
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

    let local = use_package_meta_batch(
        ProviderId::Local,
        local_project_ids(content, bundles, content_type),
    );
    for (hash, meta) in package_meta_batch(&local) {
        out.insert((ProviderId::Local, hash), meta);
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
    let mut normal_elsewhere: HashSet<String> = HashSet::new();
    for bundle in bundles {
        for (file, _status) in &bundle.files {
            if !file.hidden && file.content_type() == content_type {
                shown_elsewhere.insert(file.kind.package_id());
                if file.file_type == BundleFileType::Normal {
                    normal_elsewhere.insert(file.kind.package_id());
                }
            }
        }
    }

    let mut ordered: Vec<&BundleWithUpdateStatus> = bundles.iter().collect();
    ordered.sort_by_key(|b| !b.opted_in_types.contains(&content_type));

    let mut rows = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for bundle in ordered {
        let bundle_name = &bundle.archive.manifest.name;
        let category = bundle.archive.manifest.category.clone();
        for (file, status) in &bundle.files {
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

            let provider = file.kind.metadata_provider();
            let installed_info = match &file.kind {
                BundleFileKind::Managed { .. } => by_project
                    .get(pid.as_str())
                    .or_else(|| by_hash.get(pid.as_str())),
                BundleFileKind::External { file: ext, .. } => {
                    let hash = match status {
                        FileUpdateStatus::UpdateAvailable {
                            installed_version_id,
                            ..
                        } => installed_version_id,
                        _ => &ext.sha1,
                    };
                    seen.insert(hash.clone());
                    by_hash.get(hash.as_str())
                }
            }
            .copied();
            let ov = overrides
                .get(&(bundle_name.clone(), pid.clone()))
                .map(String::as_str);
            let enabled = match installed_info {
                Some(info) => info.enabled,
                None => oneclient_core::effective_enabled(file, ov.and_then(OverrideType::parse)),
            };

            let advanced = content_type == ContentType::Mod
                && file.file_type == BundleFileType::Advanced
                && !normal_elsewhere.contains(&pid);

            let categories = if category.is_empty() {
                Vec::new()
            } else {
                vec![category.clone()]
            };

            let opted_in =
                installed_info.is_some() || bundle.opted_in_types.contains(&content_type);
            let mut row = make_row(
                pid,
                Some(bundle_name.clone()),
                provider,
                file.size,
                categories,
                enabled,
                file.enabled,
                file.is_github_hosted(),
                installed_info,
                meta.get(&(provider, file.kind.metadata_id())),
                file.display_name(),
                false,
                // Flagged rather than dropped `HiddenFilter` filters on the row and the seen id stops the loose-content pass resurrecting it as a local file
                file.hidden,
                opted_in,
            );
            row.advanced = advanced;
            row.github_url = file.github_repo_url();
            rows.push(row);
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
        let row_meta = meta.get(&(provider, pid.clone()));
        rows.push(make_row(
            pid,
            None,
            provider,
            0,
            Vec::new(),
            info.enabled,
            true,
            false,
            Some(info),
            row_meta,
            info.display_name
                .clone()
                .unwrap_or_else(|| info.file_name.clone()),
            outdated,
            false,
            true,
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
    github_hosted: bool,
    installed_info: Option<&LinkedArtifactInfo>,
    m: Option<&CachedPackageMeta>,
    fallback_name: String,
    update_available: bool,
    hidden: bool,
    opted_in: bool,
) -> PackageEntry {
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
    let version = installed_info.and_then(|i| i.display_version.clone());
    let description = m
        .map(|p| p.summary.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_default();

    PackageEntry {
        essential: crate::essential::lookup(provider, &package_id),
        package_id,
        bundle_name,
        provider,
        github_hosted,
        github_url: None,
        name,
        file_name,
        author,
        version,
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
        opted_in,
        shadowed: false,
        advanced: false,
        seen_status: installed_info.map(|i| i.seen_status).unwrap_or_default(),
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
            Tab::Browser => "Online".to_string(),
            Tab::Local => "Local".to_string(),
        }
    }

    pub(super) fn matches(&self, p: &PackageEntry) -> bool {
        match self {
            Tab::All => p.opted_in,
            Tab::Category(c) => p.categories.iter().any(|pc| pc == c),
            Tab::Browser => (p.is_remote() || p.github_hosted) && !p.in_bundle(),
            Tab::Local => !p.is_remote() && !p.github_hosted,
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

    // Category tabs are hidden when empty All + Online + Local are always shown
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
                        tracing::debug!(
                            cleared,
                            "retired package badges after the list was viewed"
                        );
                    }
                    Ok(_) => {}
                    Err(err) => tracing::warn!(%err, "failed to retire package badges"),
                }
            });
        });

        let session_live = use_game_snapshot().is_active(cluster_id);
        let cluster = crate::hooks::use_cluster(cluster_id);
        let shares_content = cluster
            .as_ref()
            .map(|cluster| cluster.shares_content(content_type));
        let uses_bundles = cluster
            .as_ref()
            .is_none_or(|cluster| cluster.uses_bundles());
        let active = use_state(|| 0usize);

        let search = use_state(String::new);
        let enabled_filter = use_state(|| EnabledFilter::All);
        let hidden_filter = use_state(|| HiddenFilter::Hide);
        let advanced_open = use_state(|| false);
        let toolbar_width = use_state(|| 0f32);
        let selection = use_selection::<String>();
        let mutation = use_cluster_mutation();
        let mut guard = use_essential_guard();
        let warnings_query = use_disable_warnings();
        let mut confirm_delete = use_state(|| false);
        let view = use_view_state("cluster.packages");
        let sort = view.sort;
        let layout = view.layout;
        let query = SearchQuery::new(&search.read());
        let sort_mode = sort
            .read()
            .as_deref()
            .and_then(SortMode::from_key)
            .unwrap_or(SortMode::NameAsc);

        let show = *enabled_filter.read();
        let hidden = *hidden_filter.read();
        let card_layout = CardLayout::from(*layout.read());

        let disabled_essentials: Vec<&'static str> = items
            .iter()
            .filter(|package| !package.enabled)
            .filter_map(|package| package.essential.map(|essential| essential.name))
            .collect();

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

        let mut notices = Vec::new();
        if content_type.is_global()
            && let Some(shares_content) = shares_content
        {
            notices.push(if shares_content {
                views::global_notice(noun_plural)
            } else {
                views::instance_only_notice(noun_plural)
            });
        }
        if session_live {
            notices.push(views::running_notice(noun_plural, content_type));
        }
        if !disabled_essentials.is_empty() {
            notices.push(views::essential_notice(&disabled_essentials));
        }

        let (advanced, filtered): (Vec<_>, Vec<_>) = filtered.into_iter().partition(|p| p.advanced);

        let section = AdvancedSection {
            open: advanced_open,
            forced: !query.is_empty(),
        };
        let shown: Vec<&PackageEntry> = filtered
            .iter()
            .chain(advanced.iter().filter(|_| section.expanded()))
            .collect();
        let order: Vec<String> = shown.iter().map(|p| p.package_id.clone()).collect();
        let chosen: Vec<PackageEntry> = shown
            .into_iter()
            .filter(|p| selection.is_selected(&p.package_id))
            .cloned()
            .collect();
        let deletable: Vec<String> = chosen
            .iter()
            .filter(|p| p.installed && !p.in_bundle())
            .filter_map(|p| p.hash.clone())
            .collect();

        let set_enabled: EventHandler<bool> = {
            let chosen = chosen.clone();
            let warnings = disable_warnings(&warnings_query);
            (move |enabled: bool| {
                let targets: Vec<&PackageEntry> =
                    chosen.iter().filter(|p| p.enabled != enabled).collect();
                let actions: Vec<ClusterAction> = targets
                    .iter()
                    .filter_map(|p| toggle_action(p, cluster_id, enabled))
                    .collect();
                if actions.is_empty() {
                    return;
                }
                let action = ClusterAction::Batch(actions);

                let warned: Vec<String> = targets
                    .iter()
                    .filter(|_| !enabled)
                    .filter_map(|p| {
                        disable_warning_body(p, warnings.clone())
                            .map(|body| format!("**{}**\n\n{body}", p.name))
                    })
                    .collect();
                if warned.is_empty() {
                    mutation.mutate(action);
                    return;
                }
                guard.set(Some(PendingEssential {
                    name: match targets.as_slice() {
                        [only] => only.name.clone(),
                        _ => format!("{} {noun_plural}", targets.len()),
                    },
                    body: warned.join("\n\n"),
                    kind: EssentialGuardKind::Disable,
                    action,
                }));
            })
            .into()
        };

        let bulk = Bulk {
            selection,
            order,
            count: chosen.len(),
            deletable: deletable.len(),
            set_enabled,
            delete: (move |()| confirm_delete.set(true)).into(),
        };

        let delete_dialog = confirm_delete.read().then(|| {
            let count = deletable.len();
            let noun = if count == 1 {
                package_type
            } else {
                noun_plural
            };
            let shared = if shares_content == Some(true) {
                format!(" Shared {noun_plural} are deleted from every cluster that uses them.")
            } else {
                String::new()
            };
            confirm_dialog(
                format!("Delete {count} {noun}?"),
                format!("This can't be undone.{shared}"),
                move || confirm_delete.set(false),
                move || {
                    mutation.mutate(ClusterAction::Batch(
                        deletable
                            .iter()
                            .map(|hash| ClusterAction::RemoveArtifact {
                                cluster_id,
                                hash: hash.clone(),
                            })
                            .collect(),
                    ));
                    selection.exit();
                    confirm_delete.set(false);
                },
            )
        });

        selection
            .track_modifiers(rect())
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
                uses_bundles,
                layout,
                cluster_id,
                package_type,
                toolbar_width,
                &bulk,
            ))
            .child(
                ContentBox::new(
                    filtered,
                    advanced,
                    section,
                    noun_plural,
                    package_type,
                    content_type,
                    cluster_id,
                    content_kind,
                    card_layout,
                    bulk,
                )
                .notices(notices),
            )
            .maybe_child(delete_dialog)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::onboarding::test_support::{archive, file};
    use oneclient_core::{BundleFile, FileUpdateStatus};

    fn bundle(category: &str, opted_in: bool, files: Vec<BundleFile>) -> BundleWithUpdateStatus {
        BundleWithUpdateStatus {
            files: files
                .iter()
                .cloned()
                .map(|f| (f, FileUpdateStatus::NotInstalled))
                .collect(),
            archive: archive(category, true, files),
            has_updates: false,
            opted_in_types: if opted_in {
                [ContentType::Mod].into()
            } else {
                HashSet::new()
            },
        }
    }

    #[test]
    fn all_tab_skips_bundles_not_opted_into() {
        let bundles = [
            bundle(
                "Declined",
                false,
                vec![
                    file("only-declined", true, false),
                    file("shared", true, false),
                ],
            ),
            bundle(
                "Taken",
                true,
                vec![file("shared", true, false), file("only-taken", true, false)],
            ),
        ];
        let rows = bundle_packages(
            Vec::new(),
            &bundles,
            &HashMap::new(),
            &PackageMetaMap::new(),
            &HashSet::new(),
            ContentType::Mod,
        );
        let all: Vec<&str> = rows
            .iter()
            .filter(|p| Tab::All.matches(p))
            .map(|p| p.package_id.as_str())
            .collect();
        assert_eq!(all, ["shared", "only-taken"]);
        let shared = rows.iter().find(|p| p.package_id == "shared").unwrap();
        assert_eq!(shared.categories, ["Taken"]);
        let declined = Tab::Category("Declined".into());
        assert!(
            rows.iter()
                .any(|p| p.package_id == "only-declined" && declined.matches(p))
        );
    }
}
