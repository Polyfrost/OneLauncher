use std::collections::HashSet;

use freya::prelude::*;
use uuid::Uuid;

use oneclient_polyplus::Friend;

use crate::components::{Avatar, Button, Icon, IconType, OverlayPopup, ScrollArea, TextInput};
use crate::hooks::use_dispatch;
use crate::theme::colors;
use crate::ui::border_all_color;

use super::common::{hint, section_heading, use_player_name};

const CARD_BG: Color = Color::from_rgb(26, 34, 41);
const DIALOG_WIDTH_PX: f32 = 520.;
const DIALOG_HEIGHT_PX: f32 = 540.;
const LIST_RADIUS_PX: f32 = 8.;

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
        .child(Avatar::new(player.to_string()))
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
    pub friends: Vec<Friend>,
    pub on_close: EventHandler<()>,
}

impl Component for NewGroupDialog {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let name = use_state(String::new);
        let mut picked = use_state(HashSet::<Uuid>::new);

        let friends = &self.friends;
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
