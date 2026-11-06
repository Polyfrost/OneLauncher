use freya::prelude::*;
use oneclient_polyplus::party;
use uuid::Uuid;

use super::{
    FriendView, LobbyState, act, is_full, use_friends, use_lobby, use_my_uuid,
};
use crate::components::{
    Avatar, Button, Icon, IconType, OVERLAY_MAX_LEVEL, OverlayPopup, TextInput,
};
use crate::hooks::{Actions, use_dispatch};
use crate::i18n::{t, tf};
use crate::theme::{colors, rounded, text_size};
use crate::ui::border_all_color;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConfirmKind {
    Kick(Uuid),
    Promote(Uuid),
    Leave,
    Disband,
    Accept(Uuid),
}

#[derive(Clone, PartialEq, Debug)]
pub struct Confirm {
    pub kind: ConfirmKind,
    title: String,
    body: String,
    action: &'static str,
    danger: bool,
    toast: String,
    toast_body: &'static str,
}

impl Confirm {
    pub fn kick(player: Uuid, name: String) -> Self {
        Self {
            kind: ConfirmKind::Kick(player),
            title: tf("oneclient.components.lobby.confirm.kick.title", &[&name]),
            body: tf("oneclient.components.lobby.confirm.kick.body", &[&name]),
            action: t("oneclient.components.lobby.member.kick"),
            danger: true,
            toast: tf("oneclient.components.lobby.toast.kicked", &[&name]),
            toast_body: t("oneclient.components.lobby.toast.kicked_body"),
        }
    }

    pub fn promote(player: Uuid, name: String) -> Self {
        Self {
            kind: ConfirmKind::Promote(player),
            title: t("oneclient.components.lobby.confirm.promote.title").to_string(),
            body: tf("oneclient.components.lobby.confirm.promote.body", &[&name]),
            action: t("oneclient.components.lobby.member.make_leader"),
            danger: false,
            toast: tf("oneclient.components.lobby.toast.promoted", &[&name]),
            toast_body: t("oneclient.components.lobby.toast.promoted_body"),
        }
    }

    pub fn leave(leads: bool, successor: Option<String>, leader: &str) -> Self {
        let body = match (leads, successor) {
            (true, Some(next)) => tf(
                "oneclient.components.lobby.confirm.leave.body_leader",
                &[&next],
            ),
            (true, None) => t("oneclient.components.lobby.confirm.leave.body_alone").to_string(),
            (false, _) => tf(
                "oneclient.components.lobby.confirm.leave.body_member",
                &[&leader],
            ),
        };
        Self {
            kind: ConfirmKind::Leave,
            title: t("oneclient.components.lobby.confirm.leave.title").to_string(),
            body,
            action: t("oneclient.components.lobby.leave"),
            danger: true,
            toast: t("oneclient.components.lobby.toast.left").to_string(),
            toast_body: t("oneclient.components.lobby.toast.left_body"),
        }
    }

    pub fn accept(invite: Uuid, host: &str, leave: Self) -> Self {
        Self {
            kind: ConfirmKind::Accept(invite),
            title: t("oneclient.components.lobby.confirm.accept.title").to_string(),
            body: leave.body,
            action: t("oneclient.components.lobby.incoming.accept"),
            danger: true,
            toast: tf("oneclient.components.lobby.toast.joined", &[&host]),
            toast_body: t("oneclient.components.lobby.toast.joined_body"),
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
            toast_body: t("oneclient.components.lobby.toast.disbanded_body"),
        }
    }
}

fn run_confirm(state: State<LobbyState>, dispatch: Actions, confirm: Confirm) {
    let toast = confirm.toast.clone();
    let toast_body = confirm.toast_body;
    let notify = dispatch.clone();
    let announce = move || {
        notify
            .notify(toast)
            .body(toast_body)
            .info()
            .icon(IconType::Users01)
            .toast_only()
            .send();
    };
    match confirm.kind {
        ConfirmKind::Kick(player) => act(
            state,
            dispatch,
            move |account| async move { party::kick(&account, player).await },
            move |_: &mut LobbyState, ()| announce(),
        ),
        ConfirmKind::Promote(player) => act(
            state,
            dispatch,
            move |account| async move { party::transfer_leader(&account, player).await },
            move |lobby: &mut LobbyState, party| {
                lobby.party = Some(party);
                announce();
            },
        ),
        ConfirmKind::Leave => act(
            state,
            dispatch,
            |account| async move { party::leave(&account).await },
            move |lobby: &mut LobbyState, ()| {
                lobby.party = None;
                announce();
            },
        ),
        ConfirmKind::Accept(invite) => act(
            state,
            dispatch,
            move |account| async move { party::accept(&account, invite, true).await },
            move |lobby: &mut LobbyState, party| {
                lobby.apply(party::PartyPacket::PartyUpdated { party });
                announce();
            },
        ),
        ConfirmKind::Disband => act(
            state,
            dispatch,
            |account| async move { party::disband(&account).await },
            move |lobby: &mut LobbyState, ()| {
                lobby.party = None;
                announce();
            },
        ),
    }
}

