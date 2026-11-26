use freya::{
    animation::*,
    prelude::*,
    router::{RouterContext, use_route},
};

use crate::theme::{rounded, text_size};
use crate::{
    Route,
    components::{Avatar, HomeStack, Icon, IconType, use_home_stack},
    hooks::{
        settled_or_loading, try_default_account, use_browser_type, use_chat_allowed,
        use_chat_unread, use_current_account, use_dispatch, use_notifications_snapshot,
    },
    i18n::t,
    theme,
};

const NAVBAR_INTRO_MS: u64 = 460;
const LOGO_HIDE_NAVBAR_W: f32 = 1100.;
const NAVBAR_SIDE_PADDING_PX: f32 = 40.;
const NAV_LINK_SPACING_PX: f32 = 36.;
const NAV_PILL_GAP: f32 = 4.;
const NAV_LINK_HEIGHT: f32 = 36.;
const GLASS_GROUP_HEIGHT: f32 = 48.;
const GLASS_GROUP_GAP: f32 = 12.;
const COMPACT_LOGO_PX: f32 = 32.;
const GLASS_COMPACT_LOGO_PX: f32 = 40.;
/// Side padding, the compact logo and the right-hand buttons: what the links can't use
const COMPACT_RESERVED_PX: f32 = 396.;

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

        let theme = theme::colors::active_theme();
        let route = crate::layout::use_app_page();
        let stack = use_home_stack();
        let home_visible = home_page_in_view(&route, stack);
        let over_art = !theme.liquid_glass && !theme.colors.light && home_visible;
        let measured = *navbar_width.read();
        let show_logo = measured <= 0. || measured > LOGO_HIDE_NAVBAR_W;

        rect()
            .width(Size::fill())
            .height(Size::px(theme::NAVBAR_HEIGHT_PX))
            .position(Position::new_absolute().top(0.).left(0.))
            .layer(Layer::OverlayLevel(2))
            .maybe(theme.colors.light && !theme.liquid_glass, |bar| {
                bar.background(theme::colors::component_bg())
            })
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .horizontal()
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .padding(Gaps::new_symmetric(0.0, NAVBAR_SIDE_PADDING_PX))
                    .maybe(theme.liquid_glass, |row| {
                        row.padding(Gaps::new_symmetric(0., 16.))
                            .spacing(GLASS_GROUP_GAP)
                    })
                    .offset_y(slide)
                    .opacity(eased)
                    .on_sized(move |event: Event<SizedEventData>| {
                        let next = event.data().area.width();
                        if (*navbar_width.peek() - next).abs() > 0.5 {
                            navbar_width.set(next);
                        }
                    })
                    .child(navbar_left(
                        show_logo,
                        over_art || (theme.liquid_glass && home_visible),
                        theme.liquid_glass,
                    ))
                    .child(navbar_group(
                        &theme,
                        navbar_center(!show_logo, over_art, measured - COMPACT_RESERVED_PX),
                        !show_logo,
                        Alignment::Center,
                        6.,
                    ))
                    .child(navbar_group(
                        &theme,
                        NavbarRight {
                            over_art,
                            compact: !show_logo,
                        },
                        show_logo,
                        Alignment::End,
                        8.,
                    )),
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

/// Home's art is only behind the navbar while the home page (not the versions page
/// it scrolls into) is in view
fn home_page_in_view(route: &Route, stack: Option<HomeStack>) -> bool {
    matches!(route, Route::Home {}) && !stack.is_some_and(|stack| *stack.on_versions.read())
}

fn navbar_group(
    theme: &theme::Theme,
    content: impl IntoElement,
    flexible: bool,
    align: Alignment,
    inset: f32,
) -> Element {
    if !theme.liquid_glass {
        return content.into_element();
    }
    let capsule = theme::glass::navigation_panel(
        rect()
            .horizontal()
            .height(Size::px(GLASS_GROUP_HEIGHT))
            .padding(Gaps::new_symmetric(0., inset))
            .cross_align(Alignment::Center),
        theme,
        GLASS_GROUP_HEIGHT / 2.,
    );
    rect()
        .horizontal()
        .width(if flexible {
            Size::flex(1.)
        } else {
            Size::auto()
        })
        .main_align(align)
        .cross_align(Alignment::Center)
        .child(capsule.child(content))
        .into_element()
}

