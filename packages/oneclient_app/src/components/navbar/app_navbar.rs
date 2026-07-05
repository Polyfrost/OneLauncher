use freya::{
    prelude::*,
    router::{RouterContext, use_route},
};

use crate::{
    Route,
    components::{Avatar, Icon, IconType},
    hooks::{try_default_account, use_current_account, use_dispatch, use_notifications_snapshot},
    theme,
};

#[derive(PartialEq)]
pub struct Navbar;

impl Component for Navbar {
    fn render(&self) -> impl IntoElement {
        rect()
            .width(Size::fill())
            .height(Size::px(theme::NAVBAR_HEIGHT_PX))
            .horizontal()
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .padding(Gaps::new_symmetric(0.0, 40.0))
            .child(navbar_left())
            .child(navbar_center())
            .child(NavbarRight)
            .position(Position::new_absolute().top(0.).left(0.))
            .layer(Layer::RelativeOverlay(2))
        // TODO: Window Drag?
        // .child(
        //     rect()
        //         .window_drag()
        //         .width(Size::flex(1.))
        //         .height(Size::fill())
        //         .position(Position::new_absolute())
        //         .layer(Layer::RelativeOverlay(0))
        // )
    }
}

fn navbar_left() -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::flex(1.0))
        .cross_align(Alignment::Center)
        .child(navbar_logo())
}

fn navbar_logo() -> impl IntoElement {
    let bytes = use_memo(|| crate::AppAssets::get_bytes("logo.svg").unwrap_or_default());

    svg(bytes.read().cloned())
        .height(Size::px(44.))
        .width(Size::px(214.))
        .color(theme::colors::fg_primary())
}

fn navbar_center() -> impl IntoElement {
    let route = use_route::<Route>();

    rect()
        .horizontal()
        .width(Size::flex(1.0))
        .main_align(Alignment::Center)
        .cross_align(Alignment::Center)
        .spacing(64.)
        .child(NavLink {
            active: route == Route::Home {},
            target: Route::Home {},
            nav_label: "Home",
        })
        .child(NavLink {
            active: route == Route::Clusters {},
            target: Route::Clusters {},
            nav_label: "Versions",
        })
        .child(NavLink {
            active: route == Route::Stats {},
            target: Route::Stats {},
            nav_label: "Stats",
        })
}

#[derive(PartialEq)]
struct NavLink {
    active: bool,
    target: Route,
    nav_label: &'static str,
}

impl Component for NavLink {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);

        let active = self.active;
        let target = self.target.clone();
        let nav_label = self.nav_label;

        use_drop(move || {
            Cursor::set(CursorIcon::default());
        });

        let color = if active || *hovering.peek() {
            theme::colors::fg_primary()
        } else {
            theme::colors::fg_secondary()
        };

        let underline_width = if active {
            27.
        } else if *hovering.peek() {
            18.
        } else {
            0.
        };

        rect()
            .vertical()
            .cross_align(Alignment::Center)
            .spacing(2.)
            .width(Size::px(nav_label.len() as f32 * 10. + 10.))
            .on_press(move |e: Event<PressEventData>| {
                e.prevent_default();
                let _ = RouterContext::get().push(target.clone());
            })
            .on_pointer_over(move |_| hovering.set(true))
            .on_pointer_out(move |_| hovering.set(false))
            .on_pointer_enter(move |_| Cursor::set(CursorIcon::Pointer))
            .on_pointer_leave(move |_| Cursor::set(CursorIcon::default()))
            .child(
                label()
                    .text(nav_label)
                    .font_size(16.)
                    .font_weight(if active {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::NORMAL
                    })
                    .color(color),
            )
            .child(
                rect()
                    .height(Size::px(2.))
                    .width(Size::px(underline_width))
                    .corner_radius(CornerRadius::new_all(2.))
                    .background(if active {
                        theme::colors::fg_primary()
                    } else {
                        theme::colors::fg_secondary()
                    }),
            )
    }
}

#[derive(PartialEq)]
struct NavbarRight;

impl Component for NavbarRight {
    fn render(&self) -> impl IntoElement {
        let current_account = use_current_account();
        let dispatch: crate::BridgeDispatch = use_dispatch();
        let unread = use_notifications_snapshot().unread_count();

        let account_uuid = try_default_account(&current_account)
            .map(|account| account.id.to_string())
            .unwrap_or_else(|| uuid::Uuid::nil().to_string());

        // open notification center
        let open_notifications = move |_| {
            dispatch.toggle_notification_center();
        };

        // open settings
        let open_settings = |_| {
            let _ = RouterContext::get().push(Route::SettingsLauncher {});
        };

        rect()
            .horizontal()
            .width(Size::flex(1.0))
            .main_align(Alignment::End)
            .cross_align(Alignment::Center)
            .spacing(8.)
            .child(
                super::navbar_button()
                    .child(notification_bell(unread))
                    .on_press(open_notifications),
            )
            .child(
                super::navbar_button()
                    .child(Icon::new(IconType::Settings02).size(20.))
                    .on_press(open_settings),
            )
            .child(
                super::navbar_button().padding(0.0).child(
                    Avatar::new(account_uuid)
                        .width(Size::px(24.))
                        .height(Size::px(24.)),
                ),
            )
            .child(super::window_controls())
    }
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
                .corner_radius(CornerRadius::from(8.))
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
                        .font_size(10.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(theme::colors::fg_primary()),
                )
                .into_element()
        }))
}