pub(crate) fn modal(width: f32, on_close: EventHandler<()>, card: Rect) -> Element {
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
                        .corner_radius(rounded(14.))
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
                .font_size(text_size(16.))
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
        let state = use_lobby();
        let dispatch = use_dispatch();
        let confirm = self.confirm.clone();
        let on_close = self.on_close.clone();
        let cancel = self.on_close.clone();

        let run = move |_| {
            run_confirm(state, dispatch.clone(), confirm.clone());
            on_close.call(());
        };

        let icon = if self.confirm.danger {
            (IconType::AlertTriangle, colors::code_warn())
        } else {
            (IconType::Key01, colors::brand())
        };

        let action = Button::new()
            .enabled(!state.read().busy)
            .on_press(run)
            .text(self.confirm.action);
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
                        .font_size(text_size(12.))
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

#[derive(PartialEq)]
pub struct InviteDialog {
    pub on_close: EventHandler<()>,
}

impl Component for InviteDialog {
    fn render(&self) -> impl IntoElement {
        let query = use_state(String::new);
        let me = use_my_uuid();
        let friends = use_friends(me.unwrap_or_default());
        let snapshot = use_lobby().read().clone();
        let Some(party) = snapshot.party.clone() else {
            return rect().into_element();
        };
        let full = is_full(&party);

        let reader = friends.read();
        let list: Element = match (reader.state().ok(), reader.state().err()) {
            (Some(friends), _) => {
                let needle = query.read().trim().to_lowercase();
                let matches: Vec<&FriendView> = friends
                    .iter()
                    .filter(|friend| friend.name.to_lowercase().contains(&needle))
                    .collect();
                if matches.is_empty() {
                    hint(if friends.is_empty() {
                        t("oneclient.components.lobby.invite.no_friends")
                    } else {
                        t("oneclient.components.lobby.invite.no_match")
                    })
                } else {
                    matches
                        .into_iter()
                        .fold(
                            rect().vertical().width(Size::fill()).spacing(2.),
                            |list, friend| {
                                list.child(FriendRow {
                                    friend: friend.clone(),
                                    in_party: party
                                        .members
                                        .iter()
                                        .any(|member| member.player == friend.player),
                                    invited: snapshot.is_invited(friend.player),
                                    full,
                                    busy: snapshot.busy,
                                })
                            },
                        )
                        .into_element()
                }
            }
            (None, Some(err)) => hint_text(err.clone(), colors::code_warn()),
            (None, None) => hint(t("oneclient.components.lobby.invite.loading")),
        };

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
                .maybe_child(full.then(|| hint_text(
                    t("oneclient.components.lobby.full").to_string(),
                    colors::code_warn(),
                ))),
        )
    }
}

fn hint(text: &'static str) -> Element {
    hint_text(text.to_string(), colors::fg_secondary())
}

fn hint_text(text: String, color: Color) -> Element {
    label()
        .text(text)
        .width(Size::fill())
        .max_lines(3)
        .text_align(TextAlign::Center)
        .padding(Gaps::new_symmetric(12., 0.))
        .font_size(text_size(12.))
        .color(color)
        .into_element()
}

#[derive(PartialEq)]
struct FriendRow {
    friend: FriendView,
    in_party: bool,
    invited: bool,
    full: bool,
    busy: bool,
}

impl Component for FriendRow {
    fn render(&self) -> impl IntoElement {
        let state = use_lobby();
        let dispatch = use_dispatch();
        let friend = self.friend.clone();
        let (presence, presence_color) = if friend.online {
            (
                t("oneclient.components.lobby.presence.online"),
                colors::success(),
            )
        } else {
            (
                t("oneclient.components.lobby.presence.offline"),
                colors::fg_secondary(),
            )
        };

        let trailing = if self.in_party {
            label()
                .text(t("oneclient.components.lobby.invite.in_lobby"))
                .font_size(text_size(12.))
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
            let player = friend.player;
            let name = friend.name.clone();
            Button::new()
                .primary()
                .small()
                .enabled(!self.full && !self.busy)
                .on_press(move |_| {
                    let done = dispatch.clone();
                    let name = name.clone();
                    act(
                        state,
                        dispatch.clone(),
                        move |account| async move { party::invite(&account, player).await },
                        move |lobby: &mut LobbyState, invite| {
                            if !lobby.outgoing.iter().any(|known| known.id == invite.id) {
                                lobby.outgoing.push(invite);
                            }
                            done.notify(tf("oneclient.components.lobby.toast.invited", &[&name]))
                                .body(t("oneclient.components.lobby.toast.invited_body"))
                                .info()
                                .icon(IconType::Users01)
                                .toast_only()
                                .send();
                        },
                    );
                })
                .text(if friend.online {
                    t("oneclient.components.lobby.invite.invite")
                } else {
                    t("oneclient.components.lobby.invite.invite_anyway")
                })
                .into_element()
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(10.)
            .padding(Gaps::new_symmetric(6., 8.))
            .corner_radius(rounded(8.))
            .child(Avatar::new(friend.player.to_string()).size(32.))
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(2.)
                    .child(
                        label()
                            .text(friend.name.clone())
                            .font_size(text_size(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text(presence)
                            .font_size(text_size(11.))
                            .max_lines(1)
                            .color(presence_color),
                    ),
            )
            .child(trailing)
            .into_element()
    }
}
