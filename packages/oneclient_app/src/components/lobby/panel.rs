use std::time::Instant;

use freya::prelude::*;
use oneclient_polyplus::party::{
    self, LocationKind, MAX_PARTY_SIZE, Party, PartyInvite, PartyMember,
};
use uuid::Uuid;

use super::dialogs::{Confirm, ConfirmDialog, InviteDialog};
use super::{
    LobbyState, act, can_invite, is_full, is_leader, online_count, refresh, sorted_members,
    successor, use_lobby, use_my_uuid, use_player_name,
};
use crate::components::cluster_menu::open_menu_at;
use crate::components::{
    Avatar, Button, ContextMenu, Icon, IconType, ScrollArea,
};
use crate::hooks::use_dispatch;
use crate::i18n::{t, tf};
use crate::theme::colors;
use crate::ui::{border_all_color, divider, relative_time};

#[derive(Clone, PartialEq)]
pub(super) enum Dialog {
    Invite,
    Confirm(Confirm),
}

#[derive(PartialEq)]
pub struct PartyTab;

impl Component for PartyTab {
    fn render(&self) -> impl IntoElement {
        let state = use_lobby();
        let dialog = use_state(|| None::<Dialog>);

        use_hook(move || {
            spawn(refresh(state));
        });

        let body = match state.read().party.clone() {
            Some(party) => InLobby { party, dialog }.into_element(),
            None => NoLobby.into_element(),
        };

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .padding(Gaps::new_all(12.))
            .child(body)
            .child(DialogHost { dialog })
    }
}

fn section_label(text: impl Into<String>) -> Label {
    label()
        .text(text.into())
        .font_size(11.)
        .letter_spacing(0.8)
        .font_weight(FontWeight::SEMI_BOLD)
        .color(colors::fg_secondary())
}

fn section_header(title: &'static str, count: Option<String>) -> Rect {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(6.)
        .padding(Gaps::new_symmetric(0., 4.))
        .child(section_label(title))
        .maybe_child(count.map(|count| {
            section_label(count)
                .color(colors::fg_secondary_hover())
                .into_element()
        }))
}

fn header(title: String, subtitle: Option<String>) -> Rect {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(4.)
        .padding(Gaps::new(2., 0., 2., 4.))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(2.)
                .child(
                    label()
                        .text(title)
                        .font_size(16.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis)
                        .color(colors::fg_strong()),
                )
                .maybe_child(subtitle.map(|subtitle| {
                    label()
                        .text(subtitle)
                        .font_size(12.)
                        .color(colors::fg_secondary())
                        .into_element()
                })),
        )
}

fn since(at: chrono::DateTime<chrono::Utc>) -> String {
    let elapsed = (chrono::Utc::now() - at).to_std().unwrap_or_default();
    relative_time(Instant::now().checked_sub(elapsed).unwrap_or_else(Instant::now))
}

#[derive(PartialEq)]
struct NoLobby;

impl Component for NoLobby {
    fn render(&self) -> impl IntoElement {
        let state = use_lobby();
        let dispatch = use_dispatch();
        let snapshot = state.read().clone();

        let create = move |_| {
            let dispatch_done = dispatch.clone();
            act(
                state,
                dispatch.clone(),
                |account| async move { party::create(&account).await },
                move |lobby: &mut LobbyState, party| {
                    lobby.party = Some(party);
                    dispatch_done
                        .notify(t("oneclient.components.lobby.toast.created"))
                        .body(t("oneclient.components.lobby.toast.created_body"))
                        .info()
                        .icon(IconType::Users01)
                        .toast_only()
                        .send();
                },
            );
        };

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(10.)
            .child(header(
                t("oneclient.components.lobby.title").to_string(),
                None,
            ))
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .cross_align(Alignment::Center)
                    .spacing(10.)
                    .padding(Gaps::new(12., 16., 16., 16.))
                    .child(
                        rect()
                            .width(Size::px(52.))
                            .height(Size::px(52.))
                            .corner_radius(CornerRadius::new_all(14.))
                            .background(colors::component_bg())
                            .border(border_all_color(1., colors::component_border()))
                            .center()
                            .child(
                                Icon::new(IconType::Users01)
                                    .size(24.)
                                    .color(colors::fg_secondary()),
                            ),
                    )
                    .child(
                        label()
                            .text(t("oneclient.components.lobby.empty.title"))
                            .font_size(15.)
                            .font_weight(FontWeight::SEMI_BOLD)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text(
                                snapshot
                                    .error
                                    .clone()
                                    .unwrap_or_else(|| {
                                        t("oneclient.components.lobby.empty.body").to_string()
                                    }),
                            )
                            .width(Size::fill())
                            .max_lines(3)
                            .text_align(TextAlign::Center)
                            .font_size(12.)
                            .color(if snapshot.error.is_some() {
                                colors::code_warn()
                            } else {
                                colors::fg_secondary()
                            }),
                    )
                    .child(
                        Button::new()
                            .primary()
                            .margin(Gaps::new(4., 0., 0., 0.))
                            .enabled(!snapshot.busy && snapshot.error.is_none())
                            .on_press(create)
                            .child(Icon::new(IconType::Plus).size(14.))
                            .text(t("oneclient.components.lobby.empty.create")),
                    ),
            )
            .children(incoming(&snapshot.incoming, false))
    }
}

