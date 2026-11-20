use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use freya::{
    animation::{AnimNum, Ease, OnCreation, use_animation},
    prelude::*,
};

use crate::hooks::use_overlay_claim;
use crate::theme::{colors, rounded};

const SCRIM_ALPHA: f32 = 90.;
const SCRIM_MIN_ALPHA: u8 = 1;
const PANEL_RADIUS_PX: f32 = 14.;
const PANEL_SLIDE_PX: f32 = 8.;
// Keep a complete dialog below the next peer, including its deepest controls.
const DIALOG_LAYER_BAND: i16 = 128;
const OVERLAY_LEVEL_GAP: i16 = i16::MAX / 16;

/// Overlay level of a top level popup.
pub const OVERLAY_BASE_LEVEL: u8 = 12;

/// Freya maps a level to `(level.max(1) as i16) * (i16::MAX / 16)`, which
/// saturates above 16 so every higher level collides with 16.
pub const OVERLAY_MAX_LEVEL: u8 = 16;

#[derive(Clone, Copy)]
struct OpenLevel {
    window: u64,
    id: u64,
    level: u8,
    scope: EscapeScope,
}

#[derive(Clone, Copy, PartialEq)]
enum EscapeScope {
    Independent,
    Dialog(AccessibilityId, AccessibilityId),
    Transient(Option<AccessibilityId>),
}

#[derive(Clone, Copy)]
struct ContainingDialog(AccessibilityId);

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct MountedDialog {
    pub(crate) active: State<Option<AccessibilityId>>,
    order_revision: State<u64>,
}

impl MountedDialog {
    pub(crate) fn new() -> Self {
        Self {
            active: State::create(None),
            order_revision: State::create(0),
        }
    }

    fn order_changed(self) {
        if let Some(mut revision) = self.order_revision.try_write_unchecked() {
            *revision = revision.wrapping_add(1);
        }
    }
}

thread_local! {
    static OPEN_LEVELS: RefCell<Vec<OpenLevel>> = const { RefCell::new(Vec::new()) };
    static NEXT_OVERLAY_ID: Cell<u64> = const { Cell::new(0) };
}

/// True while no open overlay sits above `level`. Freya dispatches
/// `on_global_key_down` to every listener, so a handler that should only answer
/// for the topmost overlay has to gate itself on this.
pub fn overlay_is_topmost(level: u8) -> bool {
    let window = Platform::get().current_window().id;
    top_escape_owner(window, try_consume_root_context::<MountedDialog>())
        .is_none_or(|entry| entry.level <= level)
}

/// Entry progress of a panel inside an [`OverlayPopup`], for [`popup_surface`].
pub fn use_popup_intro() -> f32 {
    let intro = use_animation(|conf| {
        conf.on_creation(OnCreation::Run);
        AnimNum::new(0., 1.).time(200).ease(Ease::Out)
    });
    intro.read().value()
}

/// Chrome shared by the panels anchored inside an [`OverlayPopup`]. The caller
/// owns the size, padding and children.
pub fn popup_surface(progress: f32) -> Rect {
    let theme = colors::active_theme();
    let radius = crate::theme::rounded_px(PANEL_RADIUS_PX);
    let rect = rect()
        .vertical()
        .opacity(progress)
        .margin(Gaps::new((1. - progress) * -PANEL_SLIDE_PX, 0., 0., 0.))
        .corner_radius(rounded(PANEL_RADIUS_PX))
        .shadow(Shadow::from((
            0.,
            8.,
            32.,
            0.,
            Color::from_argb(120, 0, 0, 0),
        )));
    if theme.liquid_glass {
        let tint = colors::card().with_a(if theme.colors.light { 178 } else { 163 });
        crate::theme::glass::panel(rect, &theme, radius, tint)
    } else {
        rect.border(
            Border::new()
                .fill(colors::component_border())
                .width(1.)
                .alignment(BorderAlignment::Inner),
        )
        .background(colors::page_elevated().with_a(220))
        .backdrop_blur(12.)
    }
}

pub fn use_open_level(level: u8) {
    use_overlay_level_id(level, Some(EscapeScope::Independent));
}

