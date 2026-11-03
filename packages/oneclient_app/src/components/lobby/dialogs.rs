use freya::prelude::*;

use super::panel::apply_confirm;
use super::{FRIENDS, Friend, Lobby, Presence, display_name, use_lobby, use_my_name};
use crate::components::{
    Avatar, Button, Icon, IconType, OVERLAY_MAX_LEVEL, OverlayPopup, TextInput,
};
use crate::hooks::use_dispatch;
use crate::i18n::{t, tf};
use crate::theme::{self, colors};
use crate::ui::border_all_color;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConfirmKind {
    Kick(&'static str),
    Promote(&'static str),
    Leave,
    Disband,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Confirm {
    pub kind: ConfirmKind,
    title: String,
    body: String,
    action: &'static str,
    danger: bool,
    toast: String,
}

impl Confirm {
    pub fn kick(id: &'static str, name: String) -> Self {
        Self {
            kind: ConfirmKind::Kick(id),
            title: tf("oneclient.components.lobby.confirm.kick.title", &[&name]),
            body: tf("oneclient.components.lobby.confirm.kick.body", &[&name]),
            action: t("oneclient.components.lobby.member.kick"),
            danger: true,
            toast: tf("oneclient.components.lobby.toast.kicked", &[&name]),
        }
    }

    pub fn promote(id: &'static str, name: String) -> Self {
        Self {
            kind: ConfirmKind::Promote(id),
            title: t("oneclient.components.lobby.confirm.promote.title").to_string(),
            body: tf("oneclient.components.lobby.confirm.promote.body", &[&name]),
            action: t("oneclient.components.lobby.member.make_leader"),
            danger: false,
            toast: tf("oneclient.components.lobby.toast.promoted", &[&name]),
        }
    }

    pub fn leave(lobby: &Lobby, me: &str) -> Self {
        let body = match (lobby.is_leader(), lobby.successor()) {
            (true, Some(next)) => tf(
                "oneclient.components.lobby.confirm.leave.body_leader",
                &[&display_name(next, me)],
            ),
            (true, None) => t("oneclient.components.lobby.confirm.leave.body_alone").to_string(),
            (false, _) => tf(
                "oneclient.components.lobby.confirm.leave.body_member",
                &[&display_name(lobby.leader, me)],
            ),
        };
        Self {
            kind: ConfirmKind::Leave,
            title: t("oneclient.components.lobby.confirm.leave.title").to_string(),
            body,
            action: t("oneclient.components.lobby.leave"),
            danger: true,
            toast: t("oneclient.components.lobby.toast.left").to_string(),
        }
    }

    pub fn disband(players: usize) -> Self {
        Self {
            kind: ConfirmKind::Disband,
            title: t("oneclient.components.lobby.confirm.disband.title").to_string(),
            body: tf(
                "oneclient.components.lobby.confirm.disband.body",
                &[&players],
            ),
            action: t("oneclient.components.lobby.disband"),
            danger: true,
            toast: t("oneclient.components.lobby.toast.disbanded").to_string(),
        }
    }
}

fn modal(width: f32, on_close: EventHandler<()>, card: Rect) -> Element {
    OverlayPopup::new()
        .overlay_level(OVERLAY_MAX_LEVEL)
        .on_close(move |()| on_close.call(()))
        .child(
            rect()
                .width(Size::window_percent(100.))
                .height(Size::window_percent(100.))
                .center()
                .child(
                    card.vertical()
                        .width(Size::px(width))
                        .max_width(Size::window_percent(90.))
                        .spacing(14.)
                        .padding(Gaps::new_all(20.))
                        .corner_radius(CornerRadius::new_all(14.))
                        .background(colors::page_elevated())
                        .border(border_all_color(1., colors::component_border())),
                ),
        )
        .into_element()
}

fn modal_title(
    icon: Option<(IconType, Color)>,
    title: String,
    on_close: Option<EventHandler<()>>,
) -> Rect {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(10.)
        .maybe_child(
            icon.map(|(icon, color)| Icon::new(icon).size(20.).color(color).into_element()),
        )
        .child(
            label()
                .text(title)
                .width(Size::flex(1.0))
                .font_size(16.)
                .font_weight(FontWeight::SEMI_BOLD)
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .color(colors::fg_primary()),
        )
        .maybe_child(on_close.map(|on_close| {
            Button::new()
                .ghost()
                .icon()
                .tooltip(t("oneclient.common.close"))
                .on_press(move |_| on_close.call(()))
                .child(Icon::new(IconType::X).size(18.))
                .into_element()
        }))
}

#[derive(PartialEq)]
pub struct ConfirmDialog {
    pub confirm: Confirm,
    pub on_close: EventHandler<()>,
}

impl Component for ConfirmDialog {
    fn render(&self) -> impl IntoElement {
        let mut state = use_lobby();
        let dispatch = use_dispatch();
        let confirm = self.confirm.clone();
        let on_close = self.on_close.clone();
        let cancel = self.on_close.clone();

        let run = move |_| {
            apply_confirm(&mut state, &confirm);
            dispatch
                .notify(confirm.toast.clone())
                .info()
                .icon(IconType::Users01)
                .toast_only()
                .send();
            on_close.call(());
        };

        let icon = if self.confirm.danger {
            (IconType::AlertTriangle, colors::code_warn())
        } else {
            (IconType::Key01, colors::brand())
        };

        let action = Button::new().on_press(run).text(self.confirm.action);
        let action = if self.confirm.danger {
            action.danger()
        } else {
            action.primary()
        };

        modal(
            400.,
            self.on_close.clone(),
            rect()
                .child(modal_title(Some(icon), self.confirm.title.clone(), None))
                .child(
                    label()
                        .text(self.confirm.body.clone())
                        .width(Size::fill())
                        .max_lines(4)
                        .font_size(12.)
                        .color(colors::fg_secondary()),
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
                                .on_press(move |_| cancel.call(()))
                                .text(t("oneclient.common.cancel")),
                        )
                        .child(action),
                ),
        )
    }
}

fn presence_text(presence: Presence) -> (String, Color) {
    match presence {
        Presence::Online => (
            t("oneclient.components.lobby.presence.online").to_string(),
            colors::success(),
        ),
        Presence::Playing(server) => (
            tf("oneclient.components.lobby.presence.playing", &[&server]),
            colors::code_info(),
        ),
        Presence::Offline { hours_ago } if hours_ago < 24 => (
            tf(
                "oneclient.components.lobby.presence.offline_hours",
                &[&hours_ago],
            ),
            colors::fg_secondary(),
        ),
        Presence::Offline { hours_ago } => (
            tf(
                "oneclient.components.lobby.presence.offline_days",
                &[&(hours_ago / 24)],
            ),
            colors::fg_secondary(),
        ),
    }
}

#[derive(PartialEq)]
pub struct InviteDialog {
    pub on_close: EventHandler<()>,
}

impl Component for InviteDialog {
    fn render(&self) -> impl IntoElement {
        let query = use_state(String::new);
        let Some(lobby) = use_lobby().read().lobby.clone() else {
            return rect().into_element();
        };

        let needle = query.read().trim().to_lowercase();
        let matches: Vec<Friend> = FRIENDS
            .iter()
            .filter(|friend| friend.name.to_lowercase().contains(&needle))
            .copied()
            .collect();

        let mut list = rect().vertical().width(Size::fill()).spacing(2.);
        for friend in &matches {
            list = list.child(FriendRow {
                friend: *friend,
                in_lobby: lobby.has(friend.id),
                invited: lobby.is_invited(friend.id),
                full: lobby.is_full(),
            });
        }
        if matches.is_empty() && !needle.is_empty() {
            list = list.child(UnknownPlayerRow {
                name: query.read().trim().to_string(),
                full: lobby.is_full(),
            });
        }

        modal(
            440.,
            self.on_close.clone(),
            rect()
                .child(modal_title(
                    None,
                    t("oneclient.components.lobby.invite_players").to_string(),
                    Some(self.on_close.clone()),
                ))
                .child(
                    TextInput::new(query)
                        .auto_focus(true)
                        .placeholder(t("oneclient.components.lobby.invite.search"))
                        .leading(
                            Icon::new(IconType::SearchMd)
                                .size(16.)
                                .color(colors::fg_secondary()),
                        ),
                )
                .child(list)
                .child(
                    rect()
                        .vertical()
                        .width(Size::fill())
                        .spacing(6.)
                        .child(
                            label()
                                .text(t("oneclient.components.lobby.invite.share_code"))
                                .font_size(11.)
                                .letter_spacing(0.8)
                                .font_weight(FontWeight::SEMI_BOLD)
                                .color(colors::fg_secondary()),
                        )
                        .child(
                            label()
                                .text(lobby.code)
                                .font_family(theme::MONO_FONT)
                                .font_size(14.)
                                .letter_spacing(1.)
                                .color(colors::fg_primary()),
                        )
                        .maybe_child(lobby.is_full().then(|| {
                            label()
                                .text(t("oneclient.components.lobby.full"))
                                .width(Size::fill())
                                .max_lines(2)
                                .font_size(12.)
                                .color(colors::code_warn())
                                .into_element()
                        })),
                ),
        )
    }
}

#[derive(PartialEq)]
struct FriendRow {
    friend: Friend,
    in_lobby: bool,
    invited: bool,
    full: bool,
}

impl Component for FriendRow {
    fn render(&self) -> impl IntoElement {
        let mut state = use_lobby();
        let dispatch = use_dispatch();
        let friend = self.friend;
        let (presence, presence_color) = presence_text(friend.presence);
        let offline = matches!(friend.presence, Presence::Offline { .. });

        let trailing = if self.in_lobby {
            label()
                .text(t("oneclient.components.lobby.invite.in_lobby"))
                .font_size(12.)
                .color(colors::fg_secondary())
                .into_element()
        } else if self.invited {
            Button::new()
                .secondary()
                .small()
                .enabled(false)
                .child(Icon::new(IconType::Check).size(12.))
                .text(t("oneclient.components.lobby.invite.invited"))
                .into_element()
        } else {
            Button::new()
                .primary()
                .small()
                .enabled(!self.full)
                .on_press(move |_| {
                    if let Some(lobby) = state.write().lobby.as_mut() {
                        lobby.invite(friend.id);
                    }
                    dispatch
                        .notify(tf(
                            "oneclient.components.lobby.toast.invited",
                            &[&friend.name],
                        ))
                        .info()
                        .icon(IconType::Users01)
                        .toast_only()
                        .send();
                })
                .text(if offline {
                    t("oneclient.components.lobby.invite.invite_anyway")
                } else {
                    t("oneclient.components.lobby.invite.invite")
                })
                .into_element()
        };

        player_row(
            friend.uuid.to_string(),
            friend.name.to_string(),
            presence,
            presence_color,
            trailing,
        )
    }
}

#[derive(PartialEq)]
struct UnknownPlayerRow {
    name: String,
    full: bool,
}

impl Component for UnknownPlayerRow {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let name = self.name.clone();

        player_row(
            uuid::Uuid::nil().to_string(),
            self.name.clone(),
            t("oneclient.components.lobby.invite.not_friend").to_string(),
            colors::fg_secondary(),
            Button::new()
                .primary()
                .small()
                .enabled(!self.full)
                .on_press(move |_| {
                    dispatch
                        .notify(tf("oneclient.components.lobby.toast.invited", &[&name]))
                        .info()
                        .icon(IconType::Users01)
                        .toast_only()
                        .send();
                })
                .text(t("oneclient.components.lobby.invite.send"))
                .into_element(),
        )
    }
}

fn player_row(
    uuid: String,
    name: String,
    sub: String,
    sub_color: Color,
    trailing: Element,
) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(10.)
        .padding(Gaps::new_symmetric(6., 8.))
        .corner_radius(CornerRadius::new_all(8.))
        .child(Avatar::new(uuid).size(32.))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(2.)
                .child(
                    label()
                        .text(name)
                        .font_size(13.)
                        .font_weight(FontWeight::MEDIUM)
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(sub)
                        .font_size(11.)
                        .max_lines(1)
                        .color(sub_color),
                ),
        )
        .child(trailing)
        .into_element()
}

