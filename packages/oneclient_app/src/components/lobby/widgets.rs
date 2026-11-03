use freya::prelude::*;

use super::{Lobby, display_name, player_uuid, use_lobby, use_my_name, use_my_uuid};
use crate::components::{Avatar, Button, Icon, IconType};
use crate::hooks::use_dispatch;
use crate::i18n::{t, tf};
use crate::theme::colors;
use crate::ui::border_all_color;

const STACK_OVERLAP: f32 = 6.;

pub fn avatar_stack(lobby: &Lobby, me_uuid: &str, limit: usize, size: f32, ring: Color) -> Rect {
    lobby.sorted_members().iter().take(limit).enumerate().fold(
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
                    .child(Avatar::new(player_uuid(member.id, me_uuid)).size(size)),
            )
        },
    )
}

#[derive(PartialEq)]
pub struct NavbarLobbyButton {
    pub foreground: Option<Color>,
}

impl Component for NavbarLobbyButton {
    fn render(&self) -> impl IntoElement {
        let mut state = use_lobby();
        let me_uuid = use_my_uuid();
        let snapshot = state.read().clone();
        let toggle = move |_| {
            let open = state.peek().open;
            state.write().open = !open;
        };

        let Some(lobby) = snapshot.lobby else {
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
            .child(avatar_stack(
                &lobby,
                &me_uuid,
                3,
                20.,
                colors::page_elevated(),
            ))
            .child(
                label()
                    .text(format!("{}/{}", lobby.members.len(), lobby.max_players))
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
        let mut state = use_lobby();
        let me_uuid = use_my_uuid();
        let mut hovered = use_state(|| false);

        let Some(lobby) = state.read().lobby.clone() else {
            return rect().into_element();
        };

        let friends = lobby.friends_count();
        let text = match friends {
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
            .on_press(move |_| state.write().open = true)
            .child(avatar_stack(&lobby, &me_uuid, 4, 20., Color::TRANSPARENT))
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

#[derive(PartialEq)]
pub struct LobbyLaunchButton {
    pub cluster_id: i64,
    pub can_launch: bool,
}

impl Component for LobbyLaunchButton {
    fn render(&self) -> impl IntoElement {
        let lobby = use_lobby().read().lobby.clone();
        let me = use_my_name();
        let dispatch = use_dispatch();
        let cluster_id = self.cluster_id;

        let Some(lobby) = lobby else {
            return rect().into_element();
        };

        let button = Button::new()
            .medium()
            .font_size(16.)
            .font_weight(FontWeight::from(450))
            .padding(Gaps::new_symmetric(8., 24.));

        if !lobby.is_leader() {
            return button
                .secondary()
                .enabled(false)
                .text(tf(
                    "oneclient.components.lobby.launch.waiting_leader",
                    &[&display_name(lobby.leader, &me)],
                ))
                .into_element();
        }

        let ready = lobby.all_ready();
        let text = if ready {
            t("oneclient.components.lobby.launch.together").to_string()
        } else {
            tf(
                "oneclient.components.lobby.launch.waiting_ready",
                &[&lobby.ready_count(), &lobby.members.len()],
            )
        };

        button
            .primary()
            .enabled(ready && self.can_launch)
            .on_press(move |_| {
                dispatch
                    .notify(t("oneclient.components.lobby.toast.launching"))
                    .info()
                    .icon(IconType::Rocket02)
                    .toast_only()
                    .send();
                dispatch.launch_cluster(cluster_id);
            })
            .text(text)
            .into_element()
    }
}
