use std::collections::HashSet;

use freya::animation::{AnimNum, Ease, Function, OnCreation, use_animation};
use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_net::status::{self, ServiceStatus};

use crate::components::{Icon, IconType};
use crate::hooks::{try_default_account, use_current_account};
use crate::routes::Route;
use crate::theme::colors;

const BAR_HEIGHT: f32 = 34.;
const AMBER: Color = Color::from_rgb(191, 122, 26);

/// `Layer::RelativeOverlay(n)` saturates at `n = 16` putting children on the same
/// (unordered) layer as the bar background
/// 14 leaves headroom above every popup
const BAR_LAYER: u8 = 14;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Issue {
    NoInternet,
    SignedOut,
    McAuthDown,
    PolyfrostDown,
}

/// What the bar reacts to the network probe plus whether the active account
/// lost its sign-in
#[derive(Clone, PartialEq)]
struct Conditions {
    status: ServiceStatus,
    signed_out_as: Option<String>,
}

impl Issue {
    fn is_active(self, c: &Conditions) -> bool {
        let s = &c.status;
        match self {
            Self::NoInternet => !s.online,
            Self::SignedOut => c.signed_out_as.is_some(),
            Self::McAuthDown => s.online && !s.mc_auth_up,
            Self::PolyfrostDown => s.online && !s.polyfrost_up,
        }
    }

    fn message(self, c: &Conditions) -> String {
        match self {
            Self::NoInternet => "No internet connection.".to_string(),
            Self::SignedOut => format!(
                "You have been signed out of {}. Sign in again to keep playing.",
                c.signed_out_as.as_deref().unwrap_or("your account")
            ),
            Self::McAuthDown => {
                "Minecraft authentication servers are unreachable. Logging in may fail.".to_string()
            }
            Self::PolyfrostDown => "Polyfrost services are experiencing issues.".to_string(),
        }
    }

    fn icon(self) -> IconType {
        match self {
            Self::NoInternet => IconType::Globe01,
            Self::SignedOut => IconType::Users01,
            Self::McAuthDown => IconType::AlertTriangle,
            Self::PolyfrostDown => IconType::AlertCircle,
        }
    }

    fn background(self) -> Color {
        match self {
            Self::NoInternet | Self::SignedOut => colors::danger(),
            Self::McAuthDown | Self::PolyfrostDown => AMBER,
        }
    }

    fn closeable(self) -> bool {
        !matches!(self, Self::NoInternet)
    }

    /// Where pressing the message takes the user if anywhere
    fn route(self) -> Option<Route> {
        matches!(self, Self::SignedOut).then_some(Route::SettingsAccounts {})
    }
}

fn active_issues(c: &Conditions) -> Vec<Issue> {
    [
        Issue::NoInternet,
        Issue::SignedOut,
        Issue::McAuthDown,
        Issue::PolyfrostDown,
    ]
    .into_iter()
    .filter(|i| i.is_active(c))
    .collect()
}

#[derive(PartialEq)]
pub struct StatusBar;

impl Component for StatusBar {
    fn render(&self) -> impl IntoElement {
        let mut status = use_state(status::current);
        let mut dismissed = use_state(HashSet::<Issue>::new);
        let current_account = use_current_account();

        use_hook(move || {
            status::request_recheck();
            let mut rx = status::subscribe();
            spawn(async move {
                while rx.changed().await.is_ok() {
                    status.set(*rx.borrow());
                }
            });
        });

        let conditions = move || Conditions {
            status: *status.read(),
            signed_out_as: try_default_account(&current_account)
                .filter(|a| a.needs_sign_in())
                .map(|a| a.username),
        };

        // A dismissal lasts only while its issue does so the next sign-out
        // shows the bar again
        use_side_effect(move || {
            let c = conditions();
            let cur = dismissed.peek().clone();
            let next: HashSet<Issue> = cur.iter().copied().filter(|i| i.is_active(&c)).collect();
            if next != cur {
                dismissed.set(next);
            }
        });

        let c = conditions();
        let dset = dismissed.read();
        let visible = active_issues(&c).into_iter().find(|i| !dset.contains(i));

        match visible {
            Some(issue) => StatusBanner {
                issue,
                text: issue.message(&c),
                dismissed,
            }
            .into_element(),
            None => rect().into_element(),
        }
    }
}

