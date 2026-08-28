use std::collections::HashSet;

use freya::prelude::*;
use uuid::Uuid;

use oneclient_polyplus::{BlockedPlayer, Friend, FriendRequest, RelationshipKind};

use crate::components::{
    Button, Icon, IconType, OverlayPopup, ScrollArea, TabBar, TabItem, TextInput,
};
use crate::hooks::{
    Actions, FriendRequests, settled_or_loading, use_blocked_players, use_chat_snapshot,
    use_dispatch, use_friend_requests, use_friends,
};
use crate::theme::colors;
use crate::ui::border_all_color;

use super::common::{hint, player_avatar, section_heading, use_player_name};

const CARD_BG: Color = Color::from_rgb(26, 34, 41);
const DIALOG_WIDTH_PX: f32 = 520.;
const DIALOG_HEIGHT_PX: f32 = 540.;
const LIST_RADIUS_PX: f32 = 8.;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PeopleTab {
    Friends,
    Requests,
    Blocked,
}

#[derive(PartialEq)]
pub(super) struct PeopleDialog {
    pub on_close: EventHandler<()>,
}

impl Component for PeopleDialog {
    fn render(&self) -> impl IntoElement {
        let chat = use_chat_snapshot();
        let mut tab = use_state(|| PeopleTab::Friends);

        let friends = settled_or_loading(&use_friends()).unwrap_or_default();
        let requests = settled_or_loading(&use_friend_requests()).unwrap_or_default();
        let blocked = settled_or_loading(&use_blocked_players()).unwrap_or_default();

        let pending = requests.incoming.len() + requests.outgoing.len();
        let active = *tab.read();

        let body = match active {
            PeopleTab::Friends => friends_list(&friends),
            PeopleTab::Requests => requests_list(&requests),
            PeopleTab::Blocked => blocked_list(&blocked),
        };

        let close = self.on_close.clone();

        dialog(
            self.on_close.clone(),
            rect()
                .vertical()
                .width(Size::fill())
                .height(Size::fill())
                .content(Content::Flex)
                .spacing(14.)
                .child(dialog_title("People", close))
                .child(AddFriendField)
                .child(
                    TabBar::new()
                        .height(Size::px(28.))
                        .font_size(13.)
                        .tabs(vec![
                            TabItem::new("Friends", active == PeopleTab::Friends)
                                .count_text(friends.len().to_string())
                                .on_press(EventHandler::new(move |_| {
                                    tab.set(PeopleTab::Friends);
                                })),
                            TabItem::new("Requests", active == PeopleTab::Requests)
                                .count_text(pending.to_string())
                                .on_press(EventHandler::new(move |_| {
                                    tab.set(PeopleTab::Requests);
                                })),
                            TabItem::new("Blocked", active == PeopleTab::Blocked)
                                .count_text(blocked.len().to_string())
                                .on_press(EventHandler::new(move |_| {
                                    tab.set(PeopleTab::Blocked);
                                })),
                        ]),
                )
                .child(
                    ScrollArea::new()
                        .width(Size::fill())
                        .height(Size::flex(1.0))
                        .child(body),
                )
                .maybe_child(chat.error.as_ref().map(|error| {
                    label()
                        .text(error.clone())
                        .font_size(11.)
                        .color(colors::danger())
                }))
                .into_element(),
        )
    }
}

#[derive(PartialEq)]
struct AddFriendField;

impl Component for AddFriendField {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let draft = use_state(String::new);

        let username = draft.read().clone();
        let can_send = !username.trim().is_empty();
        let on_key = dispatch.clone();

        rect()
            .horizontal()
            .width(Size::fill())
            .spacing(8.)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .child(
                TextInput::new(draft)
                    .width(Size::flex(1.0))
                    .placeholder("Add a friend by username")
                    .on_submit(move |_| send_request(&on_key, draft)),
            )
            .child(
                Button::new()
                    .primary()
                    .medium()
                    .text("Add")
                    .enabled(can_send)
                    .on_press(move |_| send_request(&dispatch, draft)),
            )
    }
}

fn send_request(dispatch: &Actions, mut draft: State<String>) {
    let username = draft.read().trim().to_string();
    if username.is_empty() {
        return;
    }

    dispatch.add_friend(username);
    draft.set(String::new());
}

