use freya::prelude::*;
use uuid::Uuid;

use oneclient_polyplus::{BlockedPlayer, Friend, FriendRequest, GroupKind, PlayerStatus};

use crate::chat::{ChatConversation, ChatInvite, ChatRoster, FriendRequestStatus};
use crate::components::{Avatar, ContextMenu, Icon, IconType, ScrollArea, TextInput};
use crate::hooks::{
    Actions, use_chat_friend_request, use_chat_invites, use_chat_self_status, use_dispatch,
    use_player_online,
};
use crate::i18n::{t, tf};
use crate::theme::colors;

use super::common::{
    CARD_RADIUS_PX, CHIP_RADIUS_PX, HeaderButton, ROW_RADIUS_PX, ago, card_bg, chip_bg,
    chip_button, count_badge, edge_border, group_heads, hairline, head_with_dot, hint, hover_bg,
    inset_border, popup_shadow, presence_color, row_time, section_heading, use_player_name,
};
use super::panel::{PanelMode, PanelTab, PanelView};
use super::people::NewGroupDialog;

#[derive(PartialEq)]
pub(super) struct ListPane {
    pub mode: PanelMode,
    pub expanded: bool,
    pub own_id: Uuid,
    pub username: String,
    pub connected: bool,
    pub conversations: Vec<ChatConversation>,
    pub loading: bool,
    pub error: Option<String>,
    pub active: Option<i32>,
    pub roster: ChatRoster,
    pub tab: State<PanelTab>,
    pub view: State<PanelView>,
}

