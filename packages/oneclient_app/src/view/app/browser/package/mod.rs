use freya::prelude::*;
use std::collections::HashMap;

use oneclient_content::packages::types::DependencyKind;
use oneclient_content::packages::{ContentType, GameLoader, ProviderId};
use oneclient_core::clusters::ModpackSource;

use crate::components::{
    BundledInstallWarning, PendingBundledInstall, ScrollArea, use_shared_delete,
};
use crate::hooks::{
    ALL_VERSIONS, VERSIONS_PAGE_SIZE, bundle_overrides_map, bundles_with_status_items,
    cluster_content_items, content_type_for_slug, package_meta_batch, project_detail,
    use_bundle_overrides, use_bundles_with_status, use_cluster_content, use_dispatch,
    use_installs_snapshot, use_link_confirm, use_package_meta_batch, use_package_project,
    use_package_versions_when, version_list, versions_total,
};
use crate::hooks::{query_error, query_is_loading, settled_or_loading, use_cluster, use_clusters};
use crate::i18n::tf;
use crate::install::IncompatibleMode;
use crate::theme::{colors, rounded, text_size};
use crate::ui::border_all_color;

use super::index::override_summary;
use super::install_target::{BrowseInstallPicker, InstallRequest};
use super::{
    EnableButton, EnableVariant, Installed, InstalledVersion, ListStatus, ModpackInstancePrompt,
    ModpackLoaderPrompt, ModpackVersionPrompt, PackageBanner, Thumbnail, WorldInstallPrompt,
    activity_badge, cluster_version, cluster_versions, installed_badge, installed_map,
    instance_choices, loader_options, minecraft_choices, preferred_version, use_saved_override,
    use_version_filter,
};
use crate::utils::abbreviate_number;

mod gallery;
mod panels;
mod sidebar;
use gallery::gallery_panel;
use panels::{about_panel, failed_body, loading_body, tabs, versions_panel};
use sidebar::sidebar;

const SIDEBAR_W: f32 = 280.;
const SCROLLBAR_GUTTER: f32 = 18.;
/// The sidebar scrolls on its own, its thumb in the gap to the tabs
const SIDEBAR_GUTTER: f32 = 12.;
const COLUMN_GAP: f32 = 24.;

#[derive(Clone)]
struct Installer {
    dispatch: crate::Actions,
    cluster_id: i64,
    provider: ProviderId,
    world_prompt: Option<State<Option<(String, IncompatibleMode)>>>,
    modpack: bool,
    installed: Option<Installed>,
    project_name: String,
    bundled_warning: State<Option<PendingBundledInstall>>,
    modpack_prompt: Option<State<bool>>,
    loader_prompt: Option<State<bool>>,
    import_prompt: Option<State<bool>>,
    add_to_cluster: bool,
    cluster_name: Option<String>,
    incompatible: IncompatibleMode,
    browse_request: Option<State<Option<InstallRequest>>>,
}

impl Installer {
    fn install_latest(&self, project_id: String, version_id: String, version_label: String) {
        if let Some(mut request) = self.browse_request {
            request.set(Some(InstallRequest {
                project_id,
                project_name: self.project_name.clone(),
                version: None,
            }));
            return;
        }
        if let Some(mut prompt) = self.world_prompt {
            prompt.set(Some((version_id, IncompatibleMode::Ask)));
            return;
        }
        if let Some(mut prompt) = self.loader_prompt {
            prompt.set(true);
            return;
        }
        match self.modpack_prompt {
            Some(mut prompt) => prompt.set(true),
            None => self.install(project_id, version_id, version_label),
        }
    }

    fn install(&self, project_id: String, version_id: String, version_label: String) {
        if let Some(mut request) = self.browse_request {
            request.set(Some(InstallRequest {
                project_id,
                project_name: self.project_name.clone(),
                version: Some((version_id, version_label)),
            }));
            return;
        }
        if let Some(installed) = self
            .installed
            .as_ref()
            .filter(|installed| installed.conflicts_with_bundle(&version_id))
        {
            let mut warning = self.bundled_warning;
            warning.set(Some(PendingBundledInstall {
                cluster_id: self.cluster_id,
                provider: self.provider,
                project_id,
                version_id,
                project_name: self.project_name.clone(),
                version_label,
                bundled_version: installed.bundled_version_label(),
                incompatible: self.incompatible,
            }));
            return;
        }

        if self.add_to_cluster {
            self.dispatch.import_modpack(
                self.cluster_id,
                ModpackSource::Provider {
                    provider: self.provider,
                    project_id,
                    version_id,
                },
            );
            return;
        }
        if self.modpack {
            self.dispatch.install_modpack(ModpackSource::Provider {
                provider: self.provider,
                project_id,
                version_id,
            });
            return;
        }

        match self.world_prompt {
            Some(mut prompt) => prompt.set(Some((version_id, IncompatibleMode::Ask))),
            None => self.dispatch.install_package(
                self.cluster_id,
                self.provider,
                project_id,
                version_id,
                None,
                self.incompatible,
            ),
        }
    }
}

