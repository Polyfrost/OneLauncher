use std::collections::{HashMap, HashSet};

use freya::prelude::*;
use freya::router::use_route;
use oneclient_common::domain::{ContentType, GameLoader};
use oneclient_common::version::parse_mc_version;
use oneclient_content::packages::release_migration::{
    ReleaseMigrationDependency, ReleaseMigrationPackage, ReleaseMigrationPlan, ReleaseMigrationSkip,
    SkipReason, WAITLIST_DAYS,
};
use oneclient_content::packages::{CachedPackageMeta, ProviderId};
use oneclient_core::clusters::Cluster;

use crate::components::{Button, DynamicArt, Dropdown, Icon, IconType, OverlayPopup, ScrollArea};
use crate::hooks::{
    loaded_image, package_meta_batch, use_cached_image, use_dispatch, use_game_snapshot,
    use_launcher, use_notifications_snapshot, use_package_meta_batch, use_release_migration,
};
use crate::routes::Route;
use crate::state::{ReleaseMigrationPrompt, ReleasePlanState};
use crate::theme::colors;
use crate::ui::{ImageFallbackExt, border_all_color};
use crate::utils::format_size;

const DIALOG_W: f32 = 1020.;
const DIALOG_H: f32 = 616.;
const ART_W: f32 = 368.;
const PANEL_BG: Color = Color::from_rgb(22, 28, 35);
const FOOTER_BG: Color = Color::from_rgb(19, 25, 31);
const CARD_BG: Color = Color::from_argb(214, 17, 22, 28);
const ROW_ICON: f32 = 32.;
const CHECK_SIZE: f32 = 24.;
const SKIPPED_ICON: f32 = 20.;
const SKIPPED_NAME_W: f32 = 168.;
const REQUIRED_BY_W: f32 = 190.;

type MetaMap = HashMap<(ProviderId, String), CachedPackageMeta>;
type SelectionKey = (i64, String);

#[derive(Clone, Copy, PartialEq, Eq)]
enum PackageTab {
    Mods,
    ResourcePacks,
    Shaders,
}

impl PackageTab {
    const ALL: [Self; 3] = [Self::Mods, Self::ResourcePacks, Self::Shaders];

    fn label(self) -> &'static str {
        match self {
            Self::Mods => "Mods",
            Self::ResourcePacks => "Resource packs",
            Self::Shaders => "Shaders",
        }
    }

    fn content_type(self) -> ContentType {
        match self {
            Self::Mods => ContentType::Mod,
            Self::ResourcePacks => ContentType::ResourcePack,
            Self::Shaders => ContentType::Shader,
        }
    }
}

fn blocked_by_route(route: &Route) -> bool {
    matches!(
        route,
        Route::Startup {}
            | Route::Relocating {}
            | Route::OnboardingWelcome {}
            | Route::OnboardingLocation {}
            | Route::OnboardingTerms {}
            | Route::OnboardingMigration {}
            | Route::OnboardingLanguage {}
            | Route::OnboardingAccount {}
            | Route::OnboardingPreferences {}
            | Route::OnboardingBundles {}
            | Route::OnboardingDownloading {}
    )
}

fn cluster_title(cluster: &Cluster) -> String {
    cluster.name.clone()
}

fn selection_key(package: &ReleaseMigrationPackage, source_id: i64) -> SelectionKey {
    (source_id, package.source_hash.clone())
}

#[derive(PartialEq)]
pub struct ReleaseMigrationPopup;