impl Component for ListPane {
    fn render(&self) -> impl IntoElement {
        let query = use_state(String::new);
        let mut status_menu = use_state(|| false);
        let mut group_open = use_state(|| false);

        let tab = self.tab;
        let active_tab = *tab.read();
        let q = query.read().trim().to_lowercase();

        let unread = self
            .conversations
            .iter()
            .filter(|conversation| conversation.unread)
            .count();
        let incoming = self.roster.requests.incoming.len();

        let body = match active_tab {
            PanelTab::Chats => chats_tab(self, &q),
            PanelTab::Friends => FriendsTab {
                roster: self.roster.clone(),
                query: q.clone(),
                view: self.view,
            }
            .into_element(),
            PanelTab::Groups => {
                groups_tab(self, &q, EventHandler::new(move |_| group_open.set(true)))
            }
        };

        let group_friends = self.roster.friends.clone();

        rect()
            .vertical()
            .width(if self.expanded {
                Size::px(super::common::LIST_WIDTH_PX)
            } else {
                Size::fill()
            })
            .height(Size::fill())
            .content(Content::Flex)
            .maybe(self.expanded, |el| {
                el.border(edge_border(hairline(), 0., 1., 0.))
            })
            .child(PanelHeader {
                mode: self.mode,
                own_id: self.own_id,
                username: self.username.clone(),
                connected: self.connected,
                status_menu,
            })
            .child(
                rect()
                    .width(Size::fill())
                    .padding(Gaps::new(0., 16., 12., 16.))
                    .child(
                        TextInput::new(query)
                            .width(Size::fill())
                            .height(Size::px(32.))
                            .font_size(12.)
                            .placeholder(t("oneclient.chat.list.search_placeholder"))
                            .leading(
                                Icon::new(IconType::SearchMd)
                                    .size(14.)
                                    .color(colors::fg_secondary())
                                    .into_element(),
                            ),
                    ),
            )
            .child(InvitesCard { view: self.view })
            .child(tab_strip(active_tab, unread, incoming, move |next| {
                let mut tab = tab;
                tab.set(next);
            }))
            .maybe_child(self.error.clone().map(|error| ErrorBanner { error }))
            .child(
                ScrollArea::new()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .child(
                        rect()
                            .vertical()
                            .width(Size::fill())
                            .padding(Gaps::new(4., 8., 16., 8.))
                            .child(body),
                    ),
            )
            .maybe_child(status_menu().then(|| {
                StatusMenu {
                    on_close: EventHandler::new(move |_| status_menu.set(false)),
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

#[derive(PartialEq)]
struct PanelHeader {
    mode: PanelMode,
    own_id: Uuid,
    username: String,
    connected: bool,
    status_menu: State<bool>,
}

impl Component for PanelHeader {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let picked = use_chat_self_status();
        let mut status_menu = self.status_menu;

        let (dot, status) = match (self.connected, picked) {
            (false, _) => (
                super::common::offline_dot(),
                t("oneclient.chat.status.offline"),
            ),
            (true, PlayerStatus::Online) => (colors::success(), t("oneclient.chat.status.online")),
            (true, PlayerStatus::Offline) => (
                super::common::offline_dot(),
                t("oneclient.chat.status.invisible"),
            ),
        };
        let pop_out = dispatch.clone();
        let dock = dispatch.clone();
        let close = dispatch;

        rect()
            .horizontal()
            .width(Size::fill())
            .padding(Gaps::new(16., 10., 12., 16.))
            .spacing(10.)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .child(head_with_dot(self.own_id, 32., dot))
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(3.)
                    .child(
                        label()
                            .text(self.username.clone())
                            .font_size(14.)
                            .font_weight(FontWeight::MEDIUM)
                            .color(colors::fg_primary())
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis),
                    )
                    .child(
                        rect()
                            .horizontal()
                            .spacing(4.)
                            .cross_align(Alignment::Center)
                            .cursor(CursorIcon::Pointer)
                            .on_press(move |_| status_menu.toggle())
                            .child(
                                label()
                                    .text(status)
                                    .font_size(12.)
                                    .color(colors::fg_secondary()),
                            )
                            .child(
                                Icon::new(IconType::ChevronDown)
                                    .size(12.)
                                    .color(colors::fg_secondary()),
                            ),
                    ),
            )
            .child(
                rect()
                    .horizontal()
                    .spacing(2.)
                    .cross_align(Alignment::Center)
                    .maybe_child((self.mode == PanelMode::Launcher).then(|| HeaderButton {
                        icon: IconType::LinkExternal01,
                        tooltip: t("oneclient.chat.list.pop_out"),
                        on_press: EventHandler::new(move |_| pop_out.pop_out_social()),
                    }))
                    .maybe_child((self.mode == PanelMode::Window).then(|| HeaderButton {
                        icon: IconType::LayoutRight,
                        tooltip: t("oneclient.chat.list.dock"),
                        on_press: EventHandler::new(move |_| dock.dock_social()),
                    }))
                    .maybe_child((self.mode == PanelMode::Launcher).then(|| HeaderButton {
                        icon: IconType::XClose,
                        tooltip: t("oneclient.chat.common.close"),
                        on_press: EventHandler::new(move |_| close.close_social()),
                    })),
            )
    }
}

#[derive(PartialEq)]
struct StatusMenu {
    on_close: EventHandler<()>,
}

impl Component for StatusMenu {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let picked = use_chat_self_status();

        let option = |status: PlayerStatus, text: &'static str, note: &'static str| {
            let dispatch = dispatch.clone();
            let on_close = self.on_close.clone();
            StatusOption {
                dot: match status {
                    PlayerStatus::Online => colors::success(),
                    PlayerStatus::Offline => super::common::offline_dot(),
                },
                text,
                note,
                selected: picked == status,
                on_pick: EventHandler::new(move |_| {
                    dispatch.set_self_status(status);
                    on_close.call(());
                }),
            }
        };

        rect()
            .position(Position::new_absolute().top(54.).left(52.))
            .layer(Layer::Overlay)
            .vertical()
            .width(Size::px(180.))
            .padding(Gaps::new_all(6.))
            .spacing(2.)
            .corner_radius(CornerRadius::from(ROW_RADIUS_PX))
            .background(colors::component_bg())
            .border(inset_border(hairline()))
            .shadow(popup_shadow())
            .child(option(
                PlayerStatus::Online,
                t("oneclient.chat.status.online"),
                t("oneclient.chat.status.online_hint"),
            ))
            .child(option(
                PlayerStatus::Offline,
                t("oneclient.chat.status.invisible"),
                t("oneclient.chat.status.invisible_hint"),
            ))
    }
}

#[derive(PartialEq)]
struct StatusOption {
    dot: Color,
    text: &'static str,
    note: &'static str,
    selected: bool,
    on_pick: EventHandler<()>,
}

impl Component for StatusOption {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let on_pick = self.on_pick.clone();

        rect()
            .horizontal()
            .width(Size::fill())
            .padding(Gaps::new_symmetric(6., 8.))
            .spacing(8.)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .corner_radius(CornerRadius::from(CHIP_RADIUS_PX))
            .background(if hovered() {
                hover_bg()
            } else {
                Color::TRANSPARENT
            })
            .cursor(CursorIcon::Pointer)
            .on_pointer_over(move |_| hovered.set(true))
            .on_pointer_out(move |_| hovered.set(false))
            .on_press(move |_| on_pick.call(()))
            .child(
                rect()
                    .width(Size::px(8.))
                    .height(Size::px(8.))
                    .corner_radius(CornerRadius::from(4.))
                    .background(self.dot),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(2.)
                    .child(
                        label()
                            .text(self.text)
                            .font_size(12.)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text(self.note)
                            .font_size(10.)
                            .color(colors::fg_secondary()),
                    ),
            )
            .maybe_child(self.selected.then(|| {
                Icon::new(IconType::Check)
                    .size(12.)
                    .color(colors::fg_secondary())
            }))
    }
}

#[derive(PartialEq)]
struct ErrorBanner {
    error: String,
}

impl Component for ErrorBanner {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let dismiss = dispatch.clone();

