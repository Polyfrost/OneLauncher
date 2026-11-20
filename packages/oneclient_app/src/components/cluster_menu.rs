use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::clusters::Cluster;

use crate::components::{ContextMenu, Icon, IconType};
use crate::hooks::{modpack_job_running, use_game_snapshot};
use crate::i18n::t;
use crate::routes::Route;
use crate::theme::{colors, rounded};
use crate::ui::border_all_color;
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
    if !cluster.is_isolated() || cluster.user_created {
        crate::utils::instance_display_name(cluster)
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

pub(crate) fn instance_actions(mut menu: ContextMenu, id: i64, mod_tabs: bool) -> ContextMenu {
    let mut content_group = false;
    for (icon, label, route) in cluster_menu_entries(id)
        .into_iter()
        .filter(|(_, _, r)| shows_entry(r, mod_tabs))
    {
        if !content_group
            && matches!(
                route,
                Route::ClusterMods { .. } | Route::ClusterTextures { .. }
            )
        {
            menu = menu.separator();
            content_group = true;
        }
        if matches!(route, Route::ClusterSettings { .. }) {
            menu = menu.separator();
        }
        menu = menu.action(icon, label, move |()| {
            let _ = RouterContext::get().push(route.clone());
        });
    }
    menu
}

pub(crate) fn open_menu_at<T: From<(f32, f32)> + 'static>(
    mut position: State<Option<T>>,
) -> impl FnMut(Event<PressEventData>) {
    move |e: Event<PressEventData>| {
        if let PressEventData::Mouse(m) = e.data() {
            position.set(Some(
                (m.global_location.x as f32, m.global_location.y as f32).into(),
            ));
        }
    }
}

/// The menu key and Shift+F10 anchor to the focused card or row.
pub(crate) fn use_instance_menu_trigger(
    mut menu: State<Option<(f32, f32)>>,
) -> impl FnOnce(Rect) -> Rect {
    let mut anchor = use_state(|| (0., 0.));
    move |el| {
        el.on_secondary_down(open_menu_at(menu))
            .on_sized(move |e: Event<SizedEventData>| {
                anchor.set_if_modified((e.area.min_x() + 12., e.area.min_y() + 12.));
            })
            .on_key_down(move |e: Event<KeyboardEventData>| {
                if e.key == Key::Named(NamedKey::ContextMenu)
                    || (e.key == Key::Named(NamedKey::F10)
                        && e.modifiers.contains(Modifiers::SHIFT))
                {
                    e.prevent_default();
                    e.stop_propagation();
                    menu.set(Some(*anchor.peek()));
                }
            })
    }
}

pub(super) const OVERVIEW_BUTTON_PX: f32 = 32.;

#[derive(PartialEq)]
pub(crate) enum InstanceOverviewSurface {
    Artwork { reveal: bool },
    List,
}

#[derive(PartialEq)]
pub(crate) struct InstanceOverviewButton {
    pub cluster_id: i64,
    pub surface: InstanceOverviewSurface,
    pub name: String,
}

impl Component for InstanceOverviewButton {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let id = use_a11y();
        let keyboard = use_focus(id)() == Focus::Keyboard;
        let hot = hovered() || keyboard;
        let theme = colors::active_theme();
        let cluster_id = self.cluster_id;

        let base = rect()
            .width(Size::px(OVERVIEW_BUTTON_PX))
            .height(Size::px(OVERVIEW_BUTTON_PX))
            .min_width(Size::px(OVERVIEW_BUTTON_PX))
            .center();

        let button = match self.surface {
            InstanceOverviewSurface::Artwork { reveal } if theme.liquid_glass => {
                if reveal || hot {
                    let tint = if theme.colors.light {
                        Color::WHITE.with_a(if hot { 174 } else { 140 })
                    } else {
                        colors::page().with_a(if hot { 152 } else { 112 })
                    };
                    crate::theme::glass::panel(base, &theme, OVERVIEW_BUTTON_PX / 2., tint)
                } else {
                    base.opacity(0.)
                }
            }
            InstanceOverviewSurface::List if theme.liquid_glass && hot => {
                let tint = if theme.colors.light {
                    Color::WHITE.with_a(128)
                } else {
                    colors::component_bg_hover().with_a(112)
                };
                crate::theme::glass::panel(base, &theme, OVERVIEW_BUTTON_PX / 2., tint)
            }
            InstanceOverviewSurface::List => base.corner_radius(rounded(8.)).background(if hot {
                colors::ghost_overlay_hover()
            } else {
                Color::TRANSPARENT
            }),
            InstanceOverviewSurface::Artwork { reveal } => {
                // Keep the artwork disclosure visible while hover transfers to the child.
                let bg = if theme.colors.light {
                    Color::from_rgb(248, 249, 251)
                } else {
                    Color::from_rgb(31, 33, 40)
                };
                let radius = rounded(if theme.liquid_glass { 16. } else { 7. });
                base.corner_radius(radius)
                    .background(bg.with_a(if hot { 255 } else { 236 }))
                    .border(border_all_color(1., colors::component_border_hover()))
                    .opacity(if reveal || hot { 1. } else { 0. })
            }
        };