#[derive(PartialEq)]
pub struct JoinDialog {
    pub on_close: EventHandler<()>,
}

impl Component for JoinDialog {
    fn render(&self) -> impl IntoElement {
        let mut state = use_lobby();
        let dispatch = use_dispatch();
        let me = use_my_name();
        let code = use_state(String::new);
        let on_close = self.on_close.clone();

        let valid = code.read().trim().len() >= 4;
        let mut join = move || {
            let lobby = Lobby::joined();
            let leader = display_name(lobby.leader, &me);
            state.write().lobby = Some(lobby);
            dispatch
                .notify(tf("oneclient.components.lobby.toast.joined", &[&leader]))
                .info()
                .icon(IconType::Users01)
                .toast_only()
                .send();
            on_close.call(());
        };
        let mut submit = join.clone();

        modal(
            400.,
            self.on_close.clone(),
            rect()
                .child(modal_title(
                    None,
                    t("oneclient.components.lobby.join.title").to_string(),
                    Some(self.on_close.clone()),
                ))
                .child(
                    label()
                        .text(t("oneclient.components.lobby.join.body"))
                        .width(Size::fill())
                        .max_lines(3)
                        .font_size(12.)
                        .color(colors::fg_secondary()),
                )
                .child(
                    TextInput::new(code)
                        .auto_focus(true)
                        .placeholder("OC-XXXX-XXX")
                        .leading(
                            Icon::new(IconType::Key01)
                                .size(16.)
                                .color(colors::fg_secondary()),
                        )
                        .on_submit(move |value: String| {
                            if value.trim().len() >= 4 {
                                submit();
                            }
                        }),
                )
                .child(
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .main_align(Alignment::End)
                        .child(
                            Button::new()
                                .primary()
                                .enabled(valid)
                                .on_press(move |_| join())
                                .text(t("oneclient.components.lobby.join.action")),
                        ),
                ),
        )
    }
}