impl Component for ReleaseMigrationPopup {
    fn render(&self) -> impl IntoElement {
        let launcher = use_launcher();
        let dispatch = use_dispatch();
        let route = use_route::<Route>();
        let snapshot = use_notifications_snapshot();
        let game = use_game_snapshot();
        let prompt = use_release_migration();

        let mut excluded = use_state(HashSet::<SelectionKey>::new);
        let mut tab = use_state(|| PackageTab::Mods);

        let settled = launcher.ready && !launcher.fetching && !launcher.syncing_bundles;
        let checked = use_state(|| false);
        let check_dispatch = dispatch.clone();
        use_side_effect_with_deps(&settled, move |&settled| {
            let mut checked = checked;
            if settled && !*checked.peek() {
                checked.set(true);
                check_dispatch.check_release_migration();
                check_dispatch.process_release_waitlist();
            }
        });

        let mut wont_open = use_state(|| true);
        let mut deps_open = use_state(|| false);

        let projects: Vec<(ProviderId, String)> = prompt
            .as_ref()
            .and_then(|prompt| match prompt.plan() {
                Some(ReleasePlanState::Ready(plan)) => Some(
                    plan.packages
                        .iter()
                        .map(|package| (package.provider, package.project_id.clone()))
                        .chain(plan.unavailable.iter().map(|skip| (skip.provider, skip.project_id.clone())))
                        .chain(plan.dependencies.iter().map(|dependency| (dependency.provider, dependency.project.id.clone())))
                        .collect(),
                ),
                _ => None,
            })
            .unwrap_or_default();

        let mut meta = MetaMap::new();
        for provider in ProviderId::REMOTE_PROVIDERS.iter().copied() {
            let ids: Vec<String> = projects
                .iter()
                .filter(|(owner, _)| *owner == provider)
                .map(|(_, project_id)| project_id.clone())
                .collect();
            let query = use_package_meta_batch(provider, ids);
            for (project_id, cached) in package_meta_batch(&query) {
                meta.insert((provider, project_id), cached);
            }
        }

        let Some(prompt) = prompt else {
            excluded.set_if_modified(HashSet::new());
            tab.set_if_modified(PackageTab::Mods);
            wont_open.set_if_modified(true);
            deps_open.set_if_modified(false);
            return rect().into_element();
        };

        let other_popup = snapshot.cluster_update.is_some()
            || snapshot.optional_mods.is_some()
            || snapshot.package_updates.is_some()
            || snapshot.pending_prompt.is_some();
        if blocked_by_route(&route) || other_popup {
            return rect().into_element();
        }

        let target_running = game.is_running(prompt.target.id);
        let close = dispatch.clone();
        let panel_dispatch = dispatch.clone();

        OverlayPopup::new()
            .on_close(move |_| close.dismiss_release_migration())
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(dialog(&prompt, &meta, panel_dispatch, excluded, tab, wont_open, deps_open, target_running)),
            )
            .into_element()
    }
}

fn dialog(
    prompt: &ReleaseMigrationPrompt,
    meta: &MetaMap,
    dispatch: crate::Actions,
    excluded: State<HashSet<SelectionKey>>,
    tab: State<PackageTab>,
    wont_open: State<bool>,
    deps_open: State<bool>,
    target_running: bool,
) -> impl IntoElement {
    let plan = match prompt.plan() {
        Some(ReleasePlanState::Ready(plan)) => Some(plan),
        _ => None,
    };

    rect()
        .horizontal()
        .width(Size::px(DIALOG_W))
        .height(Size::px(DIALOG_H))
        .max_width(Size::window_percent(95.))
        .max_height(Size::window_percent(92.))
        .overflow(Overflow::Clip)
        .corner_radius(CornerRadius::new_all(18.))
        .background(PANEL_BG)
        .border(border_all_color(1., colors::component_border()))
        .shadow(Shadow::from((0., 24., 64., 0., Color::from_argb(160, 0, 0, 0))))
        .content(Content::Flex)
        .child(art_panel(prompt, plan, &excluded.read()))
        .child(content_panel(prompt, plan, meta, dispatch, excluded, tab, wont_open, deps_open, target_running))
}

fn art_panel(
    prompt: &ReleaseMigrationPrompt,
    plan: Option<&ReleaseMigrationPlan>,
    excluded: &HashSet<SelectionKey>,
) -> impl IntoElement {
    let target = &prompt.target;
    let parsed = parse_mc_version(&target.mc_version);
    let art = parsed.map(|parsed| {
        DynamicArt::for_version(parsed.major, parsed.key(), Some(target.mc_loader))
    });

    let mut runtime = Vec::new();
    if target.mc_loader != GameLoader::Vanilla {
        runtime.push(match target.mc_loader_version.as_deref() {
            Some(version) if !version.is_empty() => format!("{} {version}", target.mc_loader),
            _ => target.mc_loader.to_string(),
        });
    }
    if let Some(java) = prompt.java_major {
        runtime.push(format!("Java {java}"));
    }

    rect()
        .width(Size::px(ART_W))
        .height(Size::fill())
        .overflow(Overflow::Clip)
        .background(colors::page())
        .maybe_child(art.map(|art| {
            rect()
                .position(Position::new_absolute())
                .width(Size::fill())
                .height(Size::fill())
                .layer(Layer::Relative(1))
                .child(art)
        }))
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .height(Size::fill())
                .padding(Gaps::new(24., 24., 24., 24.))
                .main_align(Alignment::End)
                .layer(Layer::Relative(3))
                .background(
                    LinearGradient::new()
                        .angle(0.)
                        .stop((Color::from_af32rgb(0.0, 13, 17, 21), 28.))
                        .stop((Color::from_af32rgb(0.85, 13, 17, 21), 58.))
                        .stop((Color::from_af32rgb(0.98, 13, 17, 21), 100.)),
                )
                .child(
                    rect()
                        .vertical()
                        .width(Size::fill())
                        .child(
                            label()
                                .text(cluster_title(target))
                                .font_size(32.)
                                .font_weight(FontWeight::BOLD)
                                .max_lines(1)
                                .color(Color::WHITE),
                        )
                        .maybe_child((!runtime.is_empty()).then(|| {
                            label()
                                .text(runtime.join(" · "))
                                .font_size(13.)
                                .margin(Gaps::new(12., 0., 0., 0.))
                                .color(colors::fg_primary())
                        }))
                        .child(summary_card(plan, excluded)),
                ),
        )
}