pub(crate) fn mounted_dialogs(window: u64) -> Vec<(AccessibilityId, AccessibilityId)> {
    OPEN_LEVELS.with_borrow(|open| {
        let mut dialogs: Vec<_> = open
            .iter()
            .filter_map(|entry| match entry.scope {
                EscapeScope::Dialog(dialog, origin) if entry.window == window => {
                    Some((entry.level, dialog, origin))
                }
                _ => None,
            })
            .collect();
        // Stable sorting preserves registration order for peers on the same level.
        dialogs.sort_by_key(|&(level, _, _)| level);
        dialogs
            .into_iter()
            .map(|(_, dialog, origin)| (dialog, origin))
            .collect()
    })
}

pub(super) fn is_inside_popup() -> bool {
    try_consume_context::<ContainingDialog>().is_some()
}

pub(super) fn use_transient_escape(level: u8, open: bool) -> (bool, EscapeOwner) {
    let dialog = try_consume_context::<ContainingDialog>().map(|dialog| dialog.0);
    let top = try_consume_root_context::<MountedDialog>();
    // When a workspace opens over a retained page, remove the page's popover
    // and its input claim before the workspace starts handling input.
    let visible = open && top.is_none_or(|top| *top.active.read() == dialog);
    let owner = use_overlay_level_id(level, visible.then_some(EscapeScope::Transient(dialog)));
    (visible, owner)
}

#[derive(Clone, Copy, PartialEq)]
pub(super) struct EscapeOwner {
    window: u64,
    id: u64,
    mounted: Option<MountedDialog>,
}

impl EscapeOwner {
    pub(super) fn is_topmost(self) -> bool {
        top_escape_owner(self.window, self.mounted).is_some_and(|entry| entry.id == self.id)
    }

    fn dialog_offset(self, level: u8) -> i16 {
        if let Some(mounted) = self.mounted {
            let _ = *mounted.order_revision.read();
        }
        OPEN_LEVELS.with_borrow(|open| {
            let peers = open.iter().rev().filter(|entry| {
                entry.window == self.window
                    && entry.level == level
                    && matches!(entry.scope, EscapeScope::Dialog(..))
            });
            let other_peers = (peers.clone().count().saturating_sub(1)).max(1) as i16;
            let above = peers.take_while(|entry| entry.id != self.id).count() as i16;
            let step =
                DIALOG_LAYER_BAND.min((OVERLAY_LEVEL_GAP - DIALOG_LAYER_BAND - 2) / other_peers);
            -DIALOG_LAYER_BAND - above * step
        })
    }
}

fn use_overlay_level_id(level: u8, scope: Option<EscapeScope>) -> EscapeOwner {
    let window = Platform::get().current_window().id;
    let mounted = try_consume_root_context::<MountedDialog>();
    let id = use_hook(|| {
        NEXT_OVERLAY_ID.with(|next| {
            let id = next.get();
            next.set(id.wrapping_add(1));
            id
        })
    });
    let held = use_hook(|| Rc::new(Cell::new(None::<(u8, EscapeScope)>)));
    let next = scope.map(|scope| (level, scope));
    if held.replace(next) != next {
        let removed_dialog = release_open_level(id);
        if let Some((level, scope)) = next {
            OPEN_LEVELS.with_borrow_mut(|open| {
                open.push(OpenLevel {
                    window,
                    id,
                    level,
                    scope,
                })
            });
        }
        if (removed_dialog || matches!(scope, Some(EscapeScope::Dialog(..))))
            && let Some(mounted) = mounted
        {
            mounted.order_changed();
        }
    }
    use_drop(move || {
        if release_open_level(id)
            && let Some(mounted) = mounted
        {
            mounted.order_changed();
        }
    });
    EscapeOwner {
        window,
        id,
        mounted,
    }
}

fn release_open_level(id: u64) -> bool {
    OPEN_LEVELS.with_borrow_mut(|open| {
        let Some(index) = open.iter().position(|entry| entry.id == id) else {
            return false;
        };
        matches!(open.remove(index).scope, EscapeScope::Dialog(..))
    })
}

fn top_escape_owner(window: u64, mounted: Option<MountedDialog>) -> Option<OpenLevel> {
    OPEN_LEVELS.with_borrow(|open| {
        let entries = open.iter().filter(|entry| entry.window == window);
        let dialog = mounted.map_or_else(
            || {
                entries
                    .clone()
                    .filter(|entry| matches!(entry.scope, EscapeScope::Dialog(..)))
                    .max_by_key(|entry| entry.level)
                    .and_then(|entry| match entry.scope {
                        EscapeScope::Dialog(id, _) => Some(id),
                        _ => None,
                    })
            },
            |mounted| *mounted.active.peek(),
        );
        entries
            .filter(|entry| match entry.scope {
                EscapeScope::Independent => true,
                EscapeScope::Dialog(id, _) => Some(id) == dialog,
                EscapeScope::Transient(parent) => parent == dialog,
            })
            .max_by_key(|entry| entry.level)
            .copied()
    })
}

