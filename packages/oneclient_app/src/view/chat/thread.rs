use freya::prelude::*;
use uuid::Uuid;

use oneclient_polyplus::{GroupKind, MAX_MESSAGE_LENGTH};

use crate::chat::{ChatConversation, ChatMessage, MessageInvite, PendingMessage, ThreadStatus};
use crate::components::{Avatar, Button, ContextMenu, Icon, IconType, ScrollArea, TextInput};
use crate::hooks::{
    Actions, chat_draft, save_chat_draft, use_chat_invite_game, use_chat_online_count,
    use_chat_roster, use_chat_thread, use_dispatch, use_player_online,
};
use crate::i18n::{t, tf};
use crate::theme::{colors, rounded, text_size};

use super::common::{
    CARD_RADIUS_PX, CHIP_RADIUS_PX, EscapeGuard, HeaderButton, ROW_RADIUS_PX, area_contains, clock,
    day_label, day_of, edge_border, group_heads, head_with_dot, inset_border, popup_shadow,
    presence_color, use_player_name,
};
use super::list::{accent_button, conversation_title};
use super::people::AddMembersDialog;

const PAGE_TRIGGER_PX: f32 = 240.;

const ROW_WINDOW: usize = 200;
const ROW_WINDOW_STEP: usize = 100;

const EMOJI_CELL_PX: f32 = 34.;
const EMOJI_GAP_PX: f32 = 2.;
const EMOJI_SIDE_PX: f32 = 14.;

const EMOJI: [&str; 40] = [
    "😀", "😂", "🥲", "😍", "😎", "🤔", "😭", "😤", "👍", "👎", "👏", "🙏", "🔥", "✨", "💀", "🎉",
    "❤️", "💙", "⭐", "👀", "💯", "✅", "❌", "🎮", "⚔️", "🛡️", "⛏️", "🏹", "💎", "🧱", "🪵", "🌲",
    "🍞", "🍎", "🐷", "🐑", "🐔", "🏠", "🌋", "🚀",
];

const EDIT_PAD_X_PX: f32 = 6.;
const EDIT_PAD_Y_PX: f32 = 2.;

#[derive(PartialEq)]
pub(super) struct ThreadView {
    pub conversation: ChatConversation,
    pub own_id: Uuid,
    pub expanded: bool,
    pub windowed: bool,
    pub on_back: EventHandler<()>,
}

impl Component for ThreadView {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();

        let group_id = self.conversation.id;
        let own_id = self.own_id;

        let thread = use_chat_thread(group_id);
        let messages = thread.messages;
        let pending = thread.pending;
        let exhausted = thread.complete;
        let empty = messages.is_empty() && pending.is_empty();

        let failure = match &thread.status {
            ThreadStatus::Error(message) => Some(message.clone()),
            _ => None,
        };

        let mut pinned = use_state(|| true);
        let mut may_page = use_state(|| false);
        let mut shown = use_state(|| group_id);
        let mut editing = use_state(|| None::<(i64, String)>);
        let mut adding = use_state(|| false);
        let mut first_shown = use_state(|| None::<i64>);

        if *shown.peek() != group_id {
            shown.set(group_id);
            first_shown.set(None);
            pinned.set(true);
            may_page.set(false);
            editing.set(None);
            adding.set(false);
        }

        let edit_gone = editing.peek().as_ref().is_some_and(|(id, _)| {
            messages
                .binary_search_by_key(id, |message| message.id)
                .is_err()
        });
        if edit_gone {
            editing.set(None);
        }

        let paging = dispatch.clone();

        let hidden = match *first_shown.read() {
            Some(first) => messages.partition_point(|message| message.id < first),
            None => messages.len().saturating_sub(ROW_WINDOW),
        };
        let older = messages.clone();
        let shown_messages = &messages[hidden..];
        let oldest_shown = shown_messages.first().map(|message| message.id);