        rect()
            .width(Size::fill())
            .padding(Gaps::new(0., 16., 8., 16.))
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .padding(Gaps::new(6., 4., 6., 12.))
                    .spacing(4.)
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .corner_radius(CornerRadius::from(ROW_RADIUS_PX))
                    .background(colors::danger().with_a(30))
                    .border(inset_border(colors::danger().with_a(90)))
                    .child(
                        label()
                            .text(self.error.clone())
                            .font_size(11.)
                            .color(colors::fg_primary())
                            .width(Size::flex(1.0))
                            .max_lines(3)
                            .text_overflow(TextOverflow::Ellipsis),
                    )
                    .child(
                        rect()
                            .height(Size::px(24.))
                            .padding(Gaps::new_symmetric(0., 8.))
                            .center()
                            .corner_radius(CornerRadius::from(CHIP_RADIUS_PX))
                            .cursor(CursorIcon::Pointer)
                            .on_press(move |_| dispatch.retry_chat())
                            .child(
                                label()
                                    .text(t("oneclient.chat.common.try_again"))
                                    .font_size(11.)
                                    .font_weight(FontWeight::MEDIUM)
                                    .color(colors::fg_primary()),
                            ),
                    )
                    .child(HeaderButton {
                        icon: IconType::XClose,
                        tooltip: t("oneclient.chat.common.dismiss"),
                        on_press: EventHandler::new(move |_| dismiss.clear_chat_error()),
                    }),
            )
    }
}

#[derive(PartialEq)]
struct InvitesCard {
    view: State<PanelView>,
}

impl Component for InvitesCard {
    fn render(&self) -> impl IntoElement {
        let invites = use_chat_invites();
        let mut open = use_state(|| true);

        if invites.is_empty() {
            return rect().into_element();
        }

        let count = invites.len();

        rect()
            .vertical()
            .width(Size::fill())
            .padding(Gaps::new(0., 16., 12., 16.))
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .corner_radius(CornerRadius::from(CARD_RADIUS_PX))
                    .background(card_bg())
                    .border(inset_border(hairline()))
                    .overflow(Overflow::Clip)
                    .child(
                        rect()
                            .horizontal()
                            .width(Size::fill())
                            .height(Size::px(38.))
                            .padding(Gaps::new(0., 12., 0., 14.))
                            .spacing(8.)
                            .content(Content::Flex)
                            .cross_align(Alignment::Center)
                            .cursor(CursorIcon::Pointer)
                            .on_press(move |_| open.toggle())
                            .child(
                                Icon::new(IconType::Globe01)
                                    .size(14.)
                                    .color(colors::brand()),
                            )
                            .child(
                                label()
                                    .text(t("oneclient.chat.invites.title"))
                                    .font_size(12.)
                                    .font_weight(FontWeight::MEDIUM)
                                    .color(colors::fg_primary())
                                    .width(Size::flex(1.0)),
                            )
                            .child(count_badge(count, colors::brand(), Color::WHITE))
                            .child(
                                Icon::new(IconType::ChevronDown)
                                    .size(12.)
                                    .color(colors::fg_secondary())
                                    .rotate(if open() { 180. } else { 0. }),
                            ),
                    )
                    .maybe(open(), |el| {
                        el.children(
                            invites
                                .into_iter()
                                .map(|invite| {
                                    InviteRow {
                                        invite,
                                        view: self.view,
                                    }
                                    .into_element()
                                })
                                .collect::<Vec<_>>(),
                        )
                        .child(
                            rect()
                                .width(Size::fill())
                                .padding(Gaps::new(9., 14., 10., 14.))
                                .border(edge_border(hairline(), 1., 0., 0.))
                                .child(
                                    label()
                                        .text(t("oneclient.chat.invites.in_game_only"))
                                        .font_size(11.)
                                        .width(Size::fill())
                                        .color(colors::fg_secondary()),
                                ),
                        )
                    }),
            )
            .into_element()
    }
}

#[derive(PartialEq)]
struct InviteRow {
    invite: ChatInvite,
    view: State<PanelView>,
}

impl Component for InviteRow {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let from = use_player_name(self.invite.from);
        let id = self.invite.id;
        let group_id = self.invite.group_id;
        let view = self.view;
        let decline = dispatch.clone();
        let open = dispatch;

        rect()
            .horizontal()
            .width(Size::fill())
            .padding(Gaps::new(10., 10., 10., 14.))
            .spacing(10.)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .border(edge_border(hairline(), 1., 0., 0.))
            .cursor(CursorIcon::Pointer)
            .on_press(move |_| open_chat(&open, view, group_id))
            .child(
                Avatar::new(self.invite.from.to_string())
                    .size(28.)
                    .radius(CHIP_RADIUS_PX),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(3.)
                    .child(
                        label()
                            .text(t("oneclient.chat.invites.world_invite"))
                            .font_size(12.)
                            .font_weight(FontWeight::MEDIUM)
                            .color(colors::fg_primary())
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis),
                    )
                    .child(
                        label()
                            .text(tf("oneclient.chat.invites.from", &[&from]))
                            .font_size(11.)
                            .color(colors::fg_secondary())
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis),
                    ),
            )
            .child(
                rect()
                    .on_press(|e: Event<PressEventData>| e.stop_propagation())
                    .child(HeaderButton {
                        icon: IconType::XClose,
                        tooltip: t("oneclient.chat.common.decline"),
                        on_press: EventHandler::new(move |_| decline.decline_chat_invite(id)),
                    }),
            )
    }
}