#[derive(PartialEq)]
pub struct OverlayPopup {
    children: Vec<Element>,
    on_close: Option<EventHandler<()>>,
    position: Position,
    backdrop: bool,
    overlay_level: u8,
    key: DiffKey,
}

impl Default for OverlayPopup {
    fn default() -> Self {
        Self::new()
    }
}

impl OverlayPopup {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            on_close: None,
            position: Position::new_global().top(0.).left(0.),
            backdrop: true,
            overlay_level: OVERLAY_BASE_LEVEL,
            key: DiffKey::None,
        }
    }

    pub fn position(mut self, position: Position) -> Self {
        self.position = position;
        self
    }

    pub fn overlay_level(mut self, level: u8) -> Self {
        debug_assert!(
            (1..=OVERLAY_MAX_LEVEL).contains(&level),
            "overlay levels above {OVERLAY_MAX_LEVEL} saturate onto {OVERLAY_MAX_LEVEL}"
        );
        self.overlay_level = level;
        self
    }

    pub fn backdrop(mut self, backdrop: bool) -> Self {
        self.backdrop = backdrop;
        self
    }

    pub fn on_close(mut self, on_close: impl Into<EventHandler<()>>) -> Self {
        self.on_close = Some(on_close.into());
        self
    }
}

impl ChildrenExt for OverlayPopup {
    fn children_mut(&mut self) -> &mut Vec<Element> {
        &mut self.children
    }
}

