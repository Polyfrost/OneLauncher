use freya::prelude::*;
use uuid::Uuid;

use oneclient_polyplus::GroupKind;

use oneclient_polyplus::{BlockedPlayer, Friend};

use crate::chat::{ChatConversation, ChatInbox};
use crate::components::{Button, Icon, IconType, ScrollArea};
use crate::hooks::{
    settled_or_loading, use_blocked_players, use_chat_inbox, use_current_account, use_dispatch,
    use_friend_requests, use_friends,
};
use crate::theme::colors;

use super::common::{
    ROW_RADIUS_PX, SIDEBAR_WIDTH_PX, player_avatar, presence_dot, use_player_name,
};
use super::people::{NewGroupDialog, PeopleDialog};
use super::thread::ThreadView;

#[derive(PartialEq)]
pub(super) struct ChatSurface;

impl Component for ChatSurface {
    fn render(&self) -> impl IntoElement {
        let chat = use_chat_inbox();
        let dispatch = use_dispatch();

        let account = settled_or_loading(&use_current_account());
        if let Some(account) = account.clone() {
            dispatch.sync_chat_owner(account.map(|account| account.id));
        }

        let own_id = account
            .flatten()
            .map(|account| account.id)
            .unwrap_or_else(Uuid::nil);

        use_hook({
            let dispatch = dispatch.clone();
            move || dispatch.refresh_chat()
        });

        refetch_on_focus(&dispatch);

        let mut people_open = use_state(|| false);
        let mut group_open = use_state(|| false);

        let friends: Vec<Friend> = settled_or_loading(&use_friends()).unwrap_or_default();
        let requests = settled_or_loading(&use_friend_requests()).unwrap_or_default();
        let blocked: Vec<BlockedPlayer> =
            settled_or_loading(&use_blocked_players()).unwrap_or_default();

        let group_friends = friends.clone();

        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::fill())
            .content(Content::Flex)
            .overflow(Overflow::Clip)
            .child(sidebar(
                &chat,
                own_id,
                EventHandler::new(move |_| people_open.set(true)),
                EventHandler::new(move |_| group_open.set(true)),
            ))
            .child(
                match chat.active.and_then(|id| chat.conversation(id).cloned()) {
                    Some(conversation) => ThreadView {
                        conversation,
                        own_id,
                    }
                    .into_element(),
                    None => empty_state(&chat),
                },
            )
            .maybe_child(people_open().then(|| {
                PeopleDialog {
                    friends,
                    requests,
                    blocked,
                    on_close: EventHandler::new(move |_| people_open.set(false)),
                }
                .into_element()
            }))
            .maybe_child(group_open().then(|| {
                NewGroupDialog {
                    friends: group_friends,
                    on_close: EventHandler::new(move |_| group_open.set(false)),
                }
                .into_element()
            }))
    }
}

fn refetch_on_focus(dispatch: &crate::hooks::Actions) {
    let focused = *Platform::get().is_app_focused.read();
    let mut was_focused = use_state(|| focused);

    if focused && !*was_focused.peek() {
        dispatch.sync_chat();
    }

    was_focused.set_if_modified(focused);
}

