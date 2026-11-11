use freya::prelude::*;
use uuid::Uuid;

use crate::chat::ThreadStatus;
use crate::components::{IconType, PartyTab};
use crate::hooks::{
    settled_or_loading, use_chat_inbox, use_chat_roster, use_chat_thread, use_current_account,
    use_dispatch, use_social_tab,
};
use crate::i18n::t;
use crate::state::SocialTab;
use crate::theme::{colors, rounded, text_size};

use super::common::{HeaderButton, ROW_RADIUS_PX, inset_border};
use super::list::{BlockedPane, ListPane, PanelHeader};
use super::thread::ThreadView;

const SWITCH_HEIGHT_PX: f32 = 32.;
const SWITCH_INSET_PX: f32 = 3.;

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
            dispatch.sync_chat_owner(account.as_ref());
        }

        let (own_id, username) = account
            .flatten()
            .map(|account| (account.id, account.username))
            .unwrap_or_else(|| {
                (
                    Uuid::nil(),
                    t("oneclient.chat.panel.not_signed_in").to_string(),
                )
            });

        use_hook({
            let dispatch = dispatch.clone();
            move || dispatch.refresh_chat()
        });

        let social_tab = use_social_tab();
        let mut tab = use_state(|| PanelTab::Chats);
        let mut view = use_state(|| {
            if chat.active.is_some() {
                PanelView::Chat
            } else {
                PanelView::List
            }
        });

        let mut active_gone = use_state(|| chat.active_gone);
        if *active_gone.peek() != chat.active_gone {
            active_gone.set(chat.active_gone);
            if *view.peek() == PanelView::Chat {
                view.set(PanelView::List);
            }
        }

        let mut active_opened = use_state(|| chat.active_opened);
        if *active_opened.peek() != chat.active_opened {
            active_opened.set(chat.active_opened);
            view.set(PanelView::Chat);
        }

        if chat.open_friends {
            dispatch.friends_shown();
            tab.set(PanelTab::Friends);
            view.set(PanelView::List);
        }

        let load_failure = match use_chat_thread(chat.active.unwrap_or_default()).status {
            ThreadStatus::Error(message) => Some(message),
            _ => None,
        };

        let back_to_list = {
            let dispatch = dispatch.clone();
            EventHandler::new(move |_| {
                dispatch.close_conversation();
                view.set(PanelView::List);
            })
        };

        let current = *view.read();
        let thread = chat
            .active
            .and_then(|id| chat.conversation(id).cloned())
            .map(|conversation| {
                ThreadView {
                    conversation,
                    own_id,
                    expanded: self.expanded,
                    windowed: self.mode == PanelMode::Window,
                    on_back: back_to_list.clone(),
                }
                .into_element()
            });

        let list = ListPane {
            expanded: self.expanded,
            own_id,
            conversations: chat.conversations.clone(),
            names: chat.names.clone(),
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
                    &chat.error.clone().or(load_failure),
                    (!self.expanded).then(|| back_to_list.clone()),
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

        let body = if social_tab == SocialTab::Party {
            PartyTab.into_element()
        } else if self.expanded {
            root.child(list).child(detail).into_element()
        } else {
            root.child(match current {
                PanelView::List => list.into_element(),
                _ => detail,
            })
            .into_element()
        };

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .content(Content::Flex)
            .color(colors::fg_primary())
            .child(PanelHeader {
                mode: self.mode,
                own_id,
                username,
                connected: chat.connected,
            })
            .child(social_tabs(social_tab))
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .child(body),
            )
            .into_element()
    }
}

fn social_tabs(current: SocialTab) -> Element {
    let dispatch = use_dispatch();
    let side = |tab: SocialTab, label: &'static str| {
        let dispatch = dispatch.clone();
        SwitchSide {
            label,
            active: current == tab,
            on_press: EventHandler::new(move |_| dispatch.set_social_tab(tab)),
        }
    };

    rect()
        .width(Size::fill())
        .padding(Gaps::new(0., 16., 12., 16.))
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .height(Size::px(SWITCH_HEIGHT_PX))
                .content(Content::Flex)
                .padding(Gaps::new_all(SWITCH_INSET_PX))
                .corner_radius(rounded(ROW_RADIUS_PX))
                .background(colors::component_bg())
                .border(inset_border(colors::component_border()))
                .child(side(SocialTab::Chat, t("oneclient.chat.tabs.social_chat")))
                .child(side(SocialTab::Party, t("oneclient.chat.tabs.party"))),
        )
        .into_element()
}

#[derive(PartialEq)]
struct SwitchSide {
    label: &'static str,
    active: bool,
    on_press: EventHandler<()>,
}

impl Component for SwitchSide {
    fn render(&self) -> impl IntoElement {
        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let mut hovered = use_state(|| false);
        let on_press = self.on_press.clone();

        rect()
            .width(Size::flex(1.0))
            .height(Size::fill())
            .center()
            .corner_radius(rounded(ROW_RADIUS_PX - SWITCH_INSET_PX))
            .background(match (self.active, hovered()) {
                (true, _) => colors::component_bg(),
                (false, true) => colors::ghost_overlay(),
                (false, false) => Color::TRANSPARENT,
            })
            .maybe(focus().is_focused(), |el| {
                el.border(inset_border(colors::fg_secondary()))
            })
            .cursor(CursorIcon::Pointer)
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .on_pointer_over(move |_| hovered.set(true))
            .on_pointer_out(move |_| hovered.set(false))
            .on_press(move |_| on_press.call(()))
            .child(
                label()
                    .text(self.label)
                    .font_size(text_size(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .max_lines(1)
                    .color(if self.active {
                        colors::fg_primary()
                    } else {
                        colors::fg_secondary()
                    }),
            )
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
                .text(t("oneclient.chat.panel.no_chat_selected"))
                .font_size(text_size(14.))
                .font_weight(FontWeight::MEDIUM)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text(t("oneclient.chat.panel.no_chat_selected_hint"))
                .font_size(text_size(12.))
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
                    tooltip: t("oneclient.chat.common.back"),
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
                        .text(
                            error
                                .clone()
                                .unwrap_or_else(|| t("oneclient.chat.panel.opening").to_string()),
                        )
                        .font_size(text_size(12.))
                        .color(match error {
                            Some(_) => colors::danger(),
                            None => colors::fg_secondary(),
                        }),
                ),
        )
        .into_element()
}