fn summary_card(
    plan: Option<&ReleaseMigrationPlan>,
    excluded: &HashSet<SelectionKey>,
) -> impl IntoElement {
    let mut rows = rect().vertical().width(Size::fill()).spacing(12.);
    for tab in PackageTab::ALL {
        let value = plan.map_or_else(
            || "–".to_string(),
            |plan| {
                let dependencies = if tab.content_type() == ContentType::Mod {
                    plan.dependencies.len()
                } else {
                    0
                };
                let needed = if tab.content_type() == ContentType::Mod {
                    let chosen = selected_hashes(plan, excluded);
                    plan.dependencies
                        .iter()
                        .filter(|dependency| dependency.is_needed_by(&chosen))
                        .count()
                } else {
                    0
                };

                let total = plan.offered(tab.content_type())
                    + dependencies
                    + plan
                        .unavailable
                        .iter()
                        .filter(|skip| skip.content_type == tab.content_type())
                        .count();
                let selected = needed
                    + plan
                        .packages
                        .iter()
                        .filter(|package| package.content_type == tab.content_type())
                        .filter(|package| !excluded.contains(&selection_key(package, plan.source_cluster_id)))
                        .count();
                format!("{selected} of {total}")
            },
        );
        rows = rows.child(summary_row(tab.label(), value, colors::fg_primary()));
    }

    let wont = plan.map_or_else(|| "–".to_string(), |plan| plan.unavailable.len().to_string());

    rect()
        .vertical()
        .width(Size::fill())
        .margin(Gaps::new(20., 0., 0., 0.))
        .padding(Gaps::new(16., 16., 16., 16.))
        .corner_radius(CornerRadius::new_all(12.))
        .background(CARD_BG)
        .border(border_all_color(1., colors::component_border()))
        .child(rows)
        .child(
            rect()
                .width(Size::fill())
                .height(Size::px(1.))
                .margin(Gaps::new(14., 0., 14., 0.))
                .background(colors::component_border()),
        )
        .child(summary_row("Won't migrate", wont, colors::fg_secondary()))
}

fn summary_row(name: &str, value: String, name_color: Color) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .content(Content::Flex)
        .child(
            label()
                .text(name.to_string())
                .font_size(13.)
                .width(Size::flex(1.))
                .color(name_color),
        )
        .child(
            label()
                .text(value)
                .font_size(13.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        )
}

fn content_panel(
    prompt: &ReleaseMigrationPrompt,
    plan: Option<&ReleaseMigrationPlan>,
    meta: &MetaMap,
    dispatch: crate::Actions,
    excluded: State<HashSet<SelectionKey>>,
    tab: State<PackageTab>,
    wont_open: State<bool>,
    deps_open: State<bool>,
    target_running: bool,
) -> impl IntoElement {
    let close = dispatch.clone();
    let picker_dispatch = dispatch.clone();
    let source_title = prompt.source().map(cluster_title).unwrap_or_default();

    let subtitle = format!(
        "Files are copied into {}. Your {source_title} cluster is left exactly as it is.",
        prompt.target.mc_version
    );

    rect()
        .vertical()
        .width(Size::flex(1.))
        .height(Size::fill())
        .content(Content::Flex)
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .height(Size::flex(1.))
                .content(Content::Flex)
                .padding(Gaps::new(22., 24., 0., 24.))
                .child(
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .content(Content::Flex)
                        .cross_align(Alignment::Center)
                        .child(
                            label()
                                .text("Migrate your packages")
                                .font_size(21.)
                                .font_weight(FontWeight::SEMI_BOLD)
                                .width(Size::flex(1.))
                                .max_lines(1)
                                .color(colors::fg_primary()),
                        )
                        .child(
                            Button::new()
                                .ghost()
                                .icon()
                                .on_press(move |_| close.dismiss_release_migration())
                                .child(Icon::new(IconType::XClose).size(16.).color(colors::fg_secondary())),
                        ),
                )
                .child(
                    label()
                        .text(subtitle)
                        .font_size(13.)
                        .max_lines(2)
                        .margin(Gaps::new(4., 0., 0., 0.))
                        .color(colors::fg_secondary()),
                )
                .child(source_picker(prompt, picker_dispatch))
                .child(tab_row(plan, excluded, tab))
                .child(package_list(prompt, plan, meta, excluded, tab, wont_open, deps_open)),
        )
        .child(
            rect()
                .width(Size::fill())
                .height(Size::px(1.))
                .background(colors::component_border()),
        )
        .child(footer(prompt, plan, dispatch, excluded, target_running))
}

