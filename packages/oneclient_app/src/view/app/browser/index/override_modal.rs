use freya::prelude::*;
use oneclient_common::domain::GameLoader;
use oneclient_content::packages::ContentType;
use oneclient_core::GameVersionKind;

use crate::components::{Button, Icon, IconType, OverlayPopup, ScrollArea};
use crate::hooks::{SearchOverride, game_versions, use_game_versions};
use crate::i18n::t;
use crate::theme::{colors, rounded, text_size};
use crate::ui::border_all_color;
use crate::utils::capitalize;
use crate::view::app::browser::control_radius;

const LOADERS: [GameLoader; 3] = [GameLoader::Fabric, GameLoader::Forge, GameLoader::NeoForge];
const LIST_H: f32 = 260.;

pub(crate) fn summary(search_override: &SearchOverride, uses_loader: bool) -> String {
    let version = search_override
        .version
        .clone()
        .unwrap_or_else(|| t("oneclient.browser.override.any_version").to_string());
    if !uses_loader {
        return version;
    }
    let loader = search_override.loader.map_or_else(
        || t("oneclient.browser.index.loader.any").to_string(),
        |l| l.to_string(),
    );
    format!("{version} · {loader}")
}

#[derive(PartialEq)]
pub(super) struct OverrideButton {
    /// 28 beside the full page's pills, the toolbar's 34 inside an instance
    pub(super) height: f32,
    pub(super) content_type: ContentType,
    pub(super) search_override: State<Option<SearchOverride>>,
    pub(super) cluster: Option<(GameLoader, String)>,
}

impl Component for OverrideButton {
    fn render(&self) -> impl IntoElement {
        let mut open = use_state(|| false);
        let current = self.search_override.read().clone();
        let uses_loader = self.content_type == ContentType::Mod;

        let button = Button::new()
            .small()
            .height(Size::px(self.height))
            .padding(Gaps::new_symmetric(0., 10.))
            .on_press(move |_| open.set(true))
            .child(Icon::new(IconType::Sliders04).size(12.))
            .text(t("oneclient.browser.override.button"));
        let button = match &current {
            Some(o) => button
                .primary()
                .alt(t("oneclient.browser.override.button"))
                .tooltip(summary(o, uses_loader)),
            None => button.secondary(),
        };

        let initial = current.unwrap_or_else(|| SearchOverride {
            loader: self
                .cluster
                .as_ref()
                .map(|(loader, _)| *loader)
                .filter(|l| LOADERS.contains(l)),
            version: self.cluster.as_ref().map(|(_, version)| version.clone()),
        });

        rect()
            .child(button)
            .maybe_child(open.read().then(|| OverrideModal {
                uses_loader,
                global: self.cluster.is_none(),
                search_override: self.search_override,
                initial,
                open,
            }))
    }
}

#[derive(PartialEq)]
struct OverrideModal {
    uses_loader: bool,
    /// No instance picked yet: the override only changes what is listed, an install still
    /// resolves against the instance chosen then
    global: bool,
    search_override: State<Option<SearchOverride>>,
    initial: SearchOverride,
    open: State<bool>,
}

impl Component for OverrideModal {
    fn render(&self) -> impl IntoElement {
        let initial = self.initial.clone();
        let draft = use_state(move || initial);
        let mut open = self.open;
        let mut search_override = self.search_override;

        let versions: Vec<String> = game_versions(&use_game_versions())
            .map(|all| {
                all.iter()
                    .filter(|v| v.kind == GameVersionKind::Release)
                    .map(|v| v.id.clone())
                    .collect()
            })
            .unwrap_or_default();

        let loader_rows =
            std::iter::once((None, t("oneclient.browser.index.loader.any").to_string()))
                .chain(LOADERS.iter().map(|l| (Some(*l), l.to_string())))
                .map(move |(value, name)| {
                    let mut draft = draft;
                    OverrideRow {
                        name: name.clone(),
                        selected: draft.read().loader == value,
                        on_toggle: (move |()| draft.write().loader = value).into(),
                        key: DiffKey::None,
                    }
                    .key(name)
                    .into_element()
                });

        let version_rows = std::iter::once((
            None,
            t("oneclient.browser.override.any_version").to_string(),
        ))
        .chain(versions.into_iter().map(|v| (Some(v.clone()), v)))
        .map(move |(value, name)| {
            let mut draft = draft;
            let selected = draft.read().version == value;
            OverrideRow {
                name: name.clone(),
                selected,
                on_toggle: (move |()| draft.write().version = value.clone()).into(),
                key: DiffKey::None,
            }
            .key(name)
            .into_element()
        });

        let control = rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .spacing(12.)
            .maybe_child(self.uses_loader.then(|| {
                column(
                    t("oneclient.browser.override.loaders"),
                    loader_rows.collect(),
                )
            }))
            .child(column(
                t("oneclient.browser.override.versions"),
                version_rows.collect(),
            ))
            .into_element();

        // Reset clears the override outright (back to the instance's own loader and version), so it
        // only applies while one is active. Close, Escape and click-outside never touch it: they
        // just drop the unsaved draft.
        let has_override = search_override.read().is_some();

        override_dialog(
            t("oneclient.browser.override.title").to_string(),
            if self.global {
                t("oneclient.browser.override.description_global")
            } else {
                t("oneclient.browser.override.description")
            }
            .to_string(),
            control,
            move || open.set(false),
            Button::new()
                .secondary()
                .enabled(has_override)
                .on_press(move |_| {
                    search_override.set(None);
                    open.set(false);
                })
                .text(t("oneclient.common.reset"))
                .into_element(),
            Button::new()
                .primary()
                .on_press(move |_| {
                    search_override.set(Some(draft.read().clone()));
                    open.set(false);
                })
                .text(t("oneclient.common.apply"))
                .into_element(),
        )
    }
}

