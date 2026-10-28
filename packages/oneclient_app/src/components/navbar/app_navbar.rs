use freya::{
    animation::*,
    prelude::*,
    router::{RouterContext, use_route},
};

use crate::theme::{rounded, text_size};
use crate::{
    Route,
    components::{Avatar, Icon, IconType},
    hooks::{
        settled_or_loading, try_default_account, use_active_cluster_id, use_browser_type,
        use_clusters, use_current_account, use_dispatch, use_notifications_snapshot,
    },
    theme,
    utils::default_cluster,
};

const NAVBAR_INTRO_MS: u64 = 460;
const LOGO_HIDE_NAVBAR_W: f32 = 1100.;
const NAVBAR_SIDE_PADDING_PX: f32 = 40.;
const NAV_LINK_SPACING_PX: f32 = 36.;
const COMPACT_LOGO_PX: f32 = 32.;

#[derive(PartialEq)]
pub struct Navbar;

impl Component for Navbar {
    fn render(&self) -> impl IntoElement {
        let mut navbar_width = use_state(|| 0f32);
        let intro = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            AnimNum::new(0., 1.)
                .time(NAVBAR_INTRO_MS)
                .ease(Ease::Out)
                .function(Function::Cubic)
        });
        let eased = intro.get().value();
        let slide = (1.0 - eased) * -theme::NAVBAR_HEIGHT_PX;

        let over_art = matches!(use_route::<Route>(), Route::Home {});
        let measured = *navbar_width.read();
        let show_logo = measured <= 0. || measured > LOGO_HIDE_NAVBAR_W;

        rect()
            .width(Size::fill())
            .height(Size::px(theme::NAVBAR_HEIGHT_PX))
            .position(Position::new_absolute().top(0.).left(0.))
            .layer(Layer::OverlayLevel(2))
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .horizontal()
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .padding(Gaps::new_symmetric(0.0, NAVBAR_SIDE_PADDING_PX))
                    .offset_y(slide)
                    .opacity(eased)
                    .on_sized(move |event: Event<SizedEventData>| {
                        let next = event.data().area.width();
                        if (*navbar_width.peek() - next).abs() > 0.5 {
                            navbar_width.set(next);
                        }
                    })
                    .child(navbar_left(show_logo, over_art))
                    .child(navbar_center(!show_logo))
                    .child(NavbarRight { over_art }),
            )
            .child(
                rect()
                    .window_drag()
                    .width(Size::window_percent(100.))
                    .height(Size::px(theme::NAVBAR_HEIGHT_PX))
                    .position(Position::new_absolute().top(0.).left(0.).right(0.)),
            )
    }
}

fn navbar_left(show_logo: bool, over_art: bool) -> impl IntoElement {
    rect()
        .horizontal()
        .width(if show_logo {
            Size::flex(1.0)
        } else {
            Size::auto()
        })
        .cross_align(Alignment::Center)
        .spacing(NAV_LINK_SPACING_PX / 2.)
        .maybe(!show_logo, |el| {
            el.padding(Gaps::new(0., NAV_LINK_SPACING_PX, 0., 0.))
        })
        .child(
            Icon::new(IconType::IconLogo)
                .size(COMPACT_LOGO_PX)
                .color(primary_color(over_art))
                .into_element(),
        )
        .maybe(show_logo, |rect| {
            rect.child(NavbarLogo { over_art }.into_element())
        })
}

fn primary_color(over_art: bool) -> Color {
    if over_art {
        theme::colors::on_art()
    } else {
        theme::colors::fg_primary()
    }
}

fn secondary_color(over_art: bool) -> Color {
    if over_art {
        theme::colors::on_art_secondary()
    } else if theme::colors::is_light() {
        theme::colors::fg_secondary_pressed()
    } else {
        theme::colors::fg_secondary()
    }
}

#[derive(PartialEq)]
struct NavbarLogo {
    over_art: bool,
}

impl Component for NavbarLogo {
    fn render(&self) -> impl IntoElement {
        theme::wordmark(214., 44., primary_color(self.over_art))
    }
}

fn navbar_center(is_small: bool) -> impl IntoElement {
    let route = use_route::<Route>();
    let over_art = matches!(route, Route::Home {});
    let browse_target = browse_target();

    rect()
        .horizontal()
        .width(Size::flex(1.0))
        .main_align(if is_small {
            Alignment::Start
        } else {
            Alignment::Center
        })
        .cross_align(Alignment::Center)
        .spacing(if is_small { 12. } else { 4. })
        .child(NavLink {
            active: route == Route::Home {},
            target: NavTarget::Route(Route::Home {}),
            nav_label: "Home",
            over_art,
        })
        .child(NavLink {
            active: route == Route::Clusters {},
            target: NavTarget::Route(Route::Clusters {}),
            nav_label: "Versions",
            over_art,
        })
        .child(NavLink {
            active: matches!(
                route,
                Route::Browser {
                    pick_cluster: true,
                    ..
                }
            ),
            target: NavTarget::Route(browse_target),
            nav_label: "Browse",
            over_art,
        })
        .child(NavLink {
            active: route == Route::Wardrobe {},
            target: NavTarget::Route(Route::Wardrobe {}),
            nav_label: "Wardrobe",
            over_art,
        })
}