fn source_picker(prompt: &ReleaseMigrationPrompt, dispatch: crate::Actions) -> impl IntoElement {
    let sources: Vec<i64> = prompt.sources.iter().map(|cluster| cluster.id).collect();
    let options: Vec<String> = prompt.sources.iter().map(cluster_title).collect();
    let selected = prompt.source().map(cluster_title).unwrap_or_default();

    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(12.)
        .margin(Gaps::new(18., 0., 0., 0.))
        .child(
            label()
                .text("Migrate from")
                .font_size(13.)
                .color(colors::fg_secondary()),
        )
        .child(
            Dropdown::new(selected, options)
                .outlined()
                .width(Size::px(212.))
                .height(Size::px(32.))
                .on_select(move |index: usize| {
                    if let Some(source_id) = sources.get(index) {
                        dispatch.select_release_migration_source(*source_id);
                    }
                }),
        )
}

fn tab_row(
    plan: Option<&ReleaseMigrationPlan>,
    excluded: State<HashSet<SelectionKey>>,
    tab: State<PackageTab>,
) -> impl IntoElement {
    let active = *tab.read();

    let mut pills = rect().horizontal().spacing(8.).cross_align(Alignment::Center);
    for option in PackageTab::ALL {
        let count = plan.map_or(0, |plan| plan.offered(option.content_type()));
        let mut tab = tab;
        pills = pills.child(tab_pill(
            format!("{} · {count}", option.label()),
            option == active,
            move || tab.set(option),
        ));
    }

    let all_keys: Vec<SelectionKey> = plan
        .map(|plan| {
            plan.packages
                .iter()
                .map(|package| selection_key(package, plan.source_cluster_id))
                .collect()
        })
        .unwrap_or_default();
    let any_selected = all_keys.iter().any(|key| !excluded.read().contains(key));
    let has_packages = !all_keys.is_empty();

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .margin(Gaps::new(18., 0., 10., 0.))
        .child(rect().width(Size::flex(1.)).child(pills))
        .maybe_child(has_packages.then(|| {
            let mut excluded = excluded;
            rect()
                .padding(Gaps::new_symmetric(6., 4.))
                .cursor(CursorIcon::Pointer)
                .on_press(move |_| {
                    let mut set = excluded.write();
                    if any_selected {
                        set.extend(all_keys.iter().cloned());
                    } else {
                        for key in &all_keys {
                            set.remove(key);
                        }
                    }
                })
                .child(
                    label()
                        .text(if any_selected { "Clear all" } else { "Select all" })
                        .font_size(13.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                )
        }))
}

fn tab_pill(text: String, active: bool, mut on_press: impl FnMut() + 'static) -> impl IntoElement {
    rect()
        .height(Size::px(34.))
        .padding(Gaps::new_symmetric(0., 13.))
        .center()
        .corner_radius(CornerRadius::new_all(8.))
        .background(if active { colors::brand() } else { Color::TRANSPARENT })
        .border(border_all_color(
            1.,
            if active { colors::brand() } else { colors::component_border() },
        ))
        .cursor(CursorIcon::Pointer)
        .on_press(move |_| on_press())
        .child(
            label()
                .text(text)
                .font_size(14.)
                .font_weight(FontWeight::MEDIUM)
                .color(if active { Color::WHITE } else { colors::fg_primary() }),
        )
}

fn package_list(
    prompt: &ReleaseMigrationPrompt,
    plan: Option<&ReleaseMigrationPlan>,
    meta: &MetaMap,
    excluded: State<HashSet<SelectionKey>>,
    tab: State<PackageTab>,
    wont_open: State<bool>,
    deps_open: State<bool>,
) -> impl IntoElement {
    let source_title = prompt.source().map(cluster_title).unwrap_or_default();

    let message = match (prompt.plan(), plan) {
        (Some(ReleasePlanState::Loading) | None, _) => Some("Checking which packages have a compatible version…".to_string()),
        (Some(ReleasePlanState::Failed), _) => Some("Couldn't reach Modrinth or CurseForge. Check your connection and try again.".to_string()),
        _ => None,
    };
    if let Some(message) = message {
        return list_message(message).into_element();
    }

    let Some(plan) = plan else {
        return rect().into_element();
    };

    let content_type = tab.read().content_type();
    let rows: Vec<&ReleaseMigrationPackage> = plan
        .packages
        .iter()
        .filter(|package| package.content_type == content_type)
        .collect();

    let skipped: Vec<&ReleaseMigrationSkip> = plan
        .unavailable
        .iter()
        .filter(|skip| skip.content_type == content_type)
        .collect();

    let dependencies: Vec<&ReleaseMigrationDependency> = if content_type == ContentType::Mod {
        plan.dependencies.iter().collect()
    } else {
        Vec::new()
    };

    if rows.is_empty() && skipped.is_empty() && dependencies.is_empty() {
        return list_message(format!(
            "Nothing from {source_title} has a {} version yet.",
            prompt.target.mc_version
        ))
        .into_element();
    }

    let mut scroll = ScrollArea::new()
        .width(Size::fill())
        .height(Size::flex(1.))
        .spacing(0.);

    let mut has_content = !rows.is_empty();
    for package in rows {
        let key = selection_key(package, plan.source_cluster_id);
        let selected = !excluded.read().contains(&key);

        scroll = scroll.child(
            MigrationRow {
                name: package_name(meta, package.provider, &package.project_id, &package.display_name),
                icon_url: meta
                    .get(&(package.provider, package.project_id.clone()))
                    .and_then(|cached| cached.icon_url.clone()),
                package: package.clone(),
                selected,
                selection: key.clone(),
                excluded,
                key: DiffKey::None,
            }
            .key(format!("{}:{}", key.0, key.1))
            .into_element(),
        );
    }

    if !dependencies.is_empty() {
        let selected_hashes = selected_hashes(plan, &excluded.read());
        let open = *deps_open.read();
        scroll = scroll.child(section_header(
            IconType::Link03,
            format!("Dependencies · {}", dependencies.len()),
            Some("Installed with the packages that need them".to_string()),
            open,
            deps_open,
            has_content,
        ));
        has_content = true;

        if open {
            for dependency in dependencies {
                let required_by: Vec<String> = dependency
                    .required_by
                    .iter()
                    .filter_map(|hash| plan.packages.iter().find(|package| &package.source_hash == hash))
                    .map(|package| package_name(meta, package.provider, &package.project_id, &package.display_name))
                    .collect();
                let cached = meta.get(&(dependency.provider, dependency.project.id.clone()));

                scroll = scroll.child(
                    DependencyRow {
                        name: package_name(meta, dependency.provider, &dependency.project.id, &dependency.project.name),
                        icon_url: cached
                            .and_then(|cached| cached.icon_url.clone())
                            .or_else(|| dependency.project.icon_url.clone()),
                        provider: dependency.provider,
                        version_name: dependency.version_name(),
                        size: dependency.size(),
                        required_by: format!("Required by {}", required_by.join(", ")),
                        needed: dependency.is_needed_by(&selected_hashes),
                        key: DiffKey::None,
                    }
                    .key(format!("dependency:{}:{}", plan.source_cluster_id, dependency.project.id))
                    .into_element(),
                );
            }
        }
    }

    if !skipped.is_empty() {
        let cross_loader = prompt.is_cross_loader();
        let build = build_label(prompt);
        let mc_version = &build;
        let open = *wont_open.read();
        scroll = scroll.child(section_header(
            IconType::AlertCircle,
            format!("Won't migrate · {}", skipped.len()),
            most_common_reason(&skipped).map(|reason| reason_text(reason, mc_version)),
            open,
            wont_open,
            has_content,
        ));

        if open {
            if !cross_loader {
                scroll = scroll.child(
                    label()
                        .text(format!(
                            "Added automatically when a {mc_version} build appears (up to {WAITLIST_DAYS} days)"
                        ))
                        .font_size(12.)
                        .max_lines(1)
                        .margin(Gaps::new(0., 12., 6., 12.))
                        .color(colors::fg_secondary()),
                );
            }
            for skip in skipped {
                let cached = meta.get(&(skip.provider, skip.project_id.clone()));
                scroll = scroll.child(
                    SkippedRow {
                        name: package_name(meta, skip.provider, &skip.project_id, &skip.display_name),
                        icon_url: cached.and_then(|cached| cached.icon_url.clone()),
                        reason: reason_text(skip.reason, mc_version),
                        key: DiffKey::None,
                    }
                    .key(format!("skip:{}:{}", plan.source_cluster_id, skip.project_id))
                    .into_element(),
                );
            }
        }
    }

    scroll.into_element()
}

fn package_name(meta: &MetaMap, provider: ProviderId, project_id: &str, fallback: &str) -> String {
    meta.get(&(provider, project_id.to_string()))
        .map(|cached| cached.name.clone())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn selected_hashes(plan: &ReleaseMigrationPlan, excluded: &HashSet<SelectionKey>) -> HashSet<String> {
    plan.packages
        .iter()
        .filter(|package| !excluded.contains(&selection_key(package, plan.source_cluster_id)))
        .map(|package| package.source_hash.clone())
        .collect()
}

fn build_label(prompt: &ReleaseMigrationPrompt) -> String {
    let mc_version = &prompt.target.mc_version;
    if !prompt.is_cross_loader() {
        return mc_version.clone();
    }
    match prompt.target.mc_loader {
        GameLoader::Ornithe => format!("Ornithe {mc_version}"),
        loader => format!("{loader} {mc_version}"),
    }
}

fn reason_text(reason: SkipReason, mc_version: &str) -> String {
    match reason {
        SkipReason::NoCompatibleVersion => format!("No {mc_version} build yet"),
        SkipReason::ProviderUnavailable => "Couldn't check its provider".to_string(),
        SkipReason::MissingDependency => "Depends on a package that can't migrate".to_string(),
    }
}

fn most_common_reason(skipped: &[&ReleaseMigrationSkip]) -> Option<SkipReason> {
    let mut counts: HashMap<SkipReason, usize> = HashMap::new();
    for skip in skipped {
        *counts.entry(skip.reason).or_default() += 1;
    }
    [SkipReason::NoCompatibleVersion, SkipReason::MissingDependency, SkipReason::ProviderUnavailable]
        .into_iter()
        .filter(|reason| counts.contains_key(reason))
        .max_by_key(|reason| counts[reason])
}

fn section_header(
    icon: IconType,
    title: String,
    subtitle: Option<String>,
    open: bool,
    mut state: State<bool>,
    after_content: bool,
) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::px(44.))
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(10.)
        .margin(Gaps::new(if after_content { 10. } else { 0. }, 0., 6., 0.))
        .padding(Gaps::new_symmetric(0., 12.))
        .corner_radius(CornerRadius::new_all(8.))
        .background(colors::ghost_overlay())
        .border(border_all_color(1., colors::component_border()).alignment(BorderAlignment::Inner))
        .cursor(CursorIcon::Pointer)
        .on_press(move |_| state.toggle())
        .child(Icon::new(icon).size(16.).color(colors::fg_secondary()))
        .child(
            label()
                .text(title)
                .font_size(13.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text(subtitle.unwrap_or_default())
                .font_size(12.)
                .width(Size::flex(1.))
                .max_lines(1)
                .color(colors::fg_secondary()),
        )
        .child(
            Icon::new(if open { IconType::ChevronDown } else { IconType::ChevronRight })
                .size(16.)
                .color(colors::fg_secondary()),
        )
}

#[derive(PartialEq)]
struct DependencyRow {
    name: String,
    icon_url: Option<String>,
    provider: ProviderId,
    version_name: String,
    size: u64,
    required_by: String,
    needed: bool,
    key: DiffKey,
}

impl KeyExt for DependencyRow {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for DependencyRow {
    fn render(&self) -> impl IntoElement {
        let icon_query = use_cached_image(self.icon_url.clone(), 96);
        let icon = match loaded_image(self.icon_url.as_deref(), &icon_query) {
            Some((url, bytes)) => rect()
                .width(Size::px(ROW_ICON))
                .height(Size::px(ROW_ICON))
                .corner_radius(CornerRadius::new_all(8.))
                .overflow(Overflow::Clip)
                .child(
                    ImageViewer::new((url, bytes))
                        .width(Size::px(ROW_ICON))
                        .height(Size::px(ROW_ICON))
                        .aspect_ratio(AspectRatio::Min)
                        .fallback(placeholder_icon()),
                )
                .into_element(),
            None => placeholder_icon().into_element(),
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(56.))
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(14.)
            .padding(Gaps::new_symmetric(0., 12.))
            .opacity(if self.needed { 1. } else { 0.45 })
            .child(icon)
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.))
                    .spacing(2.)
                    .child(
                        label()
                            .text(self.name.clone())
                            .font_size(14.)
                            .font_weight(FontWeight::MEDIUM)
                            .max_lines(1)
                            .width(Size::fill())
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text(format!("{} · {}", provider_label(self.provider), self.version_name))
                            .font_size(12.)
                            .max_lines(1)
                            .width(Size::fill())
                            .color(colors::fg_secondary()),
                    ),
            )
            .maybe_child((self.size > 0).then(|| {
                label()
                    .text(format_size(self.size))
                    .font_size(12.)
                    .color(colors::fg_secondary())
            }))
            .child(
                label()
                    .text(self.required_by.clone())
                    .font_size(12.)
                    .width(Size::px(REQUIRED_BY_W))
                    .max_lines(1)
                    .text_align(TextAlign::End)
                    .color(colors::fg_secondary()),
            )
    }
}

