use freya::{
    animation::{AnimNum, Ease, OnCreation, use_animation},
    prelude::*,
};

const SCRIM_ALPHA: f32 = 90.;

/// Overlay level of a top level popup. Its scrim sits two levels below it.
const BASE_LEVEL: u8 = 12;

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
            overlay_level: BASE_LEVEL,
            key: DiffKey::None,
        }
    }

    pub fn position(mut self, position: Position) -> Self {
        self.position = position;
        self
    }

    // has to be raised on top of the ss viewer (previously context menu and image viewer were at the same level)
    pub fn overlay_level(mut self, level: u8) -> Self {
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
                    .layer(Layer::OverlayLevel(self.overlay_level.saturating_sub(2)))
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
                    .layer(Layer::OverlayLevel(self.overlay_level))
                    .on_global_key_down(move |e: Event<KeyboardEventData>| {
                        if e.key == Key::Named(NamedKey::Escape)
                            && let Some(on_close) = key_close.as_ref()
                        {
                            on_close.call(());
                        }
                    })
                    .children(self.children.clone()),
            )
    }
}
