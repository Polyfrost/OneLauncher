use freya::prelude::*;

use crate::components::{
    Button, ContextMenu, Icon, IconType, OverlayPopup, PlayerModel, TextInput, TooltipPlacement,
    use_tooltip_anchor,
};
use freya::elements::image::{AspectRatio, ImageHandle, image};
use freya::engine::prelude::{SkData, SkImage};
use oneclient_polyplus::api::{BodySlot, SwatchArt};

use crate::AppAssets;
use crate::hooks::wardrobe::{use_cosmetic_bundle, use_swatch};
use crate::hooks::{loaded_image, settled_or_loading, use_cached_image};
use crate::theme::colors;
use crate::ui::{border_all_color, clamp_to_window};

pub(super) const CARD_CHROME: f32 = 81.;
pub(super) const CAPE_ZOOM: f32 = 2.1;
pub(super) const GRID_GAP: f32 = 12.;
pub(super) const COLUMNS: usize = 4;
pub(super) const BACK_YAW: f32 = std::f32::consts::PI - 0.45;
pub(super) const FRONT_YAW: f32 = 0.45;

fn hairline(color: Color) -> Border {
    border_all_color(1., color)
}

fn global_point(event: &Event<PressEventData>) -> Option<(f32, f32)> {
    match event.data() {
        PressEventData::Mouse(mouse) => Some((
            mouse.global_location.x as f32,
            mouse.global_location.y as f32,
        )),
        _ => None,
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum CapeSource {
    Mojang(String),
    Plus(String),
}

#[derive(Clone, PartialEq)]
pub(super) struct MenuAction {
    pub icon: IconType,
    pub label: String,
    pub enabled: bool,
    pub danger: bool,
    pub on_select: EventHandler<()>,
}

impl MenuAction {
    pub fn new(
        icon: IconType,
        label: impl Into<String>,
        enabled: bool,
        on_select: impl FnMut(()) + 'static,
    ) -> Self {
        Self {
            icon,
            label: label.into(),
            enabled,
            danger: false,
            on_select: EventHandler::new(on_select),
        }
    }

    pub fn danger(
        icon: IconType,
        label: impl Into<String>,
        enabled: bool,
        on_select: impl FnMut(()) + 'static,
    ) -> Self {
        Self {
            danger: true,
            ..Self::new(icon, label, enabled, on_select)
        }
    }
}

#[derive(Clone, PartialEq)]
pub(super) struct Rename {
    pub current: String,
    pub on_save: EventHandler<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum BadgeStyle {
    New,
    Sale,
    Accent,
}

#[derive(Clone, PartialEq)]
pub(super) struct Badge {
    pub label: String,
    pub style: BadgeStyle,
    pub icon: Option<IconType>,
}

impl Badge {
    pub fn favorite() -> Self {
        Self::text("FAVOURITE", BadgeStyle::Accent)
    }

    pub fn text(label: impl Into<String>, style: BadgeStyle) -> Self {
        Self {
            label: label.into(),
            style,
            icon: None,
        }
    }
}

fn badge_tab(badge: &Badge) -> Element {
    let (background, color) = match badge.style {
        BadgeStyle::New => (colors::success(), colors::fg_primary()),
        BadgeStyle::Sale => (colors::fg_primary(), colors::page()),
        BadgeStyle::Accent => (colors::brand(), colors::fg_primary()),
    };
    rect()
        .horizontal()
        .min_width(Size::px(if badge.label.is_empty() { 40. } else { 81. }))
        .height(Size::px(21.))
        .padding(Gaps::new_symmetric(0., 10.))
        .center()
        .spacing(4.)
        .corner_radius(CornerRadius::new(0., 0., 7., 7.))
        .background(background)
        .maybe_child(
            badge
                .icon
                .map(|icon| Icon::new(icon).size(12.).color(color)),
        )
        .maybe_child((!badge.label.is_empty()).then(|| {
            label()
                .text(badge.label.clone())
                .max_lines(1)
                .font_size(11.)
                .font_weight(FontWeight::MEDIUM)
                .color(color)
        }))
        .into_element()
}

pub(super) fn detail_text(text: impl Into<String>) -> Element {
    label()
        .text(text.into())
        .width(Size::fill())
        .max_lines(1)
        .text_overflow(TextOverflow::Ellipsis)
        .font_size(12.)
        .color(colors::fg_secondary())
        .into_element()
}

pub(super) fn money(amount: f32) -> String {
    format!("${amount:.2}")
}

pub(super) fn price_label(
    base: Option<f32>,
    final_price: Option<f32>,
    discounted: bool,
) -> Element {
    let row = rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(4.);
    let row = match final_price {
        None => row.child(
            label()
                .text("—")
                .font_size(14.)
                .color(colors::fg_secondary()),
        ),
        Some(price) if price <= 0. => row.child(
            label()
                .text("FREE")
                .font_size(14.)
                .font_weight(FontWeight::MEDIUM)
                .color(colors::success()),
        ),
        Some(price) if discounted && base.is_some() => row
            .child(
                label()
                    .text(money(base.unwrap_or_default()))
                    .font_size(10.)
                    .text_decoration(TextDecoration::LineThrough)
                    .color(colors::fg_secondary()),
            )
            .child(
                label()
                    .text(money(price))
                    .font_size(14.)
                    .font_weight(FontWeight::MEDIUM)
                    .color(colors::success()),
            ),
        Some(price) => row.child(
            label()
                .text(money(price))
                .font_size(14.)
                .font_weight(FontWeight::MEDIUM)
                .color(colors::fg_primary()),
        ),
    };
    row.into_element()
}

#[derive(Clone, PartialEq)]
pub(super) struct Swatch {
    pub id: i32,
    pub name: String,
    pub texture: Option<String>,
    pub cape: bool,
    pub selected: bool,
}

#[derive(PartialEq)]
struct VariantSwatch {
    swatch: Swatch,
    on_pick: EventHandler<i32>,
}

impl Component for VariantSwatch {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let tooltip = use_tooltip_anchor(
            Some(self.swatch.name.clone().into_boxed_str()),
            TooltipPlacement::Left,
        );
        let query = use_swatch(self.swatch.texture.clone(), self.swatch.cape);
        let art = settled_or_loading(&query);
        let mut cache = use_state(|| None::<(usize, ImageHandle)>);

        let art_element = match art {
            Some(SwatchArt::Picture(bytes)) => {
                let ptr = bytes.as_ptr() as usize;
                let cached = cache.peek().clone();
                let handle = match cached {
                    Some((known, handle)) if known == ptr => Some(handle),
                    _ => {
                        let data = unsafe { SkData::new_bytes(&bytes) };
                        let decoded = SkImage::from_encoded(data)
                            .map(|img| ImageHandle::new(img, bytes.clone()));
                        if let Some(handle) = &decoded {
                            cache.set(Some((ptr, handle.clone())));
                        }
                        decoded
                    }
                };
                handle.map(|handle| {
                    image(handle)
                        .width(Size::fill())
                        .height(Size::fill())
                        .aspect_ratio(AspectRatio::None)
                        .sampling_mode(SamplingMode::Nearest)
                        .into_element()
                })
            }
            Some(SwatchArt::Stripes(colors)) => Some(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .height(Size::fill())
                    .content(Content::Flex)
                    .children(colors.into_iter().map(|[r, g, b]| {
                        rect()
                            .width(Size::fill())
                            .height(Size::flex(1.0))
                            .background(Color::from_rgb(r, g, b))
                            .into_element()
                    }))
                    .into_element(),
            ),
            None => None,
        };

        let border = if self.swatch.selected {
            colors::brand()
        } else if *hovered.read() {
            colors::component_border_hover()
        } else {
            colors::component_border()
        };
        let id = self.swatch.id;
        let on_pick = self.on_pick.clone();

        tooltip.attach(
            rect()
                .width(Size::px(14.))
                .height(Size::px(22.))
                .corner_radius(CornerRadius::new_all(5.))
                .background(colors::component_bg())
                .overflow(Overflow::Clip)
                .cursor(CursorIcon::Pointer)
                .on_pointer_enter(move |_| hovered.set(true))
                .on_pointer_leave(move |_| hovered.set(false))
                .on_press(move |e: Event<PressEventData>| {
                    e.stop_propagation();
                    on_pick.call(id);
                })
                .maybe_child(art_element)
                .child(
                    rect()
                        .position(Position::new_absolute().top(0.).left(0.))
                        .layer(Layer::Relative(1))
                        .width(Size::fill())
                        .height(Size::fill())
                        .corner_radius(CornerRadius::new_all(5.))
                        .border(hairline(border))
                        .interactive(false),
                ),
        )
    }
}

const MAX_VISIBLE_SWATCHES: usize = 4;

fn swatch_column(swatches: &[Swatch], on_pick: EventHandler<i32>) -> Element {
    let hidden = swatches.len().saturating_sub(MAX_VISIBLE_SWATCHES);
    rect()
        .vertical()
        .cross_align(Alignment::End)
        .spacing(4.)
        .children(swatches.iter().take(MAX_VISIBLE_SWATCHES).map(|swatch| {
            VariantSwatch {
                swatch: swatch.clone(),
                on_pick: on_pick.clone(),
            }
            .into_element()
        }))
        .maybe_child((hidden > 0).then(|| {
            rect()
                .min_width(Size::px(16.))
                .height(Size::px(16.))
                .padding(Gaps::new_symmetric(0., 2.))
                .center()
                .corner_radius(CornerRadius::new_all(5.))
                .background(colors::component_bg())
                .border(hairline(colors::component_border()))
                .child(
                    label()
                        .text(format!("+{hidden}"))
                        .font_size(10.)
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_primary()),
                )
        }))
        .into_element()
}

#[derive(PartialEq)]
pub(super) struct WardrobeCard {
    pub name: String,
    pub detail: Element,
    pub action_label: &'static str,
    pub action_icon: IconType,
    pub equipped: bool,
    pub action_enabled: bool,
    pub selected: bool,
    pub badge: Option<Badge>,
    pub thumbnail: Element,
    pub menu: Vec<MenuAction>,
    pub rename: Option<Rename>,
    pub enabled: bool,
    pub swatches: Vec<Swatch>,
    pub on_pick: Option<EventHandler<i32>>,
    pub on_select: EventHandler<()>,
    pub on_action: EventHandler<()>,
}

impl Component for WardrobeCard {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let mut strip_hovered = use_state(|| false);
        let mut bounds = use_state(|| (0f32, 0f32, 0f32, 0f32));
        let mut menu_at = use_state(|| None::<(f32, f32)>);
        let mut rename_at = use_state(|| None::<(f32, f32)>);

        let hover = *hovered.read();
        let menu_open = menu_at.read().is_some() || rename_at.read().is_some();
        let enabled = self.enabled;
        let outline = if self.equipped || self.selected {
            colors::brand()
        } else if hover && enabled {
            colors::component_border_hover()
        } else {
            colors::component_border()
        };

        let strip_color = colors::fg_primary();
        let strip_line = if *strip_hovered.read() && self.action_enabled {
            colors::component_border_hover()
        } else {
            colors::component_border()
        };

        let on_select = self.on_select.clone();
        let on_action = self.on_action.clone();
        let action_enabled = self.action_enabled && enabled;
        let swatches = (hover && self.swatches.len() > 1)
            .then(|| self.on_pick.clone())
            .flatten()
            .map(|on_pick| {
                rect()
                    .position(Position::new_absolute().top(6.).right(6.))
                    .layer(Layer::Relative(1))
                    .child(swatch_column(&self.swatches, on_pick))
            });
        let has_menu = !self.menu.is_empty() || self.rename.is_some();

        let menu_button = (has_menu && (hover || menu_open) && self.enabled).then(|| {
            rect()
                .width(Size::px(22.))
                .height(Size::px(22.))
                .center()
                .corner_radius(CornerRadius::new_all(6.))
                .background(colors::component_bg())
                .border(hairline(colors::component_border()))
                .cursor(CursorIcon::Pointer)
                .on_press(move |e: Event<PressEventData>| {
                    e.stop_propagation();
                    if let Some((x, y)) = global_point(&e) {
                        menu_at.set(Some((x, y)));
                    }
                })
                .child(
                    Icon::new(IconType::DotsVertical)
                        .size(14.)
                        .color(colors::fg_primary()),
                )
        });

        let menu_point = *menu_at.read();
        let menu = menu_point.map(|(x, y)| {
            let mut entries: Vec<MenuAction> = self.menu.to_vec();
            if self.rename.is_some() {
                let index = entries.len().min(1);
                entries.insert(
                    index,
                    MenuAction::new(IconType::Pencil01, "Rename", true, move |()| {
                        rename_at.set(Some((x, y)));
                    }),
                );
            }
            entries
                .into_iter()
                .filter(|action| action.enabled)
                .fold(ContextMenu::new(x, y), |menu, action| {
                    if action.danger {
                        menu.danger_action(action.icon, action.label, action.on_select)
                    } else {
                        menu.action(action.icon, action.label, action.on_select)
                    }
                })
                .on_close(move |()| menu_at.set(None))
        });

        let rename_point = *rename_at.read();
        let rename_popup = rename_point.and_then(|(x, y)| {
            self.rename.clone().map(|rename| RenamePopup {
                x,
                y,
                current: rename.current,
                on_save: rename.on_save,
                on_close: EventHandler::new(move |()| rename_at.set(None)),
            })
        });

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .content(Content::Flex)
            .corner_radius(CornerRadius::new_all(12.))
            .background(colors::page_elevated())
            .border(hairline(outline))
            .overflow(Overflow::Clip)
            .opacity(if enabled { 1. } else { 0.5 })
            .cursor(if enabled {
                CursorIcon::Pointer
            } else {
                CursorIcon::Default
            })
            .on_sized(move |event: Event<SizedEventData>| {
                let area = event.data().area;
                let next = (area.min_x(), area.min_y(), area.max_x(), area.max_y());
                if *bounds.peek() != next {
                    bounds.set(next);
                }
            })
            .on_global_pointer_move(move |event: Event<PointerEventData>| {
                let point = event.global_location();
                let (left, top, right, bottom) = *bounds.peek();
                let inside = point.x as f32 >= left
                    && point.x as f32 <= right
                    && point.y as f32 >= top
                    && point.y as f32 <= bottom;
                if *hovered.peek() != inside {
                    hovered.set(inside);
                }
            })
            .on_press(move |_| {
                if enabled {
                    on_select.call(());
                }
            })
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .padding(Gaps::new(18., 18., 0., 18.))
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::fill())
                            .corner_radius(CornerRadius::new_all(5.))
                            .background(colors::page())
                            .border(hairline(colors::component_border()))
                            .overflow(Overflow::Clip)
                            .child(self.thumbnail.clone())
                            .maybe_child(swatches),
                    ),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .padding(Gaps::new(8., 18., 12., 18.))
                    .spacing(6.)
                    .child(
                        rect()
                            .horizontal()
                            .width(Size::fill())
                            .height(Size::px(18.))
                            .content(Content::Flex)
                            .cross_align(Alignment::Center)
                            .spacing(6.)
                            .child(
                                label()
                                    .text(self.name.clone())
                                    .width(Size::flex(1.0))
                                    .max_lines(1)
                                    .text_overflow(TextOverflow::Ellipsis)
                                    .font_size(14.)
                                    .font_weight(FontWeight::MEDIUM)
                                    .color(colors::fg_primary()),
                            )
                            .maybe_child(menu_button),
                    )
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::px(18.))
                            .main_align(Alignment::Center)
                            .child(self.detail.clone()),
                    ),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::px(1.))
                    .background(if self.equipped {
                        colors::brand()
                    } else {
                        strip_line
                    }),
            )
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .height(Size::px(35.))
                    .center()
                    .spacing(8.)
                    .background(if self.equipped {
                        colors::brand()
                    } else {
                        colors::component_bg()
                    })
                    .opacity(if action_enabled || self.equipped {
                        1.
                    } else {
                        0.5
                    })
                    .on_pointer_enter(move |_| strip_hovered.set(true))
                    .on_pointer_leave(move |_| strip_hovered.set(false))
                    .on_press(move |e: Event<PressEventData>| {
                        e.stop_propagation();
                        if action_enabled {
                            on_action.call(());
                        }
                    })
                    .child(Icon::new(self.action_icon).size(16.).color(strip_color))
                    .child(
                        label()
                            .text(self.action_label)
                            .font_size(14.)
                            .font_weight(FontWeight::MEDIUM)
                            .color(strip_color),
                    ),
            )
            .maybe_child(self.badge.as_ref().map(|badge| {
                rect()
                    .position(Position::new_absolute().top(0.).left(0.))
                    .layer(Layer::Relative(1))
                    .width(Size::fill())
                    .horizontal()
                    .main_align(Alignment::Center)
                    .child(badge_tab(badge))
            }))
            .maybe_child(menu.map(shield))
            .maybe_child(rename_popup.map(shield))
    }
}