pub(super) fn accent_button(
    text: &'static str,
    on_press: impl Into<EventHandler<Event<PressEventData>>>,
) -> impl IntoElement {
    rect()
        .height(Size::px(28.))
        .padding(Gaps::new_symmetric(0., 12.))
        .center()
        .corner_radius(CornerRadius::from(ROW_RADIUS_PX))
        .background(colors::brand())
        .cursor(CursorIcon::Pointer)
        .on_press(on_press)
        .child(
            label()
                .text(text)
                .font_size(12.)
                .font_weight(FontWeight::MEDIUM)
                .color(Color::WHITE),
        )
}

fn tab_strip(
    active: PanelTab,
    unread: usize,
    incoming: usize,
    on_pick: impl Fn(PanelTab) + Clone + 'static,
) -> impl IntoElement {
    let tab = |kind: PanelTab, text: &'static str, count: usize| {
        let on_pick = on_pick.clone();
        let selected = kind == active;

        rect()
            .horizontal()
            .width(Size::flex(1.0))
            .height(Size::px(28.))
            .center()
            .spacing(6.)
            .corner_radius(CornerRadius::from(CHIP_RADIUS_PX))
            .background(if selected {
                colors::brand()
            } else {
                Color::TRANSPARENT
            })
            .cursor(CursorIcon::Pointer)
            .on_press(move |_| on_pick(kind))
            .child(
                label()
                    .text(text)
                    .font_size(12.)
                    .font_weight(FontWeight::MEDIUM)
                    .color(if selected {
                        Color::WHITE
                    } else {
                        colors::fg_secondary()
                    }),
            )
            .maybe_child((count > 0).then(|| {
                if selected {
                    label()
                        .text(count.to_string())
                        .font_size(10.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(Color::WHITE.with_a(204))
                        .into_element()
                } else {
                    count_badge(count, colors::brand(), Color::WHITE).into_element()
                }
            }))
    };

    rect()
        .width(Size::fill())
        .padding(Gaps::new(0., 16., 8., 16.))
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .padding(Gaps::new_all(3.))
                .spacing(2.)
                .content(Content::Flex)
                .corner_radius(CornerRadius::from(ROW_RADIUS_PX))
                .background(chip_bg())
                .child(tab(PanelTab::Chats, t("oneclient.chat.tabs.chats"), unread))
                .child(tab(
                    PanelTab::Friends,
                    t("oneclient.chat.tabs.friends"),
                    incoming,
                ))
                .child(tab(PanelTab::Groups, t("oneclient.chat.tabs.groups"), 0)),
        )
}

fn open_chat(dispatch: &Actions, mut view: State<PanelView>, group_id: i32) {
    dispatch.open_conversation(group_id);
    view.set(PanelView::Chat);
}

fn chats_tab(pane: &ListPane, query: &str) -> Element {
    if pane.loading {
        return hint(t("oneclient.chat.list.loading")).into_element();
    }

    if pane.conversations.is_empty() {
        return hint(t("oneclient.chat.list.empty")).into_element();
    }

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(2.)
        .children(
            pane.conversations
                .iter()
                .map(|conversation| {
                    ChatRow {
                        selected: pane.expanded
                            && *pane.view.read() == PanelView::Chat
                            && pane.active == Some(conversation.id),
                        conversation: conversation.clone(),
                        own_id: pane.own_id,
                        query: query.to_string(),
                        view: pane.view,
                    }
                    .into_element()
                })
                .collect::<Vec<_>>(),
        )
        .into_element()
}

#[derive(PartialEq)]
struct ChatRow {
    conversation: ChatConversation,
    own_id: Uuid,
    selected: bool,
    query: String,
    view: State<PanelView>,
}

impl Component for ChatRow {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let mut hovered = use_state(|| false);

        let counterpart = self.conversation.counterpart(self.own_id);
        let name = use_player_name(counterpart.unwrap_or(self.own_id));
        let online = use_player_online(counterpart.unwrap_or(self.own_id));
        let sender = self.conversation.preview_sender.unwrap_or(self.own_id);
        let sender_name = use_player_name(sender);

        let title = conversation_title(&self.conversation, counterpart.map(|_| name));
        if !self.query.is_empty() && !title.to_lowercase().contains(&self.query) {
            return rect().into_element();
        }

        let is_group = self.conversation.kind == GroupKind::Group;
        let preview = self.conversation.preview.clone().map(|text| {
            if self.conversation.preview_sender == Some(self.own_id) {
                tf("oneclient.chat.list.preview_you", &[&text])
            } else if is_group && self.conversation.preview_sender.is_some() {
                tf("oneclient.chat.list.preview_sender", &[&sender_name, &text])
            } else {
                text
            }
        });

        let time = self
            .conversation
            .last_activity
            .map(row_time)
            .unwrap_or_default();
        let unread = self.conversation.unread;
        let group_id = self.conversation.id;
        let view = self.view;