fn navbar_left(show_logo: bool, over_art: bool, glass: bool) -> impl IntoElement {
    let mut brand = crate::layout::ArtworkBrand::get();
    let stack = use_home_stack();
    let at_home = matches!(use_route::<Route>(), Route::Home {});
    let id = use_a11y();
    let keyboard = use_focus(id)() == Focus::Keyboard;
    let on_art = over_art && glass;
    let symbol_size = match (glass, show_logo) {
        (true, true) => 28.,
        (true, false) => GLASS_COMPACT_LOGO_PX,
        (false, _) => COMPACT_LOGO_PX,
    };
    let content = rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(if glass { 10. } else { NAV_LINK_SPACING_PX / 2. })
        .a11y_id(id)
        .a11y_alt("OneClient")
        .a11y_role(AccessibilityRole::Button)
        .a11y_focusable(true)
        .cursor(CursorIcon::Pointer)
        .maybe(keyboard, |el| {
            el.corner_radius(rounded(6.)).border(
                crate::ui::border_all_color(2., primary_color(over_art))
                    .alignment(BorderAlignment::Outer),
            )
        })
        .on_press(move |event: Event<PressEventData>| {
            event.prevent_default();
            event.stop_propagation();
            NavTarget::Home.activate(false, stack, at_home);
        })
        .maybe(on_art, |el| {
            el.on_sized(move |event: Event<SizedEventData>| {
                let a = event.data().area;
                brand
                    .bounds
                    .set_if_modified(Some(skia_safe::Rect::from_xywh(
                        a.min_x(),
                        a.min_y(),
                        a.width(),
                        a.height(),
                    )));
            })
        })
        .child(if on_art {
            BrandOnArtwork {
                wordmark: false,
                compact: !show_logo,
            }
            .into_element()
        } else {
            Icon::new(IconType::IconLogo)
                .size(symbol_size)
                .color(primary_color(over_art))
                .into_element()
        })
        .maybe(show_logo, |rect| {
            rect.child(if on_art {
                BrandOnArtwork {
                    wordmark: true,
                    compact: false,
                }
                .into_element()
            } else if glass {
                theme::wordmark(168., 35., primary_color(false))
            } else {
                NavbarLogo { over_art }.into_element()
            })
        });
    rect()
        .horizontal()
        .width(if show_logo {
            Size::flex(1.)
        } else {
            Size::auto()
        })
        .cross_align(Alignment::Center)
        .maybe(glass, |el| el.padding(Gaps::new_symmetric(0., 8.)))
        .maybe(!glass && !show_logo, |el| {
            el.padding(Gaps::new(0., 12., 0., 0.))
        })
        .child(content)
}

#[derive(PartialEq)]
struct BrandOnArtwork {
    wordmark: bool,
    compact: bool,
}