fn shield(popup: impl IntoElement) -> Element {
    rect()
        .on_press(|event: Event<PressEventData>| event.stop_propagation())
        .child(popup)
        .into_element()
}

const RENAME_WIDTH: f32 = 240.;
const RENAME_HEIGHT: f32 = 112.;

#[derive(PartialEq)]
struct RenamePopup {
    x: f32,
    y: f32,
    current: String,
    on_save: EventHandler<String>,
    on_close: EventHandler<()>,
}

impl Component for RenamePopup {
    fn render(&self) -> impl IntoElement {
        let value = use_state(|| self.current.clone());
        let current = self.current.clone();
        let on_save = self.on_save.clone();
        let on_close = self.on_close.clone();
        let close = self.on_close.clone();
        let cancel = self.on_close.clone();

        let save = move || {
            let name = value.read().trim().to_string();
            if name.is_empty() {
                return;
            }
            on_close.call(());
            if name != current.trim() {
                on_save.call(name);
            }
        };
        let submit = save.clone();
        let press = save;

        let (x, y) = clamp_to_window(self.x, self.y, RENAME_WIDTH, RENAME_HEIGHT);

        OverlayPopup::new()
            .backdrop(false)
            .position(Position::new_global().top(y).left(x))
            .on_close(move |_| close.call(()))
            .child(
                rect()
                    .vertical()
                    .width(Size::px(RENAME_WIDTH))
                    .padding(Gaps::new_all(10.))
                    .spacing(8.)
                    .corner_radius(CornerRadius::new_all(12.))
                    .background(colors::page_elevated())
                    .border(hairline(colors::component_border()))
                    .child(
                        label()
                            .text("Rename")
                            .font_size(12.)
                            .font_weight(FontWeight::MEDIUM)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        TextInput::new(value)
                            .placeholder(self.current.clone())
                            .compact()
                            .auto_focus(true)
                            .width(Size::fill())
                            .on_submit(move |_: String| submit()),
                    )
                    .child(
                        rect()
                            .horizontal()
                            .width(Size::fill())
                            .main_align(Alignment::End)
                            .spacing(8.)
                            .child(
                                Button::new()
                                    .secondary()
                                    .small()
                                    .on_press(move |_| cancel.call(()))
                                    .text("Cancel"),
                            )
                            .child(
                                Button::new()
                                    .primary()
                                    .small()
                                    .on_press(move |_| press())
                                    .text("Save"),
                            ),
                    )
                    .into_element(),
            )
    }
}

