use freya::{
    animation::{AnimNum, Ease, OnCreation, use_animation},
    prelude::*,
    router::RouterContext,
};

use crate::{
    Route,
    components::{Avatar, Icon, IconType, OverlayPopup},
    hooks::{
        RemoveAccountKeys, query_error, query_is_loading, try_accounts, try_default_account,
        use_accounts, use_control_center_open, use_current_account, use_dispatch,
        use_remove_account, use_settings_snapshot,
    },
    theme::colors,
    ui::divider,
};

const PANEL_WIDTH: f32 = 344.;

#[derive(PartialEq)]
pub struct ControlCenter;

impl Component for ControlCenter {
    fn render(&self) -> impl IntoElement {
        let open = use_control_center_open();
        let dispatch = use_dispatch();

        if !open {
            return rect().into_element();
        }

        OverlayPopup::new()
            .position(Position::new_global().top(72.).right(40.))
            .on_close(move |()| dispatch.close_control_center())
            .child(ControlPanel)
            .into_element()
    }
}

#[derive(PartialEq)]
struct ControlPanel;

impl Component for ControlPanel {
    fn render(&self) -> impl IntoElement {
        let intro = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            AnimNum::new(0., 1.).time(200).ease(Ease::Out)
        });
        let progress = intro.read().value();

        rect()
            .vertical()
            .width(Size::px(PANEL_WIDTH))
            .padding(Gaps::new_all(12.))
            .spacing(10.)
            .opacity(progress)
            .margin(Gaps::new((1.0 - progress) * -8.0, 0., 0., 0.))
            .background(colors::page_elevated().with_a(220))
            .blur(12.)
            .corner_radius(CornerRadius::new_all(14.))
            .border(
                Border::new()
                    .fill(colors::component_border())
                    .width(1.)
                    .alignment(BorderAlignment::Inner),
            )
            .shadow(Shadow::from((
                0.,
                8.,
                32.,
                0.,
                Color::from_argb(120, 0, 0, 0),
            )))
            .child(AccountHeader)
            .child(divider())
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .spacing(2.)
                    .child(NavRow {
                        icon: IconType::BarChartSquare02,
                        row_label: "Stats",
                        target: Route::Stats {},
                    })
                    .child(NavRow {
                        icon: IconType::Settings02,
                        row_label: "Settings",
                        target: Route::SettingsLauncher {},
                    }),
            )
            .child(section_label("QUICK SETTINGS"))
            .child(QuickSettings)
            .child(divider())
            .child(PanelFooter)
    }
}

fn section_label(text: &'static str) -> impl IntoElement {
    label()
        .text(text)
        .font_size(11.)
        .letter_spacing(0.8)
        .font_weight(FontWeight::SEMI_BOLD)
        .color(colors::fg_secondary())
}

#[derive(PartialEq)]
struct AccountHeader;

impl Component for AccountHeader {
    fn render(&self) -> impl IntoElement {
        let current = use_current_account();
        let accounts_query = use_accounts();
        let dispatch = use_dispatch();
        let mut hovered = use_state(|| false);

        let loading = query_is_loading(&current);
        let unusable = query_error(&current).is_some();
        let account = try_default_account(&current).or_else(|| {
            unusable
                .then(|| try_accounts(&accounts_query).unwrap_or_default().into_iter().next())
                .flatten()
        });

        let uuid = account
            .as_ref()
            .map(|account| account.id.to_string())
            .unwrap_or_else(|| uuid::Uuid::nil().to_string());
        let username = match account.as_ref() {
            Some(account) => account.username.clone(),
            None if loading => "Loading…".to_string(),
            None => "Not signed in".to_string(),
        };
        let subtitle = match (account.as_ref(), unusable) {
            (_, true) => "Microsoft Account required",
            (Some(account), false) if account.is_microsoft() => "Microsoft account · Active",
            (Some(_), false) => "Offline account · Active",
            (None, false) if loading => "Checking accounts",
            (None, false) => "No account selected",
        };
        let subtitle_color = if unusable {
            colors::danger()
        } else {
            colors::fg_secondary()
        };

        let open_switcher = move |_| {
            dispatch.close_control_center();
            dispatch.open_account_switcher();
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(10.)
            .padding(Gaps::new_all(4.))
            .child(
                Avatar::new(uuid)
                    .width(Size::px(40.))
                    .height(Size::px(40.)),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(2.)
                    .child(
                        label()
                            .text(username)
                            .font_size(16.)
                            .font_weight(FontWeight::MEDIUM)
                            .max_lines(1)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text(subtitle)
                            .font_size(12.)
                            .max_lines(1)
                            .color(subtitle_color),
                    ),
            )
            .child(
                rect()
                    .horizontal()
                    .cross_align(Alignment::Center)
                    .spacing(6.)
                    .padding(Gaps::new_symmetric(6., 8.))
                    .corner_radius(CornerRadius::new_all(8.))
                    .background(Color::RED.with_a(0))
                    .maybe(*hovered.read(), |el| {
                        el.background(colors::ghost_overlay_hover())
                    })
                    .cursor(CursorIcon::Pointer)
                    .a11y_role(AccessibilityRole::Button)
                    .on_pointer_enter(move |_| hovered.set(true))
                    .on_pointer_leave(move |_| hovered.set(false))
                    .on_press(open_switcher)
                    .child(
                        label()
                            .text("Switch")
                            .font_size(13.)
                            .color(colors::fg_secondary()),
                    )
                    .child(
                        Icon::new(IconType::ArrowRight)
                            .size(14.)
                            .color(colors::fg_secondary()),
                    ),
            )
    }
}

#[derive(PartialEq)]
struct NavRow {
    icon: IconType,
    row_label: &'static str,
    target: Route,
}

impl Component for NavRow {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let mut hovered = use_state(|| false);

