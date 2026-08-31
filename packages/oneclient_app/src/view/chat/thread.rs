use freya::prelude::*;
use uuid::Uuid;

use oneclient_polyplus::{GroupKind, MAX_MESSAGE_LENGTH};

use crate::chat::{ChatConversation, ChatMessage, PendingMessage, ThreadStatus};
use crate::components::{Button, Icon, IconType, ScrollArea, TextInput, auto_scroll_toggle};
use crate::hooks::{Actions, use_chat_thread, use_dispatch};
use crate::theme::colors;

use super::common::{clock, use_player_name};

const BUBBLE_RADIUS_PX: f32 = 12.;
const BUBBLE_MAX_PERCENT: f32 = 74.;
const PAGE_TRIGGER_PX: f32 = 240.;

#[derive(PartialEq)]
pub(super) struct ThreadView {
    pub conversation: ChatConversation,
    pub own_id: Uuid,
}

impl Component for ThreadView {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();

        let group_id = self.conversation.id;
        let own_id = self.own_id;
        let is_group = self.conversation.kind == GroupKind::Group;

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

        let rows: Vec<Element> = messages
            .iter()
            .cloned()
            .map(|message| {
                let own = message.sender == own_id;
                MessageRow {
                    show_sender: is_group && !own,
                    message,
                    own,
                }
                .into_element()
            })
            .chain(
                pending
                    .into_iter()
                    .map(|pending| PendingRow { group_id, pending }.into_element()),
            )
            .collect();

        rect()
            .vertical()
            .width(Size::flex(1.0))
            .height(Size::fill())
            .content(Content::Flex)
            .overflow(Overflow::Clip)
            .child(ThreadHeader {
                conversation: self.conversation.clone(),
                own_id,
                pinned,
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
                            .padding(Gaps::new(16., 20., 8., 20.))
                            .spacing(8.)
                            .children(rows),
                    )
                    .into_element(),
            })
            .child(failure.filter(|_| !empty).map_or_else(
                || {
                    rect()
                        .width(Size::fill())
                        .height(Size::px(0.))
                        .into_element()
                },
                |message| {
                    thread_banner(
                        message,
                        EventHandler::new(move |_| dispatch.reload_conversation(group_id)),
                    )
                },
            ))
            .child(
                Composer {
                    group_id,
                    on_sent: EventHandler::new(move |_| pinned.set(true)),
                    key: DiffKey::None,
                }
                .key(group_id),
            )
    }
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
                .font_size(13.)
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
        .padding(Gaps::new(8., 20., 0., 20.))
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
    pinned: State<bool>,
}

impl Component for ThreadHeader {
    fn render(&self) -> impl IntoElement {
        let counterpart = self.conversation.counterpart(self.own_id);
        let name = use_player_name(counterpart.unwrap_or(self.own_id));
        let resolved = counterpart.map(|_| name);

        let title = self
            .conversation
            .name
            .clone()
            .or(resolved)
            .unwrap_or_else(|| match self.conversation.kind {
                GroupKind::Group => "Group".to_string(),
                _ => "Direct message".to_string(),
            });

        rect()
            .horizontal()
            .width(Size::fill())
            .padding(Gaps::new(18., 20., 18., 20.))
            .cross_align(Alignment::Center)
            .content(Content::Flex)
            .spacing(8.)
            .child(
                label()
                    .text(title)
                    .font_size(16.)
                    .font_weight(FontWeight::BOLD)
                    .color(colors::fg_primary())
                    .max_lines(1),
            )
            .child(rect().width(Size::flex(1.0)))
            .maybe_child(self.conversation.special.then(|| {
                label()
                    .text("Special Chat")
                    .font_size(11.)
                    .color(colors::brand())
            }))
            .child(auto_scroll_toggle(self.pinned))
    }
}

#[derive(PartialEq)]
struct MessageRow {
    message: ChatMessage,
    own: bool,
    show_sender: bool,
}