impl Component for BrandOnArtwork {
    fn render(&self) -> impl IntoElement {
        let (asset, w, h, scale) = if self.wordmark {
            ("logo.svg", 168., 35., 168. / 156.)
        } else {
            let size = if self.compact {
                GLASS_COMPACT_LOGO_PX
            } else {
                28.
            };
            ("icons/icon-logo.svg", size, size, size / 80.)
        };
        let bytes: Bytes = use_hook(move || {
            let original = crate::AppAssets::get_bytes(asset).unwrap_or_default();
            let svg = String::from_utf8_lossy(&original).replace(r#" fill-opacity="0.85""#, "");
            // A close, opposing edge protects individual strokes where patterned artwork
            // crosses the mark. It is cached with the glyph; no live backdrop blur is needed.
            format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="-5 -5 {} {}">
                <defs><filter id="shade" x="-50%" y="-50%" width="200%" height="200%">
                <feGaussianBlur in="SourceAlpha" stdDeviation=".8"/>
                <feOffset dx="0" dy=".8" result="blur"/>
                <feFlood flood-color="black" flood-opacity=".50"/>
                <feComposite in2="blur" operator="in"/>
                <feMerge><feMergeNode/><feMergeNode in="SourceGraphic"/></feMerge></filter></defs>
                <g filter="url(#shade)"><g transform="scale({scale})">{svg}</g></g></svg>"##,
                w + 10.,
                h + 10.
            )
            .into_bytes()
            .into()
        });
        rect().width(Size::px(w)).height(Size::px(h)).child(
            rect()
                .position(Position::new_absolute().left(-5.).top(-5.))
                .interactive(false)
                .child(
                    SvgViewer::new((("navbar-brand-contrast", asset, self.compact), bytes))
                        .show_loader(false)
                        .color(Color::WHITE)
                        .width(Size::px(w + 10.))
                        .height(Size::px(h + 10.)),
                ),
        )
    }