        let mut rows: Vec<Element> = Vec::with_capacity(shown_messages.len() + pending.len());
        let mut previous: Option<&ChatMessage> = None;
        for message in shown_messages {
            let day = message.sent_at.map(day_of);
            let new_day =
                day.is_some() && day != previous.and_then(|prev| prev.sent_at.map(day_of));
            if let Some(at) = message.sent_at.filter(|_| new_day) {
                rows.push(
                    rect()
                        .key(day)
                        .width(Size::fill())
                        .child(day_divider(day_label(at)))
                        .into_element(),
                );
            }

            let show_head = new_day || previous.is_none_or(|prev| prev.sender != message.sender);
            rows.push(
                rect()
                    .key(message.id)
                    .width(Size::fill())
                    .child(MessageRow {
                        message: message.clone(),
                        group_id,
                        own: message.sender == own_id,
                        show_head,
                        first: previous.is_none(),
                        editing,
                        windowed: self.windowed,
                    })
                    .into_element(),
            );
            previous = Some(message);
        }

        let mut last_sender = previous.map(|prev| prev.sender);
        for pending in pending {
            rows.push(
                rect()
                    .key(pending.key)
                    .width(Size::fill())
                    .child(PendingRow {
                        group_id,
                        show_head: last_sender != Some(own_id),
                        own_id,
                        pending,
                    })
                    .into_element(),
            );
            last_sender = Some(own_id);
        }

        let title_hint = self.conversation.clone();
        let members = self.conversation.members.clone();
        let leave = dispatch.clone();
        let back = self.on_back.clone();

        rect()
            .vertical()
            .width(Size::flex(1.0))
            .height(Size::fill())
            .content(Content::Flex)
            .child(ThreadHeader {
                conversation: self.conversation.clone(),
                own_id,
                show_back: !self.expanded,
                on_back: self.on_back.clone(),
                on_add_members: EventHandler::new(move |_| adding.set(true)),
                on_leave: EventHandler::new(move |_| {
                    leave.leave_group(group_id);
                    back.call(());
                }),
            })
            .child(match (empty, &thread.status) {
                (true, ThreadStatus::Loading) => thread_notice(
                    t("oneclient.chat.thread.loading"),
                    colors::fg_secondary(),
                    None,
                ),
                (true, ThreadStatus::Error(message)) => thread_notice(
                    message.clone(),
                    colors::danger(),
                    Some(EventHandler::new(move |_| {
                        paging.reload_conversation(group_id);
                    })),
                ),
                (true, _) => thread_notice(
                    t("oneclient.chat.thread.empty"),
                    colors::fg_secondary(),
                    None,
                ),
                _ => ScrollArea::new()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .stick_bottom(*pinned.read())
                    .anchor_bottom(oldest_shown)
                    .on_user_scroll(move |_| may_page.set_if_modified(true))
                    .on_ctx(move |ctx: crate::components::ScrollAreaCtx| {
                        if ctx.at_bottom {
                            first_shown.set_if_modified(None);
                        } else if first_shown.peek().is_none() {
                            first_shown.set(oldest_shown);
                        }
                        pinned.set_if_modified(ctx.at_bottom);

                        if !*may_page.peek() || ctx.viewport_h <= 0. {
                            return;
                        }
                        if ctx.corrected_y < -PAGE_TRIGGER_PX {
                            return;
                        }

                        may_page.set(false);
                        if hidden > 0 {
                            let first = older[hidden.saturating_sub(ROW_WINDOW_STEP)].id;
                            first_shown.set(Some(first));
                        } else if !exhausted {
                            first_shown.set(Some(i64::MIN));
                            paging.load_older_messages(group_id);
                        }
                    })
                    .child(
                        rect()
                            .vertical()
                            .width(Size::fill())
                            .padding(Gaps::new(8., 16., 16., 16.))
                            .children(rows),
                    )
                    .into_element(),
            })
            .maybe_child(failure.filter(|_| !empty).map(|message| {
                thread_banner(
                    message,
                    EventHandler::new(move |_| dispatch.reload_conversation(group_id)),
                )
            }))
            .child(
                Composer {
                    group_id,
                    placeholder: title_hint,
                    own_id,
                    editing,
                    on_sent: EventHandler::new(move |_| pinned.set(true)),
                    key: DiffKey::None,
                }
                .key(group_id),
            )
            .maybe_child(adding().then(|| {
                AddMembersDialog {
                    group_id,
                    members,
                    on_close: EventHandler::new(move |_| adding.set(false)),
                }
                .into_element()
            }))
    }
}

