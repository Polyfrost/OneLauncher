use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::clusters::Cluster;

use crate::components::{ContextMenu, IconType};
use crate::hooks::{modpack_job_running, use_game_snapshot};
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
            "Overview",
            Route::ClusterOverview { cluster_id },
        ),
        (
            IconType::Terminal,
            "Logs",
            Route::ClusterLogs { cluster_id },
        ),
        (
            IconType::Eye,
            "Screenshots",
            Route::ClusterScreenshots { cluster_id },
        ),
        (
            IconType::CodeSnippet02,
            "Mods",
            Route::ClusterMods { cluster_id },
        ),
        (
            IconType::PaintPour,
            "Shaders",
            Route::ClusterShaders { cluster_id },
        ),
        (
            IconType::Colors,
            "Textures",
            Route::ClusterTextures { cluster_id },
        ),
        (
            IconType::Settings01,
            "Settings",
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
            if can_delete {
                context =
                    context
                        .separator()
                        .danger_action(IconType::Trash01, "Delete", move |()| {
                            position.set(None);
                            deleting.set(true);
                        });
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
