use std::collections::HashSet;

use freya::animation::{AnimNum, Ease, Function, OnCreation, use_animation};
use freya::prelude::*;
use oneclient_net::status::{self, ServiceStatus};

use crate::components::{Icon, IconType};
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
    McAuthDown,
    PolyfrostDown,
}

impl Issue {
    fn is_active(self, s: &ServiceStatus) -> bool {
        match self {
            Self::NoInternet => !s.online,
            Self::McAuthDown => s.online && !s.mc_auth_up,
            Self::PolyfrostDown => s.online && !s.polyfrost_up,
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::NoInternet => "No internet connection.",
            Self::McAuthDown => {
                "Minecraft authentication servers are unreachable. Logging in may fail."
            }
            Self::PolyfrostDown => "Polyfrost services are experiencing issues.",
        }
    }

    fn icon(self) -> IconType {
        match self {
            Self::NoInternet => IconType::Globe01,
            Self::McAuthDown => IconType::AlertTriangle,
            Self::PolyfrostDown => IconType::AlertCircle,
        }
    }

    fn background(self) -> Color {
        match self {
            Self::NoInternet => colors::danger(),
            Self::McAuthDown | Self::PolyfrostDown => AMBER,
        }
    }

    fn closeable(self) -> bool {
        !matches!(self, Self::NoInternet)
    }
}

fn active_issues(s: &ServiceStatus) -> Vec<Issue> {
    [Issue::NoInternet, Issue::McAuthDown, Issue::PolyfrostDown]
        .into_iter()
        .filter(|i| i.is_active(s))
        .collect()
}

#[derive(PartialEq)]
pub struct StatusBar;

impl Component for StatusBar {
    fn render(&self) -> impl IntoElement {
        let mut status = use_state(status::current);
        let mut dismissed = use_state(HashSet::<Issue>::new);

        use_hook(move || {
            status::request_recheck();
            let mut rx = status::subscribe();
            spawn(async move {
                while rx.changed().await.is_ok() {
                    status.set(*rx.borrow());
                }
            });
        });

        use_side_effect(move || {
            let s = *status.read();
            let cur = dismissed.peek().clone();
            let next: HashSet<Issue> = cur.iter().copied().filter(|i| i.is_active(&s)).collect();
            if next != cur {
                dismissed.set(next);
            }
        });

        let s = *status.read();
        let dset = dismissed.read();
        let visible = active_issues(&s).into_iter().find(|i| !dset.contains(i));

        match visible {
            Some(issue) => StatusBanner { issue, dismissed }.into_element(),
            None => rect().into_element(),
        }
    }
}

#[derive(PartialEq)]
struct StatusBanner {
    issue: Issue,
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

        let message = rect()
            .horizontal()
            .height(Size::fill())
            .cross_align(Alignment::Center)
            .spacing(8.)
            .layer(Layer::OverlayLevel(u8::MAX - 20))
            .child(Icon::new(issue.icon()).size(15.).color(Color::WHITE))
            .child(
                label()
                    .text(issue.message())
                    .font_size(12.)
                    .font_weight(FontWeight::MEDIUM)
                    .max_lines(1)
                    .color(Color::WHITE),
            );

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