fn day_divider(day: String) -> impl IntoElement {
    rect()
        .width(Size::fill())
        .padding(Gaps::new(16., 0., 4., 0.))
        .main_align(Alignment::Center)
        .child(
            label()
                .text(day)
                .font_size(text_size(11.))
                .width(Size::fill())
                .text_align(TextAlign::Center)
                .color(colors::fg_secondary()),
        )
}

fn thread_notice(
    text: impl Into<String>,
    color: Color,
    on_retry: Option<EventHandler<Event<PressEventData>>>,
) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::flex(1.0))
        .center()
        .padding(Gaps::new_all(24.))
        .spacing(12.)
        .child(
            label()
                .text(text.into())
                .font_size(text_size(12.))
                .color(color)
                .max_lines(3),
        )
        .maybe_child(on_retry.map(|on_retry| {
            Button::new()
                .secondary()
                .small()
                .text(t("oneclient.chat.common.try_again"))
                .on_press(on_retry)
        }))
        .into_element()
}

fn thread_banner(message: String, on_retry: EventHandler<Event<PressEventData>>) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .padding(Gaps::new(8., 16., 0., 16.))
        .spacing(8.)
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .child(
            label()
                .text(message)
                .font_size(text_size(11.))
                .color(colors::danger())
                .width(Size::flex(1.0))
                .max_lines(2),
        )
        .child(
            Button::new()
                .ghost()
                .small()
                .text(t("oneclient.chat.common.try_again"))
                .on_press(on_retry),
        )
        .into_element()
}

#[derive(PartialEq)]
struct ThreadHeader {
    conversation: ChatConversation,
    own_id: Uuid,
    show_back: bool,
    on_back: EventHandler<()>,
    on_add_members: EventHandler<()>,
    on_leave: EventHandler<()>,
}

impl Component for ThreadHeader {
    fn render(&self) -> impl IntoElement {
        let mut menu = use_state(|| None::<(f32, f32)>);
        let is_group = self.conversation.kind == GroupKind::Group;
        let can_leave = !self.conversation.special;

        let context = (*menu.read()).map(|(x, y)| {
            let add = self.on_add_members.clone();
            let leave = self.on_leave.clone();
            let options = ContextMenu::new(x, y)
                .on_close(move |_| menu.set(None))
                .action(
                    IconType::Plus,
                    t("oneclient.chat.groups.add_members"),
                    move |()| add.call(()),
                );

            if can_leave {
                options
                    .separator()
                    .danger_action(
                        IconType::LogOut01,
                        t("oneclient.chat.groups.leave"),
                        move |()| leave.call(()),
                    )
                    .into_element()
            } else {
                options.into_element()
            }
        });
        let counterpart = self.conversation.counterpart(self.own_id);
        let name = use_player_name(counterpart.unwrap_or(self.own_id));
        let online = use_player_online(counterpart.unwrap_or(self.own_id));
        let online_members = use_chat_online_count(&self.conversation.members);

        let title = conversation_title(&self.conversation, counterpart.map(|_| name));

        let sub = match (self.conversation.kind, counterpart) {
            (kind, _) if kind.is_shared() => tf(
                "oneclient.chat.groups.members_online",
                &[&online_members, &self.conversation.members.len()],
            ),
            (_, Some(_)) if online => t("oneclient.chat.status.online").to_string(),
            (_, Some(_)) => t("oneclient.chat.status.offline").to_string(),
            _ => String::new(),
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .padding(Gaps::new(
                12.,
                10.,
                12.,
                if self.show_back { 8. } else { 16. },
            ))
            .spacing(10.)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .border(edge_border(colors::component_border(), 0., 0., 1.))
            .maybe_child(self.show_back.then(|| HeaderButton {
                icon: IconType::ArrowLeft,
                tooltip: t("oneclient.chat.common.back"),
                on_press: self.on_back.clone(),
            }))
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
                        label()
                            .text(title)
                            .font_size(text_size(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .color(colors::fg_primary())
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis),
                    )
                    .child(
                        label()
                            .text(sub)
                            .font_size(text_size(11.))
                            .color(colors::fg_secondary())
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis),
                    ),
            )
            .maybe_child(is_group.then(|| {
                rect()
                    .on_press(move |e: Event<PressEventData>| {
                        if let PressEventData::Mouse(m) = e.data() {
                            menu.set(Some((
                                m.global_location.x as f32,
                                m.global_location.y as f32,
                            )));
                        }
                    })
                    .child(HeaderButton {
                        icon: IconType::DotsVertical,
                        tooltip: t("oneclient.chat.groups.options"),
                        on_press: EventHandler::new(|_| {}),
                    })
            }))
            .maybe_child(context)
    }
}

