use freya::animation::*;
use freya::prelude::*;
use freya::router::*;

use super::app_shell::{appshell_overlay, back_button, hides_overlay};
use crate::Route;
use crate::theme;
use crate::ui::entrance_motion_layer;

#[derive(Clone, Copy, PartialEq)]
enum Enter {
    None,
    Up,
    Fade,
}

fn is_sidebar_route(route: &Route) -> bool {
    matches!(
        route,
        Route::SettingsAppearance {}
            | Route::SettingsMinecraft {}
            | Route::SettingsLauncher {}
            | Route::SettingsJava {}
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

        let history = use_previous_and_current(use_route::<Route>());
        let back_title = history.read().0.title();

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
            .maybe_child(overlay)
            .child(column)
    }
}
