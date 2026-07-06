use freya::animation::*;
use freya::prelude::*;
use freya::query::QueryStateData;
use freya::router::*;
use oneclient_core::clusters::Cluster;
use oneclient_core::parse_mc_version;

use crate::components::{Button, Icon, IconType, TabBar, TabItem};
use crate::hooks::{use_clusters, use_dispatch, use_game_snapshot, use_version_metadata};
use crate::view::app::launch_button_state;
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::entrance_motion_layer;

const HEADER_HEIGHT: f32 = 64.;
const TABS_HEIGHT: f32 = 44.;
const BAR_SPACING: f32 = 16.;

#[derive(PartialEq, Clone, Copy)]
pub enum ClusterViewShellTab {
    Overview,
    Logs,
    GameLog,
    Screenshots,
    Mods,
    Shaders,
    Textures,
    Settings,
}

impl ClusterViewShellTab {
    fn label(&self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Logs => "Logs",
            Self::GameLog => "Game Log",
            Self::Screenshots => "Screenshots",
            Self::Mods => "Mods",
            Self::Shaders => "Shaders",
            Self::Textures => "Textures",
            Self::Settings => "Settings",
        }
    }

    fn route(&self, cluster_id: i64) -> Option<Route> {
        match self {
            Self::Overview => Some(Route::ClusterOverview { cluster_id }),
            Self::Logs => Some(Route::ClusterLogs { cluster_id }),
            Self::GameLog => Some(Route::ProcessLogs { cluster_id }),
            Self::Screenshots => Some(Route::ClusterScreenshots { cluster_id }),
            Self::Mods => Some(Route::ClusterMods { cluster_id }),
            Self::Shaders => Some(Route::ClusterShaders { cluster_id }),
            Self::Textures => Some(Route::ClusterTextures { cluster_id }),
            Self::Settings => Some(Route::ClusterSettings { cluster_id }),
        }
    }

    fn index(self) -> i32 {
        match self {
            Self::Overview => 0,
            Self::Logs => 1,
            Self::Screenshots => 2,
            Self::Mods => 3,
            Self::Shaders => 4,
            Self::Textures => 5,
            Self::Settings => 6,
            Self::GameLog => 7,
        }
    }
}

const SIDE_PADDING: f32 = 40.;

pub fn cluster_content() -> Rect {
    rect().vertical().width(Size::fill()).height(Size::fill())
}

fn route_cluster(route: &Route) -> Option<(i64, ClusterViewShellTab)> {
    Some(match route {
        Route::ClusterOverview { cluster_id } => (*cluster_id, ClusterViewShellTab::Overview),
        Route::ClusterLogs { cluster_id } => (*cluster_id, ClusterViewShellTab::Logs),
        Route::ProcessLogs { cluster_id } => (*cluster_id, ClusterViewShellTab::GameLog),
        Route::ClusterScreenshots { cluster_id } => {
            (*cluster_id, ClusterViewShellTab::Screenshots)
        }
        Route::ClusterMods { cluster_id } => (*cluster_id, ClusterViewShellTab::Mods),
        Route::ClusterShaders { cluster_id } => (*cluster_id, ClusterViewShellTab::Shaders),
        Route::ClusterTextures { cluster_id } => (*cluster_id, ClusterViewShellTab::Textures),
        Route::ClusterSettings { cluster_id } => (*cluster_id, ClusterViewShellTab::Settings),
        _ => return None,
    })
}

fn load_cluster(cluster_id: i64) -> Option<Cluster> {
    let clusters_query = use_clusters();
    let reader = clusters_query.read();
    let list = match &*reader.state() {
        QueryStateData::Settled { res: Ok(list), .. }
        | QueryStateData::Loading {
            res: Some(Ok(list)),
        } => list.clone(),
        _ => Vec::new(),
    };
    list.into_iter().find(|c| c.id == cluster_id)
}

#[derive(PartialEq)]
pub struct ClusterShell;

impl Component for ClusterShell {
    fn render(&self) -> impl IntoElement {
        let route = use_route::<Route>();
        let (cluster_id, active_tab) =
            route_cluster(&route).unwrap_or((0, ClusterViewShellTab::Overview));

        let dispatch = use_dispatch();
        let game = use_game_snapshot();
        let cluster = load_cluster(cluster_id);

        let show_game_log = game.is_active(cluster_id);
        let launch_state = launch_button_state(&game, cluster_id);

        let header = cluster.as_ref().map(|cluster| {
            let title = format!("{} {}", cluster.mc_loader, cluster.mc_version);
            let parsed = parse_mc_version(&cluster.mc_version);
            let metadata = use_version_metadata(
                parsed.as_ref().map(|p| p.major),
                parsed.and_then(|p| p.minor),
                Some(cluster.mc_loader),
            );
            let subtitle = metadata
                .and_then(|m| m.long_description)
                .unwrap_or_else(|| cluster.name.clone());
            cluster_header(
                title,
                subtitle,
                cluster_id,
                dispatch,
                launch_state,
                game.is_running(cluster_id),
                cluster.game_dir().ok(),
            )
            .into_element()
        });

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .padding(Gaps::new(BAR_SPACING, SIDE_PADDING, BAR_SPACING, SIDE_PADDING))
                    .spacing(BAR_SPACING)
                    .maybe_child(header)
                    .child(cluster_tabs(active_tab, cluster_id, show_game_log)),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .overflow(Overflow::Clip)
                    .padding(Gaps::new(0., SIDE_PADDING, SIDE_PADDING, SIDE_PADDING))
                    .child(AnimatedRouter::<Route>::new(ClusterContentOutlet)),
            )
    }
}