impl KeyExt for OverlayPopup {
    fn key_mut(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for OverlayPopup {
    fn render_key(&self) -> DiffKey {
        self.key.clone()
    }

    fn render(&self) -> impl IntoElement {
        use_overlay_claim();

        let a11y_id = use_a11y();
        let window = Platform::get().current_window();
        let previous_focus = use_hook(|| *window.focused_accessibility_id.peek());
        use_provide_context(|| ContainingDialog(a11y_id));
        let scrim_close = self.on_close.clone();
        let key_close = self.on_close.clone();
        let level = self.overlay_level;

        let escape =
            use_overlay_level_id(level, Some(EscapeScope::Dialog(a11y_id, previous_focus)));
        let offset = escape.dialog_offset(level);

        let fade = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            AnimNum::new(0., 1.).time(180).ease(Ease::Out)
        });

        let scrim_alpha = if self.backdrop {
            if colors::active_theme().liquid_glass {
                48.
            } else {
                SCRIM_ALPHA
            }
        } else {
            0.
        };
        let alpha = ((fade.read().value() * scrim_alpha) as u8).max(SCRIM_MIN_ALPHA);

        rect()
            .layer(Layer::OverlayLevel(level))
            .position(Position::new_global().top(0.).left(0.))
            .width(Size::px(0.))
            .height(Size::px(0.))
            .child(
                rect()
                    .position(Position::new_global().top(0.).left(0.))
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .layer(Layer::Relative(offset - 1))
                    .background(Color::from_argb(alpha, 0, 0, 0))
                    .on_all_press(move |e: Event<PressEventData>| {
                        e.stop_propagation();
                        if let Some(on_close) = scrim_close.as_ref() {
                            on_close.call(());
                        }
                    }),
            )
            .child(
                rect()
                    .position(self.position.clone())
                    .a11y_id(a11y_id)
                    .a11y_focusable(true)
                    .a11y_auto_focus(escape.mounted.is_none())
                    .a11y_role(AccessibilityRole::Dialog)
                    .a11y_builder(|node| node.set_modal())
                    .layer(Layer::Relative(offset))
                    .on_pointer_press(|e: Event<PointerEventData>| e.stop_propagation())
                    .on_global_key_down(move |e: Event<KeyboardEventData>| {
                        if e.key == Key::Named(NamedKey::Escape)
                            && escape.is_topmost()
                            && let Some(on_close) = key_close.as_ref()
                        {
                            if crate::ui::is_text_entry(
                                window.focused_accessibility_node.peek().role(),
                            ) {
                                a11y_id.request_focus();
                            } else {
                                on_close.call(());
                            }
                        }
                    })
                    .children(self.children.clone()),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn newest_equal_level_dialog_owns_pointer_and_escape_after_reopening() {
        use freya_testing::TestingRunner;

        #[derive(Clone, Default)]
        struct Calls {
            older: Rc<Cell<bool>>,
            newer: Rc<Cell<bool>>,
            closed: Rc<Cell<Option<u8>>>,
        }

        fn app() -> impl IntoElement {
            let calls = use_consume::<Calls>();
            let generation = use_consume::<State<u64>>();
            let button = |called: Rc<Cell<bool>>| {
                rect()
                    .width(Size::px(100.))
                    .height(Size::px(60.))
                    .background(Color::WHITE)
                    .on_press(move |_| called.set(true))
            };
            let mut older = button(calls.older).into_element();
            for _ in 0..12 {
                older = rect().child(older).into_element();
            }
            let closed_older = calls.closed.clone();
            rect()
                .child(
                    OverlayPopup::new()
                        .key(*generation.read())
                        .overlay_level(OVERLAY_MAX_LEVEL)
                        .on_close(move |_| closed_older.set(Some(1)))
                        .child(older),
                )
                .child(
                    OverlayPopup::new()
                        .key(100u64)
                        .overlay_level(OVERLAY_MAX_LEVEL)
                        .on_close(move |_| calls.closed.set(Some(2)))
                        .child(button(calls.newer)),
                )
        }

        fn sync_active() -> [AccessibilityId; 2] {
            let window = Platform::get().current_window().id;
            let [first, second]: [(AccessibilityId, AccessibilityId); 2] =
                mounted_dialogs(window).try_into().unwrap();
            let mut active = consume_root_context::<MountedDialog>().active;
            active.set(Some(second.0));
            [first.0, second.0]
        }

        let (mut runner, (calls, mut generation)) = TestingRunner::new(
            app,
            (240., 160.).into(),
            |runner| {
                runner.provide_root_context(MountedDialog::new);
                (
                    runner.provide_root_context(Calls::default),
                    runner.provide_root_context(|| State::create(0u64)),
                )
            },
            1.,
        );
        runner.poll_n(Duration::ZERO, 5);
        let [older, newer] = runner.run_in(sync_active);
        runner.click_cursor((12., 12.));
        assert!(calls.newer.get());
        assert!(!calls.older.get());
        runner.press_key(Key::Named(NamedKey::Escape));
        assert_eq!(calls.closed.get(), Some(2));

        calls.newer.set(false);
        runner.run_in(|| generation.set(1));
        runner.poll_n(Duration::ZERO, 5);
        let [remaining, reopened] = runner.run_in(sync_active);
        assert_eq!(remaining, newer);
        assert_ne!(reopened, older);
        runner.click_cursor((12., 12.));
        assert!(calls.older.get());
        assert!(!calls.newer.get());
        runner.press_key(Key::Named(NamedKey::Escape));
        assert_eq!(calls.closed.get(), Some(1));
    }

    #[test]
    fn escape_closes_a_dropdown_before_its_dialog() {
        use freya_testing::TestingRunner;

        fn app() -> impl IntoElement {
            crate::hooks::use_provide_overlay_claims();
            let mut open = use_consume::<State<bool>>();
            rect().maybe_child(open().then(|| {
                OverlayPopup::new()
                    .overlay_level(7)
                    .on_close(move |()| open.set(false))
                    .child(crate::components::Dropdown::new(
                        "Choices",
                        vec!["One".into()],
                    ))
            }))
        }

        let (mut runner, open) = TestingRunner::new(
            app,
            (240., 160.).into(),
            |runner| runner.provide_root_context(|| State::create(true)),
            1.,
        );
        runner.poll_n(Duration::ZERO, 5);
        runner.click_cursor((32., 12.));
        runner.run_in(|| assert!(!overlay_is_topmost(7), "the dropdown should own Escape"));
        runner.press_key(Key::Named(NamedKey::Escape));
        runner.poll_n(Duration::ZERO, 3);
        runner.run_in(|| assert!(*open.peek(), "Escape should only close the dropdown"));
        runner.run_in(|| assert!(overlay_is_topmost(7)));
        runner.press_key(Key::Named(NamedKey::Escape));
        runner.poll_n(Duration::ZERO, 3);
        runner.run_in(|| assert!(!*open.peek()));
    }
}