fn decode_package_id(package_id: &str) -> (ProviderId, String) {
    match package_id.split_once(':') {
        Some((repr, id)) => {
            let provider = repr
                .parse::<u8>()
                .ok()
                .and_then(ProviderId::from_repr)
                .unwrap_or(ProviderId::Modrinth);
            (provider, id.to_string())
        }
        None => (ProviderId::Modrinth, package_id.to_string()),
    }
}

fn pill(text: String) -> impl IntoElement {
    rect()
        .center()
        .padding(Gaps::new_symmetric(2., 8.))
        .corner_radius(rounded(999.))
        .background(colors::component_bg())
        .border(border_all_color(1., colors::component_border()))
        .child(
            label()
                .text(text)
                .font_size(text_size(10.))
                .max_lines(1)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn pill_flow(items: &[String], max: usize) -> impl IntoElement {
    let shown: Vec<String> = items.iter().take(max).cloned().collect();
    let overflow = items.len().saturating_sub(shown.len());
    let mut all = shown;
    if overflow > 0 {
        all.push(format!("+{overflow}"));
    }

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::wrap_spacing(4.))
        .spacing(4.)
        .children(all.into_iter().map(|t| pill(t).into_element()))
        .into_element()
}

fn pill_detail(key: &str, items: &[String]) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(6.)
        .child(
            label()
                .text(key.to_string())
                .font_size(text_size(12.))
                .color(colors::fg_secondary()),
        )
        .child(pill_flow(items, 18))
        .into_element()
}

#[derive(PartialEq)]
pub struct BrowserPackage {
    pub cluster_id: i64,
    pub package_type: String,
    pub package_id: String,
    pub add_to_cluster: bool,
    pub in_instance: bool,
}

impl Component for BrowserPackage {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let (provider, project_id) = decode_package_id(&self.package_id);
        let content_type = content_type_for_slug(&self.package_type);
        let is_modpack_type = content_type == ContentType::Modpack;

        let active_tab = use_state(|| 0usize);
        let versions_page = use_state(|| 0usize);
        let dispatch = use_dispatch();
        let confirm = use_link_confirm();
        let world_prompt = use_state(|| None::<(String, IncompatibleMode)>);
        let bundled_warning = use_state(|| None::<PendingBundledInstall>);
        let modpack_prompt = use_state(|| false);
        let loader_prompt = use_state(|| false);
        let chosen_loader = use_state(|| None::<GameLoader>);
        let import_prompt = use_state(|| false);
        let mut browse_request = use_state(|| None::<InstallRequest>);
        let clusters = settled_or_loading(&use_clusters()).unwrap_or_default();
        let is_datapack = content_type == ContentType::DataPack;
        let add_to_cluster = self.add_to_cluster && content_type == ContentType::Modpack;

        let cluster = use_cluster(cluster_id);
        let search_override =
            use_saved_override(cluster_id, &self.package_type).filter(|_| !is_modpack_type);
        let (game_version, loader, incompatible) =
            use_version_filter(cluster_id, content_type, search_override.clone());

        let mut versions_page_state = versions_page;
        let filter = (game_version.clone(), loader);
        let mut last_filter = use_state(|| filter.clone());
        if *last_filter.peek() != filter {
            last_filter.set(filter);
            if *versions_page_state.peek() != 0 {
                versions_page_state.set(0);
            }
        }

        let project_query = use_package_project(provider, project_id.clone());
        let is_modpack = content_type == ContentType::Modpack;
        let versions_query = use_package_versions_when(
            !is_modpack,
            provider,
            project_id.clone(),
            game_version.clone(),
            loader,
            *versions_page.read(),
            VERSIONS_PAGE_SIZE,
        );
        // "Install latest" and its badge read the newest page whichever page of history is open; on
        // the first page this is the same cached query
        let newest_query = use_package_versions_when(
            !is_modpack,
            provider,
            project_id.clone(),
            game_version,
            loader,
            0,
            VERSIONS_PAGE_SIZE,
        );
        let all_versions_query = use_package_versions_when(
            is_modpack,
            provider,
            project_id.clone(),
            None,
            None,
            0,
            ALL_VERSIONS,
        );
        let all_versions = version_list(&all_versions_query);
        let all_versions = match (add_to_cluster, &cluster) {
            (true, Some(c)) => cluster_versions(&all_versions, c),
            _ => all_versions,
        };

        let (installing, waiting) = use_installs_snapshot().package_busy(
            content_type == ContentType::Modpack,
            cluster_id,
            provider,
            &project_id,
        );