        let (background, border) = if self.selected {
            (card_bg(), inset_border(colors::brand()))
        } else if hovered() {
            (hover_bg(), inset_border(hairline()))
        } else {
            (Color::TRANSPARENT, inset_border(Color::TRANSPARENT))
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .padding(Gaps::new_all(8.))
            .spacing(10.)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .corner_radius(CornerRadius::from(ROW_RADIUS_PX))
            .background(background)
            .border(border)
            .cursor(CursorIcon::Pointer)
            .on_pointer_over(move |_| hovered.set(true))
            .on_pointer_out(move |_| hovered.set(false))
            .on_press(move |_| open_chat(&dispatch, view, group_id))
            .child(match counterpart {
                Some(player) => head_with_dot(player, 32., presence_color(online)).into_element(),
                None => group_heads(&self.conversation.members).into_element(),
            })
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(4.)
                    .child(
                        rect()
                            .horizontal()
                            .width(Size::fill())
                            .spacing(8.)
                            .content(Content::Flex)
                            .cross_align(Alignment::Center)
                            .child(
                                label()
                                    .text(title)
                                    .font_size(14.)
                                    .font_weight(if unread {
                                        FontWeight::MEDIUM
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .color(colors::fg_primary())
                                    .width(Size::flex(1.0))
                                    .max_lines(1)
                                    .text_overflow(TextOverflow::Ellipsis),
                            )
                            .child(
                                label()
                                    .text(time)
                                    .font_size(11.)
                                    .color(colors::fg_secondary()),
                            ),
                    )
                    .child(
                        rect()
                            .horizontal()
                            .width(Size::fill())
                            .spacing(8.)
                            .content(Content::Flex)
                            .cross_align(Alignment::Center)
                            .child(
                                label()
                                    .text(preview.unwrap_or_default())
                                    .font_size(12.)
                                    .color(colors::fg_secondary())
                                    .width(Size::flex(1.0))
                                    .max_lines(1)
                                    .text_overflow(TextOverflow::Ellipsis),
                            )
                            .maybe_child(unread.then(|| {
                                rect()
                                    .width(Size::px(8.))
                                    .height(Size::px(8.))
                                    .corner_radius(CornerRadius::from(4.))
                                    .background(colors::brand())
                            })),
                    ),
            )
            .into_element()
    }
}

pub(super) fn conversation_title(
    conversation: &ChatConversation,
    resolved: Option<String>,
) -> String {
    conversation
        .name
        .clone()
        .or(resolved)
        .unwrap_or_else(|| match conversation.kind {
            GroupKind::Group => t("oneclient.chat.list.kind_group").to_string(),
            _ => t("oneclient.chat.list.kind_direct").to_string(),
        })
}

#[derive(PartialEq)]
struct FriendsTab {
    roster: ChatRoster,
    query: String,
    view: State<PanelView>,
}

impl Component for FriendsTab {
    fn render(&self) -> impl IntoElement {
        let presence = crate::hooks::use_chat_online_set(
            self.roster
                .friends
                .iter()
                .map(|friend| friend.player)
                .collect(),
        );

        let requests: Vec<(FriendRequest, bool)> = self
            .roster
            .requests
            .incoming
            .iter()
            .map(|request| (request.clone(), true))
            .chain(
                self.roster
                    .requests
                    .outgoing
                    .iter()
                    .map(|request| (request.clone(), false)),
            )
            .collect();

        let (online, offline): (Vec<Friend>, Vec<Friend>) = self
            .roster
            .friends
            .iter()
            .cloned()
            .partition(|friend| presence.contains(&friend.player));

        let query = self.query.clone();
        let view = self.view;
        let blocked = self.roster.blocked.len();

        let request_rows: Vec<Element> = requests
            .into_iter()
            .map(|(request, incoming)| {
                RequestRow {
                    request,
                    incoming,
                    query: query.clone(),
                }
                .into_element()
            })
            .collect();

        let friend_rows = |friends: Vec<Friend>, online: bool| -> Vec<Element> {
            friends
                .into_iter()
                .map(|friend| {
                    FriendRow {
                        player: friend.player,
                        online,
                        query: query.clone(),
                        view,
                    }
                    .into_element()
                })
                .collect()
        };

        let online_count = online.len();
        let offline_count = offline.len();
        let has_requests = !request_rows.is_empty();

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(2.)
            .child(
                rect()
                    .width(Size::fill())
                    .padding(Gaps::new(0., 8., 6., 8.))
                    .child(AddFriend),
            )
            .maybe_child(has_requests.then(|| {
                section_heading(tf(
                    "oneclient.chat.friends.requests_heading",
                    &[&request_rows.len()],
                ))
                .into_element()
            }))
            .children(request_rows)
            .child(section_heading(tf(
                "oneclient.chat.friends.online_heading",
                &[&online_count],
            )))
            .children(friend_rows(online, true))
            .child(section_heading(tf(
                "oneclient.chat.friends.offline_heading",
                &[&offline_count],
            )))
            .children(friend_rows(offline, false))
            .child(BlockedLink {
                count: blocked,
                view,
            })
    }
}

#[derive(PartialEq)]
struct AddFriend;

impl Component for AddFriend {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let status = use_chat_friend_request();
        let mut open = use_state(|| false);
        let name = use_state(String::new);

