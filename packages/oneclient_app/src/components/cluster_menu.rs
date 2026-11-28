use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::clusters::Cluster;

use crate::components::{ContextMenu, IconType};
use crate::hooks::{modpack_job_running, use_dispatch, use_game_snapshot, use_settings_snapshot};
use crate::i18n::t;
use crate::routes::Route;
use crate::view::app::DeleteInstanceModal;

#[derive(Clone, PartialEq)]
pub struct ClusterMenuTarget {
    pub cluster_id: i64,
    pub title: String,
    pub mod_tabs: bool,
    pub deletable: bool,
}

impl ClusterMenuTarget {
    pub fn for_cluster(cluster: &Cluster, deletable: bool) -> Self {
        Self {
            cluster_id: cluster.id,
            title: cluster_title(cluster),
            mod_tabs: !cluster.lacks_mod_loader(),
            deletable: deletable && cluster.user_created,
        }
    }
}

pub fn cluster_title(cluster: &Cluster) -> String {
    if cluster.user_created {
        cluster.name.clone()
    } else {
        format!("{} {}", cluster.mc_version, cluster.mc_loader)
    }
}

pub(crate) fn cluster_menu_entries(cluster_id: i64) -> [(IconType, &'static str, Route); 7] {
    [
        (
            IconType::InfoCircle,
            t("oneclient.components.cluster_menu.overview"),
            Route::ClusterOverview { cluster_id },
        ),
        (
            IconType::Terminal,
            t("oneclient.components.cluster_menu.logs"),
            Route::ClusterLogs { cluster_id },
        ),
        (
            IconType::Eye,
            t("oneclient.components.cluster_menu.screenshots"),
            Route::ClusterScreenshots { cluster_id },
        ),
        (
            IconType::CodeSnippet02,
            t("oneclient.components.cluster_menu.mods"),
            Route::ClusterMods { cluster_id },
        ),
        (
            IconType::PaintPour,
            t("oneclient.components.cluster_menu.shaders"),
            Route::ClusterShaders { cluster_id },
        ),
        (
            IconType::Colors,
            t("oneclient.components.cluster_menu.textures"),
            Route::ClusterTextures { cluster_id },
        ),
        (
            IconType::Settings01,
            t("oneclient.components.cluster_menu.settings"),
            Route::ClusterSettings { cluster_id },
        ),
    ]
}

pub(crate) fn shows_entry(route: &Route, mod_tabs: bool) -> bool {
    mod_tabs
        || !matches!(
            route,
            Route::ClusterMods { .. } | Route::ClusterShaders { .. }
        )
}

pub(crate) fn open_menu_at(
    mut position: State<Option<(f32, f32)>>,
) -> impl FnMut(Event<PressEventData>) {
    move |e: Event<PressEventData>| {
        if let PressEventData::Mouse(m) = e.data() {
            position.set(Some((
                m.global_location.x as f32,
                m.global_location.y as f32,
            )));
        }
    }
}

#[derive(PartialEq)]
pub struct ClusterContextMenu {
    pub target: ClusterMenuTarget,
    pub position: State<Option<(f32, f32)>>,
}

impl Component for ClusterContextMenu {
    fn render(&self) -> impl IntoElement {
        let game = use_game_snapshot();
        let dispatch = use_dispatch();
        let pins = use_settings_snapshot().settings.pinned_cluster_ids;
        let mut deleting = use_state(|| false);
        let mut position = self.position;
        let target = self.target.clone();
        let cluster_id = target.cluster_id;
        let can_delete =
            target.deletable && !game.is_running(cluster_id) && !modpack_job_running(cluster_id);

        let menu = (*position.read()).map(|(x, y)| {
            let mut context = ContextMenu::new(x, y)
                .title(target.title.clone())
                .on_close(move |_| position.set(None));
            for (icon, label, route) in cluster_menu_entries(cluster_id)
                .into_iter()
                .filter(|(_, _, route)| shows_entry(route, target.mod_tabs))
            {
                context = context.action(icon, label, move |()| {
                    position.set(None);
                    let _ = RouterContext::get().push(route.clone());
                });
            }

            let pin_index = pins.iter().position(|id| *id == cluster_id);
            if let Some(index) = pin_index {
                if index > 0 {
                    let dispatch = dispatch.clone();
                    context = context.action(
                        IconType::ArrowLeft,
                        t("oneclient.components.cluster_menu.move_earlier"),
                        move |()| {
                            position.set(None);
                            dispatch.move_pinned_cluster(cluster_id, -1);
                        },
                    );
                }
                if index < pins.len() - 1 {
                    let dispatch = dispatch.clone();
                    context = context.action(
                        IconType::ArrowRight,
                        t("oneclient.components.cluster_menu.move_later"),
                        move |()| {
                            position.set(None);
                            dispatch.move_pinned_cluster(cluster_id, 1);
                        },
                    );
                }
                let dispatch = dispatch.clone();
                context = context.action(
                    IconType::Pin01,
                    t("oneclient.components.cluster_menu.unpin"),
                    move |()| {
                        position.set(None);
                        dispatch.unpin_cluster(cluster_id);
                    },
                );
            } else {
                let dispatch = dispatch.clone();
                context = context.action(
                    IconType::Pin01,
                    t("oneclient.components.cluster_menu.pin"),
                    move |()| {
                        position.set(None);
                        dispatch.pin_cluster(cluster_id);
                    },
                );
            }

            if can_delete {
                context = context.separator().danger_action(
                    IconType::Trash01,
                    t("oneclient.common.delete"),
                    move |()| {
                        position.set(None);
                        deleting.set(true);
                    },
                );
            }
            context.into_element()
        });

        let modal = deleting.read().then(|| {
            DeleteInstanceModal::new(cluster_id, target.title.clone(), move |()| {
                deleting.set(false)
            })
            .stay_on_page()
            .into_element()
        });

        rect().maybe_child(menu).maybe_child(modal)
    }
}