#[derive(PartialEq)]
struct InLobby {
    party: Party,
    dialog: State<Option<Dialog>>,
}

impl Component for InLobby {
    fn render(&self) -> impl IntoElement {
        let me = use_my_uuid();
        let snapshot = use_lobby().read().clone();
        let party = &self.party;
        let mut dialog = self.dialog;
        let leads = is_leader(party, me);
        let leader_name = use_player_name(party.leader);
        let next = successor(party, me);
        let next_name = use_player_name(next.unwrap_or_default());

        let title = if leads {
            t("oneclient.components.lobby.your_lobby").to_string()
        } else {
            tf("oneclient.components.lobby.someones_lobby", &[&leader_name])
        };
        let subtitle = tf(
            "oneclient.components.lobby.summary",
            &[&party.members.len(), &MAX_PARTY_SIZE, &online_count(party)],
        );

        let invitable = can_invite(party, me) && !is_full(party);
        let outgoing = snapshot.party_outgoing();
        let leave = Confirm::leave(leads, next.map(|_| next_name), &leader_name);

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .content(Content::Flex)
            .spacing(10.)
            .child(header(title, Some(subtitle)))
            .child(
                ScrollArea::new()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .spacing(14.)
                    .child(
                        rect()
                            .vertical()
                            .width(Size::fill())
                            .spacing(4.)
                            .child(section_header(
                                t("oneclient.components.lobby.players"),
                                None,
                            ))
                            .children(sorted_members(party, me).into_iter().map(|member| {
                                MemberRow {
                                    member,
                                    leader: party.leader,
                                    me,
                                    viewer_leads: leads,
                                    dialog,
                                }
                                .into_element()
                            }))
                            .maybe_child(invitable.then(|| {
                                InviteSlot {
                                    on_press: (move |()| dialog.set(Some(Dialog::Invite))).into(),
                                }
                                .into_element()
                            })),
                    )
                    .append_children((!outgoing.is_empty()).then(|| pending(&outgoing)))
                    .append_children(incoming(&snapshot.incoming, true)),
            )
            .child(divider())
            .child(footer(
                party,
                me,
                &leader_name,
                leave,
                snapshot.busy,
                dialog,
            ))
    }
}

fn presence(member: &PartyMember) -> (String, Color) {
    if !member.online {
        return (
            t("oneclient.components.lobby.presence.offline").to_string(),
            colors::fg_secondary(),
        );
    }
    let Some(state) = &member.state else {
        return (
            t("oneclient.components.lobby.presence.online").to_string(),
            colors::success(),
        );
    };
    let location = &state.location;
    match location.kind {
        LocationKind::Menu => (
            t("oneclient.components.lobby.presence.menu").to_string(),
            colors::success(),
        ),
        LocationKind::Singleplayer => (
            t("oneclient.components.lobby.presence.singleplayer").to_string(),
            colors::code_info(),
        ),
        LocationKind::Server => (
            location
                .display
                .as_deref()
                .filter(|display| !display.trim().is_empty())
                .map_or_else(
                    || t("oneclient.components.lobby.presence.server").to_string(),
                    |display| tf("oneclient.components.lobby.presence.playing", &[&display]),
                ),
            colors::code_info(),
        ),
        LocationKind::HostedWorld => (
            t("oneclient.components.lobby.presence.hosting").to_string(),
            colors::code_info(),
        ),
        LocationKind::Other => (
            t("oneclient.components.lobby.presence.online").to_string(),
            colors::success(),
        ),
    }
}

