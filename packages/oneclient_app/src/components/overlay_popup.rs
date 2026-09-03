use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use freya::{
    animation::{AnimNum, Ease, OnCreation, use_animation},
    prelude::*,
};

const SCRIM_ALPHA: f32 = 90.;

const CONTENT_LEVEL: u8 = 12;
const SCRIM_OFFSET: u8 = 1;

static NEXT_OVERLAY: AtomicU64 = AtomicU64::new(0);
static MOUNTED: Mutex<Vec<(u64, u8)>> = Mutex::new(Vec::new());

fn register(level: u8) -> u64 {
    let id = NEXT_OVERLAY.fetch_add(1, Ordering::Relaxed);

    if let Ok(mut mounted) = MOUNTED.lock() {
        mounted.push((id, level));
    }

    id
}

fn unregister(id: u64) {
    if let Ok(mut mounted) = MOUNTED.lock() {
        mounted.retain(|(other, _)| *other != id);
    }
}

fn is_topmost(id: u64) -> bool {
    let Ok(mounted) = MOUNTED.lock() else {
        return true;
    };

    mounted
        .iter()
        .max_by_key(|(other, level)| (*level, *other))
        .is_none_or(|(top, _)| *top == id)
}

#[derive(PartialEq)]
pub struct OverlayPopup {
    children: Vec<Element>,
    on_close: Option<EventHandler<()>>,
    position: Position,
    backdrop: bool,
    level: u8,
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
            level: CONTENT_LEVEL,
            key: DiffKey::None,
        }
    }

    pub fn position(mut self, position: Position) -> Self {
        self.position = position;
        self
    }

    pub fn level(mut self, level: u8) -> Self {
        self.level = level;
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
    fn get_children(&mut self) -> &mut Vec<Element> {
        &mut self.children
    }
}

impl KeyExt for OverlayPopup {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for OverlayPopup {
    fn render(&self) -> impl IntoElement {
        let a11y_id = use_a11y();
        let scrim_close = self.on_close.clone();
        let key_close = self.on_close.clone();

        let level = self.level;
        let id = use_hook(move || register(level));
        use_drop(move || unregister(id));

        let fade = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            AnimNum::new(0., 1.).time(180).ease(Ease::Out)
        });

        let scrim_alpha = if self.backdrop { SCRIM_ALPHA } else { 0. };
        let alpha = (fade.read().value() * scrim_alpha) as u8;

        rect()
            .layer(Layer::Overlay)
            .position(Position::new_global().top(0.).left(0.))
            .width(Size::window_percent(100.))
            .height(Size::window_percent(100.))
            .child(
                rect()
                    .position(Position::new_global().top(0.).left(0.))
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .layer(Layer::OverlayLevel(self.level.saturating_sub(SCRIM_OFFSET)))
                    .background(Color::from_argb(alpha, 0, 0, 0))
                    .on_press(move |_| {
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
                    .a11y_auto_focus(true)
                    .a11y_role(AccessibilityRole::Dialog)
                    .layer(Layer::OverlayLevel(self.level))
                    .on_global_key_down(move |e: Event<KeyboardEventData>| {
                        if e.key == Key::Named(NamedKey::Escape)
                            && is_topmost(id)
                            && let Some(on_close) = key_close.as_ref()
                        {
                            on_close.call(());
                        }
                    })
                    .children(self.children.clone()),
            )
    }
}