        let target = self.target.clone();
        let go = move |_| {
            dispatch.close_control_center();
            let _ = RouterContext::get().push(target.clone());
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(42.))
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(12.)
            .padding(Gaps::new_symmetric(0., 8.))
            .corner_radius(CornerRadius::new_all(8.))
            .background(Color::RED.with_a(0))
            .maybe(*hovered.read(), |el| {
                el.background(colors::ghost_overlay_hover())
            })
            .cursor(CursorIcon::Pointer)
            .a11y_role(AccessibilityRole::Button)
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_leave(move |_| hovered.set(false))
            .on_press(go)
            .child(
                Icon::new(self.icon)
                    .size(20.)
                    .color(colors::fg_secondary()),
            )
            .child(
                label()
                    .text(self.row_label)
                    .font_size(15.)
                    .width(Size::flex(1.0))
                    .color(colors::fg_primary()),
            )
            .child(
                Icon::new(IconType::ArrowRight)
                    .size(16.)
                    .color(colors::fg_secondary()),
            )
    }
}

#[derive(PartialEq)]
struct QuickSettings;

impl Component for QuickSettings {
    fn render(&self) -> impl IntoElement {
        let settings = use_settings_snapshot().settings;
        let dispatch = use_dispatch();

        let discord_on = settings.discord_enabled;
        let toggle_discord = move |_| {
            dispatch.edit_settings(|s| s.discord_enabled = !s.discord_enabled);
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .spacing(8.)
            .child(
                QuickTile::new(IconType::Eye, "Discord RPC", discord_on)
                    .on_press(toggle_discord),
            )
            .child(QuickTile::new(IconType::Moon01, "Close on launch", false).disabled())
    }
}

struct QuickTile {
    icon: IconType,
    title: &'static str,
    on: bool,
    enabled: bool,
    on_press: Option<EventHandler<Event<PressEventData>>>,
}

impl PartialEq for QuickTile {
    fn eq(&self, other: &Self) -> bool {
        self.icon == other.icon
            && self.title == other.title
            && self.on == other.on
            && self.enabled == other.enabled
    }
}

impl QuickTile {
    fn new(icon: IconType, title: &'static str, on: bool) -> Self {
        Self {
            icon,
            title,
            on,
            enabled: true,
            on_press: None,
        }
    }

    fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    fn on_press(mut self, on_press: impl Into<EventHandler<Event<PressEventData>>>) -> Self {
        self.on_press = Some(on_press.into());
        self
    }
}

impl Component for QuickTile {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);

        let on = self.on;
        let enabled = self.enabled;
        let handler = self.on_press.clone();

        let background = if !enabled {
            colors::component_bg().with_a(120)
        } else if on {
            if *hovered.read() {
                colors::brand_hover()
            } else {
                colors::brand()
            }
        } else if *hovered.read() {
            colors::component_bg_hover()
        } else {
            colors::component_bg()
        };

        let title_color = if enabled {
            colors::fg_primary()
        } else {
            colors::fg_primary_disabled()
        };

