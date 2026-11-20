use freya::animation::*;
use freya::prelude::*;
use freya::router::*;

use super::app_shell::{appshell_overlay, back_button, hides_overlay, navigate_back};
use super::workspace_dialog::{RoutedPage, WorkspaceDialog, WorkspacePopup};
use crate::Route;
use crate::hooks::use_overlay_claims;
use crate::theme;
use crate::ui::entrance_motion_layer;

#[derive(Clone, Copy, PartialEq)]
enum Enter {
    None,
    Up,
    Fade,
    Slide,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EscapeAction {
    /// The root screen has nothing above it so Escape must not walk history
    Ignore,
    HomeStack,
    Back,
}

fn escape_action(route: &Route) -> EscapeAction {
    if matches!(route, Route::Home {}) {
        EscapeAction::HomeStack
    } else if WorkspaceDialog::for_route(route).is_some() {
        EscapeAction::Ignore
    } else {
        EscapeAction::Back
    }
}

fn enter_kind(from: &Route, to: &Route) -> Enter {
    if WorkspaceDialog::for_route(to).is_some()
        || (matches!(from, Route::Browser { .. }) && matches!(to, Route::Browser { .. }))
    {
        Enter::None
    } else if matches!(to, Route::Browser { .. }) {
        // Slide once on entry. Keeping the results opaque avoids compositing the whole grid.
        Enter::Slide
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
        let home_stack = crate::components::use_home_stack();

        let route = use_route::<Route>();
        let page = super::use_app_page();
        let dialog = WorkspaceDialog::for_route(&route);
        let at_home = matches!(&page, Route::Home {});
        let escape = escape_action(&route);

        let mut origin_route = use_state(|| page.clone());
        let mut last_route = use_state(|| page.clone());

        use_side_effect_with_deps(&page, move |current| {
            let prev = last_route.peek().clone();
            if prev != *current {
                origin_route.set(prev);
                last_route.set(current.clone());
            }
        });

        let back_title = origin_route.read().title();

        let window = Platform::get().current_window();
        let focused_node = window.focused_accessibility_node;
        let focused_id = window.focused_accessibility_id;

        let key_claims = use_overlay_claims();
        let pointer_claims = key_claims.clone();

        let on_global_key = move |e: Event<KeyboardEventData>| {
            if e.code == Code::KeyO
                && e.modifiers
                    .contains(Modifiers::ctrl_or_meta() | Modifiers::SHIFT)
            {
                if let Ok(dir) = oneclient_common::paths::logs_dir() {
                    crate::platform::open_path(&dir.to_string_lossy());
                }
                return;
            }

            if e.key != Key::Named(NamedKey::Escape) || key_claims.any() {
                return;
            }

            if crate::ui::is_text_entry(focused_node.peek().role()) {
                focused_id.peek().request_unfocus();
                return;
            }

            match escape {
                EscapeAction::Ignore => {}
                EscapeAction::HomeStack => {
                    if let Some(stack) = home_stack {
                        stack.back_to_home();
                    }
                }
                EscapeAction::Back => navigate_back(at_home),
            }
        };

        let on_global_pointer = move |e: Event<PointerEventData>| {
            if dialog.is_some() || pointer_claims.any() {
                return;
            }
            match e.button() {
                Some(MouseButton::Back) => {
                    if at_home && home_stack.is_some_and(|stack| stack.back_to_home()) {
                        e.stop_propagation();
                    } else {
                        navigate_back(at_home);
                    }
                }
                Some(MouseButton::Forward) => {
                    let router = RouterContext::get();
                    if router.can_go_forward() {
                        router.go_forward();
                    }
                }
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

        let transition_kind = |from: &Route, to: &Route| {
            if WorkspaceDialog::for_route(from).is_some() && last_route.peek().eq(to) {
                Enter::None
            } else {
                enter_kind(from, to)
            }
        };
        let (entrance, to, is_transition) = match &*router.read() {
            AnimatedRouterContext::FromTo(from, to) => {
                (transition_kind(from, to), to.clone(), true)
            }
            AnimatedRouterContext::In(to) => (Enter::None, to.clone(), false),
        };

        if last_to.peek().as_ref() != Some(&to) {
            last_to.set(Some(to.clone()));
            if is_transition {
                if entrance == Enter::None {
                    router.write().settle();
                } else {
                    anim.run(AnimDirection::Forward);
                }
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

        let kind = match &*router.read() {
            AnimatedRouterContext::FromTo(from, to) => transition_kind(from, to),
            AnimatedRouterContext::In(_) => Enter::None,
        };

        let is_home = matches!(page, Route::Home {});
        let show_overlay = !hides_overlay(&page);

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
                Enter::Slide => ((1. - p) * 32., 1.),
            }
        };

        // Home's scroll stack fades the regular page backdrop in as the versions page rises
        let overlay = if is_home {
            let stacked = home_stack.map_or(0., |stack| *stack.progress.read());
            (stacked > 0.001).then(|| appshell_overlay(stacked))
        } else {
            show_overlay.then(|| appshell_overlay(chrome_opacity))
        };

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
            .maybe(dialog.is_some(), |el| {
                el.a11y_builder(|node| node.set_hidden())
            })
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
                        RoutedPage(page.clone()),
                    )),
            );

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .on_global_key_down(on_global_key)
            .on_global_pointer_up(on_global_pointer)
            // Always occupy the slot: inserting the overlay ahead of `column` shifts its
            // index and remounts the page, which resets Home's scroll stack mid-scroll
            .child(overlay.map_or_else(|| rect().into_element(), IntoElement::into_element))
            .child(column)
            .maybe_child(dialog.map(|kind| WorkspacePopup {
                kind,
                route,
                origin: page,
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browse_animates_on_entry_and_stays_still_for_in_place_changes() {
        let browse = |cluster_id, package_type: &str| Route::Browser {
            cluster_id,
            package_type: package_type.into(),
            pick_cluster: true,
            in_instance: false,
        };
        let mods = browse(1, "mod");
        for from in [
            Route::Home {},
            Route::SettingsAppearance {},
            Route::Clusters {},
        ] {
            assert!(enter_kind(&from, &mods) == Enter::Slide);
        }
        for to in [mods.clone(), browse(2, "mod"), browse(1, "shader")] {
            assert!(enter_kind(&mods, &to) == Enter::None);
        }
    }

    #[test]
    fn escape_action_per_route() {
        assert_eq!(escape_action(&Route::Home {}), EscapeAction::HomeStack);
        assert_eq!(
            escape_action(&Route::ClusterSettings { cluster_id: 1 }),
            EscapeAction::Ignore
        );
        assert_eq!(escape_action(&Route::SettingsJava {}), EscapeAction::Ignore);
        assert_eq!(escape_action(&Route::Clusters {}), EscapeAction::Back);
    }
}