fn tag(text: &'static str, color: Color) -> Element {
    rect()
        .padding(Gaps::new_symmetric(1., 6.))
        .corner_radius(CornerRadius::new_all(5.))
        .background(color.with_a(36))
        .child(
            label()
                .text(text)
                .font_size(10.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(color),
        )
        .into_element()
}

#[derive(PartialEq)]
struct MemberRow {
    member: PartyMember,
    leader: Uuid,
    me: Option<Uuid>,
    viewer_leads: bool,
    dialog: State<Option<Dialog>>,
}

impl Component for MemberRow {
    fn render(&self) -> impl IntoElement {
        let menu = use_state(|| None::<(f32, f32)>);
        let mut dialog = self.dialog;

        let player = self.member.player;
        let name = use_player_name(player);
        let is_me = self.me == Some(player);
        let (status_label, status_color) = presence(&self.member);
        let manageable = self.viewer_leads && !is_me;

        let context = (*menu.read()).map(|(x, y)| {
            let mut menu = menu;
            let promote_name = name.clone();
            let kick_name = name.clone();
            ContextMenu::new(x, y)
                .title(name.clone())
                .on_close(move |()| menu.set(None))
                .action(
                    IconType::Key01,
                    t("oneclient.components.lobby.member.make_leader"),
                    move |()| {
                        menu.set(None);
                        dialog.set(Some(Dialog::Confirm(Confirm::promote(
                            player,
                            promote_name.clone(),
                        ))));
                    },
                )
                .separator()
                .danger_action(
                    IconType::LogOut01,
                    t("oneclient.components.lobby.member.kick"),
                    move |()| {
                        menu.set(None);
                        dialog.set(Some(Dialog::Confirm(Confirm::kick(
                            player,
                            kick_name.clone(),
                        ))));
                    },
                )
                .into_element()
        });

        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(10.)
            .padding(Gaps::new_all(8.))
            .corner_radius(CornerRadius::new_all(10.))
            .background(colors::card())
            .border(border_all_color(1., colors::component_border()))
            .maybe(manageable, |el| el.on_secondary_down(open_menu_at(menu)))
            .child(
                rect()
                    .opacity(if self.member.online { 1. } else { 0.6 })
                    .child(Avatar::new(player.to_string()).size(32.)),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(3.)
                    .child(
                        rect()
                            .horizontal()
                            .cross_align(Alignment::Center)
                            .spacing(6.)
                            .child(
                                label()
                                    .text(name.clone())
                                    .font_size(13.)
                                    .font_weight(FontWeight::MEDIUM)
                                    .max_lines(1)
                                    .text_overflow(TextOverflow::Ellipsis)
                                    .color(colors::fg_primary()),
                            )
                            .maybe_child((player == self.leader).then(|| {
                                tag(
                                    t("oneclient.components.lobby.member.leader"),
                                    colors::brand(),
                                )
                            }))
                            .maybe_child(is_me.then(|| {
                                tag(
                                    t("oneclient.components.lobby.member.you"),
                                    colors::fg_secondary(),
                                )
                            })),
                    )
                    .child(
                        label()
                            .text(status_label)
                            .font_size(11.)
                            .font_weight(FontWeight::MEDIUM)
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .color(status_color),
                    ),
            )
            .maybe_child(manageable.then(|| {
                Button::new()
                    .ghost()
                    .icon()
                    .tooltip(t("oneclient.components.lobby.member.more"))
                    .on_press(open_menu_at(menu))
                    .child(
                        Icon::new(IconType::DotsVertical)
                            .size(16.)
                            .color(colors::fg_primary().with_a(178)),
                    )
                    .into_element()
            }))
            .maybe_child(context)
    }
}

#[derive(PartialEq)]
struct InviteSlot {
    on_press: EventHandler<()>,
}

impl Component for InviteSlot {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let on_press = self.on_press.clone();
        let color = if *hovered.read() {
            colors::fg_primary()
        } else {
            colors::fg_secondary()
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(48.))
            .center()
            .spacing(8.)
            .corner_radius(CornerRadius::new_all(10.))
            .background(if *hovered.read() {
                colors::ghost_overlay_hover()
            } else {
                colors::ghost_overlay()
            })
            .border(border_all_color(1., colors::component_border()))
            .cursor(CursorIcon::Pointer)
            .a11y_role(AccessibilityRole::Button)
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_leave(move |_| hovered.set(false))
            .on_press(move |_| on_press.call(()))
            .child(Icon::new(IconType::Plus).size(16.).color(color))
            .child(
                label()
                    .text(t("oneclient.components.lobby.invite_slot"))
                    .font_size(13.)
                    .font_weight(FontWeight::MEDIUM)
                    .color(color),
            )
    }
}