/// Falls back active cluster then most recently played then Versions when none exist
fn browse_target() -> Route {
    let clusters = settled_or_loading(&use_clusters()).unwrap_or_default();
    let active = *use_active_cluster_id().read();
    let package_type = use_browser_type().read().clone();

    match default_cluster(clusters, active) {
        Some(cluster) => Route::Browser {
            cluster_id: cluster.id,
            package_type: crate::view::app::browser::browsable_type(&package_type, &cluster),
            pick_cluster: true,
        },
        None => Route::Clusters {},
    }
}

#[derive(PartialEq, Clone)]
enum NavTarget {
    Route(Route),
}

#[derive(PartialEq)]
struct NavLink {
    active: bool,
    target: NavTarget,
    nav_label: &'static str,
    over_art: bool,
}

impl Component for NavLink {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);
        let a11y_id = use_a11y();
        let focused = use_focus(a11y_id);

        let active = self.active;
        let target = self.target.clone();
        let nav_label = self.nav_label;
        let over_art = self.over_art;

        let color = if active || hovering() || focused().is_focused() {
            primary_color(over_art)
        } else {
            secondary_color(over_art)
        };

        let background = if active {
            theme::colors::ghost_overlay()
        } else if hovering() || focused().is_focused() {
            theme::colors::ghost_overlay_hover()
        } else {
            Color::TRANSPARENT
        };

        rect()
            .horizontal()
            .main_align(Alignment::Center)
            .cross_align(Alignment::Center)
            .height(Size::px(36.))
            .width(Size::px(nav_label.len() as f32 * 10. + 34.))
            .corner_radius(rounded(10.))
            .background(background)
            .maybe(active, |el| {
                el.border(
                    Border::new()
                        .fill(theme::colors::component_border())
                        .width(1.)
                        .alignment(BorderAlignment::Inner),
                )
            })
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .on_press(move |e: Event<PressEventData>| {
                e.prevent_default();
                match &target {
                    NavTarget::Route(_) if active => {}
                    NavTarget::Route(route) => {
                        let _ = RouterContext::get().push(route.clone());
                    }
                }
            })
            .on_pointer_over(move |_| hovering.set(true))
            .on_pointer_out(move |_| hovering.set(false))
            .cursor(CursorIcon::Pointer)
            .child(
                label()
                    .text(nav_label)
                    .font_size(text_size(16.))
                    .font_weight(if active {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::NORMAL
                    })
                    .color(color),
            )
    }
}

#[derive(PartialEq)]
struct NavbarRight {
    over_art: bool,
}

impl Component for NavbarRight {
    fn render(&self) -> impl IntoElement {
        let foreground = self.over_art.then(theme::colors::on_art);
        let current_account = use_current_account();
        let dispatch = use_dispatch();
        let unread = use_notifications_snapshot().unread_count();

        let account_uuid = try_default_account(&current_account)
            .map(|account| account.id.to_string())
            .unwrap_or_else(|| uuid::Uuid::nil().to_string());

        let notif_dispatch = dispatch.clone();
        let open_notifications = move |_| {
            notif_dispatch.toggle_notification_center();
        };

        let open_control_center = move |_| {
            dispatch.toggle_control_center();
        };

        let open_stats = |_| {
            let _ = RouterContext::get().push(Route::Stats {});
        };

        rect()
            .horizontal()
            .width(Size::flex(1.0))
            .main_align(Alignment::End)
            .cross_align(Alignment::Center)
            .spacing(8.)
            .child(
                super::navbar_button(foreground)
                    .tooltip("Stats")
                    .child(Icon::new(IconType::LineChartUp01).size(20.))
                    .on_press(open_stats),
            )
            .child(
                super::navbar_button(foreground)
                    .tooltip("Notifications")
                    .child(notification_bell(unread))
                    .on_press(open_notifications),
            )
            .child(
                super::navbar_button(foreground)
                    .overflow(Overflow::None)
                    .tooltip("Control Center")
                    .padding(0.0)
                    .on_press(open_control_center)
                    .child(avatar_with_gear(account_uuid)),
            )
            .child(super::window_controls_with(foreground))
    }
}

fn avatar_with_gear(uuid: String) -> impl IntoElement {
    rect()
        .width(Size::px(28.))
        .height(Size::px(28.))
        .center()
        .child(Avatar::new(uuid).width(Size::px(28.)).height(Size::px(28.)))
        .child(
            rect()
                .position(Position::new_absolute().bottom(-7.).right(-7.))
                .width(Size::px(16.))
                .height(Size::px(16.))
                .corner_radius(rounded(7.))
                .background(theme::colors::page_elevated())
                .border(
                    Border::new()
                        .fill(theme::colors::component_border())
                        .width(1.)
                        .alignment(BorderAlignment::Inner),
                )
                .layer(Layer::Relative(3))
                .center()
                .child(
                    Icon::new(IconType::Settings02)
                        .size(12.)
                        .color(theme::colors::fg_secondary()),
                ),
        )
}

fn notification_bell(unread: usize) -> impl IntoElement {
    rect()
        .width(Size::px(20.))
        .height(Size::px(20.))
        .child(Icon::new(IconType::Bell01).size(20.))
        .maybe_child((unread > 0).then(|| {
            rect()
                .position(Position::new_absolute().top(-4.).right(-4.))
                .width(Size::px(16.))
                .height(Size::px(16.))
                .corner_radius(rounded(8.))
                .background(theme::colors::danger())
                .layer(Layer::Relative(3))
                .center()
                .child(
                    label()
                        .text(if unread > 9 {
                            "9+".to_string()
                        } else {
                            unread.to_string()
                        })
                        .font_size(text_size(10.))
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(theme::colors::fg_primary()),
                )
                .into_element()
        }))
}
