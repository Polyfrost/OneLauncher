use freya::animation::*;
use freya::prelude::*;
use freya::router::*;

use super::app_shell::{appshell_overlay, back_button, hides_overlay, navigate_back};
use crate::Route;
use crate::hooks::use_overlay_claims;
use crate::theme;
use crate::ui::entrance_motion_layer;

#[derive(Clone, Copy, PartialEq)]
enum Enter {
    None,
    Up,
    Fade,
}

/// This outlet must stay still for these the shell runs its own content-only
/// transition else the sidebar animates along with the page
fn is_sidebar_route(route: &Route) -> bool {
    matches!(
        route,
        Route::SettingsAppearance {}
            | Route::SettingsMinecraft {}
            | Route::SettingsAccounts {}
            | Route::SettingsLauncher {}
            | Route::SettingsJava {}
            | Route::SettingsStorage {}
            | Route::SettingsApis {}
            | Route::SettingsLanguage {}
            | Route::SettingsDeveloper {}
            | Route::SettingsChangelog {}
    )
}

fn is_cluster_route(route: &Route) -> bool {
    matches!(
        route,
        Route::ClusterOverview { .. }
            | Route::ClusterLogs { .. }
            | Route::ProcessLogs { .. }
            | Route::ClusterScreenshots { .. }
            | Route::ClusterMods { .. }
            | Route::ClusterShaders { .. }
            | Route::ClusterTextures { .. }
            | Route::ClusterSettings { .. }
    )
}

/// Checks if it should go back to home or somewhere else
fn escape_exits_section(route: &Route) -> bool {
    is_cluster_route(route) || is_sidebar_route(route)
}

/// Escape belongs to whatever the user is typing in before it belongs to the
/// router so a search field does not throw the page away mid-word
fn is_text_entry(role: AccessibilityRole) -> bool {
    matches!(
        role,
        AccessibilityRole::TextInput
            | AccessibilityRole::MultilineTextInput
            | AccessibilityRole::SearchInput
            | AccessibilityRole::PasswordInput
    )
}

fn enter_kind(from: &Route, to: &Route) -> Enter {
    if (is_sidebar_route(from) && is_sidebar_route(to))
        || (is_cluster_route(from) && is_cluster_route(to))
    {
        Enter::None
    } else if hides_overlay(from) {
        Enter::Up
    } else {
        Enter::Fade
    }
}

#[derive(PartialEq)]
pub struct AnimatedAppOutlet;

impl Component for AnimatedAppOutlet {
    fn render(&self) -> impl IntoElement {
        let mut router = use_animated_router::<Route>();

        let route = use_route::<Route>();
        let at_home = matches!(&route, Route::Home {});
        let exits_section = escape_exits_section(&route);
        let history = use_previous_and_current(route);
        let back_title = history.read().0.title();

        let platform = Platform::get();
        let focused_node = platform.focused_accessibility_node;
        let focused_id = platform.focused_accessibility_id;

        let key_claims = use_overlay_claims();
        let pointer_claims = key_claims.clone();

        let on_global_key = move |e: Event<KeyboardEventData>| {
            if e.key != Key::Named(NamedKey::Escape) || key_claims.any() {
                return;
            }

            if is_text_entry(focused_node.peek().role()) {
                focused_id.peek().request_unfocus();
                return;
            }

            if exits_section {
                let _ = RouterContext::get().push(Route::Home {});
            } else {
                navigate_back(at_home);
            }
        };

        let on_global_pointer = move |e: Event<PointerEventData>| {
            if pointer_claims.any() {
                return;
            }
            match e.button() {
                Some(MouseButton::Back) => navigate_back(at_home),
                Some(MouseButton::Forward) => RouterContext::get().go_forward(),
                _ => {}
            }
        };

        let anim = use_animation(|_conf| {
            AnimNum::new(0., 1.)
                .time(430)
                .ease(Ease::Out)
                .function(Function::Cubic)
        });
        let mut last_to = use_state(|| None::<Route>);

        let (_, to, is_transition) = match &*router.read() {
            AnimatedRouterContext::FromTo(from, to) => (enter_kind(from, to), to.clone(), true),
            AnimatedRouterContext::In(to) => (Enter::None, to.clone(), false),
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

        if anim_finished && matches!(&*router.peek(), AnimatedRouterContext::FromTo(_, _)) {
            router.write().settle();
        }

        let kind = match &*router.read() {
            AnimatedRouterContext::FromTo(from, to) => enter_kind(from, to),
            AnimatedRouterContext::In(_) => Enter::None,
        };

        let is_home = matches!(to, Route::Home {});
        let show_overlay = !hides_overlay(&to);

        let p = if anim_finished {
            1.0
        } else {
            anim.get().value()
        };

        let chrome_opacity = if kind == Enter::Up { p } else { 1.0 };

        let (content_dy, content_opacity) = if anim_finished || matches!(kind, Enter::None) {
            (0., 1.)
        } else {
            match kind {
                Enter::None => (0., 1.),
                Enter::Up => ((1. - p) * 48., p),
                Enter::Fade => ((1. - p) * 22., p),
            }
        };

        let overlay = show_overlay.then(|| appshell_overlay().opacity(chrome_opacity));

        let back = (!is_home).then(|| {
            rect()
                .opacity(chrome_opacity)
                .child(back_button(&back_title))
        });

        let column = rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .margin(Gaps::new(theme::NAVBAR_HEIGHT_PX, 0., 0., 0.))
            .overflow(Overflow::Clip)
            .layer(Layer::Relative(3))
            .maybe_child(back)
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .overflow(Overflow::Clip)
                    .child(entrance_motion_layer(
                        0.,
                        content_dy,
                        content_opacity,
                        Outlet::<Route>::new(),
                    )),
            );

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .on_global_key_down(on_global_key)
            .on_global_pointer_press(on_global_pointer)
            .maybe_child(overlay)
            .child(column)
    }
}