#[derive(PartialEq)]
pub(super) struct SectionChip {
    pub text: &'static str,
    pub icon: IconType,
    pub selected: bool,
    pub on_press: EventHandler<()>,
}

impl Component for SectionChip {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let color = if self.selected {
            colors::fg_primary()
        } else {
            colors::fg_secondary()
        };
        let on_press = self.on_press.clone();

        rect()
            .horizontal()
            .height(Size::px(33.))
            .padding(Gaps::new_symmetric(0., 12.))
            .cross_align(Alignment::Center)
            .spacing(8.)
            .corner_radius(CornerRadius::new_all(8.))
            .background(if self.selected {
                colors::brand()
            } else {
                colors::component_bg()
            })
            .border(hairline(if *hovered.read() && !self.selected {
                colors::component_border_hover()
            } else {
                colors::component_border()
            }))
            .cursor(CursorIcon::Pointer)
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_leave(move |_| hovered.set(false))
            .on_press(move |_| on_press.call(()))
            .child(Icon::new(self.icon).size(14.).color(color))
            .child(
                label()
                    .text(self.text)
                    .font_size(14.)
                    .font_weight(if self.selected {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::NORMAL
                    })
                    .color(color),
            )
    }
}

#[derive(PartialEq)]
pub(super) struct AnchorChip {
    pub text: String,
    pub active: bool,
    pub on_press: EventHandler<()>,
}

