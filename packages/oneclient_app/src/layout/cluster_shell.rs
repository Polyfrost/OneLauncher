use freya::animation::*;
use freya::prelude::*;
use freya::router::*;

use crate::components::{
    ART_PREVIEW_EDGE, Button, DynamicArt, Icon, IconType, ScrollArea, TooltipPlacement,
    use_tooltip_anchor,
};
use crate::hooks::{use_bisect, use_cluster, use_dispatch, use_game_snapshot, use_launcher};
use crate::i18n::t;
use crate::routes::Route;
use crate::theme::{colors, glass, rounded, text_size};

use super::{workspace_dialog::WorkspaceLayout, workspace_rail};
use crate::ui::entrance_motion_layer;
use crate::view::app::ExportInstanceModal;
use crate::view::app::{
    launch_button_label, launch_button_state, launch_syncing, use_shared_dir_taken,
};

/// Launch button height; the icon buttons beside it are squares of the same size
const ACTION_HEIGHT: f32 = 40.;
const ACTION_ICON_SIZE: f32 = 20.;

#[derive(PartialEq, Clone, Copy)]
pub enum ClusterViewShellTab {
    Overview,
    Logs,
    GameLog,
    Screenshots,
    Mods,
    Shaders,
    Textures,
    Worlds,
    DataPacks,
    Settings,
}

impl ClusterViewShellTab {
    fn label(&self) -> &'static str {
        match self {
            Self::Overview => t("oneclient.layout.cluster.tab.overview"),
            Self::Logs => t("oneclient.layout.cluster.tab.logs"),
            Self::GameLog => t("oneclient.layout.cluster.tab.live_log"),
            Self::Screenshots => t("oneclient.layout.cluster.tab.screenshots"),
            Self::Mods => t("oneclient.layout.cluster.tab.mods"),
            Self::Shaders => t("oneclient.layout.cluster.tab.shaders"),
            Self::Textures => t("oneclient.layout.cluster.tab.textures"),
            Self::Worlds => t("oneclient.layout.cluster.tab.worlds"),
            Self::DataPacks => t("oneclient.layout.cluster.tab.data_packs"),
            Self::Settings => t("oneclient.layout.cluster.tab.settings"),
        }
    }

    fn route(&self, cluster_id: i64) -> Route {
        match self {
            Self::Overview => Route::ClusterOverview { cluster_id },
            Self::Logs => Route::ClusterLogs { cluster_id },
            Self::GameLog => Route::ProcessLogs { cluster_id },
            Self::Screenshots => Route::ClusterScreenshots { cluster_id },
            Self::Mods => Route::ClusterMods { cluster_id },
            Self::Shaders => Route::ClusterShaders { cluster_id },
            Self::Textures => Route::ClusterTextures { cluster_id },
            Self::Worlds => Route::ClusterWorlds { cluster_id },
            Self::DataPacks => Route::ClusterDataPacks { cluster_id },
            Self::Settings => Route::ClusterSettings { cluster_id },
        }
    }

    fn icon(self) -> IconType {
        match self {
            Self::Overview => IconType::LayoutTop,
            Self::Mods => IconType::Cube01,
            Self::Shaders => IconType::PaintPour,
            Self::Textures => IconType::LayersThree01,
            Self::DataPacks => IconType::Database01,
            Self::Worlds => IconType::Globe01,
            Self::Screenshots => IconType::Eye,
            Self::Logs => IconType::File02,
            Self::GameLog => IconType::Terminal,
            Self::Settings => IconType::Settings01,
        }
    }
}

pub fn cluster_content() -> Rect {
    rect().vertical().width(Size::fill()).height(Size::fill())
}

fn route_cluster(route: &Route) -> Option<(i64, ClusterViewShellTab)> {
    Some(match route {
        Route::ClusterOverview { cluster_id } => (*cluster_id, ClusterViewShellTab::Overview),
        Route::ClusterLogs { cluster_id } => (*cluster_id, ClusterViewShellTab::Logs),
        Route::ProcessLogs { cluster_id } => (*cluster_id, ClusterViewShellTab::GameLog),
        Route::ClusterScreenshots { cluster_id } => (*cluster_id, ClusterViewShellTab::Screenshots),
        Route::ClusterMods { cluster_id } => (*cluster_id, ClusterViewShellTab::Mods),
        Route::ClusterShaders { cluster_id } => (*cluster_id, ClusterViewShellTab::Shaders),
        Route::ClusterTextures { cluster_id } => (*cluster_id, ClusterViewShellTab::Textures),
        Route::ClusterWorlds { cluster_id } => (*cluster_id, ClusterViewShellTab::Worlds),
        Route::ClusterDataPacks { cluster_id } => (*cluster_id, ClusterViewShellTab::DataPacks),
        Route::ClusterSettings { cluster_id } => (*cluster_id, ClusterViewShellTab::Settings),
        _ => return None,
    })
}