/// Opaque, theme-aware list well so the selected / hover / focus rows keep their contrast.
fn column(title: &'static str, rows: Vec<Element>) -> Element {
    rect()
        .vertical()
        .width(Size::flex(1.0))
        .spacing(8.)
        .child(
            label()
                .text(title)
                .font_size(text_size(11.))
                .font_weight(FontWeight::MEDIUM)
                .letter_spacing(1.4)
                .color(colors::fg_secondary()),
        )
        .child(
            rect()
                .width(Size::fill())
                .corner_radius(rounded(10.))
                .background(colors::page().with_a(255))
                .border(border_all_color(1., colors::component_border()))
                .overflow(Overflow::Clip)
                .child(
                    ScrollArea::new()
                        .width(Size::fill())
                        .height(Size::px(LIST_H))
                        .children([rect()
                            .vertical()
                            .width(Size::fill())
                            .spacing(1.)
                            .padding(Gaps::new_all(4.))
                            .children(rows)
                            .into_element()]),
                ),
        )
        .into_element()
}

/// Opaque, theme-aware surface: the shared dialog's card token is translucent in PolyGlass, which
/// lets the package art behind it bleed through the filter lists.
///
/// Closing: the X button, Escape (via the overlay) and a press on the dimmed area outside the card
/// all call `on_close`. The card swallows its own presses so interacting inside never closes it.
fn override_dialog(
    title: String,
    body: String,
    control: Element,
    on_close: impl FnMut() + 'static,
    reset: Element,
    apply: Element,
) -> Element {
    let on_close = std::rc::Rc::new(std::cell::RefCell::new(on_close));
    let close_overlay = on_close.clone();
    let close_backdrop = on_close.clone();
    let close_button = on_close;

    let edge = if colors::is_light() {
        colors::fg_primary().with_a(58)
    } else {
        colors::component_border_hover()
    };

    let header = rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Start)
        .spacing(12.)
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(4.)
                .child(
                    label()
                        .text(title)
                        .font_size(text_size(16.))
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(body)
                        .width(Size::fill())
                        .font_size(text_size(12.))
                        .color(colors::fg_secondary()),
                ),
        )
        .child(
            Button::new()
                .ghost()
                .small()
                .icon()
                .tooltip(t("oneclient.common.close"))
                .on_press(move |_| (close_button.borrow_mut())())
                .child(Icon::new(IconType::X).size(16.)),
        );

    let footer = rect()
        .horizontal()
        .width(Size::fill())
        .main_align(Alignment::End)
        .spacing(8.)
        .child(reset)
        .child(apply);

    let card = rect()
        .vertical()
        .width(Size::px(460.))
        .max_width(Size::window_percent(90.))
        .max_height(Size::window_percent(90.))
        .spacing(16.)
        .padding(Gaps::new_all(20.))
        .corner_radius(rounded(14.))
        .background(colors::page_elevated().with_a(255))
        .border(border_all_color(1., edge))
        .shadow(Shadow::from((
            0.,
            12.,
            36.,
            0.,
            Color::from_argb(140, 0, 0, 0),
        )))
        // Presses inside the card must not reach the click-outside backdrop beneath it.
        .on_press(|e: Event<PressEventData>| e.stop_propagation())
        .child(header)
        .child(control)
        .child(
            rect()
                .width(Size::fill())
                .height(Size::px(1.))
                .background(colors::component_border()),
        )
        .child(footer);

    OverlayPopup::new()
        .on_close(move |_| (close_overlay.borrow_mut())())
        .child(
            // Full-window hit area: a press that lands here (outside the card) closes the dialog.
            rect()
                .width(Size::window_percent(100.))
                .height(Size::window_percent(100.))
                .center()
                .on_press(move |_| (close_backdrop.borrow_mut())())
                .child(card),
        )
        .into_element()
}

/// Filter choice row. Selected is a solid accent fill, hover a raised component tint and keyboard
/// focus a full-contrast outline, so each state stays distinct on the solid panel.
#[derive(PartialEq)]
struct OverrideRow {
    name: String,
    selected: bool,
    on_toggle: EventHandler<()>,
    key: DiffKey,
}

impl KeyExt for OverrideRow {
    fn key_mut(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for OverrideRow {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);

        let selected = self.selected;
        let on_toggle = self.on_toggle.clone();
        let hovered = *hovering.read();
        let focused = focus().is_focused();

        let (background, text) = match (selected, hovered) {
            (true, true) => (Some(colors::brand_hover()), colors::on_brand_strong()),
            (true, false) => (Some(colors::brand()), colors::on_brand_strong()),
            (false, true) => (Some(colors::component_bg_pressed()), colors::fg_primary()),
            (false, false) => (None, colors::fg_primary()),
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(29.))
            .cross_align(Alignment::Center)
            .padding(Gaps::new_symmetric(0., 10.))
            .corner_radius(control_radius(29., 8.))
            .overflow(Overflow::Clip)
            .map(background, |el, color| el.background(color))
            .maybe(focused, |el| {
                el.border(border_all_color(2., colors::fg_primary()))
            })
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .cursor(CursorIcon::Pointer)
            .on_pointer_enter(move |_| hovering.set(true))
            .on_pointer_leave(move |_| hovering.set(false))
            .on_press(move |_| on_toggle.call(()))
            .child(
                label()
                    .text(capitalize(&self.name))
                    .font_size(text_size(12.))
                    .width(Size::fill())
                    .max_lines(1)
                    .text_overflow(TextOverflow::Ellipsis)
                    .font_weight(if selected {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::NORMAL
                    })
                    .color(text),
            )
    }
}