fn message_grid(head: Option<Uuid>, gap: f32, body: impl IntoElement) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .margin(Gaps::new(gap, 0., 0., 0.))
        .spacing(10.)
        .content(Content::Flex)
        .child(rect().width(Size::px(28.)).maybe_child(head.map(|player| {
            Avatar::new(player.to_string())
                .size(28.)
                .radius(CHIP_RADIUS_PX)
        })))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(4.)
                .child(body),
        )
}

fn sender_line(name: String, time: Option<String>, edited: bool) -> impl IntoElement {
    rect()
        .horizontal()
        .spacing(6.)
        .cross_align(Alignment::End)
        .child(
            label()
                .text(name)
                .font_size(text_size(12.))
                .font_weight(FontWeight::MEDIUM)
                .color(colors::fg_primary()),
        )
        .maybe_child(time.map(|time| {
            label()
                .text(time)
                .font_size(text_size(10.))
                .color(colors::fg_secondary())
        }))
        .maybe_child(edited.then(|| {
            label()
                .text(t("oneclient.chat.thread.edited"))
                .font_size(text_size(10.))
                .color(colors::fg_secondary())
        }))
}

#[derive(PartialEq)]
struct MessageRow {
    message: ChatMessage,
    group_id: i32,
    own: bool,
    show_head: bool,
    first: bool,
    editing: State<Option<(i64, String)>>,
    windowed: bool,
}

impl Component for MessageRow {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let name = use_player_name(self.message.sender);
        let mut menu = use_state(|| None::<(f32, f32)>);
        let editable = self.own && self.message.invite.is_none();
        let being_edited = self
            .editing
            .read()
            .as_ref()
            .is_some_and(|(message_id, _)| *message_id == self.message.id);

        let context = (*menu.read()).filter(|_| editable).map(|(x, y)| {
            let mut editing = self.editing;
            let group_id = self.group_id;
            let message_id = self.message.id;
            let content = self.message.content.clone();
            ContextMenu::new(x, y)
                .on_close(move |_| menu.set(None))
                .action(
                    IconType::Pencil01,
                    t("oneclient.chat.thread.edit"),
                    move |()| editing.set(Some((message_id, content.clone()))),
                )
                .separator()
                .danger_action(
                    IconType::Trash01,
                    t("oneclient.chat.thread.delete"),
                    move |()| dispatch.delete_chat_message(group_id, message_id),
                )
                .into_element()
        });
        let gap = if self.first {
            8.
        } else if self.show_head {
            16.
        } else {
            4.
        };

        let body =
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(4.)
                .maybe_child(self.show_head.then(|| {
                    sender_line(name, self.message.sent_at.map(clock), self.message.edited)
                }))
                .child(match &self.message.invite {
                    Some(invite) => InviteCard {
                        invite: invite.clone(),
                        sender: self.message.sender,
                        own: self.own,
                        windowed: self.windowed,
                    }
                    .into_element(),
                    None if being_edited => rect()
                        .padding(Gaps::new_symmetric(EDIT_PAD_Y_PX, EDIT_PAD_X_PX))
                        .margin(Gaps::new_symmetric(-EDIT_PAD_Y_PX, -EDIT_PAD_X_PX))
                        .corner_radius(rounded(CHIP_RADIUS_PX))
                        .background(colors::brand().with_a(32))
                        .border(inset_border(colors::brand().with_a(140)))
                        .child(message_text(&self.message.content))
                        .into_element(),
                    None => message_text(&self.message.content)
                        .width(Size::fill())
                        .into_element(),
                })
                .maybe_child((!self.show_head && self.message.edited).then(|| {
                    label()
                        .text(t("oneclient.chat.thread.edited"))
                        .font_size(text_size(10.))
                        .color(colors::fg_secondary())
                }));

