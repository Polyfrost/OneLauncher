use freya::prelude::*;
use oneclient_polyplus::party::{MAX_PARTY_SIZE, Party};
use uuid::Uuid;

use super::{LobbyState, refresh, sorted_members, use_lobby, use_my_uuid};
use crate::components::{Avatar, Button, Icon, IconType};
use crate::i18n::{t, tf};
use crate::theme::colors;
use crate::ui::border_all_color;

const STACK_OVERLAP: f32 = 6.;

pub fn avatar_stack(party: &Party, me: Option<Uuid>, limit: usize, size: f32, ring: Color) -> Rect {
    sorted_members(party, me).iter().take(limit).enumerate().fold(
        rect().horizontal(),
        |stack, (index, member)| {
            stack.child(
                rect()
                    .corner_radius(CornerRadius::from(size / 4. + 2.))
                    .border(border_all_color(2., ring))
                    .padding(Gaps::new_all(2.))
                    .maybe(index > 0, |el| {
                        el.margin(Gaps::new(0., 0., 0., -STACK_OVERLAP))
                    })
                    .child(Avatar::new(member.player.to_string()).size(size)),
            )
        },
    )
}

fn set_open(mut state: State<LobbyState>, open: bool) {
    state.write().open = open;
    if open {
        spawn(refresh(state));
    }
}

#[derive(PartialEq)]
pub struct NavbarLobbyButton {
    pub foreground: Option<Color>,
}

impl Component for NavbarLobbyButton {
    fn render(&self) -> impl IntoElement {
        let state = use_lobby();
        let me = use_my_uuid();
        let snapshot = state.read().clone();
        let toggle = move |_| {
            let open = state.peek().open;
            set_open(state, !open);
        };

        let Some(party) = snapshot.party else {
            return super::super::navbar::navbar_button(self.foreground)
                .tooltip(t("oneclient.components.lobby.title"))
                .on_press(toggle)
                .child(Icon::new(IconType::Users01).size(20.))
                .into_element();
        };

        let ring = if snapshot.open {
            colors::brand()
        } else {
            colors::component_border_hover()
        };

        Button::new()
            .ghost()
            .foreground(self.foreground)
            .height(Size::px(36.))
            .padding(Gaps::new(0., 12., 0., 4.))
            .corner_radius(CornerRadius::new_all(8.))
            .border(border_all_color(1., ring))
            .tooltip(t("oneclient.components.lobby.title"))
            .on_press(toggle)
            .child(avatar_stack(&party, me, 3, 20., colors::page_elevated()))
            .child(
                label()
                    .text(format!("{}/{MAX_PARTY_SIZE}", party.members.len()))
                    .font_size(13.)
                    .font_weight(FontWeight::SEMI_BOLD),
            )
            .into_element()
    }
}

#[derive(PartialEq)]
pub struct LobbyHeroChip;

impl Component for LobbyHeroChip {
    fn render(&self) -> impl IntoElement {
        let state = use_lobby();
        let me = use_my_uuid();
        let mut hovered = use_state(|| false);

        let Some(party) = state.read().party.clone() else {
            return rect().into_element();
        };

        let text = match party.members.len().saturating_sub(1) {
            0 => t("oneclient.components.lobby.hero.alone").to_string(),
            1 => t("oneclient.components.lobby.hero.one_friend").to_string(),
            count => tf("oneclient.components.lobby.hero.friends", &[&count]),
        };

        rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(8.)
            .margin(Gaps::new(10., 0., 0., 0.))
            .padding(Gaps::new(3., 12., 3., 3.))
            .corner_radius(CornerRadius::new_all(8.))
            .background(colors::shade().with_a(if *hovered.read() { 170 } else { 110 }))
            .border(border_all_color(1., colors::component_border_hover()))
            .cursor(CursorIcon::Pointer)
            .a11y_role(AccessibilityRole::Button)
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_leave(move |_| hovered.set(false))
            .on_press(move |_| set_open(state, true))
            .child(avatar_stack(&party, me, 4, 20., Color::TRANSPARENT))
            .child(
                label()
                    .text(text)
                    .font_size(12.)
                    .font_weight(FontWeight::MEDIUM)
                    .color(colors::on_media()),
            )
            .into_element()
    }
}