fn provider_label(provider: ProviderId) -> &'static str {
    match provider {
        ProviderId::Modrinth => "Modrinth",
        ProviderId::CurseForge => "CurseForge",
        ProviderId::Local => "Local",
    }
}

#[derive(PartialEq)]
struct SkippedRow {
    name: String,
    icon_url: Option<String>,
    reason: String,
    key: DiffKey,
}

impl KeyExt for SkippedRow {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for SkippedRow {
    fn render(&self) -> impl IntoElement {
        let icon_query = use_cached_image(self.icon_url.clone(), 96);
        let icon = match loaded_image(self.icon_url.as_deref(), &icon_query) {
            Some((url, bytes)) => rect()
                .width(Size::px(SKIPPED_ICON))
                .height(Size::px(SKIPPED_ICON))
                .corner_radius(CornerRadius::new_all(6.))
                .overflow(Overflow::Clip)
                .child(
                    ImageViewer::new((url, bytes))
                        .width(Size::px(SKIPPED_ICON))
                        .height(Size::px(SKIPPED_ICON))
                        .aspect_ratio(AspectRatio::Min),
                )
                .into_element(),
            None => Icon::new(IconType::DotsGrid)
                .size(16.)
                .color(colors::fg_secondary())
                .into_element(),
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(40.))
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(14.)
            .padding(Gaps::new_symmetric(0., 12.))
            .opacity(0.6)
            .child(
                rect()
                    .width(Size::px(ROW_ICON))
                    .center()
                    .child(icon),
            )
            .child(
                label()
                    .text(self.name.clone())
                    .font_size(14.)
                    .font_weight(FontWeight::MEDIUM)
                    .width(Size::px(SKIPPED_NAME_W))
                    .max_lines(1)
                    .color(colors::fg_primary()),
            )
            .child(
                label()
                    .text(self.reason.clone())
                    .font_size(12.)
                    .width(Size::flex(1.))
                    .max_lines(1)
                    .color(colors::fg_secondary()),
            )
    }
}

fn list_message(text: String) -> impl IntoElement {
    rect()
        .width(Size::fill())
        .height(Size::flex(1.))
        .center()
        .child(
            label()
                .text(text)
                .font_size(13.)
                .color(colors::fg_secondary()),
        )
}

fn footer(
    prompt: &ReleaseMigrationPrompt,
    plan: Option<&ReleaseMigrationPlan>,
    dispatch: crate::Actions,
    excluded: State<HashSet<SelectionKey>>,
    target_running: bool,
) -> impl IntoElement {
    let dismiss = dispatch.clone();

    let chosen: Vec<ReleaseMigrationPackage> = plan
        .map(|plan| {
            plan.packages
                .iter()
                .filter(|package| !excluded.read().contains(&selection_key(package, plan.source_cluster_id)))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let total = plan.map_or(0, |plan| plan.packages.len());
    let dependency_bytes: u64 = plan.map_or(0, |plan| {
        let selected = selected_hashes(plan, &excluded.read());
        plan.dependencies
            .iter()
            .filter(|dependency| dependency.is_needed_by(&selected))
            .map(ReleaseMigrationDependency::size)
            .sum()
    });
    let bytes: u64 = chosen.iter().map(|package| package.size).sum::<u64>() + dependency_bytes;
    let count = chosen.len();

    let summary = format!(
        "{count} of {total} packages selected · {} to copy",
        format_size(bytes)
    );

    let enabled = count > 0 && !target_running;
    let mut migrate = Button::new()
        .primary()
        .enabled(enabled)
        .width(Size::px(176.))
        .height(Size::px(52.))
        .font_size(15.)
        .font_weight(FontWeight::SEMI_BOLD)
        .on_press(move |_| dispatch.migrate_release_packages(chosen.clone()))
        .text("Migrate");
    if target_running {
        migrate = migrate.tooltip(format!("Close {} before migrating", prompt.target.name));
    }

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .padding(Gaps::new(16., 24., 16., 24.))
        .background(FOOTER_BG)
        .child(
            label()
                .text(summary)
                .font_size(12.)
                .width(Size::flex(1.))
                .max_lines(2)
                .color(colors::fg_secondary()),
        )
        .child(
            Button::new()
                .ghost()
                .height(Size::px(52.))
                .padding(Gaps::new_symmetric(0., 16.))
                .font_size(15.)
                .font_weight(FontWeight::SEMI_BOLD)
                .on_press(move |_| dismiss.dismiss_release_migration())
                .text("Not now"),
        )
        .child(migrate)
}

struct MigrationRow {
    name: String,
    icon_url: Option<String>,
    package: ReleaseMigrationPackage,
    selected: bool,
    selection: SelectionKey,
    excluded: State<HashSet<SelectionKey>>,
    key: DiffKey,
}

impl PartialEq for MigrationRow {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.icon_url == other.icon_url
            && self.package == other.package
            && self.selected == other.selected
            && self.selection == other.selection
    }
}

impl KeyExt for MigrationRow {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for MigrationRow {
    fn render(&self) -> impl IntoElement {
        let icon_query = use_cached_image(self.icon_url.clone(), 96);
        let mut hovered = use_state(|| false);
        let mut excluded = self.excluded;
        let selection = self.selection.clone();
        let selected = self.selected;

        let icon = match loaded_image(self.icon_url.as_deref(), &icon_query) {
            Some((url, bytes)) => rect()
                .width(Size::px(ROW_ICON))
                .height(Size::px(ROW_ICON))
                .corner_radius(CornerRadius::new_all(8.))
                .overflow(Overflow::Clip)
                .child(
                    ImageViewer::new((url, bytes))
                        .width(Size::px(ROW_ICON))
                        .height(Size::px(ROW_ICON))
                        .aspect_ratio(AspectRatio::Min)
                        .fallback(placeholder_icon()),
                )
                .into_element(),
            None => placeholder_icon().into_element(),
        };

        let provider = provider_label(self.package.provider);

        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(56.))
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(14.)
            .padding(Gaps::new_symmetric(0., 12.))
            .corner_radius(CornerRadius::new_all(10.))
            .background(if *hovered.read() { colors::ghost_overlay() } else { Color::TRANSPARENT })
            .cursor(CursorIcon::Pointer)
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_leave(move |_| hovered.set(false))
            .on_press(move |_| {
                let mut set = excluded.write();
                if selected {
                    set.insert(selection.clone());
                } else {
                    set.remove(&selection);
                }
            })
            .child(icon)
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.))
                    .spacing(2.)
                    .child(
                        label()
                            .text(self.name.clone())
                            .font_size(14.)
                            .font_weight(FontWeight::MEDIUM)
                            .max_lines(1)
                            .width(Size::fill())
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text(if self.package.enabled {
                                format!("{provider} · {}", self.package.version_name)
                            } else {
                                format!("{provider} · {} · Disabled", self.package.version_name)
                            })
                            .font_size(12.)
                            .max_lines(1)
                            .width(Size::fill())
                            .color(colors::fg_secondary()),
                    ),
            )
            .maybe_child((self.package.size > 0).then(|| {
                label()
                    .text(format_size(self.package.size))
                    .font_size(12.)
                    .color(colors::fg_secondary())
            }))
            .child(check_box(selected))
    }
}

fn placeholder_icon() -> impl IntoElement {
    rect()
        .width(Size::px(ROW_ICON))
        .height(Size::px(ROW_ICON))
        .center()
        .corner_radius(CornerRadius::new_all(8.))
        .border(border_all_color(1., colors::component_border()).alignment(BorderAlignment::Inner))
        .child(
            Icon::new(IconType::DotsGrid)
                .size(14.)
                .color(colors::fg_secondary()),
        )
}

fn check_box(checked: bool) -> impl IntoElement {
    rect()
        .width(Size::px(CHECK_SIZE))
        .height(Size::px(CHECK_SIZE))
        .center()
        .corner_radius(CornerRadius::new_all(6.))
        .background(if checked { colors::brand() } else { Color::TRANSPARENT })
        .border(border_all_color(
            1.,
            if checked { colors::brand() } else { colors::component_border() },
        ).alignment(BorderAlignment::Inner))
        .maybe_child(checked.then(|| Icon::new(IconType::Check).size(14.).color(Color::WHITE)))
}