        let send_key = dispatch.clone();
        let reset = dispatch.clone();

        let (note, note_color) = match &status {
            FriendRequestStatus::Idle => (None, colors::fg_secondary()),
            FriendRequestStatus::Sending(username) => (
                Some(tf("oneclient.chat.friends.request_sending", &[&username])),
                colors::fg_secondary(),
            ),
            FriendRequestStatus::Sent(username) => (
                Some(tf("oneclient.chat.friends.request_sent", &[&username])),
                colors::fg_secondary(),
            ),
            FriendRequestStatus::Failed(error) => (Some(error.clone()), colors::danger()),
        };

        let note = note.map(|note| {
            rect().padding(Gaps::new(8., 2., 0., 2.)).child(
                label()
                    .text(note)
                    .font_size(11.)
                    .width(Size::fill())
                    .color(note_color),
            )
        });

        if !open() {
            return rect()
                .vertical()
                .width(Size::fill())
                .child(chip_button(
                    IconType::Plus,
                    t("oneclient.chat.friends.add"),
                    move |_| {
                        reset.reset_friend_request();
                        open.set(true);
                    },
                ))
                .maybe_child(note)
                .into_element();
        }

        rect()
            .vertical()
            .width(Size::fill())
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .spacing(6.)
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .child(
                        TextInput::new(name)
                            .width(Size::flex(1.0))
                            .height(Size::px(32.))
                            .font_size(12.)
                            .auto_focus(true)
                            .placeholder(t("oneclient.chat.friends.username_placeholder"))
                            .on_submit(move |_| send_request(&send_key, name, open)),
                    )
                    .child(accent_button(t("oneclient.chat.common.send"), move |_| {
                        send_request(&dispatch, name, open)
                    }))
                    .child(HeaderButton {
                        icon: IconType::XClose,
                        tooltip: t("oneclient.chat.common.cancel"),
                        on_press: EventHandler::new(move |_| {
                            let mut name = name;
                            name.set(String::new());
                            open.set(false);
                        }),
                    }),
            )
            .maybe_child(note)
            .into_element()
    }
}

fn send_request(dispatch: &Actions, mut name: State<String>, mut open: State<bool>) {
    let username = name.read().trim().to_string();
    if username.is_empty() {
        return;
    }

    dispatch.add_friend(username);
    name.set(String::new());
    open.set(false);
}

#[derive(PartialEq)]
struct RequestRow {
    request: FriendRequest,
    incoming: bool,
    query: String,
}

impl Component for RequestRow {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let name = use_player_name(self.request.player);

        if !self.query.is_empty() && !name.to_lowercase().contains(&self.query) {
            return rect().into_element();
        }

        let id = self.request.id;
        let age = ago(self.request.created_at);
        let sub = if self.incoming {
            tf("oneclient.chat.friends.incoming_request", &[&age])
        } else {
            tf("oneclient.chat.friends.outgoing_request", &[&age])
        };

        let accept = dispatch.clone();
        let incoming = self.incoming;

        person_row(self.request.player, name, sub, false)
            .maybe_child(incoming.then(|| {
                square_button(
                    IconType::Check,
                    t("oneclient.chat.common.accept"),
                    true,
                    move |_| accept.accept_friend_request(id),
                )
            }))
            .child(square_button(
                IconType::XClose,
                if incoming {
                    t("oneclient.chat.common.decline")
                } else {
                    t("oneclient.chat.friends.cancel_request")
                },
                false,
                move |_| {
                    if incoming {
                        dispatch.decline_friend_request(id);
                    } else {
                        dispatch.cancel_friend_request(id);
                    }
                },
            ))
            .into_element()
    }
}

fn square_button(
    icon: IconType,
    alt: &'static str,
    accent: bool,
    on_press: impl Into<EventHandler<Event<PressEventData>>>,
) -> impl IntoElement {
    rect()
        .width(Size::px(28.))
        .height(Size::px(28.))
        .center()
        .corner_radius(CornerRadius::from(ROW_RADIUS_PX))
        .background(if accent { colors::brand() } else { chip_bg() })
        .border(inset_border(if accent {
            Color::TRANSPARENT
        } else {
            hairline()
        }))
        .cursor(CursorIcon::Pointer)
        .a11y_alt(alt)
        .on_press(on_press)
        .child(Icon::new(icon).size(14.).color(if accent {
            Color::WHITE
        } else {
            colors::fg_primary()
        }))
}

fn person_row(player: Uuid, name: String, sub: String, faded: bool) -> Rect {
    rect()
        .horizontal()
        .width(Size::fill())
        .padding(Gaps::new_symmetric(6., 8.))
        .spacing(10.)
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .corner_radius(CornerRadius::from(ROW_RADIUS_PX))
        .child(
            rect().opacity(if faded { 0.45 } else { 1. }).child(
                Avatar::new(player.to_string())
                    .size(32.)
                    .radius(CHIP_RADIUS_PX),
            ),
        )
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(4.)
                .child(
                    label()
                        .text(name)
                        .font_size(14.)
                        .color(if faded {
                            colors::fg_secondary()
                        } else {
                            colors::fg_primary()
                        })
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis),
                )
                .child(
                    label()
                        .text(sub)
                        .font_size(12.)
                        .color(colors::fg_secondary())
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis),
                ),
        )
}