        rect()
            .width(Size::fill())
            .on_secondary_down(move |e: Event<PressEventData>| {
                if !editable {
                    return;
                }
                if let PressEventData::Mouse(m) = e.data() {
                    menu.set(Some((
                        m.global_location.x as f32,
                        m.global_location.y as f32,
                    )));
                }
            })
            .child(message_grid(
                self.show_head.then_some(self.message.sender),
                gap,
                body,
            ))
            .maybe_child(context)
    }
}

fn message_text(content: &str) -> Label {
    label()
        .text(content.to_string())
        .font_size(text_size(14.))
        .line_height(1.4)
        .color(colors::fg_primary())
}

#[derive(PartialEq)]
struct InviteCard {
    invite: MessageInvite,
    sender: Uuid,
    own: bool,
    windowed: bool,
}

impl Component for InviteCard {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let invite_id = self.invite.id;
        let game = use_chat_invite_game(invite_id);
        let blocked = use_chat_roster().is_blocked(self.sender);
        let mut card_height = use_state(|| 0f32);
        let mut footer_height = use_state(|| 0f32);
        let facts = (self.invite.is_open() || game.is_some()).then(|| {
            let game = game.unwrap_or_default();
            let unknown = || t("oneclient.chat.invites.unknown").to_string();
            let facts = if self.windowed {
                rect().horizontal().spacing(20.)
            } else {
                rect().vertical().width(Size::fill()).spacing(4.)
            };
            facts
                .child(invite_fact(
                    t("oneclient.chat.invites.version"),
                    game.version.unwrap_or_else(unknown),
                    self.windowed,
                ))
                .child(invite_fact(
                    t("oneclient.chat.invites.loader"),
                    game.loader
                        .map_or_else(unknown, |loader| loader.real_name()),
                    self.windowed,
                ))
                .into_element()
        });

        let footer = if self.invite.is_open() && blocked {
            rect().into_element()
        } else if self.invite.is_open() && !self.own {
            let join = accent_button(t("oneclient.chat.invites.join"), move |_| {
                dispatch.join_invite(invite_id)
            });
            if self.windowed {
                join.into_element()
            } else {
                join.width(Size::fill()).into_element()
            }
        } else if self.invite.is_open() {
            status_line(
                None,
                t("oneclient.chat.invites.join_in_game"),
                colors::fg_secondary(),
            )
        } else {
            match self.invite.status.to_ascii_lowercase().as_str() {
                "accepted" => status_line(
                    Some(IconType::Check),
                    t("oneclient.chat.invites.running"),
                    colors::success(),
                ),
                "declined" => status_line(
                    None,
                    t("oneclient.chat.invites.declined"),
                    colors::fg_secondary(),
                ),
                "expired" => status_line(
                    None,
                    t("oneclient.chat.invites.expired"),
                    colors::fg_secondary(),
                ),
                _ => status_line(
                    None,
                    t("oneclient.chat.invites.cancelled"),
                    colors::fg_secondary(),
                ),
            }
        };

        let card = rect()
            .margin(Gaps::new(2., 0., 0., 0.))
            .padding(Gaps::new_all(12.))
            .corner_radius(rounded(CARD_RADIUS_PX))
            .background(colors::card())
            .border(inset_border(colors::component_border()));
        let card = if self.windowed {
            let right = ((*card_height.read() - *footer_height.read()) / 2.).max(12.);
            card.horizontal()
                .width(Size::fill())
                .content(Content::Flex)
                .cross_align(Alignment::Center)
                .spacing(24.)
                .padding(Gaps::new(12., right, 12., 12.))
                .on_sized(move |e: Event<SizedEventData>| {
                    card_height.set_if_modified(e.area.height());
                })
        } else {
            card.vertical().width(Size::px(240.)).spacing(10.)
        };

        let footer = if self.windowed {
            rect()
                .horizontal()
                .width(Size::flex(1.))
                .main_align(Alignment::End)
                .child(
                    rect()
                        .on_sized(move |e: Event<SizedEventData>| {
                            footer_height.set_if_modified(e.area.height());
                        })
                        .child(footer),
                )
                .into_element()
        } else {
            rect()
                .horizontal()
                .width(Size::fill())
                .main_align(Alignment::Center)
                .child(footer)
                .into_element()
        };