fn pending(invites: &[PartyInvite]) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(4.)
        .child(section_header(
            t("oneclient.components.lobby.pending"),
            Some(invites.len().to_string()),
        ))
        .children(invites.iter().map(|invite| {
            PendingRow {
                invite: invite.clone(),
            }
            .into_element()
        }))
        .into_element()
}

#[derive(PartialEq)]
struct PendingRow {
    invite: PartyInvite,
}

impl Component for PendingRow {
    fn render(&self) -> impl IntoElement {
        let state = use_lobby();
        let busy = state.read().busy;
        let dispatch = use_dispatch();
        let recipient = self.invite.recipient;
        let id = self.invite.id;
        let name = use_player_name(recipient);

        let cancel = {
            let name = name.clone();
            move |_| {
                let dispatch_done = dispatch.clone();
                let name = name.clone();
                act(
                    state,
                    dispatch.clone(),
                    move |account| async move { party::cancel_invite(&account, id).await },
                    move |lobby: &mut LobbyState, ()| {
                        lobby.drop_invite(id);
                        dispatch_done
                            .notify(tf(
                                "oneclient.components.lobby.toast.invite_cancelled",
                                &[&name],
                            ))
                            .body(t("oneclient.components.lobby.toast.invite_cancelled_body"))
                            .info()
                            .toast_only()
                            .send();
                    },
                );
            }
        };

        invite_row(
            recipient,
            name,
            format!(
                "{} · {}",
                t("oneclient.components.lobby.invited_by_you"),
                since(self.invite.created_at)
            ),
            Button::new()
                .ghost()
                .small()
                .enabled(!busy)
                .on_press(cancel)
                .text(t("oneclient.common.cancel"))
                .into_element(),
        )
    }
}

fn invite_row(player: Uuid, name: String, sub: String, trailing: Element) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(10.)
        .padding(Gaps::new_symmetric(6., 8.))
        .child(
            rect()
                .opacity(0.6)
                .child(Avatar::new(player.to_string()).size(32.)),
        )
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
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(sub)
                        .font_size(11.)
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(trailing)
        .into_element()
}

fn incoming(invites: &[PartyInvite], in_party: bool) -> Option<Element> {
    (!invites.is_empty()).then(|| {
        rect()
            .vertical()
            .width(Size::fill())
            .spacing(4.)
            .child(section_header(
                t("oneclient.components.lobby.incoming.title"),
                Some(invites.len().to_string()),
            ))
            .children(invites.iter().map(|invite| {
                IncomingRow {
                    invite: invite.clone(),
                    in_party,
                }
                .into_element()
            }))
            .into_element()
    })
}

#[derive(PartialEq)]
struct IncomingRow {
    invite: PartyInvite,
    in_party: bool,
}

impl Component for IncomingRow {
    fn render(&self) -> impl IntoElement {
        let state = use_lobby();
        let busy = state.read().busy;
        let dispatch = use_dispatch();
        let sender = self.invite.sender;
        let id = self.invite.id;
        let leave_current = self.in_party;
        let name = use_player_name(sender);

        let accept = {
            let dispatch = dispatch.clone();
            let name = name.clone();
            move |_| {
                let dispatch_done = dispatch.clone();
                let name = name.clone();
                act(
                    state,
                    dispatch.clone(),
                    move |account| async move { party::accept(&account, id, leave_current).await },
                    move |lobby: &mut LobbyState, party| {
                        lobby.party = Some(party);
                        lobby.incoming.clear();
                        dispatch_done
                            .notify(tf("oneclient.components.lobby.toast.joined", &[&name]))
                            .body(t("oneclient.components.lobby.toast.joined_body"))
                            .info()
                            .icon(IconType::Users01)
                            .toast_only()
                            .send();
                    },
                );
            }
        };
        let decline = move |_| {
            act(
                state,
                dispatch.clone(),
                move |account| async move { party::decline(&account, id).await },
                move |lobby: &mut LobbyState, ()| lobby.drop_invite(id),
            );
        };

        invite_row(
            sender,
            name,
            format!(
                "{} · {}",
                tf(
                    "oneclient.components.lobby.incoming.size",
                    &[&self.invite.party_size, &MAX_PARTY_SIZE]
                ),
                since(self.invite.created_at)
            ),
            rect()
                .horizontal()
                .spacing(6.)
                .child(
                    Button::new()
                        .ghost()
                        .small()
                        .enabled(!busy)
                        .on_press(decline)
                        .text(t("oneclient.components.lobby.incoming.decline")),
                )
                .child(
                    Button::new()
                        .primary()
                        .small()
                        .enabled(!busy)
                        .on_press(accept)
                        .text(t("oneclient.components.lobby.incoming.accept")),
                )
                .into_element(),
        )
    }
}