#[derive(PartialEq)]
pub struct ClusterShell;

impl Component for ClusterShell {
    fn render(&self) -> impl IntoElement {
        let route = use_route::<Route>();
        let layout = WorkspaceLayout::current();
        let (cluster_id, active_tab) =
            route_cluster(&route).unwrap_or((0, ClusterViewShellTab::Overview));

        let dispatch = use_dispatch();
        let game = use_game_snapshot();
        let launcher = use_launcher();
        let cluster = use_cluster(cluster_id);
        let syncing = launch_syncing(&launcher, cluster.as_ref().is_none_or(|c| c.uses_bundles()));
        let shared_dir_taken = use_shared_dir_taken(cluster_id);

        let show_game_log = game.is_active(cluster_id);
        let show_datapacks = cluster
            .as_ref()
            .is_none_or(|c| crate::view::app::cluster::supports_datapacks(&c.mc_version));
        let show_mod_tabs = cluster.as_ref().is_none_or(|c| !c.lacks_mod_loader());
        let launch_state = launch_button_state(&game, cluster_id, syncing, shared_dir_taken);
        let bisecting = use_bisect(cluster_id).is_some();
        let mut question_hidden = use_state(|| false);
        let mut sharing = use_state(|| false);
        let open_game_output = use_state(|| false);
        use_hook(move || {
            provide_root_context(crate::view::app::cluster::OpenGameOutput(open_game_output))
        });
        use_side_effect_with_deps(&(cluster_id, show_game_log), move |_: &(i64, bool)| {
            question_hidden.set(false)
        });

        let hidden_tab = match active_tab {
            ClusterViewShellTab::Mods | ClusterViewShellTab::Shaders => !show_mod_tabs,
            ClusterViewShellTab::DataPacks => !show_datapacks,
            _ => false,
        };
        use_side_effect_with_deps(
            &hidden_tab.then_some(cluster_id),
            move |redirect: &Option<i64>| {
                if let Some(cluster_id) = *redirect {
                    let _ = RouterContext::get().replace(Route::ClusterOverview { cluster_id });
                }
            },
        );

        // Scoped to the whole instance, unlike the per-tab folder actions in each toolbar
        let game_folder = cluster.as_ref().and_then(|cluster| {
            Some(GameFolder {
                path: cluster.game_dir().ok()?,
                shared: !cluster.uses_dedicated_dir(),
            })
        });

        let header = cluster.as_ref().map(|cluster| {
            let branded = crate::utils::builtin_oneclient_title(cluster);
            let subtitle = if branded.is_some() {
                Some(cluster.mc_loader.to_string())
            } else {
                Some(format!(
                    "{} \u{b7} {}",
                    cluster.mc_version, cluster.mc_loader
                ))
            };
            let title = branded.unwrap_or_else(|| crate::utils::instance_display_name(cluster));
            cluster_header(
                DynamicArt::for_cluster(cluster).max_edge(ART_PREVIEW_EDGE),
                title,
                subtitle,
                cluster_id,
                dispatch,
                launch_state,
                game.is_running(cluster_id),
                sharing,
                layout,
            )
            .into_element()
        });

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .maybe_child(header)
            .child(
                rect()
                    .horizontal()
                    .content(Content::Flex)
                    .width(Size::fill())
                    .height(Size::flex(1.))
                    .spacing(layout.inset)
                    .padding(Gaps::new(0., layout.inset, layout.inset, layout.inset))
                    .child(cluster_tabs(
                        active_tab,
                        cluster_id,
                        show_game_log,
                        show_datapacks,
                        show_mod_tabs,
                        game_folder,
                        layout,
                    ))
                    .child(
                        rect()
                            .vertical()
                            .content(Content::Flex)
                            .width(Size::flex(1.))
                            .height(Size::fill())
                            .overflow(Overflow::Clip)
                            .maybe_child(bisecting.then(|| {
                                crate::view::app::cluster::BisectBanner {
                                    cluster_id,
                                    question_hidden,
                                }
                                .into_element()
                            }))
                            .child(
                                rect()
                                    .width(Size::fill())
                                    .height(Size::flex(1.))
                                    .corner_radius(rounded(12.))
                                    .overflow(Overflow::Clip)
                                    .child(AnimatedRouter::<Route>::new(ClusterContentOutlet)),
                            ),
                    ),
            )
            .maybe_child(bisecting.then(|| {
                crate::view::app::cluster::BisectQuestion {
                    cluster_id,
                    hidden: question_hidden,
                }
                .into_element()
            }))
            .maybe_child(cluster.as_ref().filter(|_| *sharing.read()).map(|cluster| {
                ExportInstanceModal::new(
                    cluster_id,
                    crate::utils::instance_display_name(cluster),
                    move |()| sharing.set(false),
                )
                .into_element()
            }))
    }
}