fn sidebar(
    chat: &ChatInbox,
    own_id: Uuid,
    on_people: EventHandler<Event<PressEventData>>,
    on_new_group: EventHandler<Event<PressEventData>>,
) -> Element {
    let conversations = chat.conversations.clone();
    let active = chat.active;

    crate::ui::glass_panel()
        .vertical()
        .width(Size::px(SIDEBAR_WIDTH_PX))
        .height(Size::fill())
        .content(Content::Flex)
        .overflow(Overflow::Clip)
        .border(
            Border::new()
                .fill(colors::component_border())
                .width(BorderWidth {
                    top: 0.,
                    right: 1.,
                    bottom: 0.,
                    left: 0.,
                })
                .alignment(BorderAlignment::Inner),
        )
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .padding(Gaps::new(20., 12., 12., 16.))
                .spacing(4.)
                .content(Content::Flex)
                .cross_align(Alignment::Center)
                .child(
                    label()
                        .text("Conversations")
                        .font_size(20.)
                        .font_weight(FontWeight::BOLD)
                        .color(colors::fg_primary())
                        .width(Size::flex(1.0)),
                )
                .child(presence_dot(chat.connected))
                .child(
                    Button::new()
                        .ghost()
                        .icon()
                        .alt("People")
                        .on_press(on_people)
                        .child(Icon::new(IconType::Users01).size(18.)),
                )
                .child(
                    Button::new()
                        .ghost()
                        .icon()
                        .alt("New group")
                        .on_press(on_new_group)
                        .child(Icon::new(IconType::Plus).size(18.)),
                ),
        )
        .child(
            ScrollArea::new()
                .width(Size::fill())
                .height(Size::flex(1.0))
                .child(
                    rect()
                        .vertical()
                        .width(Size::fill())
                        .padding(Gaps::new(0., 8., 12., 8.))
                        .spacing(2.)
                        .children(
                            conversations
                                .into_iter()
                                .map(|conversation| {
                                    ConversationRow {
                                        selected: active == Some(conversation.id),
                                        conversation,
                                        own_id,
                                    }
                                    .into_element()
                                })
                                .collect::<Vec<_>>(),
                        ),
                ),
        )
        .into_element()
}

#[derive(PartialEq)]
struct ConversationRow {
    conversation: ChatConversation,
    own_id: Uuid,
    selected: bool,
}

impl Component for ConversationRow {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let counterpart = self.conversation.counterpart(self.own_id);
        let name = use_player_name(counterpart.unwrap_or(self.own_id));

        let title = self
            .conversation
            .name
            .clone()
            .or_else(|| counterpart.map(|_| name))
            .unwrap_or_else(|| match self.conversation.kind {
                GroupKind::Group => "Group".to_string(),
                _ => "Direct message".to_string(),
            });

        let group_id = self.conversation.id;
        let unread = self.conversation.unread;

        let background = if self.selected {
            colors::component_bg()
        } else {
            Color::TRANSPARENT
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .padding(Gaps::new(8., 10., 8., 10.))
            .spacing(10.)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .corner_radius(CornerRadius::from(ROW_RADIUS_PX))
            .background(background)
            .cursor(CursorIcon::Pointer)
            .on_all_press(move |_| dispatch.open_conversation(group_id))
            .maybe_child(counterpart.map(player_avatar))
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .overflow(Overflow::Clip)
                    .spacing(2.)
                    .child(
                        label()
                            .text(title)
                            .font_size(14.)
                            .font_weight(if unread {
                                FontWeight::BOLD
                            } else {
                                FontWeight::NORMAL
                            })
                            .color(colors::fg_primary())
                            .width(Size::fill())
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis),
                    )
                    .maybe_child(self.conversation.preview.as_ref().map(|preview| {
                        label()
                            .text(preview.clone())
                            .font_size(12.)
                            .color(colors::fg_secondary())
                            .width(Size::fill())
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                    })),
            )
            .maybe_child(unread.then(|| {
                rect()
                    .width(Size::px(8.))
                    .height(Size::px(8.))
                    .corner_radius(CornerRadius::from(4.))
                    .background(colors::brand())
            }))
    }
}

fn empty_state(chat: &ChatInbox) -> Element {
    let message = if chat.conversations.is_empty() {
        "No conversations yet. Add a friend to start talking."
    } else {
        "Pick a conversation to start reading."
    };

    rect()
        .vertical()
        .width(Size::flex(1.0))
        .height(Size::fill())
        .center()
        .spacing(8.)
        .child(Icon::new(IconType::DotsGrid).size(28.))
        .child(
            label()
                .text(message)
                .font_size(14.)
                .color(colors::fg_secondary()),
        )
        .maybe_child(chat.error.as_ref().map(|error| {
            label()
                .text(error.clone())
                .font_size(12.)
                .color(colors::danger())
        }))
        .into_element()
}
