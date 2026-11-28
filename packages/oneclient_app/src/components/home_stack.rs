use freya::prelude::*;

/// Home renders the home page and the versions page as one vertical stack. This state
/// lives on `AppShell` so the navbar and the outlet backdrop can follow it too, and so
/// coming back to Home (e.g. from a cluster page) returns to whichever page you left.
#[derive(Clone, Copy)]
pub struct HomeStack {
    /// `true` = settled on (or heading to) the versions page
    pub open: State<bool>,
    /// Changes on every explicit navigation, even when the destination is already selected.
    pub navigation: State<u64>,
    wheel_pending: State<bool>,
    /// 0 = home page in view, 1 = versions page in view; updated every frame of a slide
    pub progress: State<f32>,
    /// `progress` past the halfway mark; changes rarely, so cheap for the navbar to watch
    pub on_versions: State<bool>,
}

impl HomeStack {
    pub(crate) fn handle_page_key(self, event: &Event<KeyboardEventData>, blocked: bool) {
        if !blocked && let Some(down) = page_key_is_down(event.data()) {
            event.stop_propagation();
            self.set_open(down);
        }
    }

    pub fn set_open(self, open: bool) {
        let mut state = self.open;
        let mut navigation = self.navigation;
        self.set_wheel_pending(false);
        state.set_if_modified(open);
        let next = navigation.peek().wrapping_add(1);
        navigation.set(next);
    }

    /// Return from Versions or cancel a partial gesture without walking the route history.
    pub fn back_to_home(self) -> bool {
        let can_go_back =
            *self.open.peek() || *self.wheel_pending.peek() || *self.progress.peek() > 0.;
        if can_go_back {
            self.set_open(false);
            true
        } else {
            false
        }
    }

    pub fn set_wheel_pending(self, pending: bool) {
        let mut state = self.wheel_pending;
        state.set_if_modified(pending);
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

/// Reject modified paging shortcuts and keep editing keys in the focused field.
pub(super) fn page_key_is_down(event: &KeyboardEventData) -> Option<bool> {
    let down = match event.key {
        Key::Named(NamedKey::PageUp) => false,
        Key::Named(NamedKey::PageDown) => true,
        _ => return None,
    };
    let window = Platform::get().current_window();
    (event.modifiers.is_empty()
        && !crate::ui::is_text_entry(window.focused_accessibility_node.peek().role()))
    .then_some(down)
}

pub fn use_provide_home_stack() -> HomeStack {
    let open = use_state(|| false);
    let navigation = use_state(|| 0);
    let wheel_pending = use_state(|| false);
    let progress = use_state(|| 0f32);
    let on_versions = use_state(|| false);
    use_hook(|| {
        let stack = HomeStack {
            open,
            navigation,
            wheel_pending,
            progress,
            on_versions,
        };
        provide_context(stack);
        stack
    })
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