        card.child(
            rect()
                .vertical()
                .spacing(5.)
                .child(
                    label()
                        .text(t("oneclient.chat.invites.card_label"))
                        .font_size(text_size(10.))
                        .font_weight(FontWeight::MEDIUM)
                        .letter_spacing(0.6)
                        .color(colors::fg_secondary()),
                )
                .child(
                    label()
                        .text(t("oneclient.chat.invites.shared_session"))
                        .font_size(text_size(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_primary()),
                ),
        )
        .maybe_child(facts)
        .child(footer)
    }
}

fn invite_fact(name: &'static str, value: String, stacked: bool) -> Element {
    let fact = if stacked {
        rect().vertical().spacing(2.)
    } else {
        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .spacing(8.)
    };
    fact.child(
        label()
            .text(name)
            .font_size(text_size(12.))
            .width(if stacked {
                Size::auto()
            } else {
                Size::flex(1.0)
            })
            .color(colors::fg_secondary()),
    )
    .child(
        label()
            .text(value)
            .font_size(text_size(12.))
            .font_weight(FontWeight::MEDIUM)
            .max_lines(1)
            .text_overflow(TextOverflow::Ellipsis)
            .color(colors::fg_primary()),
    )
    .into_element()
}

fn status_line(icon: Option<IconType>, text: &'static str, color: Color) -> Element {
    rect()
        .horizontal()
        .height(Size::px(30.))
        .center()
        .spacing(6.)
        .maybe_child(icon.map(|icon| Icon::new(icon).size(14.).color(color)))
        .child(
            label()
                .text(text)
                .font_size(text_size(12.))
                .font_weight(FontWeight::MEDIUM)
                .color(color),
        )
        .into_element()
}

#[derive(PartialEq)]
struct PendingRow {
    group_id: i32,
    own_id: Uuid,
    show_head: bool,
    pending: PendingMessage,
}

impl Component for PendingRow {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let name = use_player_name(self.own_id);
        let group_id = self.group_id;
        let key = self.pending.key;
        let failed = self.pending.failed;
        let discard = dispatch.clone();

        let body = rect()
            .vertical()
            .width(Size::fill())
            .spacing(4.)
            .maybe_child(self.show_head.then(|| sender_line(name, None, false)))
            .child(
                label()
                    .text(self.pending.content.clone())
                    .font_size(text_size(14.))
                    .line_height(1.4)
                    .width(Size::fill())
                    .color(colors::fg_secondary()),
            )
            .child(
                rect()
                    .horizontal()
                    .spacing(4.)
                    .cross_align(Alignment::Center)
                    .child(
                        label()
                            .text(if failed {
                                t("oneclient.chat.thread.not_sent")
                            } else {
                                t("oneclient.chat.thread.sending")
                            })
                            .font_size(text_size(10.))
                            .color(if failed {
                                colors::danger()
                            } else {
                                colors::fg_secondary()
                            }),
                    )
                    .maybe_child(failed.then(|| {
                        Button::new()
                            .ghost()
                            .small()
                            .text(t("oneclient.chat.common.retry"))
                            .on_press(move |_| dispatch.retry_chat_message(group_id, key))
                    }))
                    .maybe_child(failed.then(|| {
                        Button::new()
                            .ghost()
                            .small()
                            .text(t("oneclient.chat.thread.discard"))
                            .on_press(move |_| discard.discard_chat_message(group_id, key))
                    })),
            );

        message_grid(
            self.show_head.then_some(self.own_id),
            if self.show_head { 16. } else { 4. },
            body,
        )
    }
}

fn submit_draft(
    dispatch: &Actions,
    mut draft: State<String>,
    mut editing: State<Option<(i64, String)>>,
    group_id: i32,
    on_sent: &EventHandler<()>,
) {
    let content = draft.read().clone();
    if content.trim().is_empty() || content.len() > MAX_MESSAGE_LENGTH {
        return;
    }

    let edit = editing.read().clone();
    match edit {
        Some((message_id, original)) => {
            if content.trim() != original.trim() {
                dispatch.edit_chat_message(group_id, message_id, content);
            }
            editing.set(None);
        }
        None => {
            dispatch.send_chat_message(group_id, content);
            on_sent.call(());
        }
    }
    draft.set(String::new());
}