        let installed = installed_map(
            cluster_content_items(&use_cluster_content(cluster_id, content_type)),
            &bundles_with_status_items(&use_bundles_with_status(cluster_id)),
            &bundle_overrides_map(&use_bundle_overrides(cluster_id)),
        )
        .remove(&(provider, project_id.clone()))
        .filter(|_| !is_datapack);

        let project = project_detail(&project_query);

        let mut installer = Installer {
            dispatch: dispatch.clone(),
            cluster_id,
            provider,
            world_prompt: is_datapack.then_some(world_prompt),
            modpack: content_type == ContentType::Modpack,
            installed: installed.clone(),
            project_name: project.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
            bundled_warning,
            modpack_prompt: None,
            loader_prompt: None,
            import_prompt: (content_type == ContentType::Modpack && !add_to_cluster)
                .then_some(import_prompt),
            add_to_cluster,
            cluster_name: cluster.as_ref().map(|c| c.name.clone()),
            incompatible,
            browse_request: (cluster_id == 0 && !is_modpack_type).then_some(browse_request),
        };

        let remove_id = project.as_ref().map(|p| p.id.clone());
        let remove_dispatch = dispatch.clone();
        let (on_remove, remove_dialog) = use_shared_delete(cluster_id, move |(name, hash)| {
            if let Some(project_id) = &remove_id {
                remove_dispatch.remove_package_version(
                    cluster_id,
                    provider,
                    project_id.clone(),
                    hash,
                    name,
                );
            }
        });

        let (versions, total_versions) = if is_modpack {
            let page = all_versions
                .iter()
                .skip(versions_page.read().saturating_mul(VERSIONS_PAGE_SIZE))
                .take(VERSIONS_PAGE_SIZE)
                .cloned()
                .collect();
            (page, all_versions.len())
        } else {
            (
                version_list(&versions_query),
                versions_total(&versions_query),
            )
        };
        let dependency_ids: Vec<String> = versions
            .iter()
            .flat_map(|v| &v.dependencies)
            .filter(|d| d.kind == DependencyKind::Required)
            .filter_map(|d| d.project_id.clone())
            .collect();
        let dependency_names: HashMap<String, String> =
            package_meta_batch(&use_package_meta_batch(provider, dependency_ids))
                .into_iter()
                .map(|(id, meta)| (id, meta.name))
                .collect();
        let newest = version_list(&newest_query);
        let latest_source = latest_candidates(is_modpack, &all_versions, &newest);
        let latest_version = if add_to_cluster {
            cluster
                .as_ref()
                .and_then(|c| cluster_version(latest_source, c))
        } else {
            preferred_version(latest_source, content_type)
        }
        .cloned();
        let loaders = if is_modpack && !add_to_cluster {
            loader_options(&all_versions)
        } else {
            Vec::new()
        };
        let loader_choices = match *chosen_loader.read() {
            Some(loader) if loaders.len() > 1 => loaders
                .iter()
                .find(|option| option.loader == loader)
                .map(|option| option.choices.clone()),
            _ => None,
        };
        let choices = loader_choices.unwrap_or_else(|| minecraft_choices(&all_versions));
        installer.modpack_prompt =
            (is_modpack && !choices.is_empty() && !add_to_cluster).then_some(modpack_prompt);
        installer.loader_prompt = (loaders.len() > 1).then_some(loader_prompt);

        let gallery = project
            .as_ref()
            .map(|p| p.gallery.clone())
            .unwrap_or_default();
        let has_gallery = !gallery.is_empty();
        let max_tab = if has_gallery { 2 } else { 1 };
        let current = (*active_tab.read()).min(max_tab);

        let shown_versions_query = if is_modpack {
            all_versions_query
        } else {
            versions_query
        };
        let versions_status = ListStatus::of(
            !versions.is_empty(),
            query_is_loading(&shown_versions_query),
            query_error(&shown_versions_query).is_some(),
        );
        let project_failed = project.is_none() && query_error(&project_query).is_some();

        let body = match (&project, current) {
            (None, _) if project_failed => {
                failed_body("oneclient.browser.package.load_failed", move |()| {
                    project_query.invalidate()
                })
            }
            (None, _) => loading_body().into_element(),
            (Some(project), 0) => about_panel(project).into_element(),
            (Some(project), 1) => versions_panel(
                versions,
                total_versions,
                versions_page,
                dependency_names,
                project.id.clone(),
                installer.clone(),
                on_remove,
                installed.clone(),
                installing || waiting,
                versions_status,
                move |()| shown_versions_query.invalidate(),
            )
            .into_element(),
            (Some(_), _) => gallery_panel(gallery).into_element(),
        };