impl Component for MessageRow {
    fn render(&self) -> impl IntoElement {
        let name = use_player_name(self.message.sender);
        let sender = self.show_sender.then_some(name);

        let stamp = self.message.sent_at.map(clock);
        let own = self.own;

        let (background, foreground, meta) = if own {
            (colors::brand(), colors::fg_primary(), colors::fg_primary())
        } else {
            (
                colors::component_bg(),
                colors::fg_primary(),
                colors::fg_secondary(),
            )
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .main_align(if own {
                Alignment::End
            } else {
                Alignment::Start
            })
            .child(
                rect()
                    .vertical()
                    .max_width(Size::percent(BUBBLE_MAX_PERCENT))
                    .padding(Gaps::new(7., 11., 8., 11.))
                    .spacing(2.)
                    .corner_radius(CornerRadius::from(BUBBLE_RADIUS_PX))
                    .background(background)
                    .cross_align(if own {
                        Alignment::End
                    } else {
                        Alignment::Start
                    })
                    .child(
                        rect()
                            .horizontal()
                            .spacing(6.)
                            .cross_align(Alignment::Center)
                            .maybe_child(sender.map(|sender| {
                                label()
                                    .text(sender)
                                    .font_size(11.)
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .color(colors::fg_secondary())
                            }))
                            .maybe_child(
                                stamp
                                    .map(|stamp| label().text(stamp).font_size(10.).color(meta)),
                            )
                            .maybe_child(self.message.edited.then(|| {
                                label().text("edited").font_size(10.).color(meta)
                            })),
                    )
                    .child(
                        label()
                            .text(self.message.content.clone())
                            .font_size(14.)
                            .color(foreground),
                    ),
            )
    }
}

#[derive(PartialEq)]
struct PendingRow {
    group_id: i32,
    pending: PendingMessage,
}

impl Component for PendingRow {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let group_id = self.group_id;
        let key = self.pending.key;
        let failed = self.pending.failed;
        let discard = dispatch.clone();

        rect()
            .horizontal()
            .width(Size::fill())
            .main_align(Alignment::End)
            .child(
                rect()
                    .vertical()
                    .max_width(Size::percent(BUBBLE_MAX_PERCENT))
                    .padding(Gaps::new(7., 11., 8., 11.))
                    .spacing(2.)
                    .corner_radius(CornerRadius::from(BUBBLE_RADIUS_PX))
                    .background(colors::component_bg())
                    .cross_align(Alignment::End)
                    .child(
                        label()
                            .text(if failed { "Not sent" } else { "Sending" })
                            .font_size(10.)
                            .color(if failed {
                                colors::danger()
                            } else {
                                colors::fg_secondary()
                            }),
                    )
                    .child(
                        label()
                            .text(self.pending.content.clone())
                            .font_size(14.)
                            .color(colors::fg_secondary()),
                    )
                    .maybe_child(failed.then(|| {
                        rect()
                            .horizontal()
                            .spacing(4.)
                            .child(
                                Button::new()
                                    .ghost()
                                    .small()
                                    .text("Retry")
                                    .on_press(move |_| dispatch.retry_chat_message(group_id, key)),
                            )
                            .child(
                                Button::new()
                                    .ghost()
                                    .small()
                                    .text("Discard")
                                    .on_press(move |_| discard.discard_chat_message(group_id, key)),
                            )
                    })),
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
        let draft = use_state(String::new);
        let group_id = self.group_id;

        let text = draft.read().clone();
        let too_long = text.len() > MAX_MESSAGE_LENGTH;
        let can_send = !text.trim().is_empty() && !too_long;

        let on_key = dispatch.clone();
        let sent_by_key = self.on_sent.clone();
        let sent_by_press = self.on_sent.clone();

        rect()
            .vertical()
            .width(Size::fill())
            .padding(Gaps::new(10., 20., 16., 20.))
            .spacing(6.)
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .spacing(8.)
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .child(
                        TextInput::new(draft)
                            .width(Size::flex(1.0))
                            .placeholder("Write a message")
                            .on_submit(move |_| {
                                submit_draft(&on_key, draft, group_id, &sent_by_key)
                            }),
                    )
                    .child(
                        Button::new()
                            .primary()
                            .icon()
                            .alt("Send")
                            .enabled(can_send)
                            .on_press(move |_| {
                                submit_draft(&dispatch, draft, group_id, &sent_by_press)
                            })
                            .child(Icon::new(IconType::ArrowRight).size(18.)),
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