impl Component for AnchorChip {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let on_press = self.on_press.clone();

        rect()
            .height(Size::px(26.))
            .padding(Gaps::new_symmetric(0., 10.))
            .main_align(Alignment::Center)
            .corner_radius(CornerRadius::new_all(6.))
            .background(if self.active {
                colors::component_bg()
            } else {
                Color::TRANSPARENT
            })
            .cursor(CursorIcon::Pointer)
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_leave(move |_| hovered.set(false))
            .on_press(move |_| on_press.call(()))
            .child(
                label()
                    .text(self.text.clone())
                    .max_lines(1)
                    .font_size(12.)
                    .font_weight(if self.active {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::NORMAL
                    })
                    .color(if self.active || *hovered.read() {
                        colors::fg_primary()
                    } else {
                        colors::fg_secondary()
                    }),
            )
    }
}

#[derive(PartialEq)]
pub(super) struct InfoHeading {
    pub title: String,
    pub note: String,
}

impl Component for InfoHeading {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let tooltip = use_tooltip_anchor(
            Some(self.note.clone().into_boxed_str()),
            TooltipPlacement::Top,
        );

        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(28.))
            .padding(Gaps::new(4., 0., 0., 0.))
            .cross_align(Alignment::Center)
            .spacing(6.)
            .child(
                label()
                    .text(self.title.clone())
                    .font_size(14.)
                    .font_weight(FontWeight::MEDIUM)
                    .color(colors::fg_primary()),
            )
            .child(
                tooltip.attach(
                    rect()
                        .width(Size::px(16.))
                        .height(Size::px(16.))
                        .center()
                        .on_pointer_enter(move |_| hovered.set(true))
                        .on_pointer_leave(move |_| hovered.set(false))
                        .child(Icon::new(IconType::InfoCircle).size(14.).color(
                            if *hovered.read() {
                                colors::fg_primary()
                            } else {
                                colors::fg_secondary()
                            },
                        )),
                ),
            )
    }
}

