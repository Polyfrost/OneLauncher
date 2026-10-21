use freya::prelude::*;
use uuid::Uuid;

use oneclient_polyplus::{GroupKind, MAX_MESSAGE_LENGTH};

use crate::chat::{ChatConversation, ChatMessage, MessageInvite, PendingMessage, ThreadStatus};
use crate::components::{Avatar, Button, Icon, IconType, ScrollArea, TextInput};
use crate::hooks::{
    Actions, use_chat_online_set, use_chat_thread, use_dispatch, use_player_online,
};
use crate::theme::colors;

use super::common::{
    CARD_RADIUS_PX, CHIP_RADIUS_PX, HeaderButton, ROW_RADIUS_PX, card_bg, clock, day_label,
    edge_border, group_heads, hairline, head_with_dot, hover_bg, inset_border, popup_shadow,
    presence_color, use_player_name,
};
use super::list::{accent_button, conversation_title};

const PAGE_TRIGGER_PX: f32 = 240.;

const EMOJI_CELL_PX: f32 = 34.;
const EMOJI_GAP_PX: f32 = 2.;
const EMOJI_SIDE_PX: f32 = 14.;

const EMOJI: [&str; 40] = [
    "😀", "😂", "🥲", "😍", "😎", "🤔", "😭", "😤", "👍", "👎", "👏", "🙏", "🔥", "✨", "💀", "🎉",
    "❤️", "💙", "⭐", "👀", "💯", "✅", "❌", "🎮", "⚔️", "🛡️", "⛏️", "🏹", "💎", "🧱", "🪵", "🌲",
    "🍞", "🍎", "🐷", "🐑", "🐔", "🏠", "🌋", "🚀",
];

#[derive(PartialEq)]
pub(super) struct ThreadView {
    pub conversation: ChatConversation,
    pub own_id: Uuid,
    pub expanded: bool,
    pub can_close: bool,
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

        if *shown.peek() != group_id {
            shown.set(group_id);
            pinned.set(true);
            may_page.set(false);
        }

        let paging = dispatch.clone();

        let mut rows: Vec<Element> = Vec::with_capacity(messages.len() + pending.len());
        let mut previous: Option<&ChatMessage> = None;
        for message in messages.iter() {
            let day = message.sent_at.map(day_label);
            let new_day =
                day.is_some() && day != previous.and_then(|prev| prev.sent_at.map(day_label));
            if let Some(day) = day.filter(|_| new_day) {
                rows.push(day_divider(day).into_element());
            }

            let show_head = new_day || previous.is_none_or(|prev| prev.sender != message.sender);
            rows.push(
                MessageRow {
                    message: message.clone(),
                    show_head,
                    first: previous.is_none(),
                }
                .into_element(),
            );
            previous = Some(message);
        }

        let mut last_sender = previous.map(|prev| prev.sender);
        for pending in pending {
            rows.push(
                PendingRow {
                    group_id,
                    show_head: last_sender != Some(own_id),
                    own_id,
                    pending,
                }
                .into_element(),
            );
            last_sender = Some(own_id);
        }

        let title_hint = self.conversation.clone();