        // Both columns end in the surface's fade rather than a hard cut; glass has no solid surface
        let fade = if self.in_instance {
            (!colors::active_theme().liquid_glass).then(colors::page_elevated)
        } else {
            Some(colors::page())
        };
        let faded = move |area: ScrollArea| match fade {
            Some(surface) => area.bottom_fade(surface),
            None => area,
        };

        // The sidebar (with Install) and the tabs stay put; only the tab's content scrolls under them
        let row = rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::fill())
            .spacing(COLUMN_GAP - SIDEBAR_GUTTER)
            .content(Content::Flex)
            .child(
                faded(ScrollArea::new())
                    .width(Size::px(SIDEBAR_W + SIDEBAR_GUTTER))
                    .height(Size::fill())
                    .auto_hide_scrollbar()
                    .padding(Gaps::new(0., SIDEBAR_GUTTER, 0., 0.))
                    .children([sidebar(
                        project,
                        latest_version,
                        installer,
                        confirm,
                        installed,
                        installing,
                        waiting,
                    )
                    .into_element()]),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .height(Size::fill())
                    .spacing(12.)
                    .content(Content::Flex)
                    .child(tabs(active_tab, has_gallery))
                    .maybe_child(search_override.map(|o| {
                        label()
                            .text(tf(
                                // With no instance yet, Install latest resolves for the one picked then
                                if cluster_id == 0 {
                                    "oneclient.browser.package.override_active_global"
                                } else {
                                    "oneclient.browser.package.override_active"
                                },
                                &[&override_summary(&o, content_type == ContentType::Mod)],
                            ))
                            .font_size(text_size(12.))
                            .color(colors::fg_secondary())
                    }))
                    .child(
                        faded(ScrollArea::new())
                            .width(Size::fill())
                            .height(Size::flex(1.))
                            .reset_key(current as u64)
                            .padding(Gaps::new(0., SCROLLBAR_GUTTER, 0., 0.))
                            .children([body]),
                    ),
            )
            .into_element();

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .padding(if self.in_instance {
                Gaps::new(16., 18., 14., 18.)
            } else {
                Gaps::new(0., 40., 40., 40.)
            })
            .child(row)
            .maybe_child(remove_dialog)
            .maybe_child(
                browse_request
                    .read()
                    .clone()
                    .map(|install| BrowseInstallPicker {
                        provider,
                        content_type,
                        package_type: self.package_type.clone(),
                        install,
                        on_close: (move |()| browse_request.set(None)).into(),
                    }),
            )
            .maybe_child(world_prompt.read().is_some().then(|| WorldInstallPrompt {
                cluster_id,
                provider,
                project_id: project_id.clone(),
                pending: world_prompt,
            }))
            .maybe_child(loader_prompt.read().then(|| ModpackLoaderPrompt {
                options: loaders.clone(),
                open: loader_prompt,
                chosen: chosen_loader,
                version_open: modpack_prompt,
            }))
            .child(BundledInstallWarning {
                pending: bundled_warning,
            })
            .maybe_child(modpack_prompt.read().then(|| ModpackVersionPrompt {
                provider,
                project_id: project_id.clone(),
                choices,
                open: modpack_prompt,
            }))
            .maybe_child(import_prompt.read().then(|| ModpackInstancePrompt {
                provider,
                project_id: project_id.clone(),
                choices: instance_choices(&all_versions, &clusters),
                open: import_prompt,
            }))
            .into_element()
    }
}

/// "Install latest" never reads the page of history that happens to be open
fn latest_candidates<'a>(
    is_modpack: bool,
    all_versions: &'a [oneclient_content::packages::types::VersionSummary],
    newest_page: &'a [oneclient_content::packages::types::VersionSummary],
) -> &'a [oneclient_content::packages::types::VersionSummary] {
    if is_modpack {
        all_versions
    } else {
        newest_page
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oneclient_content::packages::types::VersionSummary;

    fn version(id: &str) -> VersionSummary {
        VersionSummary {
            version_id: id.into(),
            project_id: "p".into(),
            name: id.into(),
            version_number: id.into(),
            published: chrono::Utc::now(),
            release_type: Default::default(),
            game_versions: Vec::new(),
            loaders: Vec::new(),
            downloads: 0,
            file_size: 0,
            dependencies: Vec::new(),
            blocked: false,
        }
    }

    #[test]
    fn install_latest_ignores_the_open_page_of_history() {
        let newest = [version("3.0"), version("2.9")];
        let all = [version("pack-1")];
        let latest = latest_candidates(false, &all, &newest);
        assert_eq!(
            preferred_version(latest, ContentType::Mod).map(|v| v.version_id.as_str()),
            Some("3.0")
        );
        // Modpacks list every version at once and pick from all of them
        assert_eq!(
            latest_candidates(true, &all, &newest)[0].version_id,
            "pack-1"
        );
    }
}
