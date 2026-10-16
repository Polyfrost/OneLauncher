use freya::prelude::*;
use uuid::Uuid;

use crate::components::IconType;
use crate::hooks::{
    settled_or_loading, use_chat_inbox, use_chat_roster, use_current_account, use_dispatch,
};
use crate::theme::colors;

use super::common::HeaderButton;
use super::list::{BlockedPane, ListPane};
use super::thread::ThreadView;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PanelMode {
    Launcher,
    Window,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PanelView {
    List,
    Chat,
    Blocked,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PanelTab {
    Chats,
    Friends,
    Groups,
}

#[derive(PartialEq)]
pub(super) struct SocialPanel {
    pub mode: PanelMode,
    pub expanded: bool,
}

impl Component for SocialPanel {
    fn render(&self) -> impl IntoElement {
        let chat = use_chat_inbox();
        let roster = use_chat_roster();
        let dispatch = use_dispatch();

        let account = settled_or_loading(&use_current_account());
        if let Some(account) = account.clone() {
            dispatch.sync_chat_owner(account.as_ref().map(|account| account.id));
        }

        let (own_id, username) = account
            .flatten()
            .map(|account| (account.id, account.username))
            .unwrap_or_else(|| (Uuid::nil(), "Not signed in".to_string()));

        use_hook({
            let dispatch = dispatch.clone();
            move || dispatch.refresh_chat()
        });

        let tab = use_state(|| PanelTab::Chats);
        let mut view = use_state(|| {
            if chat.active.is_some() {
                PanelView::Chat
            } else {
                PanelView::List
            }
        });

        let current = *view.read();
        let thread = chat
            .active
            .and_then(|id| chat.conversation(id).cloned())
            .map(|conversation| {
                ThreadView {
                    conversation,
                    own_id,
                    expanded: self.expanded,
                    can_close: self.mode == PanelMode::Launcher,
                    on_back: EventHandler::new(move |_| view.set(PanelView::List)),
                }
                .into_element()
            });

        let list = ListPane {
            mode: self.mode,
            expanded: self.expanded,
            own_id,
            username,
            conversations: chat.conversations.clone(),
            loading: chat.loading,
            error: chat
                .error
                .clone()
                .filter(|_| !(current == PanelView::Chat && chat.active.is_none())),
            active: chat.active,
            roster: roster.clone(),
            tab,
            view,
        };

        let blocked = || {
            BlockedPane {
                blocked: roster.blocked.clone(),
                view,
            }
            .into_element()
        };

        let detail = match current {
            PanelView::Blocked => blocked(),
            PanelView::Chat => thread.unwrap_or_else(|| {
                opening(
                    &chat.error,
                    (!self.expanded).then(|| EventHandler::new(move |_| view.set(PanelView::List))),
                )
            }),
            PanelView::List => empty_detail(),
        };

        let root = rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::fill())
            .content(Content::Flex)
            .color(colors::fg_primary());

        if self.expanded {
            root.child(list).child(detail).into_element()
        } else {
            root.child(match current {
                PanelView::List => list.into_element(),
                _ => detail,
            })
            .into_element()
        }
    }
}

fn empty_detail() -> Element {
    rect()
        .vertical()
        .width(Size::flex(1.0))
        .height(Size::fill())
        .center()
        .padding(Gaps::new_all(24.))
        .spacing(6.)
        .child(
            label()
                .text("No chat selected")
                .font_size(14.)
                .font_weight(FontWeight::MEDIUM)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text("Pick a chat, friend or group from the list.")
                .font_size(12.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn opening(error: &Option<String>, on_back: Option<EventHandler<()>>) -> Element {
    rect()
        .vertical()
        .width(Size::flex(1.0))
        .height(Size::fill())
        .content(Content::Flex)
        .maybe_child(on_back.map(|on_back| {
            rect()
                .padding(Gaps::new(12., 10., 12., 8.))
                .child(HeaderButton {
                    icon: IconType::ArrowLeft,
                    tooltip: "Back",
                    on_press: on_back,
                })
        }))
        .child(
            rect()
                .width(Size::fill())
                .height(Size::flex(1.0))
                .center()
                .padding(Gaps::new_all(24.))
                .child(
                    label()
                        .text(error.clone().unwrap_or_else(|| "Opening chat…".to_string()))
                        .font_size(12.)
                        .color(match error {
                            Some(_) => colors::danger(),
                            None => colors::fg_secondary(),
                        }),
                ),
        )
        .into_element()
}