        rect()
            .vertical()
            .width(Size::flex(1.0))
            .height(Size::fill())
            .content(Content::Flex)
            .child(ThreadHeader {
                conversation: self.conversation.clone(),
                own_id,
                show_back: !self.expanded,
                can_close: self.can_close && !self.expanded,
                on_back: self.on_back.clone(),
            })
            .child(match (empty, &thread.status) {
                (true, ThreadStatus::Loading) => {
                    thread_notice("Loading messages…", colors::fg_secondary(), None)
                }
                (true, ThreadStatus::Error(message)) => thread_notice(
                    message.clone(),
                    colors::danger(),
                    Some(EventHandler::new(move |_| {
                        paging.reload_conversation(group_id);
                    })),
                ),
                (true, _) => thread_notice("No messages yet.", colors::fg_secondary(), None),
                _ => ScrollArea::new()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .stick_bottom(*pinned.read())
                    .on_user_scroll(move |_| {
                        pinned.set_if_modified(false);
                        may_page.set_if_modified(true);
                    })
                    .on_ctx(move |ctx: crate::components::ScrollAreaCtx| {
                        if exhausted || !*may_page.peek() || ctx.viewport_h <= 0. {
                            return;
                        }
                        if ctx.corrected_y < -PAGE_TRIGGER_PX {
                            return;
                        }

                        may_page.set(false);
                        paging.load_older_messages(group_id);
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
                    on_sent: EventHandler::new(move |_| pinned.set(true)),
                    key: DiffKey::None,
                }
                .key(group_id),
            )
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
                .font_size(11.)
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
                .font_size(12.)
                .color(color)
                .max_lines(3),
        )
        .maybe_child(on_retry.map(|on_retry| {
            Button::new()
                .secondary()
                .small()
                .text("Try again")
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
                .font_size(11.)
                .color(colors::danger())
                .width(Size::flex(1.0))
                .max_lines(2),
        )
        .child(
            Button::new()
                .ghost()
                .small()
                .text("Try again")
                .on_press(on_retry),
        )
        .into_element()
}

#[derive(PartialEq)]
struct ThreadHeader {
    conversation: ChatConversation,
    own_id: Uuid,
    show_back: bool,
    can_close: bool,
    on_back: EventHandler<()>,
}

impl Component for ThreadHeader {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let counterpart = self.conversation.counterpart(self.own_id);
        let name = use_player_name(counterpart.unwrap_or(self.own_id));
        let online = use_player_online(counterpart.unwrap_or(self.own_id));
        let others: Vec<Uuid> = self
            .conversation
            .members
            .iter()
            .copied()
            .filter(|member| *member != self.own_id)
            .collect();
        let online_members = use_chat_online_set(others).len();

        let title = conversation_title(&self.conversation, counterpart.map(|_| name));

        let sub = match (self.conversation.kind, counterpart) {
            (GroupKind::Group, _) => format!(
                "{} members · {online_members} online",
                self.conversation.members.len()
            ),
            (_, Some(_)) if online => "Online".to_string(),
            (_, Some(_)) => "Offline".to_string(),
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
            .border(edge_border(hairline(), 0., 0., 1.))
            .maybe_child(self.show_back.then(|| HeaderButton {
                icon: IconType::ArrowLeft,
                tooltip: "Back",
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
                            .font_size(14.)
                            .font_weight(FontWeight::MEDIUM)
                            .color(colors::fg_primary())
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis),
                    )
                    .child(
                        label()
                            .text(sub)
                            .font_size(11.)
                            .color(colors::fg_secondary())
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis),
                    ),
            )
            .maybe_child(self.can_close.then(|| {
                let pop_out = dispatch.clone();
                HeaderButton {
                    icon: IconType::LinkExternal01,
                    tooltip: "Open in a window",
                    on_press: EventHandler::new(move |_| pop_out.pop_out_social()),
                }
            }))
            .maybe_child(self.can_close.then(|| HeaderButton {
                icon: IconType::XClose,
                tooltip: "Close",
                on_press: EventHandler::new(move |_| dispatch.close_social()),
            }))
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
                .font_size(12.)
                .font_weight(FontWeight::MEDIUM)
                .color(colors::fg_primary()),
        )
        .maybe_child(time.map(|time| {
            label()
                .text(time)
                .font_size(10.)
                .color(colors::fg_secondary())
        }))
        .maybe_child(edited.then(|| {
            label()
                .text("edited")
                .font_size(10.)
                .color(colors::fg_secondary())
        }))
}

#[derive(PartialEq)]
struct MessageRow {
    message: ChatMessage,
    show_head: bool,
    first: bool,
}

impl Component for MessageRow {
    fn render(&self) -> impl IntoElement {
        let name = use_player_name(self.message.sender);
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
                    }
                    .into_element(),
                    None => label()
                        .text(self.message.content.clone())
                        .font_size(14.)
                        .line_height(1.4)
                        .width(Size::fill())
                        .color(colors::fg_primary())
                        .into_element(),
                });

        message_grid(self.show_head.then_some(self.message.sender), gap, body)
    }
}

#[derive(PartialEq)]
struct InviteCard {
    invite: MessageInvite,
}

impl Component for InviteCard {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let id = self.invite.id;

        let footer = match self.invite.status.as_str() {
            "accepted" => status_line(Some(IconType::Check), "Running", colors::success()),
            "declined" => status_line(None, "Declined", colors::fg_secondary()),
            "expired" => status_line(None, "Expired", colors::fg_secondary()),
            _ => rect()
                .width(Size::fill())
                .child(accent_button("Launch", move |_| {
                    dispatch.accept_chat_invite(id)
                }))
                .into_element(),
        };