fn tab_dir(from: &Route, to: &Route) -> f32 {
    match (route_cluster(from), route_cluster(to)) {
        (Some((_, a)), Some((_, b))) => (b.index() - a.index()).signum() as f32,
        _ => 0.,
    }
}

#[derive(PartialEq)]
struct ClusterContentOutlet;

impl Component for ClusterContentOutlet {
    fn render(&self) -> impl IntoElement {
        let mut router = use_animated_router::<Route>();

        let anim = use_animation(|_conf| {
            AnimNum::new(0., 1.)
                .time(300)
                .ease(Ease::Out)
                .function(Function::Cubic)
        });
        let mut last_to = use_state(|| None::<Route>);

        let (dir, to, is_transition) = match &*router.read() {
            AnimatedRouterContext::FromTo(from, to) => (tab_dir(from, to), to.clone(), true),
            AnimatedRouterContext::In(to) => (0., to.clone(), false),
        };

        if last_to.peek().as_ref() != Some(&to) {
            last_to.set(Some(to.clone()));
            if is_transition {
                anim.run(AnimDirection::Forward);
            }
        }

        let anim_finished = *anim.has_run_yet().read() && !*anim.is_running().read();

        use_side_effect_with_deps(&anim_finished, move |&finished| {
            if finished {
                Platform::get().send(UserEvent::RequestRedraw);
            }
        });

        if anim_finished {
            if matches!(&*router.peek(), AnimatedRouterContext::FromTo(_, _)) {
                router.write().settle();
            }
        }

        const CONTENT_DX: f32 = 44.;
        let p = if anim_finished { 1.0 } else { anim.get().value() };
        let slide_x = if anim_finished || dir == 0. {
            0.
        } else {
            dir * (1.0 - p) * CONTENT_DX
        };
        let opacity = if anim_finished || dir == 0. { 1. } else { p };

        entrance_motion_layer(slide_x, 0., opacity, Outlet::<Route>::new())
    }
}

fn cluster_header(
    title: String,
    subtitle: String,
    cluster_id: i64,
    dispatch: crate::BridgeDispatch,
    launch_state: (&'static str, bool),
    running: bool,
    folder: Option<std::path::PathBuf>,
) -> impl IntoElement {
    let (launch_label, launch_enabled) = launch_state;
    let kill_dispatch = dispatch.clone();
    rect()
        .horizontal()
        .content(Content::Flex)
        .width(Size::fill())
        .height(Size::px(HEADER_HEIGHT))
        .cross_align(Alignment::Center)
        .spacing(12.)
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .height(Size::fill())
                .main_align(Alignment::Center)
                .spacing(6.)
                .child(
                    label()
                        .text(title)
                        .font_size(28.)
                        .font_weight(FontWeight::BOLD)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(subtitle)
                        .font_size(12.)
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(10.)
                .maybe_child(folder.map(crate::components::open_folder_button))
                .maybe(running, |el| {
                    el.child(
                        Button::new()
                            .danger()
                            .icon()
                            .on_press(move |_| kill_dispatch.kill_cluster(cluster_id))
                            .child(Icon::new(IconType::Square).size(16.)),
                    )
                })
                .child(
                    Button::new()
                        .primary()
                        .large()
                        .enabled(launch_enabled)
                        .on_press(move |_| {
                            if launch_enabled {
                                dispatch.launch_cluster(cluster_id);
                            }
                        })
                        .text(launch_label),
                ),
        )
}

fn cluster_tabs(
    active: ClusterViewShellTab,
    cluster_id: i64,
    show_game_log: bool,
) -> impl IntoElement {
    let tabs = [
        Some(ClusterViewShellTab::Overview),
        Some(ClusterViewShellTab::Logs),
        Some(ClusterViewShellTab::Screenshots),
        Some(ClusterViewShellTab::Mods),
        Some(ClusterViewShellTab::Shaders),
        Some(ClusterViewShellTab::Textures),
        Some(ClusterViewShellTab::Settings),
        show_game_log.then_some(ClusterViewShellTab::GameLog),
    ];

    let tab_items = tabs.into_iter().flatten().map(|tab| {
        let route = tab.route(cluster_id);
        let mut item = TabItem::new(tab.label(), tab == active);
        if let Some(route) = route {
            item = item.on_press(move |_| {
                let _ = RouterContext::get().push(route.clone());
            });
        }
        item
    });

    rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::px(TABS_HEIGHT))
        .cross_align(Alignment::Center)
        .padding(Gaps::new(0., 24., 0., 24.))
        .background(colors::page_elevated())
        .corner_radius(CornerRadius::new_all(12.))
        .child(
            TabBar::new()
                .width(Size::fill())
                .height(Size::fill())
                .spacing(24.)
                .font_size(12.)
                .tabs(tab_items),
        )
}