        let sub_color = if !enabled {
            colors::fg_primary_disabled()
        } else if on {
            colors::fg_primary().with_a(200)
        } else {
            colors::fg_secondary()
        };

        rect()
            .horizontal()
            .width(Size::flex(1.0))
            .height(Size::px(64.))
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(10.)
            .padding(Gaps::new_symmetric(0., 12.))
            .corner_radius(CornerRadius::new_all(10.))
            .background(background)
            .maybe(!on || !enabled, |el| {
                el.border(
                    Border::new()
                        .fill(colors::component_border())
                        .width(1.)
                        .alignment(BorderAlignment::Inner),
                )
            })
            .maybe(enabled, |el| {
                el.cursor(CursorIcon::Pointer)
                    .a11y_role(AccessibilityRole::Button)
                    .on_pointer_enter(move |_| hovered.set(true))
                    .on_pointer_leave(move |_| hovered.set(false))
            })
            .map(handler, |el, handler| el.on_press(move |e| handler.call(e)))
            .child(Icon::new(self.icon).size(24.).color(title_color))
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .child(
                        label()
                            .text(self.title)
                            .font_size(12.)
                            .font_weight(FontWeight::MEDIUM)
                            .max_lines(1)
                            .color(title_color),
                    )
                    .child(
                        label()
                            .text(if on { "On" } else { "Off" })
                            .font_size(11.)
                            .color(sub_color),
                    ),
            )
    }
}

#[derive(PartialEq)]
struct PanelFooter;

impl Component for PanelFooter {
    fn render(&self) -> impl IntoElement {
        let settings = use_settings_snapshot().settings;
        let current = use_current_account();
        let remove = use_remove_account();

        let mut confirming = use_state(|| false);

        let folder = settings
            .data_dir
            .clone()
            .or_else(|| oneclient_core::settings::data_dir::default_path().ok());

        let account_id = try_default_account(&current).map(|account| account.id);

        let sign_out = move |_| {
            if !*confirming.peek() {
                confirming.set(true);
                return;
            }
            if let Some(id) = account_id {
                remove.mutate(RemoveAccountKeys { id });
            }
            confirming.set(false);
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .child(
                footer_action(
                    IconType::Folder,
                    "Open launcher folder",
                    colors::fg_primary(),
                )
                .on_press(move |_| {
                    if let Some(folder) = folder.clone() {
                        std::fs::create_dir_all(&folder).ok();
                        crate::platform::open_path(&folder.to_string_lossy());
                    }
                }),
            )
            .child(rect().width(Size::flex(1.0)))
            .map(account_id, |el, _| {
                el.child(
                    footer_action(
                        IconType::LogOut01,
                        if *confirming.read() {
                            "Confirm sign out"
                        } else {
                            "Sign out"
                        },
                        if *confirming.read() {
                            colors::danger()
                        } else {
                            colors::fg_primary()
                        },
                    )
                    .on_press(sign_out),
                )
            })
    }
}

fn footer_action(icon: IconType, text: &'static str, color: Color) -> FooterAction {
    FooterAction {
        icon,
        text,
        color,
        on_press: None,
    }
}

struct FooterAction {
    icon: IconType,
    text: &'static str,
    color: Color,
    on_press: Option<EventHandler<Event<PressEventData>>>,
}

impl PartialEq for FooterAction {
    fn eq(&self, other: &Self) -> bool {
        self.icon == other.icon && self.text == other.text && self.color == other.color
    }
}

impl FooterAction {
    fn on_press(mut self, on_press: impl Into<EventHandler<Event<PressEventData>>>) -> Self {
        self.on_press = Some(on_press.into());
        self
    }
}

impl Component for FooterAction {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let handler = self.on_press.clone();

        rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(8.)
            .height(Size::px(34.))
            .padding(Gaps::new_symmetric(0., 8.))
            .corner_radius(CornerRadius::new_all(8.))
            .background(Color::RED.with_a(0))
            .maybe(*hovered.read(), |el| {
                el.background(colors::ghost_overlay_hover())
            })
            .cursor(CursorIcon::Pointer)
            .a11y_role(AccessibilityRole::Button)
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_leave(move |_| hovered.set(false))
            .map(handler, |el, handler| {
                el.on_press(move |e| handler.call(e))
            })
            .child(Icon::new(self.icon).size(16.).color(self.color))
            .child(
                label()
                    .text(self.text)
                    .font_size(13.)
                    .max_lines(1)
                    .color(self.color),
            )
    }
}
