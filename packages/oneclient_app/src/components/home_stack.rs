use freya::prelude::*;
use oneclient_core::settings::StartTab;

/// Home renders the home page and the versions page as one vertical stack. This state
/// lives on `AppShell` so the navbar and the outlet backdrop can follow it too, and so
/// coming back to Home (e.g. from a cluster page) returns to whichever page you left.
#[derive(Clone, Copy)]
pub struct HomeStack {
    /// `true` = settled on (or heading to) the versions page
    pub open: State<bool>,
    /// 0 = home page in view, 1 = versions page in view; updated every frame of a slide
    pub progress: State<f32>,
    /// `progress` past the halfway mark; changes rarely, so cheap for the navbar to watch
    pub on_versions: State<bool>,
}

impl HomeStack {
    pub fn set_open(self, open: bool) {
        let mut state = self.open;
        if *state.peek() != open {
            state.set(open);
        }
    }

    pub fn publish_progress(self, value: f32) {
        let mut progress = self.progress;
        if (*progress.peek() - value).abs() > f32::EPSILON {
            progress.set(value);
        }
        let mut on_versions = self.on_versions;
        let past_half = value > 0.5;
        if *on_versions.peek() != past_half {
            on_versions.set(past_half);
        }
    }
}

pub fn use_provide_home_stack() {
    let start_on_versions = use_hook(|| {
        crate::hooks::use_dispatch()
            .station()
            .peek()
            .settings
            .settings
            .start_tab
            == StartTab::Versions
    });
    let open = use_state(|| start_on_versions);
    let progress = use_state(|| if start_on_versions { 1f32 } else { 0f32 });
    let on_versions = use_state(|| start_on_versions);
    use_hook(|| {
        provide_context(HomeStack {
            open,
            progress,
            on_versions,
        })
    });
}

pub fn use_home_stack() -> Option<HomeStack> {
    use_hook(try_consume_context::<HomeStack>)
}

/// When present in context, `ScrollArea` hands wheel input up to the home stack:
/// always while `pass_all` is set (the stack is mid-scroll), and otherwise once the
/// area is pinned against an edge, so scrolling flows between the list and the stack.
/// A wheel stream that was scrolling the area is held at the edge until it pauses for
/// `EDGE_SETTLE`, so the tail of a scroll (or its momentum) never jumps to Home.
#[derive(Clone, Copy)]
pub struct WheelEdgeChain {
    pub pass_all: State<bool>,
    /// When the area last took the wheel, or held it at the edge
    pub inner_wheel_at: State<Option<std::time::Instant>>,
    /// Last momentum event the stack swallowed after a touchpad lift settled it
    pub coast_at: State<Option<std::time::Instant>>,
}

impl WheelEdgeChain {
    /// A touchpad fling that already settled the stack is still coasting, so neither the
    /// stack nor a list inside it should act on it
    pub fn coasting(&self) -> bool {
        self.coast_at.peek().is_some_and(|at| at.elapsed() < COAST_GAP)
    }
}

/// Momentum events arrive every frame; a gap this long means the fling has died out
pub const COAST_GAP: std::time::Duration = std::time::Duration::from_millis(100);

/// How long the wheel must go quiet after scrolling a list before it may move the stack
pub const EDGE_SETTLE: std::time::Duration = std::time::Duration::from_millis(300);