fn friends_list(friends: &[Friend]) -> Element {
    if friends.is_empty() {
        return hint("No friends yet. Add one by username above.").into_element();
    }

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(4.)
        .children(
            friends
                .iter()
                .map(|friend| {
                    FriendRow {
                        player: friend.player,
                        best: friend.kind == RelationshipKind::BestFriend,
                    }
                    .into_element()
                })
                .collect::<Vec<_>>(),
        )
        .into_element()
}

#[derive(PartialEq)]
struct FriendRow {
    player: Uuid,
    best: bool,
}

impl Component for FriendRow {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let name = use_player_name(self.player);
        let player = self.player;

        let remove = dispatch.clone();
        let block = dispatch.clone();

        person_row(
            self.player,
            name,
            self.best.then(|| "Best friend".to_string()),
        )
        .child(
            Button::new()
                .secondary()
                .small()
                .text("Message")
                .on_press(move |_| dispatch.start_direct_message(player)),
        )
        .child(
            Button::new()
                .ghost()
                .small()
                .text("Remove")
                .on_press(move |_| remove.remove_friend(player)),
        )
        .child(
            Button::new()
                .ghost()
                .small()
                .text("Block")
                .on_press(move |_| block.block_player(player)),
        )
    }
}

fn requests_list(requests: &FriendRequests) -> Element {
    if requests.incoming.is_empty() && requests.outgoing.is_empty() {
        return hint("No pending friend requests.").into_element();
    }

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .maybe_child(
            (!requests.incoming.is_empty())
                .then(|| request_section("Incoming", &requests.incoming, true)),
        )
        .maybe_child(
            (!requests.outgoing.is_empty())
                .then(|| request_section("Sent", &requests.outgoing, false)),
        )
        .into_element()
}

fn request_section(title: &str, requests: &[FriendRequest], incoming: bool) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(4.)
        .child(section_heading(title.to_string()))
        .children(
            requests
                .iter()
                .map(|request| {
                    RequestRow {
                        request_id: request.id,
                        player: request.player,
                        incoming,
                    }
                    .into_element()
                })
                .collect::<Vec<_>>(),
        )
        .into_element()
}

#[derive(PartialEq)]
struct RequestRow {
    request_id: i32,
    player: Uuid,
    incoming: bool,
}

impl Component for RequestRow {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let name = use_player_name(self.player);
        let request_id = self.request_id;
        let decline = dispatch.clone();

        let row = person_row(self.player, name, None);

        if self.incoming {
            row.child(
                Button::new()
                    .primary()
                    .small()
                    .text("Accept")
                    .on_press(move |_| dispatch.accept_friend_request(request_id)),
            )
            .child(
                Button::new()
                    .ghost()
                    .small()
                    .text("Decline")
                    .on_press(move |_| decline.decline_friend_request(request_id)),
            )
        } else {
            row.child(
                Button::new()
                    .ghost()
                    .small()
                    .text("Cancel")
                    .on_press(move |_| dispatch.cancel_friend_request(request_id)),
            )
        }
    }
}

fn blocked_list(blocked: &[BlockedPlayer]) -> Element {
    if blocked.is_empty() {
        return hint("Nobody is blocked.").into_element();
    }

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(4.)
        .children(
            blocked
                .iter()
                .map(|entry| {
                    BlockedRow {
                        player: entry.player,
                    }
                    .into_element()
                })
                .collect::<Vec<_>>(),
        )
        .into_element()
}

#[derive(PartialEq)]
struct BlockedRow {
    player: Uuid,
}

impl Component for BlockedRow {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let name = use_player_name(self.player);
        let player = self.player;

        person_row(self.player, name, None).child(
            Button::new()
                .secondary()
                .small()
                .text("Unblock")
                .on_press(move |_| dispatch.unblock_player(player)),
        )
    }
}

fn person_row(player: Uuid, name: String, note: Option<String>) -> Rect {
    rect()
        .horizontal()
        .width(Size::fill())
        .padding(Gaps::new(6., 8., 6., 8.))
        .spacing(8.)
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .corner_radius(CornerRadius::from(LIST_RADIUS_PX))
        .background(colors::page_elevated())
        .child(player_avatar(player))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .child(
                    label()
                        .text(name)
                        .font_size(13.)
                        .color(colors::fg_primary())
                        .max_lines(1),
                )
                .maybe_child(note.map(|note| {
                    label()
                        .text(note)
                        .font_size(10.)
                        .color(colors::fg_secondary())
                })),
        )
}