#[derive(PartialEq)]
struct FriendRow {
    player: Uuid,
    online: bool,
    query: String,
    view: State<PanelView>,
}

impl Component for FriendRow {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let name = use_player_name(self.player);
        let mut hovered = use_state(|| false);
        let mut menu = use_state(|| None::<(f32, f32)>);

        if !self.query.is_empty() && !name.to_lowercase().contains(&self.query) {
            return rect().into_element();
        }

        let player = self.player;
        let online = self.online;
        let mut view = self.view;

        let activity = if online {
            t("oneclient.chat.status.online")
        } else {
            t("oneclient.chat.status.offline")
        };

        let open = dispatch.clone();
        let context = (*menu.read()).map(|(x, y)| {
            let message = dispatch.clone();
            let remove = dispatch.clone();
            let block = dispatch.clone();
            ContextMenu::new(x, y)
                .title(name.clone())
                .on_close(move |_| menu.set(None))
                .action(
                    IconType::MessageTextSquare01,
                    t("oneclient.chat.friends.message"),
                    move |()| {
                        message.start_direct_message(player);
                        view.set(PanelView::Chat);
                    },
                )
                .separator()
                .danger_action(
                    IconType::Trash01,
                    t("oneclient.chat.friends.remove"),
                    move |()| remove.remove_friend(player),
                )
                .danger_action(
                    IconType::XClose,
                    t("oneclient.chat.friends.block"),
                    move |()| block.block_player(player),
                )
                .into_element()
        });

        let avatar = if online {
            head_with_dot(player, 32., presence_color(true)).into_element()
        } else {
            rect()
                .opacity(0.45)
                .child(
                    Avatar::new(player.to_string())
                        .size(32.)
                        .radius(CHIP_RADIUS_PX),
                )
                .into_element()
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .padding(Gaps::new_symmetric(6., 8.))
            .spacing(10.)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .corner_radius(CornerRadius::from(ROW_RADIUS_PX))
            .background(if hovered() {
                hover_bg()
            } else {
                Color::TRANSPARENT
            })
            .border(inset_border(if hovered() {
                hairline()
            } else {
                Color::TRANSPARENT
            }))
            .cursor(CursorIcon::Pointer)
            .on_pointer_over(move |_| hovered.set(true))
            .on_pointer_out(move |_| hovered.set(false))
            .on_press(move |_| {
                open.start_direct_message(player);
                view.set(PanelView::Chat);
            })
            .on_secondary_down(move |e: Event<PressEventData>| {
                if let PressEventData::Mouse(m) = e.data() {
                    menu.set(Some((
                        m.global_location.x as f32,
                        m.global_location.y as f32,
                    )));
                }
            })
            .child(avatar)
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(4.)
                    .child(
                        label()
                            .text(name)
                            .font_size(14.)
                            .color(if online {
                                colors::fg_primary()
                            } else {
                                colors::fg_secondary()
                            })
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis),
                    )
                    .child(
                        label()
                            .text(activity)
                            .font_size(12.)
                            .color(colors::fg_secondary()),
                    ),
            )
            .maybe_child(context)
            .into_element()
    }
}

#[derive(PartialEq)]
struct BlockedLink {
    count: usize,
    view: State<PanelView>,
}

impl Component for BlockedLink {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let mut view = self.view;
        let color = if hovered() {
            colors::fg_primary()
        } else {
            colors::fg_secondary()
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .margin(Gaps::new(10., 0., 0., 0.))
            .padding(Gaps::new_symmetric(10., 8.))
            .spacing(10.)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .border(edge_border(hairline(), 1., 0., 0.))
            .cursor(CursorIcon::Pointer)
            .on_pointer_over(move |_| hovered.set(true))
            .on_pointer_out(move |_| hovered.set(false))
            .on_press(move |_| view.set(PanelView::Blocked))
            .child(
                label()
                    .text(t("oneclient.chat.blocked.title"))
                    .font_size(12.)
                    .color(color)
                    .width(Size::flex(1.0)),
            )
            .child(
                label()
                    .text(self.count.to_string())
                    .font_size(12.)
                    .color(color),
            )
            .child(Icon::new(IconType::ChevronRight).size(14.).color(color))
    }
}

fn groups_tab(pane: &ListPane, query: &str, on_create: EventHandler<()>) -> Element {
    let groups: Vec<Element> = pane
        .conversations
        .iter()
        .filter(|conversation| conversation.kind == GroupKind::Group)
        .filter(|conversation| {
            query.is_empty()
                || conversation_title(conversation, None)
                    .to_lowercase()
                    .contains(query)
        })
        .map(|conversation| {
            GroupRow {
                conversation: conversation.clone(),
                view: pane.view,
            }
            .into_element()
        })
        .collect();

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(2.)
        .child(
            rect()
                .width(Size::fill())
                .padding(Gaps::new(0., 8., 6., 8.))
                .child(chip_button(
                    IconType::Plus,
                    t("oneclient.chat.groups.create"),
                    move |_| on_create.call(()),
                )),
        )
        .maybe_child(
            groups
                .is_empty()
                .then(|| hint(t("oneclient.chat.groups.empty")).into_element()),
        )
        .children(groups)
        .into_element()
}