#[derive(PartialEq)]
struct Composer {
    group_id: i32,
    placeholder: ChatConversation,
    own_id: Uuid,
    editing: State<Option<(i64, String)>>,
    on_sent: EventHandler<()>,
    key: DiffKey,
}

impl KeyExt for Composer {
    fn key_mut(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for Composer {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let own_id = self.own_id;
        let group_id = self.group_id;
        let mut draft = use_state(|| chat_draft(own_id, group_id));
        let mut emoji_open = use_state(|| false);
        let mut picker_area = use_state(Area::default);
        let mut toggle_area = use_state(Area::default);
        let mut loaded_edit = use_state(|| None::<i64>);
        let mut stashed = use_state(String::new);
        let mut edit_ended = use_state(|| false);
        let editing = self.editing;
        let mut cancel_edit = self.editing;

        let current_edit = editing.read().clone();
        let edit_id = current_edit.as_ref().map(|(id, _)| *id);
        if edit_id != *loaded_edit.peek() {
            if loaded_edit.peek().is_none() {
                stashed.set(draft.peek().clone());
            }
            match &current_edit {
                Some((_, content)) => draft.set(content.clone()),
                None => draft.set(std::mem::take(&mut *stashed.write())),
            }
            loaded_edit.set(edit_id);
            edit_ended.set(edit_id.is_none());
        }

        save_chat_draft(
            own_id,
            group_id,
            if edit_id.is_some() {
                stashed.peek().clone()
            } else {
                draft.peek().clone()
            },
        );

        let counterpart = self.placeholder.counterpart(self.own_id);
        let name = use_player_name(counterpart.unwrap_or(self.own_id));
        let title = conversation_title(&self.placeholder, counterpart.map(|_| name));

        let text = draft.read().clone();
        let too_long = text.len() > MAX_MESSAGE_LENGTH;

        let on_key = dispatch.clone();
        let sent_by_key = self.on_sent.clone();
        let sent_by_press = self.on_sent.clone();

        let picker = emoji_open().then(|| {
            rect()
                .position(Position::new_absolute().left(12.).bottom(56.))
                .layer(Layer::Overlay)
                .on_sized(move |e: Event<SizedEventData>| picker_area.set_if_modified(e.area))
                .on_global_pointer_down(move |e: Event<PointerEventData>| {
                    let at = e.data().global_location();
                    let (x, y) = (at.x as f32, at.y as f32);
                    if !area_contains(&picker_area.read(), x, y)
                        && !area_contains(&toggle_area.read(), x, y)
                    {
                        emoji_open.set(false);
                    }
                })
                .vertical()
                .padding(Gaps::new_symmetric(10., EMOJI_SIDE_PX))
                .spacing(8.)
                .corner_radius(rounded(CARD_RADIUS_PX))
                .background(colors::component_bg())
                .border(inset_border(colors::component_border()))
                .shadow(popup_shadow())
                .child(
                    rect().padding(Gaps::new(2., 0., 0., 0.)).child(
                        label()
                            .text(t("oneclient.chat.thread.emoji_heading"))
                            .font_size(text_size(11.))
                            .font_weight(FontWeight::MEDIUM)
                            .letter_spacing(0.44)
                            .color(colors::fg_secondary()),
                    ),
                )
                .children(
                    EMOJI
                        .chunks(8)
                        .map(|row| {
                            rect()
                                .horizontal()
                                .spacing(EMOJI_GAP_PX)
                                .children(
                                    row.iter()
                                        .map(|emoji| {
                                            EmojiCell {
                                                emoji,
                                                on_pick: EventHandler::new(move |_| {
                                                    draft.write().push_str(emoji);
                                                }),
                                            }
                                            .into_element()
                                        })
                                        .collect::<Vec<_>>(),
                                )
                                .into_element()
                        })
                        .collect::<Vec<_>>(),
                )
        });

        rect()
            .vertical()
            .width(Size::fill())
            .padding(Gaps::new(8., 12., 12., 12.))
            .spacing(6.)
            .maybe_child(picker)
            .maybe_child(current_edit.is_some().then(|| {
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .spacing(6.)
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .child(
                        Icon::new(IconType::Pencil01)
                            .size(12.)
                            .color(colors::fg_secondary()),
                    )
                    .child(
                        label()
                            .text(t("oneclient.chat.thread.editing"))
                            .font_size(text_size(11.))
                            .width(Size::flex(1.0))
                            .color(colors::fg_secondary()),
                    )
                    .child(
                        Button::new()
                            .ghost()
                            .small()
                            .text(t("oneclient.chat.common.cancel"))
                            .on_press(move |_| cancel_edit.set(None)),
                    )
            }))
            .child(
                rect()
                    .key("input")
                    .horizontal()
                    .width(Size::fill())
                    .height(Size::px(38.))
                    .padding(Gaps::new_all(4.))
                    .spacing(4.)
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .corner_radius(rounded(ROW_RADIUS_PX))
                    .background(colors::component_bg())
                    .border(inset_border(colors::component_border()))
                    .child(
                        rect()
                            .width(Size::px(30.))
                            .height(Size::px(30.))
                            .center()
                            .corner_radius(rounded(CHIP_RADIUS_PX))
                            .cursor(CursorIcon::Pointer)
                            .a11y_alt(t("oneclient.chat.thread.emoji"))
                            .on_sized(move |e: Event<SizedEventData>| {
                                toggle_area.set_if_modified(e.area)
                            })
                            .on_press(move |_| emoji_open.toggle())
                            .child(Icon::new(IconType::FaceSmile).size(18.).color(
                                colors::fg_primary().with_a(if emoji_open() { 255 } else { 178 }),
                            )),
                    )
                    .child(
                        TextInput::new(draft)
                            .bare()
                            .width(Size::flex(1.0))
                            .font_size(text_size(14.))
                            .auto_focus(*edit_ended.peek())
                            .placeholder(tf("oneclient.chat.thread.message_placeholder", &[&title]))
                            .on_submit(move |_| {
                                emoji_open.set(false);
                                submit_draft(&on_key, draft, editing, group_id, &sent_by_key);
                            })
                            .key(edit_id),
                    )
                    .child(
                        rect()
                            .width(Size::px(30.))
                            .height(Size::px(30.))
                            .center()
                            .corner_radius(rounded(CHIP_RADIUS_PX))
                            .background(colors::brand())
                            .cursor(CursorIcon::Pointer)
                            .a11y_alt(t("oneclient.chat.common.send"))
                            .on_press(move |_| {
                                emoji_open.set(false);
                                submit_draft(&dispatch, draft, editing, group_id, &sent_by_press);
                            })
                            .child(Icon::new(IconType::Send01).size(14.).color(Color::WHITE)),
                    ),
            )
            .maybe_child(too_long.then(|| {
                label()
                    .text(t("oneclient.chat.thread.too_long"))
                    .font_size(text_size(11.))
                    .color(colors::danger())
            }))
            .maybe_child((emoji_open() || current_edit.is_some()).then(|| {
                EscapeGuard {
                    on_escape: EventHandler::new(move |_| {
                        if emoji_open() {
                            emoji_open.set(false);
                        } else {
                            cancel_edit.set(None);
                        }
                    }),
                }
                .into_element()
            }))
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

#[derive(PartialEq)]
struct EmojiCell {
    emoji: &'static str,
    on_pick: EventHandler<()>,
}

impl Component for EmojiCell {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let on_pick = self.on_pick.clone();

        rect()
            .width(Size::px(EMOJI_CELL_PX))
            .height(Size::px(EMOJI_CELL_PX))
            .center()
            .corner_radius(rounded(CHIP_RADIUS_PX))
            .background(if hovered() {
                colors::ghost_overlay()
            } else {
                Color::TRANSPARENT
            })
            .cursor(CursorIcon::Pointer)
            .on_pointer_over(move |_| hovered.set(true))
            .on_pointer_out(move |_| hovered.set(false))
            .on_press(move |_| on_pick.call(()))
            .child(
                label()
                    .text(self.emoji)
                    .font_size(text_size(20.))
                    .line_height(1.)
                    .text_align(TextAlign::Center)
                    .width(Size::fill()),
            )
    }
}