pub(super) fn divider(text: &str) -> Element {
    let line = || {
        rect()
            .width(Size::flex(1.0))
            .height(Size::px(1.))
            .background(colors::component_border())
    };

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .child(line())
        .child(
            label()
                .text(text.to_uppercase())
                .font_size(11.)
                .font_weight(FontWeight::MEDIUM)
                .color(colors::fg_secondary()),
        )
        .child(line())
        .into_element()
}

pub(super) fn notice(text: impl Into<String>) -> Element {
    rect()
        .width(Size::fill())
        .height(Size::px(57.))
        .padding(Gaps::new_symmetric(0., 16.))
        .main_align(Alignment::Center)
        .corner_radius(CornerRadius::new_all(16.))
        .border(hairline(colors::component_border()))
        .child(
            label()
                .text(text.into())
                .width(Size::fill())
                .max_lines(2)
                .text_overflow(TextOverflow::Ellipsis)
                .font_size(12.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

pub(super) fn library_intro(text: String, button: Option<Element>) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::px(33.))
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .child(
            label()
                .text(text)
                .width(Size::flex(1.0))
                .font_size(12.)
                .color(colors::fg_secondary()),
        )
        .maybe_child(button)
        .into_element()
}

fn default_skin(slim: bool) -> bytes::Bytes {
    static STEVE: std::sync::LazyLock<bytes::Bytes> =
        std::sync::LazyLock::new(|| AppAssets::get_bytes("steve.png").unwrap_or_default());
    static ALEX: std::sync::LazyLock<bytes::Bytes> =
        std::sync::LazyLock::new(|| AppAssets::get_bytes("alex.png").unwrap_or_default());
    if slim { ALEX.clone() } else { STEVE.clone() }
}

#[derive(PartialEq)]
pub(super) struct LookThumb {
    pub uuid: String,
    pub skin: Option<(String, bool)>,
    pub cape: Option<CapeSource>,
    pub cosmetics: Vec<(BodySlot, String)>,
    pub emote: Option<String>,
    pub yaw: f32,
    pub zoom: f32,
    pub interactive: bool,
}

const COSMETIC_SLOTS: [BodySlot; 10] = [
    BodySlot::Backpack,
    BodySlot::Glasses,
    BodySlot::Wings,
    BodySlot::LeftHand,
    BodySlot::RightHand,
    BodySlot::Hat,
    BodySlot::Aura,
    BodySlot::Boots,
    BodySlot::Shoulder,
    BodySlot::Pet,
];

impl Component for LookThumb {
    fn render(&self) -> impl IntoElement {
        let skin_url = self.skin.as_ref().map(|(url, _)| url.clone());
        let skin_query = use_cached_image(skin_url.clone(), 0);

        let mojang_url = match &self.cape {
            Some(CapeSource::Mojang(url)) => Some(url.clone()),
            _ => None,
        };
        let plus_url = match &self.cape {
            Some(CapeSource::Plus(url)) => Some(url.clone()),
            _ => None,
        };
        let mojang_query = use_cached_image(mojang_url.clone(), 0);
        let plus_query = use_cosmetic_bundle(plus_url.clone());
        let url_for = |slot: BodySlot| {
            self.cosmetics
                .iter()
                .find(|(worn, _)| *worn == slot)
                .map(|(_, url)| url.clone())
        };
        let cosmetic_queries = COSMETIC_SLOTS.map(|slot| use_cosmetic_bundle(url_for(slot)));
        let emote_query = use_cosmetic_bundle(self.emote.clone());

        let skin = self.skin.as_ref().map(|(url, slim)| {
            let bytes = loaded_image(Some(url.as_str()), &skin_query)
                .map(|(_, bytes)| bytes)
                .unwrap_or_else(|| default_skin(*slim));
            (bytes, *slim)
        });
        let cape = match (&mojang_url, &plus_url) {
            (Some(url), _) => {
                loaded_image(Some(url.as_str()), &mojang_query).map(|(_, bytes)| bytes)
            }
            (None, Some(_)) => {
                settled_or_loading(&plus_query).and_then(|bundle| bundle.sheet.or(bundle.texture))
            }
            (None, None) => None,
        };
        let cosmetics = COSMETIC_SLOTS
            .iter()
            .zip(&cosmetic_queries)
            .filter(|(slot, _)| url_for(**slot).is_some())
            .filter_map(|(slot, query)| Some((*slot, settled_or_loading(query)?)))
            .collect();
        let emote = self
            .emote
            .as_ref()
            .and_then(|_| settled_or_loading(&emote_query)?.animation);

        PlayerModel::new(self.uuid.clone())
            .skin(skin)
            .cape(cape)
            .cosmetics(cosmetics)
            .emote(emote)
            .yaw(self.yaw)
            .zoom(self.zoom)
            .draggable(self.interactive)
            .focus_y(if self.zoom > 1.2 { 1.5 } else { 0. })
            .width(Size::fill())
            .height(Size::fill())
    }
}

#[derive(PartialEq)]
pub(super) struct CoverThumb {
    pub url: Option<String>,
}

impl Component for CoverThumb {
    fn render(&self) -> impl IntoElement {
        let query = use_cached_image(self.url.clone(), 512);

        match loaded_image(self.url.as_deref(), &query) {
            Some((url, bytes)) => ImageViewer::new((url, bytes))
                .width(Size::fill())
                .height(Size::fill())
                .aspect_ratio(AspectRatio::Min)
                .into_element(),
            None => rect()
                .width(Size::fill())
                .height(Size::fill())
                .center()
                .child(
                    Icon::new(IconType::Colors)
                        .size(28.)
                        .color(colors::fg_secondary()),
                )
                .into_element(),
        }
    }
}