#[derive(PartialEq)]
pub(super) struct NewGroupDialog {
    pub on_close: EventHandler<()>,
}

impl Component for NewGroupDialog {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let name = use_state(String::new);
        let mut picked = use_state(HashSet::<Uuid>::new);

        let friends = settled_or_loading(&use_friends()).unwrap_or_default();
        let chosen = picked.read().clone();

        let title = name.read().clone();
        let can_create = !title.trim().is_empty() && !chosen.is_empty();

        let close = self.on_close.clone();
        let creating = self.on_close.clone();

        let rows: Vec<Element> = friends
            .iter()
            .map(|friend| {
                let player = friend.player;
                MemberOption {
                    player,
                    selected: chosen.contains(&player),
                    on_toggle: EventHandler::new(move |_| {
                        let mut next = picked.read().clone();
                        if !next.remove(&player) {
                            next.insert(player);
                        }
                        picked.set(next);
                    }),
                }
                .into_element()
            })
            .collect();

        let members: Vec<Uuid> = chosen.iter().copied().collect();

        dialog(
            self.on_close.clone(),
            rect()
                .vertical()
                .width(Size::fill())
                .height(Size::fill())
                .content(Content::Flex)
                .spacing(14.)
                .child(dialog_title("New group", close))
                .child(
                    TextInput::new(name)
                        .width(Size::fill())
                        .placeholder("Group name"),
                )
                .child(section_heading("Members"))
                .child(
                    ScrollArea::new()
                        .width(Size::fill())
                        .height(Size::flex(1.0))
                        .child(if rows.is_empty() {
                            hint("Add friends first, then you can group them.").into_element()
                        } else {
                            rect()
                                .vertical()
                                .width(Size::fill())
                                .spacing(4.)
                                .children(rows)
                                .into_element()
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
                                .medium()
                                .text("Create")
                                .enabled(can_create)
                                .on_press(move |_| {
                                    dispatch.create_chat_group(title.clone(), members.clone());
                                    creating.call(());
                                }),
                        ),
                )
                .into_element(),
        )
    }
}

#[derive(PartialEq)]
struct MemberOption {
    player: Uuid,
    selected: bool,
    on_toggle: EventHandler<()>,
}

impl Component for MemberOption {
    fn render(&self) -> impl IntoElement {
        let name = use_player_name(self.player);
        let toggle = self.on_toggle.clone();
        let selected = self.selected;

        person_row(self.player, name, None)
            .cursor(CursorIcon::Pointer)
            .on_all_press(move |_| toggle.call(()))
            .child(
                rect()
                    .width(Size::px(18.))
                    .height(Size::px(18.))
                    .center()
                    .corner_radius(CornerRadius::from(4.))
                    .background(if selected {
                        colors::brand()
                    } else {
                        colors::component_bg()
                    })
                    .maybe_child(selected.then(|| Icon::new(IconType::Check).size(12.))),
            )
    }
}

fn dialog_title(text: &str, on_close: EventHandler<()>) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .child(
            label()
                .text(text.to_string())
                .font_size(16.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary())
                .width(Size::flex(1.0)),
        )
        .child(
            Button::new()
                .ghost()
                .icon()
                .on_press(move |_| on_close.call(()))
                .child(Icon::new(IconType::XClose).size(16.)),
        )
}

fn dialog(on_close: EventHandler<()>, body: Element) -> Element {
    OverlayPopup::new()
        .on_close(on_close)
        .child(
            rect()
                .width(Size::window_percent(100.))
                .height(Size::window_percent(100.))
                .center()
                .child(
                    rect()
                        .vertical()
                        .width(Size::px(DIALOG_WIDTH_PX))
                        .height(Size::px(DIALOG_HEIGHT_PX))
                        .max_width(Size::window_percent(92.))
                        .padding(Gaps::new_all(20.))
                        .corner_radius(CornerRadius::new_all(14.))
                        .background(CARD_BG)
                        .border(border_all_color(1., colors::component_border()))
                        .child(body),
                ),
        )
        .into_element()
}