        rect()
            .vertical()
            .width(Size::px(240.))
            .margin(Gaps::new(2., 0., 0., 0.))
            .padding(Gaps::new_all(12.))
            .spacing(10.)
            .corner_radius(CornerRadius::from(CARD_RADIUS_PX))
            .background(card_bg())
            .border(inset_border(hairline()))
            .child(
                rect()
                    .vertical()
                    .spacing(5.)
                    .child(
                        label()
                            .text("WORLD INVITE")
                            .font_size(10.)
                            .font_weight(FontWeight::MEDIUM)
                            .letter_spacing(0.6)
                            .color(colors::fg_secondary()),
                    )
                    .child(
                        label()
                            .text("Shared session")
                            .font_size(14.)
                            .font_weight(FontWeight::MEDIUM)
                            .color(colors::fg_primary()),
                    ),
            )
            .child(footer)
    }
}

fn status_line(icon: Option<IconType>, text: &'static str, color: Color) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::px(30.))
        .center()
        .spacing(6.)
        .maybe_child(icon.map(|icon| Icon::new(icon).size(14.).color(color)))
        .child(
            label()
                .text(text)
                .font_size(12.)
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
                    .font_size(14.)
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
                            .text(if failed { "Not sent" } else { "Sending…" })
                            .font_size(10.)
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
                            .text("Retry")
                            .on_press(move |_| dispatch.retry_chat_message(group_id, key))
                    }))
                    .maybe_child(failed.then(|| {
                        Button::new()
                            .ghost()
                            .small()
                            .text("Discard")
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
    group_id: i32,
    on_sent: &EventHandler<()>,
) {
    let content = draft.read().clone();
    if content.trim().is_empty() || content.len() > MAX_MESSAGE_LENGTH {
        return;
    }

    dispatch.send_chat_message(group_id, content);
    draft.set(String::new());
    on_sent.call(());
}

#[derive(PartialEq)]
struct Composer {
    group_id: i32,
    placeholder: ChatConversation,
    own_id: Uuid,
    on_sent: EventHandler<()>,
    key: DiffKey,
}

impl KeyExt for Composer {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for Composer {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let mut draft = use_state(String::new);
        let mut emoji_open = use_state(|| false);
        let mut picker_area = use_state(Area::default);
        let mut toggle_area = use_state(Area::default);
        let group_id = self.group_id;

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
                    if !contains(&picker_area.read(), x, y) && !contains(&toggle_area.read(), x, y)
                    {
                        emoji_open.set(false);
                    }
                })
                .vertical()
                .padding(Gaps::new_symmetric(10., EMOJI_SIDE_PX))
                .spacing(8.)
                .corner_radius(CornerRadius::from(CARD_RADIUS_PX))
                .background(colors::component_bg())
                .border(inset_border(hairline()))
                .shadow(popup_shadow())
                .child(
                    rect().padding(Gaps::new(2., 0., 0., 0.)).child(
                        label()
                            .text("EMOJI")
                            .font_size(11.)
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
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .height(Size::px(38.))
                    .padding(Gaps::new_all(4.))
                    .spacing(4.)
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .corner_radius(CornerRadius::from(ROW_RADIUS_PX))
                    .background(colors::component_bg())
                    .border(inset_border(hairline()))
                    .child(
                        rect()
                            .width(Size::px(30.))
                            .height(Size::px(30.))
                            .center()
                            .corner_radius(CornerRadius::from(CHIP_RADIUS_PX))
                            .cursor(CursorIcon::Pointer)
                            .a11y_alt("Emoji")
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
                            .font_size(14.)
                            .placeholder(format!("Message {title}"))
                            .on_submit(move |_| {
                                emoji_open.set(false);
                                submit_draft(&on_key, draft, group_id, &sent_by_key);
                            }),
                    )
                    .child(
                        rect()
                            .width(Size::px(30.))
                            .height(Size::px(30.))
                            .center()
                            .corner_radius(CornerRadius::from(CHIP_RADIUS_PX))
                            .background(colors::brand())
                            .cursor(CursorIcon::Pointer)
                            .a11y_alt("Send")
                            .on_press(move |_| {
                                emoji_open.set(false);
                                submit_draft(&dispatch, draft, group_id, &sent_by_press);
                            })
                            .child(Icon::new(IconType::Send01).size(14.).color(Color::WHITE)),
                    ),
            )
            .maybe_child(too_long.then(|| {
                label()
                    .text(format!(
                        "Messages are limited to {MAX_MESSAGE_LENGTH} characters."
                    ))
                    .font_size(11.)
                    .color(colors::danger())
            }))
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

fn contains(area: &Area, x: f32, y: f32) -> bool {
    x >= area.min_x() && x <= area.max_x() && y >= area.min_y() && y <= area.max_y()
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
                label()
                    .text(self.emoji)
                    .font_size(20.)
                    .line_height(1.)
                    .text_align(TextAlign::Center)
                    .width(Size::fill()),
            )
    }
}