    fn render_key(&self) -> DiffKey {
        DiffKey::from(&(self.wordmark, self.compact))
    }
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
        if theme::colors::active_theme().liquid_glass {
            // The light glass bar floats over live art, where a muted grey falls to about
            // 3:1. Idle links keep the primary ink, set apart by weight and the lens.
            theme::colors::fg_primary().with_a(224)
        } else {
            theme::colors::fg_secondary_pressed()
        }
    } else if theme::colors::active_theme().liquid_glass {
        theme::colors::fg_primary().with_a(225)
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

fn navbar_center(is_small: bool, over_art: bool, room: f32) -> impl IntoElement {
    let route = crate::layout::use_app_page();
    let stack = use_home_stack();
    let at_home = route == Route::Home {};
    let browse_target = browse_target();
    let theme = theme::colors::active_theme();
    let track = theme::glass::use_pill_track(4);
    let links = [
        (
            home_page_in_view(&route, stack),
            NavTarget::Home,
            t("oneclient.components.navbar.home"),
        ),
        (
            (at_home && !home_page_in_view(&route, stack)) || route == Route::Clusters {},
            NavTarget::Versions,
            t("oneclient.components.navbar.versions"),
        ),
        (
            matches!(
                route,
                Route::Browser {
                    pick_cluster: true,
                    ..
                }
            ),
            NavTarget::Route(browse_target),
            t("oneclient.components.navbar.browse"),
        ),
        (
            route == Route::Wardrobe {},
            NavTarget::Route(Route::Wardrobe {}),
            t("oneclient.components.navbar.wardrobe"),
        ),
    ];
    let active = links.iter().position(|(active, _, _)| *active);
    let indicator = theme.liquid_glass.then(|| track.indicator(active));
    let routes = links.each_ref().map(|(_, route, _)| route.clone());
    // Compact links keep their own width, so the lens stays label-sized. Only a window too
    // narrow for that shares the room out evenly and lets the labels truncate.
    let natural = links
        .iter()
        .map(|(_, _, label)| nav_link_width(label, is_small))
        .sum::<f32>()
        + NAV_PILL_GAP * (links.len() - 1) as f32;
    let squeeze = is_small && natural > room;

    rect()
        .horizontal()
        .height(Size::px(NAV_LINK_HEIGHT))
        .width(if theme.liquid_glass {
            Size::px(if squeeze { room.max(0.) } else { natural })
        } else {
            Size::flex(1.)
        })
        .maybe(squeeze, |el| el.content(Content::Flex))
        .main_align(if is_small {
            Alignment::Start
        } else {
            Alignment::Center
        })
        .cross_align(Alignment::Center)
        .spacing(NAV_PILL_GAP)
        .maybe(theme.liquid_glass, |el| {
            track.events(el, move |index| {
                routes[index].activate(Some(index) == active, stack, at_home);
            })
        })
        .maybe_child(indicator)
        .children(
            links
                .into_iter()
                .enumerate()
                .map(|(index, (active, target, nav_label))| {
                    NavLink {
                        active,
                        target,
                        nav_label,
                        over_art,
                        compact: is_small,
                        squeeze,
                        pill: theme.liquid_glass.then_some((track, index)),
                    }
                    .into_element()
                }),
        )
}

fn nav_link_width(label: &str, compact: bool) -> f32 {
    let letters = label.chars().count() as f32;
    if compact {
        letters * 9. + 30.
    } else {
        letters * 10. + 34.
    }
}

fn browse_target() -> Route {
    Route::Browser {
        cluster_id: 0,
        package_type: use_browser_type().read().clone(),
        pick_cluster: true,
        in_instance: false,
    }
}

#[derive(PartialEq, Clone)]
enum NavTarget {
    Route(Route),
    /// Home's page, scrolled back up to the top if it was on the versions page
    Home,
    Versions,
}

impl NavTarget {
    fn activate(&self, active: bool, stack: Option<HomeStack>, at_home: bool) {
        match self {
            Self::Versions if active && !at_home => {}
            Self::Versions => match stack {
                Some(stack) => {
                    stack.set_open(true);
                    if !at_home {
                        let _ = RouterContext::get().push(Route::Home {});
                    }
                }
                None => {
                    let _ = RouterContext::get().push(Route::Clusters {});
                }
            },
            Self::Route(_) if active => {}
            Self::Route(route) => {
                let _ = RouterContext::get().push(route.clone());
            }
            Self::Home => {
                if let Some(stack) = stack {
                    stack.set_open(false);
                }
                if !at_home {
                    let _ = RouterContext::get().push(Route::Home {});
                }
            }
        }
    }
}

#[derive(PartialEq)]
struct NavLink {
    active: bool,
    target: NavTarget,
    nav_label: &'static str,
    over_art: bool,
    compact: bool,
    /// Share the row evenly and truncate, for windows too narrow for label-sized links
    squeeze: bool,
    pill: Option<(theme::glass::PillTrack, usize)>,
}

impl Component for NavLink {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);
        let a11y_id = use_a11y();
        let focused = use_focus(a11y_id);

        let active = self.active;
        let target = self.target.clone();
        let stack = use_home_stack();
        let at_home = matches!(use_route::<Route>(), Route::Home {});
        let nav_label = self.nav_label;
        let over_art = self.over_art;
        // The label under a dragged lens takes the selected look as the lens arrives
        let lit = self
            .pill
            .map_or(active, |(track, index)| track.highlights(index, active));

        let color = if lit || hovering() || focused().is_focused() {
            primary_color(over_art)
        } else {
            secondary_color(over_art)
        };

        let glass = self.pill.is_some();
        let background = if active && !glass && theme::colors::is_light() {
            Color::WHITE
        } else if active && !glass {
            theme::colors::ghost_overlay()
        } else if !glass && (hovering() || focused().is_focused()) {
            theme::colors::ghost_overlay_hover()
        } else {
            Color::TRANSPARENT
        };

        rect()
            .horizontal()
            .main_align(Alignment::Center)
            .cross_align(Alignment::Center)
            .height(Size::px(NAV_LINK_HEIGHT))
            .width(if self.squeeze {
                Size::flex(1.)
            } else {
                Size::px(nav_link_width(nav_label, self.compact))
            })
            .corner_radius(if glass {
                CornerRadius::new_all(NAV_LINK_HEIGHT / 2.)
            } else {
                rounded(10.)
            })
            .background(background)
            .maybe(
                (active && !glass) || (glass && focused() == Focus::Keyboard),
                |el| {
                    el.border(
                        Border::new()
                            .fill(if glass {
                                theme::colors::brand()
                            } else {
                                theme::colors::component_border()
                            })
                            .width(1.)
                            .alignment(BorderAlignment::Inner),
                    )
                },
            )
            .a11y_id(a11y_id)
            .a11y_alt(nav_label)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .on_press(move |e: Event<PressEventData>| {
                if glass
                    && matches!(
                        e.data(),
                        PressEventData::Mouse(_) | PressEventData::Touch(_)
                    )
                {
                    return;
                }
                e.prevent_default();
                target.activate(active, stack, at_home);
            })
            .map(self.pill, |el, (track, index)| {
                track.option(el, index, true)
            })
            .on_pointer_over(move |_| hovering.set(true))
            .on_pointer_out(move |_| hovering.set(false))
            .cursor(CursorIcon::Pointer)
            .child(
                label()
                    .text(nav_label)
                    .maybe(self.squeeze, |el| {
                        el.width(Size::fill())
                            .text_align(TextAlign::Center)
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                    })
                    .font_size(text_size(if self.compact { 14. } else { 16. }))
                    .font_weight(if lit {
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
    compact: bool,
}

impl Component for NavbarRight {
    fn render(&self) -> impl IntoElement {
        let foreground = self.over_art.then(theme::colors::on_art);
        let current_account = use_current_account();
        let dispatch = use_dispatch();
        let unread = use_notifications_snapshot().unread_count();
        let unread_chats = use_chat_unread();
        let chat_allowed = use_chat_allowed();

        let has_account = settled_or_loading(&current_account)
            .is_none_or(|account| account.is_some_and(|account| account.is_microsoft()));

        let account_uuid = try_default_account(&current_account)
            .map(|account| account.id.to_string())
            .unwrap_or_else(|| uuid::Uuid::nil().to_string());

        let social_dispatch = dispatch.clone();
        let toggle_social = move |_| {
            if crate::view::chat::is_chat_window_open() {
                crate::view::chat::open_chat_window();
            } else {
                social_dispatch.toggle_social();
            }
        };

        let notif_dispatch = dispatch.clone();
        let open_notifications = move |_| {
            notif_dispatch.toggle_notification_center();
        };

        let open_control_center = move |_| {
            dispatch.toggle_control_center();
        };

        rect()
            .horizontal()
            .width(
                if self.compact || theme::colors::active_theme().liquid_glass {
                    Size::auto()
                } else {
                    Size::flex(1.)
                },
            )
            .main_align(Alignment::End)
            .cross_align(Alignment::Center)
            .spacing(8.)
            .maybe_child(chat_allowed.then(|| {
                super::navbar_button(foreground)
                    .tooltip(t("oneclient.components.navbar.social"))
                    .enabled(has_account)
                    .child(social_icon(unread_chats))
                    .on_press(toggle_social)
            }))
            .child(
                super::navbar_button(foreground)
                    .tooltip(t("oneclient.components.navbar.notifications"))
                    .child(notification_bell(unread))
                    .on_press(open_notifications),
            )
            .child(
                super::navbar_button(foreground)
                    .overflow(Overflow::None)
                    .alt(t("oneclient.components.navbar.control_center"))
                    .tooltip(t("oneclient.components.navbar.control_center"))
                    .padding(0.0)
                    .on_press(open_control_center)
                    .child(avatar_with_gear(account_uuid)),
            )
            .child(super::window_controls_with(
                foreground,
                super::close_launcher,
            ))
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

fn social_icon(unread: bool) -> impl IntoElement {
    rect()
        .width(Size::px(20.))
        .height(Size::px(20.))
        .child(Icon::new(IconType::Users01).size(20.))
        .maybe_child(unread.then(|| {
            rect()
                .position(Position::new_absolute().top(-2.).right(-2.))
                .width(Size::px(8.))
                .height(Size::px(8.))
                .corner_radius(theme::rounded(4.))
                .background(theme::colors::brand())
                .layer(Layer::Relative(3))
                .into_element()
        }))
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