#[derive(PartialEq)]
struct ClusterContentOutlet;

impl Component for ClusterContentOutlet {
    fn render(&self) -> impl IntoElement {
        let mut router = use_animated_router::<Route>();

        let anim = use_animation(|_conf| {
            AnimNum::new(0., 1.)
                .time(160)
                .ease(Ease::Out)
                .function(Function::Cubic)
        });
        let mut last_to = use_state(|| None::<Route>);

        let (to, is_transition) = match &*router.read() {
            AnimatedRouterContext::FromTo(_, to) => (to.clone(), true),
            AnimatedRouterContext::In(to) => (to.clone(), false),
        };
        let motion = crate::motion::use_animations_enabled();

        if last_to.peek().as_ref() != Some(&to) {
            last_to.set(Some(to.clone()));
            if is_transition {
                anim.run(AnimDirection::Forward);
            }
        }

        let anim_finished = *anim.has_run_yet().read() && !*anim.is_running().read();

        use_side_effect_with_deps(&anim_finished, move |&finished| {
            if finished {
                Platform::get().current_window().request_redraw();
            }
        });

        if anim_finished && matches!(&*router.peek(), AnimatedRouterContext::FromTo(_, _)) {
            router.write().settle();
        }

        let opacity = if anim_finished || !is_transition || !motion {
            1.
        } else {
            0.82 + anim.get().value() * 0.18
        };
        entrance_motion_layer(0., 0., opacity, Outlet::<Route>::new())
    }
}