fn footer(
    party: &Party,
    me: Option<Uuid>,
    leader_name: &str,
    leave: Confirm,
    busy: bool,
    dialog: State<Option<Dialog>>,
) -> Element {
    let mut dialog = dialog;
    let full = is_full(party);
    let invite = if can_invite(party, me) {
        Button::new()
            .primary()
            .width(Size::fill())
            .enabled(!full && !busy)
            .tooltip(if full {
                t("oneclient.components.lobby.full")
            } else {
                t("oneclient.components.lobby.invite_players")
            })
            .on_press(move |_| dialog.set(Some(Dialog::Invite)))
            .child(Icon::new(IconType::Plus).size(16.))
            .text(t("oneclient.components.lobby.invite_players"))
            .into_element()
    } else {
        label()
            .text(tf(
                "oneclient.components.lobby.only_leader_invites",
                &[&leader_name],
            ))
            .width(Size::fill())
            .text_align(TextAlign::Center)
            .font_size(12.)
            .color(colors::fg_secondary())
            .into_element()
    };

    let disband = Confirm::disband(party.members.len());

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(8.)
        .child(invite)
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::Flex)
                .spacing(8.)
                .child(
                    Button::new()
                        .secondary()
                        .width(Size::flex(1.0))
                        .enabled(!busy)
                        .on_press(move |_| dialog.set(Some(Dialog::Confirm(leave.clone()))))
                        .child(Icon::new(IconType::LogOut01).size(14.))
                        .text(t("oneclient.components.lobby.leave")),
                )
                .maybe_child(is_leader(party, me).then(|| {
                    Button::new()
                        .secondary()
                        .width(Size::flex(1.0))
                        .enabled(!busy)
                        .foreground(colors::danger())
                        .on_press(move |_| dialog.set(Some(Dialog::Confirm(disband.clone()))))
                        .child(
                            Icon::new(IconType::Trash01)
                                .size(14.)
                                .color(colors::danger()),
                        )
                        .text(t("oneclient.components.lobby.disband"))
                        .into_element()
                })),
        )
        .into_element()
}

#[derive(PartialEq)]
struct DialogHost {
    dialog: State<Option<Dialog>>,
}

impl Component for DialogHost {
    fn render(&self) -> impl IntoElement {
        let mut dialog = self.dialog;
        let close = move |()| dialog.set(None);

        match dialog.read().clone() {
            Some(Dialog::Invite) => InviteDialog {
                on_close: close.into(),
            }
            .into_element(),
            Some(Dialog::Confirm(confirm)) => ConfirmDialog {
                confirm,
                on_close: close.into(),
            }
            .into_element(),
            None => rect().into_element(),
        }
    }
}

#[cfg(test)]
mod tests {
    use freya_testing::prelude::*;
    use oneclient_polyplus::party::{InvitePolicy, PartySettings};
    use tokio::sync::mpsc;

    use super::*;
    use crate::hooks::{Actions, use_provide_station};
    use crate::state::AppState;

    const NAMED: u128 = 0xabcdef12 << 96;

    fn member(id: u128) -> PartyMember {
        PartyMember {
            player: Uuid::from_u128(id),
            joined_at: chrono::Utc::now(),
            online: true,
            state: None,
        }
    }

    fn app() -> impl IntoElement {
        let station = use_provide_station(AppState::default);
        use_hook(move || {
            let (pump, _) = mpsc::unbounded_channel();
            let (events, _) = oneclient_events::EventBus::channel();
            GlobalContexts::get().insert_context(Actions::new(station, pump, events));
        });
        let dialog = use_state(|| None::<Dialog>);

        rect()
            .width(Size::px(348.))
            .height(Size::px(600.))
            .child(InLobby {
                party: Party {
                    id: Uuid::from_u128(100),
                    leader: Uuid::from_u128(1),
                    settings: PartySettings {
                        invite_policy: InvitePolicy::Leader,
                    },
                    chat_group_id: None,
                    members: vec![member(1), member(NAMED)],
                },
                dialog,
            })
    }

    #[test]
    fn a_party_with_no_invites_still_lists_its_members() {
        let test = launch_test(app);

        let row = test.find(|_, element| {
            Label::try_downcast(element).filter(|label| label.text.as_ref() == "abcdef12")
        });

        assert!(row.is_some(), "the member row should be mounted");
    }
}
