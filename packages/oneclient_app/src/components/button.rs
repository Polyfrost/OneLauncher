#![allow(dead_code)]
use std::borrow::Cow;

use freya::prelude::*;

use crate::{
    components::{Icon, IconTint, IconType, TooltipPlacement, tooltip::use_tooltip_anchor},
    theme::colors,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Danger,
    Ghost,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonSize {
    Small,
    #[default]
    Medium,
    Large,
    Icon,
}

const DISABLED_OPACITY: f32 = 0.5;

struct ButtonColors {
    background: Color,
    hover_background: Color,
    pressed_background: Color,
    disabled_background: Color,
    foreground: Color,
    disabled_foreground: Color,
    border: Option<Color>,
    hover_border: Option<Color>,
    pressed_border: Option<Color>,
    focus_border: Option<Color>,
}

#[derive(Clone, PartialEq)]
pub struct Button {
    size: ButtonSize,
    variant: ButtonVariant,

    layout: LayoutData,
    style: StyleState,
    text_style: TextStyleData,

    enabled: bool,
    focusable: bool,
    alt: Option<Box<str>>,
    tooltip: Option<Box<str>>,
    tooltip_placement: TooltipPlacement,
    on_press: Option<EventHandler<Event<PressEventData>>>,
    overflow: Overflow,

    elements: Vec<Element>,
    key: DiffKey,
    cursor_icon: CursorIcon,
}

impl Default for Button {
    fn default() -> Self {
        Self::new()
    }
}

impl Button {
    pub fn new() -> Self {
        let size = ButtonSize::default();
        let variant = ButtonVariant::default();

        let mut layout = LayoutData::default();
        let mut style = StyleState::default();
        let mut text_style = TextStyleData::default();

        let (padding, default_radius, font_size, width, height) = size_layout(size);
        let colors = variant_colors(variant);

        layout.padding = padding;
        layout.width = width;
        layout.height = height;

        layout.content = Content::Flex;
        layout.cross_alignment = Alignment::Center;
        layout.main_alignment = Alignment::Center;
        layout.direction = Direction::Horizontal;
        layout.spacing.0 = 4.;

        style.background = Fill::Color(colors.background);
        if let Some(border_color) = colors.border {
            style.borders.push(
                Border::new()
                    .fill(border_color)
                    .width(1.)
                    .alignment(BorderAlignment::Inner),
            );
        }

        style.corner_radius = default_radius;

        text_style.font_size = Some(font_size.into());

        Self {
            size,
            layout,
            style,
            text_style,
            variant,
            enabled: true,
            focusable: true,
            alt: None,
            tooltip: None,
            tooltip_placement: TooltipPlacement::default(),
            on_press: None,
            elements: Vec::new(),
            overflow: Overflow::Clip,
            key: DiffKey::None,
            cursor_icon: CursorIcon::Pointer,
        }
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn primary(self) -> Self {
        self.variant(ButtonVariant::Primary)
    }

    pub fn secondary(self) -> Self {
        self.variant(ButtonVariant::Secondary)
    }

    pub fn danger(self) -> Self {
        self.variant(ButtonVariant::Danger)
    }

    pub fn ghost(self) -> Self {
        self.variant(ButtonVariant::Ghost)
    }

    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;

        let (padding, corner_radius, font_size, width, height) = size_layout(size);
        self.layout.width = width;
        self.layout.height = height;
        self.layout.padding = padding;
        self.style.corner_radius = corner_radius;
        self.text_style.font_size = Some(font_size.into());

        self
    }

    pub fn overflow(mut self, overflow: Overflow) -> Self {
        self.overflow = overflow;
        self
    }

    pub fn font_size(mut self, font_size: impl Into<f32>) -> Self {
        self.text_style.font_size = Some(font_size.into().into());
        self
    }

    pub fn font_weight(mut self, weight: FontWeight) -> Self {
        self.text_style.font_weight = Some(weight);
        self
    }

    pub fn small(self) -> Self {
        self.size(ButtonSize::Small)
    }

    pub fn medium(self) -> Self {
        self.size(ButtonSize::Medium)
    }

    pub fn large(self) -> Self {
        self.size(ButtonSize::Large)
    }

    pub fn icon(self) -> Self {
        self.size(ButtonSize::Icon)
    }

    pub fn enabled(mut self, enabled: impl Into<bool>) -> Self {
        self.enabled = enabled.into();
        self
    }

    pub fn focusable(mut self, focusable: impl Into<bool>) -> Self {
        self.focusable = focusable.into();
        self
    }

    pub fn disabled(self, disabled: bool) -> Self {
        self.enabled(!disabled)
    }

    pub fn on_press(mut self, on_press: impl Into<EventHandler<Event<PressEventData>>>) -> Self {
        self.on_press = Some(on_press.into());
        self
    }

    /// Accessible name for icon-only buttons which have no label to read out
    pub fn alt(mut self, alt: impl Into<Box<str>>) -> Self {
        self.alt = Some(alt.into());
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<Box<str>>) -> Self {
        let tooltip = tooltip.into();
        if self.alt.is_none() {
            self.alt = Some(tooltip.clone());
        }
        self.tooltip = Some(tooltip);
        self
    }

    pub fn tooltip_placement(mut self, placement: TooltipPlacement) -> Self {
        self.tooltip_placement = placement;
        self
    }

    pub fn cursor_icon(mut self, cursor_icon: CursorIcon) -> Self {
        self.cursor_icon = cursor_icon;
        self
    }

    pub fn text(self, text: impl Into<Cow<'static, str>>) -> Self {
        self.child(label().text(text))
    }
}

impl ChildrenExt for Button {
    fn get_children(&mut self) -> &mut Vec<Element> {
        &mut self.elements
    }
}

impl KeyExt for Button {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl LayoutExt for Button {
    fn get_layout(&mut self) -> &mut LayoutData {
        &mut self.layout
    }
}

impl ContainerExt for Button {}

impl StyleExt for Button {
    fn get_style(&mut self) -> &mut StyleState {
        &mut self.style
    }
}

impl Component for Button {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);
        let mut pressing = use_state(|| false);

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);

        let tooltip = use_tooltip_anchor(self.tooltip.clone(), self.tooltip_placement);

        let enabled = use_reactive(&self.enabled);

        let cursor_icon = self.cursor_icon;

        let palette = variant_colors(self.variant);

        let background = if !enabled() {
            palette.disabled_background
        } else if pressing() {
            palette.pressed_background
        } else if hovering() {
            palette.hover_background
        } else {
            palette.background
        };

        let foreground = if enabled() {
            palette.foreground
        } else {
            palette.disabled_foreground
        };

        // Child icons would otherwise inherit this through a `styled` event that lands a couple of
        // frames after the button paints, so hand it straight to them instead
        let mut icon_tint = use_state(|| foreground);
        icon_tint.set_if_modified(foreground);
        use_hook(|| provide_context(IconTint(icon_tint)));

        let border_color = if !enabled() {
            palette.border
        } else if pressing() {
            palette.pressed_border
        } else if hovering() {
            palette.hover_border
        } else if focus().is_focused() {
            palette.focus_border.or(Some(Color::WHITE))
        } else {
            palette.border
        };

        let on_press = self.on_press.clone();

        let mut rect = rect()
            .a11y_id(a11y_id)
            .a11y_focusable(enabled() && self.focusable)
            .a11y_role(AccessibilityRole::Button)
            .map(self.alt.clone(), |rect, alt| rect.a11y_alt(alt))
            .overflow(self.overflow)
            .layout(self.layout.clone())
            .text_style(self.text_style.clone())
            .children(self.elements.clone());

        rect.get_style().clone_from(&self.style);

        rect = rect.background(background).color(foreground);

        if !enabled() {
            rect = rect.opacity(DISABLED_OPACITY);
        }

        if let Some(border) = border_color {
            rect = rect.border(
                Border::new()
                    .fill(border)
                    .width(1.)
                    .alignment(BorderAlignment::Inner),
            );
        }

        if enabled() {
            rect = rect
                .on_pointer_down(move |_| pressing.set(true))
                .on_global_pointer_press(move |_| pressing.set(false))
                .on_pointer_over(move |_| hovering.set(true))
                .on_pointer_out(move |_| {
                    hovering.set(false);
                    pressing.set(false);
                })
                .cursor(cursor_icon)
                .map(on_press.clone(), |rect, handler| {
                    rect.on_press(move |event: Event<PressEventData>| {
                        handler.call(event);
                    })
                });
        } else {
            rect = rect.cursor(CursorIcon::NotAllowed);
        }

        tooltip.attach(rect)
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

fn variant_colors(variant: ButtonVariant) -> ButtonColors {
    match variant {
        ButtonVariant::Primary => ButtonColors {
            background: colors::brand(),
            hover_background: colors::brand_hover(),
            pressed_background: colors::brand_pressed(),
            disabled_background: colors::brand_disabled(),
            foreground: colors::fg_primary(),
            disabled_foreground: colors::fg_primary_disabled(),
            border: None,
            hover_border: None,
            pressed_border: None,
            focus_border: None,
        },
        ButtonVariant::Secondary => ButtonColors {
            background: colors::component_bg(),
            hover_background: colors::component_bg_hover(),
            pressed_background: colors::component_bg_pressed(),
            disabled_background: colors::component_bg_disabled(),
            foreground: colors::fg_primary(),
            disabled_foreground: colors::fg_primary_disabled(),
            border: Some(colors::component_border()),
            hover_border: Some(colors::component_border_hover()),
            pressed_border: Some(colors::component_border_pressed()),
            focus_border: Some(colors::brand()),
        },
        ButtonVariant::Danger => ButtonColors {
            background: colors::danger(),
            hover_background: colors::danger_hover(),
            pressed_background: colors::danger_pressed(),
            disabled_background: colors::danger_disabled(),
            foreground: colors::fg_primary(),
            disabled_foreground: colors::fg_primary_disabled(),
            border: None,
            hover_border: None,
            pressed_border: None,
            focus_border: None,
        },
        ButtonVariant::Ghost => ButtonColors {
            background: Color::TRANSPARENT,
            hover_background: colors::ghost_overlay_hover(),
            pressed_background: colors::ghost_overlay_pressed(),
            disabled_background: Color::TRANSPARENT,
            foreground: colors::fg_primary(),
            disabled_foreground: colors::fg_primary_disabled(),
            border: None,
            hover_border: None,
            pressed_border: None,
            focus_border: None,
        },
    }
}

fn size_layout(size: ButtonSize) -> (Gaps, CornerRadius, f32, Size, Size) {
    match size {
        ButtonSize::Small => (
            Gaps::new_symmetric(4., 8.),
            CornerRadius::new_all(6.),
            12.,
            Size::auto(),
            Size::auto(),
        ),
        ButtonSize::Medium => (
            Gaps::new_symmetric(6., 12.),
            CornerRadius::new_all(8.),
            14.,
            Size::auto(),
            Size::auto(),
        ),
        ButtonSize::Large => (
            Gaps::new_symmetric(8., 32.),
            CornerRadius::new_all(10.),
            18.,
            Size::auto(),
            Size::auto(),
        ),
        ButtonSize::Icon => (
            Gaps::new_all(6.),
            CornerRadius::new_all(8.),
            14.,
            Size::px(32.),
            Size::px(32.),
        ),
    }
}

pub fn link_button() -> Button {
    Button::new()
        .primary()
        .child(
            Icon::new(IconType::LinkExternal01)
                .size(14.)
                .color(colors::fg_primary()),
        )
        .child(label().text("Open"))
}

pub fn open_folder_button(folder: std::path::PathBuf) -> Button {
    Button::new()
        .secondary()
        .icon()
        .tooltip("Open folder")
        .on_press(move |_| {
            std::fs::create_dir_all(&folder).ok();
            crate::platform::open_path(&folder.to_string_lossy());
        })
        .child(Icon::new(IconType::Folder).size(16.))
}