#[derive(PartialEq)]
struct StatusBanner {
    issue: Issue,
    text: String,
    dismissed: State<HashSet<Issue>>,
}

impl Component for StatusBanner {
    fn render(&self) -> impl IntoElement {
        let issue = self.issue;
        let mut dismissed = self.dismissed;

        let intro = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            AnimNum::new(0., 1.)
                .time(260)
                .ease(Ease::Out)
                .function(Function::Cubic)
        });
        let p = intro.get().value();

        let mut close_hover = use_state(|| false);

        let close = issue.closeable().then(|| {
            rect()
                .center()
                .width(Size::px(22.))
                .height(Size::px(22.))
                .corner_radius(CornerRadius::new_all(6.))
                .background(if *close_hover.read() {
                    Color::WHITE.with_a(46)
                } else {
                    Color::TRANSPARENT
                })
                .cursor(CursorIcon::Pointer)
                .on_pointer_enter(move |_| close_hover.set(true))
                .on_pointer_leave(move |_| close_hover.set(false))
                .on_press(move |_| {
                    dismissed.write().insert(issue);
                })
                .child(Icon::new(IconType::XClose).size(14.).color(Color::WHITE))
                .into_element()
        });

        // Flex spacers centre the message absolute positioning pinned the close button
        // to the top of the content box inside the bar's padding
        let leading = rect().width(Size::flex(1.0)).height(Size::fill());

        let mut action_hover = use_state(|| false);

        let action = issue.route().map(|route| {
            rect()
                .center()
                .height(Size::px(22.))
                .padding(Gaps::new_symmetric(0., 8.))
                .corner_radius(CornerRadius::new_all(6.))
                .background(if *action_hover.read() {
                    Color::WHITE.with_a(46)
                } else {
                    Color::WHITE.with_a(26)
                })
                .cursor(CursorIcon::Pointer)
                .on_pointer_enter(move |_| action_hover.set(true))
                .on_pointer_leave(move |_| action_hover.set(false))
                .on_press(move |_| {
                    let _ = RouterContext::get().push(route.clone());
                })
                .child(
                    label()
                        .text("Sign in")
                        .font_size(12.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(Color::WHITE),
                )
                .into_element()
        });

        let message = rect()
            .horizontal()
            .height(Size::fill())
            .cross_align(Alignment::Center)
            .spacing(8.)
            .layer(Layer::OverlayLevel(u8::MAX - 20))
            .child(Icon::new(issue.icon()).size(15.).color(Color::WHITE))
            .child(
                label()
                    .text(self.text.clone())
                    .font_size(12.)
                    .font_weight(FontWeight::MEDIUM)
                    .max_lines(1)
                    .color(Color::WHITE),
            )
            .maybe_child(action);

        let trailing = rect()
            .width(Size::flex(1.0))
            .height(Size::fill())
            .horizontal()
            .main_align(Alignment::End)
            .cross_align(Alignment::Center)
            .layer(Layer::Relative(1))
            .maybe_child(close);

        rect()
            .width(Size::window_percent(100.))
            .height(Size::px(BAR_HEIGHT))
            .position(Position::new_global().bottom(0.).left(0.))
            .layer(Layer::OverlayLevel(BAR_LAYER))
            .background(issue.background())
            .opacity(p)
            .horizontal()
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(8.)
            .padding(Gaps::new_symmetric(0., 10.))
            .child(leading)
            .child(message)
            .child(trailing)
    }
}