#[expect(clippy::too_many_arguments)]
fn cluster_header(
    artwork: DynamicArt,
    title: String,
    subtitle: Option<String>,
    cluster_id: i64,
    dispatch: crate::Actions,
    launch_state: (&'static str, bool),
    running: bool,
    mut sharing: State<bool>,
    layout: WorkspaceLayout,
) -> impl IntoElement {
    let (launch_label, launch_enabled) = launch_state;
    let kill_dispatch = dispatch.clone();
    let (action_height, artwork_width, artwork_height) = if layout.compact {
        (34., 64., 40.)
    } else {
        (ACTION_HEIGHT, 74., 48.)
    };
    let launch_button = Button::new().primary();
    let launch_button = if layout.compact {
        launch_button.medium()
    } else {
        launch_button.large()
    };
    rect()
        .horizontal()
        .content(Content::Flex)
        .width(Size::fill())
        .padding(Gaps::new_symmetric(
            if layout.compact { 8. } else { 12. },
            layout.inset,
        ))
        .cross_align(Alignment::Center)
        .spacing(12.)
        .child(
            rect()
                .width(Size::px(artwork_width))
                .height(Size::px(artwork_height))
                .corner_radius(rounded(8.))
                .overflow(Overflow::Clip)
                .child(artwork),
        )
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .main_align(Alignment::Center)
                .spacing(if layout.compact { 3. } else { 6. })
                .child(
                    label()
                        .text(title)
                        .font_size(text_size(if layout.compact { 22. } else { 24. }))
                        .max_lines(if layout.compact { 1 } else { 2 })
                        .text_overflow(TextOverflow::Ellipsis)
                        .font_weight(FontWeight::BOLD)
                        .color(colors::fg_primary()),
                )
                .maybe_child(subtitle.map(|subtitle| {
                    label()
                        .text(subtitle)
                        .font_size(text_size(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_secondary())
                })),
        )
        .child(
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(if layout.compact { 8. } else { 10. })
                .child(
                    Button::new()
                        .secondary()
                        .icon()
                        .width(Size::px(action_height))
                        .height(Size::px(action_height))
                        .tooltip(t("oneclient.layout.cluster.share"))
                        .on_press(move |_| sharing.set(true))
                        .child(Icon::new(IconType::Share01).size(ACTION_ICON_SIZE)),
                )
                .maybe(running, |el| {
                    el.child(
                        Button::new()
                            .danger()
                            .icon()
                            .width(Size::px(action_height))
                            .height(Size::px(action_height))
                            .tooltip(t("oneclient.layout.cluster.stop_game"))
                            .on_press(move |_| kill_dispatch.kill_cluster(cluster_id))
                            .child(Icon::new(IconType::Square).size(ACTION_ICON_SIZE)),
                    )
                })
                .child(launch_button_label(
                    launch_button
                        .height(Size::px(action_height))
                        .enabled(launch_enabled)
                        .on_press(move |_| {
                            if launch_enabled {
                                dispatch.launch_cluster(cluster_id);
                            }
                        }),
                    launch_label,
                )),
        )
}

fn cluster_tabs(
    active: ClusterViewShellTab,
    cluster_id: i64,
    show_game_log: bool,
    show_datapacks: bool,
    show_mod_tabs: bool,
    game_folder: Option<GameFolder>,
    layout: WorkspaceLayout,
) -> impl IntoElement {
    use ClusterViewShellTab::*;

    let groups = [
        [Some(Overview), show_mod_tabs.then_some(Mods), None, None],
        [
            show_mod_tabs.then_some(Shaders),
            Some(Textures),
            show_datapacks.then_some(DataPacks),
            Some(Worlds),
        ],
        [
            Some(Screenshots),
            Some(Logs),
            (show_game_log || active == GameLog).then_some(GameLog),
            None,
        ],
    ];

    let tabs: Vec<_> = groups
        .iter()
        .flatten()
        .copied()
        .flatten()
        .chain([Settings])
        .collect();
    let track = glass::use_vertical_pill_track(tabs.len());
    let lens = colors::active_theme().liquid_glass.then_some(track);
    let selected = tabs.iter().position(|tab| *tab == active);
    let make_tab = move |tab: ClusterViewShellTab, index| {
        workspace_rail::Tab {
            label: tab.label(),
            icon: tab.icon(),
            route: tab.route(cluster_id),
            active: tab == active,
            has_dot: false,
            height: layout.row_height,
            lens: lens.map(|track| (track, index)),
        }
        .into_element()
    };

    // Leave enough flexible space to place the two lower actions at the foot of
    // the panel. On short windows they scroll with the rest of the sidebar.
    let row_count = tabs.len() + usize::from(game_folder.is_some());
    let divider_height = if layout.compact { 7. } else { 9. };
    let navigation_height = row_count as f32 * layout.row_height
        + 3. * divider_height
        + (row_count + 3) as f32 * layout.row_gap;
    let settings_index = tabs.len() - 1;

    workspace_rail::surface()
        .map(lens, |el, track| {
            track.events(el, move |index| {
                if Some(index) != selected {
                    workspace_rail::select(&tabs[index].route(cluster_id));
                }
            })
        })
        .child(
            rect()
                .width(Size::fill())
                .height(Size::flex(1.))
                .map(lens, |el, track| track.viewport(el))
                .child(
                    ScrollArea::new()
                        .width(Size::fill())
                        .height(Size::fill())
                        .scrollbar_gutter(true)
                        .auto_hide_scrollbar()
                        .content(move |ctx| {
                            let mut index = 0;
                            let mut content = rect()
                                .vertical()
                                .width(Size::fill())
                                .spacing(layout.row_gap)
                                .maybe_child(lens.map(|track| track.indicator(selected)));
                            for (group_index, group) in groups.iter().enumerate() {
                                if group_index != 0 {
                                    content = content.child(nav_divider(divider_height));
                                }
                                for tab in group.iter().flatten() {
                                    content = content.child(make_tab(*tab, index));
                                    index += 1;
                                }
                            }

                            content
                                .child(
                                    rect().height(Size::px(
                                        (ctx.viewport_h - navigation_height).max(0.),
                                    )),
                                )
                                .child(nav_divider(divider_height))
                                .maybe_child(game_folder.clone().map(|folder| ClusterFolderRow {
                                    folder,
                                    height: layout.row_height,
                                }))
                                .child(make_tab(Settings, settings_index))
                                .into_element()
                        }),
                ),
        )
}

/// The instance's whole game folder, shown above Settings so it is not mistaken for a tab's own folder
#[derive(Clone, PartialEq)]
struct GameFolder {
    path: std::path::PathBuf,
    shared: bool,
}

#[derive(PartialEq)]
struct ClusterFolderRow {
    folder: GameFolder,
    height: f32,
}

impl Component for ClusterFolderRow {
    fn render(&self) -> impl IntoElement {
        let mut hover = use_state(|| false);
        let id = use_a11y();
        let focus = use_focus(id);
        let keyboard = focus() == Focus::Keyboard;
        let name = t("oneclient.layout.cluster.game_folder");
        let tip = if self.folder.shared {
            format!(
                "{} {}",
                t("oneclient.layout.cluster.game_folder.tooltip"),
                t("oneclient.layout.cluster.game_folder.shared")
            )
        } else {
            t("oneclient.layout.cluster.game_folder.tooltip").to_string()
        };
        let tooltip = use_tooltip_anchor(Some(tip.into()), TooltipPlacement::Right);
        let path = self.folder.path.clone();
        let ink = if hover() || keyboard {
            colors::fg_primary()
        } else {
            colors::fg_secondary()
        };

        tooltip
            .attach(rect())
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::px(self.height))
            .padding(Gaps::new_symmetric(0., 7.))
            .spacing(7.)
            .cross_align(Alignment::Center)
            .corner_radius(rounded(7.))
            .background(if hover() {
                colors::ghost_overlay_hover()
            } else {
                Color::TRANSPARENT
            })
            .maybe(keyboard, |el| {
                el.border(crate::ui::border_all_color(1., colors::fg_primary()))
            })
            .a11y_id(id)
            .a11y_role(AccessibilityRole::Button)
            .a11y_focusable(true)
            .a11y_alt(name)
            .cursor(CursorIcon::Pointer)
            .on_pointer_over(move |_| hover.set_if_modified(true))
            .on_pointer_out(move |_| hover.set_if_modified(false))
            .on_press(move |_| {
                std::fs::create_dir_all(&path).ok();
                crate::platform::open_path(&path.to_string_lossy());
            })
            .child(Icon::new(IconType::Folder).size(17.).color(ink))
            .child(
                label()
                    .text(name)
                    .width(Size::flex(1.))
                    .max_lines(1)
                    .text_overflow(TextOverflow::Ellipsis)
                    .font_size(text_size(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .color(ink),
            )
            .child(
                Icon::new(IconType::LinkExternal01)
                    .size(13.)
                    .color(colors::fg_secondary().with_a(150)),
            )
    }
}

fn nav_divider(height: f32) -> Rect {
    rect()
        .width(Size::fill())
        .height(Size::px(1.))
        .margin(Gaps::new_symmetric((height - 1.) / 2., 7.))
        .background(colors::component_border())
}

#[cfg(test)]
mod tests {
    use super::*;
    use freya_testing::TestingRunner;
    use std::time::Duration;

    #[test]
    fn compact_instance_rail_keeps_settings_below_all_optional_tabs() {
        let (mut runner, ()) = TestingRunner::new(
            || {
                cluster_tabs(
                    ClusterViewShellTab::Textures,
                    1,
                    true,
                    true,
                    true,
                    Some(GameFolder {
                        path: std::path::PathBuf::new(),
                        shared: true,
                    }),
                    WorkspaceLayout::for_size(800., 600.),
                )
            },
            (800., 390.).into(),
            |_| (),
            1.,
        );
        runner.poll_n(Duration::ZERO, 5);

        let bottom = |name| {
            runner
                .find(|node, element| {
                    let mut access = element.accessibility().builder.clone();
                    element.finish_accessibility(&mut access);
                    (access.label() == Some(name)).then(|| node.layout().area.max_y())
                })
                .unwrap()
        };
        let settings = bottom(ClusterViewShellTab::Settings.label());
        assert!(settings > bottom(ClusterViewShellTab::GameLog.label()));
        assert!(settings > bottom(t("oneclient.layout.cluster.game_folder")));
        assert!(
            settings <= 390.,
            "settings ended below the rail: {settings}"
        );
    }
}