#[derive(PartialEq)]
struct GroupRow {
    conversation: ChatConversation,
    view: State<PanelView>,
}

impl Component for GroupRow {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let mut hovered = use_state(|| false);
        let online = crate::hooks::use_chat_online_set(self.conversation.members.clone());

        let group_id = self.conversation.id;
        let view = self.view;
        let members = self.conversation.members.len();
        let sub = tf(
            "oneclient.chat.groups.members_online",
            &[&members, &online.len()],
        );

        rect()
            .horizontal()
            .width(Size::fill())
            .padding(Gaps::new_all(8.))
            .spacing(10.)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .corner_radius(CornerRadius::from(ROW_RADIUS_PX))
            .background(if hovered() {
                hover_bg()
            } else {
                Color::TRANSPARENT
            })
            .border(inset_border(if hovered() {
                hairline()
            } else {
                Color::TRANSPARENT
            }))
            .cursor(CursorIcon::Pointer)
            .on_pointer_over(move |_| hovered.set(true))
            .on_pointer_out(move |_| hovered.set(false))
            .on_press(move |_| open_chat(&dispatch, view, group_id))
            .child(group_heads(&self.conversation.members))
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(4.)
                    .child(
                        label()
                            .text(conversation_title(&self.conversation, None))
                            .font_size(14.)
                            .color(colors::fg_primary())
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis),
                    )
                    .child(
                        label()
                            .text(sub)
                            .font_size(12.)
                            .color(colors::fg_secondary()),
                    ),
            )
            .child(
                Icon::new(IconType::ChevronRight)
                    .size(14.)
                    .color(colors::fg_secondary()),
            )
    }
}

#[derive(PartialEq)]
pub(super) struct BlockedPane {
    pub blocked: Vec<BlockedPlayer>,
    pub view: State<PanelView>,
}

impl Component for BlockedPane {
    fn render(&self) -> impl IntoElement {
        let mut view = self.view;

        rect()
            .vertical()
            .width(Size::flex(1.0))
            .height(Size::fill())
            .content(Content::Flex)
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .padding(Gaps::new(12., 10., 12., 8.))
                    .spacing(8.)
                    .cross_align(Alignment::Center)
                    .border(edge_border(hairline(), 0., 0., 1.))
                    .child(HeaderButton {
                        icon: IconType::ArrowLeft,
                        tooltip: t("oneclient.chat.common.back"),
                        on_press: EventHandler::new(move |_| view.set(PanelView::List)),
                    })
                    .child(
                        label()
                            .text(t("oneclient.chat.blocked.title"))
                            .font_size(14.)
                            .font_weight(FontWeight::MEDIUM)
                            .color(colors::fg_primary()),
                    ),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .padding(Gaps::new(12., 16., 4., 16.))
                    .child(
                        label()
                            .text(t("oneclient.chat.blocked.description"))
                            .font_size(12.)
                            .width(Size::fill())
                            .color(colors::fg_secondary()),
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
                            .padding(Gaps::new_all(8.))
                            .maybe_child(
                                self.blocked.is_empty().then(|| {
                                    hint(t("oneclient.chat.blocked.empty")).into_element()
                                }),
                            )
                            .children(
                                self.blocked
                                    .iter()
                                    .map(|entry| {
                                        BlockedRow {
                                            player: entry.player,
                                        }
                                        .into_element()
                                    })
                                    .collect::<Vec<_>>(),
                            ),
                    ),
            )
    }
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

        rect()
            .horizontal()
            .width(Size::fill())
            .padding(Gaps::new_symmetric(6., 8.))
            .spacing(10.)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .child(
                rect().opacity(0.45).child(
                    Avatar::new(player.to_string())
                        .size(32.)
                        .radius(CHIP_RADIUS_PX),
                ),
            )
            .child(
                label()
                    .text(name)
                    .font_size(14.)
                    .color(colors::fg_primary())
                    .width(Size::flex(1.0))
                    .max_lines(1),
            )
            .child(
                rect()
                    .height(Size::px(28.))
                    .padding(Gaps::new_symmetric(0., 12.))
                    .center()
                    .corner_radius(CornerRadius::from(ROW_RADIUS_PX))
                    .background(chip_bg())
                    .border(inset_border(hairline()))
                    .cursor(CursorIcon::Pointer)
                    .on_press(move |_| dispatch.unblock_player(player))
                    .child(
                        label()
                            .text(t("oneclient.chat.blocked.unblock"))
                            .font_size(12.)
                            .font_weight(FontWeight::MEDIUM)
                            .color(colors::fg_primary()),
                    ),
            )
    }
}
