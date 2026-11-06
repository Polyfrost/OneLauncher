use std::collections::HashSet;

use freya::prelude::*;
use uuid::Uuid;

use oneclient_polyplus::Friend;

use crate::components::{Button, Icon, IconType, OverlayPopup, ScrollArea, TextInput};
use crate::hooks::{use_chat_roster, use_dispatch};
use crate::i18n::t;
use crate::theme::{colors, rounded, text_size};
use crate::ui::border_all_color;

use super::common::{hint, section_heading, use_player_name};
use super::list::{FRIEND_ROW_HEIGHT_PX, person_row};

const DIALOG_WIDTH_PX: f32 = 520.;
const DIALOG_HEIGHT_PX: f32 = 540.;
const MEMBER_ROW_GAP_PX: f32 = 4.;

fn member_list(
    players: Vec<Uuid>,
    chosen: HashSet<Uuid>,
    mut picked: State<HashSet<Uuid>>,
    empty: &'static str,
) -> Element {
    if players.is_empty() {
        return rect()
            .width(Size::fill())
            .height(Size::flex(1.0))
            .child(hint(empty))
            .into_element();
    }

    ScrollArea::new()
        .width(Size::fill())
        .height(Size::flex(1.0))
        .lazy(
            players.len(),
            FRIEND_ROW_HEIGHT_PX,
            MEMBER_ROW_GAP_PX,
            move |index| {
                let player = players[index];
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
            },
        )
        .into_element()
}

#[derive(PartialEq)]
pub(super) struct NewGroupDialog {
    pub friends: Vec<Friend>,
    pub on_close: EventHandler<()>,
}

impl Component for NewGroupDialog {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let name = use_state(String::new);
        let picked = use_state(HashSet::<Uuid>::new);

        let chosen = picked.read().clone();

        let title = name.read().clone();
        let can_create = !title.trim().is_empty() && !chosen.is_empty();

        let close = self.on_close.clone();
        let creating = self.on_close.clone();

        let players: Vec<Uuid> = self.friends.iter().map(|friend| friend.player).collect();

        let members: Vec<Uuid> = chosen.iter().copied().collect();

        dialog(
            self.on_close.clone(),
            rect()
                .vertical()
                .width(Size::fill())
                .height(Size::fill())
                .content(Content::Flex)
                .spacing(14.)
                .child(dialog_title(t("oneclient.chat.new_group.title"), close))
                .child(
                    TextInput::new(name)
                        .width(Size::fill())
                        .placeholder(t("oneclient.chat.new_group.name_placeholder")),
                )
                .child(section_heading(t("oneclient.chat.new_group.members")))
                .child(member_list(
                    players,
                    chosen,
                    picked,
                    t("oneclient.chat.new_group.no_friends"),
                ))
                .child(
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .main_align(Alignment::End)
                        .child(
                            Button::new()
                                .primary()
                                .medium()
                                .text(t("oneclient.chat.new_group.create"))
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
pub(super) struct AddMembersDialog {
    pub group_id: i32,
    pub members: Vec<Uuid>,
    pub on_close: EventHandler<()>,
}

impl Component for AddMembersDialog {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let roster = use_chat_roster();
        let picked = use_state(HashSet::<Uuid>::new);

        let chosen = picked.read().clone();
        let group_id = self.group_id;
        let close = self.on_close.clone();
        let adding = self.on_close.clone();

        let players: Vec<Uuid> = roster
            .friends
            .iter()
            .map(|friend| friend.player)
            .filter(|player| !self.members.contains(player))
            .collect();

        let additions: Vec<Uuid> = chosen.iter().copied().collect();

        dialog(
            self.on_close.clone(),
            rect()
                .vertical()
                .width(Size::fill())
                .height(Size::fill())
                .content(Content::Flex)
                .spacing(14.)
                .child(dialog_title(t("oneclient.chat.groups.add_members"), close))
                .child(member_list(
                    players,
                    chosen,
                    picked,
                    t("oneclient.chat.groups.no_friends_to_add"),
                ))
                .child(
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .main_align(Alignment::End)
                        .child(
                            Button::new()
                                .primary()
                                .medium()
                                .text(t("oneclient.chat.groups.add"))
                                .enabled(!additions.is_empty())
                                .on_press(move |_| {
                                    dispatch.add_group_members(group_id, additions.clone());
                                    adding.call(());
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

        person_row(self.player, name, None, false)
            .background(colors::page_elevated())
            .cursor(CursorIcon::Pointer)
            .on_all_press(move |_| toggle.call(()))
            .child(
                rect()
                    .width(Size::px(18.))
                    .height(Size::px(18.))
                    .center()
                    .corner_radius(rounded(4.))
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
                .font_size(text_size(16.))
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
                        .max_height(Size::window_percent(92.))
                        .padding(Gaps::new_all(20.))
                        .corner_radius(rounded(14.))
                        .background(colors::card())
                        .border(border_all_color(1., colors::component_border()))
                        .child(body),
                ),
        )
        .into_element()
}