        let tooltip = crate::components::use_tooltip_anchor(
            Some(t("oneclient.components.cluster_menu.overview").into()),
            crate::components::TooltipPlacement::Top,
        );
        tooltip.attach(
            button
                .a11y_id(id)
                .a11y_role(AccessibilityRole::Button)
                .a11y_focusable(true)
                .a11y_alt(crate::i18n::tf(
                    "oneclient.components.cluster_menu.open_overview",
                    &[&self.name],
                ))
                .cursor(CursorIcon::Pointer)
                .maybe(keyboard, |button| {
                    button.border(
                        border_all_color(2., colors::brand()).alignment(BorderAlignment::Inner),
                    )
                })
                .on_pointer_over(move |_| hovered.set_if_modified(true))
                .on_pointer_move(move |_| hovered.set_if_modified(true))
                // The tooltip owns pointer-leave on this rect; use pointer-out for the face.
                .on_pointer_out(move |_| hovered.set_if_modified(false))
                .on_press(move |e: Event<PressEventData>| {
                    e.stop_propagation();
                    let _ = RouterContext::get().push(Route::ClusterOverview { cluster_id });
                })
                .child(
                    Icon::new(IconType::DotsVertical)
                        .size(if theme.pixel_icons { 26. } else { 18. })
                        .color(colors::fg_primary()),
                ),
        )
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
            let context = ContextMenu::new(x, y)
                .title(target.title.clone())
                .on_close(move |_| position.set(None));
            let mut context = instance_actions(context, cluster_id, target.mod_tabs);
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

#[cfg(test)]
mod tests {
    use super::*;
    use freya_testing::prelude::*;

    #[tokio::test]
    async fn overview_shortcut_clears_hover_with_tooltips_enabled() {
        let mut test = launch_test(|| {
            crate::components::use_provide_tooltips();
            rect()
                .expanded()
                .horizontal()
                .background(Color::BLUE)
                .children((1..=2).map(|cluster_id| InstanceOverviewButton {
                    cluster_id,
                    surface: InstanceOverviewSurface::List,
                    name: cluster_id.to_string(),
                }))
        });
        let pixels = |test: &mut TestingRunner| {
            let image = skia_safe::Image::from_encoded(test.render()).unwrap();
            let raster = image.make_raster_image(None, None).unwrap();
            let pixels = raster.peek_pixels().unwrap();
            [pixels.get_color((8, 14)), pixels.get_color((40, 14))]
        };

        test.move_cursor((8., 14.));
        let [first, second] = pixels(&mut test);
        assert_ne!(first, skia_safe::Color::BLUE);
        assert_eq!(second, skia_safe::Color::BLUE);

        test.move_cursor((40., 14.));
        let [first, second] = pixels(&mut test);
        assert_eq!(first, skia_safe::Color::BLUE);
        assert_ne!(second, skia_safe::Color::BLUE);

        test.move_cursor((70., 70.));
        assert_eq!(pixels(&mut test), [skia_safe::Color::BLUE; 2]);
    }

    #[tokio::test]
    async fn overview_shortcut_reveals_when_the_pointer_reaches_it() {
        let mut test = launch_test(|| {
            crate::components::use_provide_tooltips();
            rect()
                .expanded()
                .background(Color::BLUE)
                .child(InstanceOverviewButton {
                    cluster_id: 1,
                    surface: InstanceOverviewSurface::Artwork { reveal: false },
                    name: "Version".into(),
                })
        });
        let button_pixel = |test: &mut TestingRunner| {
            let image = skia_safe::Image::from_encoded(test.render()).unwrap();
            let raster = image.make_raster_image(None, None).unwrap();
            raster.peek_pixels().unwrap().get_color((8, 14))
        };

        test.move_cursor((70., 70.));
        assert_eq!(button_pixel(&mut test), skia_safe::Color::BLUE);
        for point in [(2., 15.), (16., 16.), (27., 10.)] {
            test.move_cursor(point);
            assert_ne!(button_pixel(&mut test), skia_safe::Color::BLUE);
        }
        test.move_cursor((70., 70.));
        assert_eq!(button_pixel(&mut test), skia_safe::Color::BLUE);
    }
}
