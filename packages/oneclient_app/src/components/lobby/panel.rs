use freya::animation::{AnimNum, Ease, OnCreation, use_animation};
use freya::prelude::*;

use super::dialogs::{Confirm, ConfirmDialog, InviteDialog, JoinDialog};
use super::{
    Lobby, LobbyState, ME, Member, MemberStatus, display_name, player_uuid, use_lobby, use_my_name,
    use_my_uuid,
};
use crate::components::cluster_menu::open_menu_at;
use crate::components::{
    Avatar, Button, ButtonVariant, ContextMenu, Icon, IconType, OverlayPopup, ScrollArea, Segment,
    SegmentedControl, toggle_controlled,
};
use crate::hooks::{settled_or_loading, use_active_cluster_id, use_clusters, use_dispatch};
use crate::i18n::{t, tf};
use crate::theme::{self, colors};
use crate::ui::{border_all_color, divider, relative_time};

const PANEL_WIDTH: f32 = 380.;
const PANEL_TOP: f32 = 72.;

#[derive(Clone, PartialEq)]
pub(super) enum Dialog {
    Invite,
    Join,
    Confirm(Confirm),
}

#[derive(PartialEq)]
pub struct LobbyPanel;

impl Component for LobbyPanel {
    fn render(&self) -> impl IntoElement {
        let mut state = use_lobby();
        let dialog = use_state(|| None::<Dialog>);

        if !state.read().open {
            return rect().into_element();
        }

        OverlayPopup::new()
            .backdrop(false)
            .position(Position::new_global().top(PANEL_TOP).right(40.))
            .on_close(move |()| state.write().open = false)
            .child(PanelCard { dialog })
            .child(DialogHost { dialog })
            .into_element()
    }
}

#[derive(PartialEq)]
struct PanelCard {
    dialog: State<Option<Dialog>>,
}

impl Component for PanelCard {
    fn render(&self) -> impl IntoElement {
        let intro = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            AnimNum::new(0., 1.).time(200).ease(Ease::Out)
        });
        let progress = intro.read().value();
        let lobby = use_lobby().read().lobby.clone();

        let body = match lobby {
            Some(lobby) => InLobby {
                lobby,
                dialog: self.dialog,
            }
            .into_element(),
            None => NoLobby {
                dialog: self.dialog,
            }
            .into_element(),
        };

        rect()
            .vertical()
            .width(Size::px(PANEL_WIDTH))
            .max_height(Size::window_percent(100.))
            .padding(Gaps::new_all(12.))
            .spacing(10.)
            .opacity(progress)
            .margin(Gaps::new((1.0 - progress) * -8.0, 0., 0., 0.))
            .background(colors::page_elevated().with_a(230))
            .backdrop_blur(12.)
            .corner_radius(CornerRadius::new_all(14.))
            .border(border_all_color(1., colors::component_border()))
            .shadow(Shadow::from((
                0.,
                8.,
                32.,
                0.,
                Color::from_argb(120, 0, 0, 0),
            )))
            .child(body)
    }
}

fn section_label(text: impl Into<String>) -> Label {
    label()
        .text(text.into())
        .font_size(11.)
        .letter_spacing(0.8)
        .font_weight(FontWeight::SEMI_BOLD)
        .color(colors::fg_secondary())
}

fn section_header(title: &'static str, count: Option<String>, trailing: Option<String>) -> Rect {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(6.)
        .padding(Gaps::new_symmetric(0., 4.))
        .child(section_label(title))
        .maybe_child(count.map(|count| {
            section_label(count)
                .color(colors::fg_secondary_hover())
                .into_element()
        }))
        .child(rect().width(Size::flex(1.0)))
        .maybe_child(trailing.map(|text| {
            label()
                .text(text)
                .font_size(12.)
                .color(colors::fg_secondary())
                .into_element()
        }))
}

fn header(title: String, subtitle: Option<String>, settings: Option<Element>) -> Rect {
    let mut state = use_lobby();

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(4.)
        .padding(Gaps::new(2., 0., 2., 4.))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(2.)
                .child(
                    label()
                        .text(title)
                        .font_size(16.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis)
                        .color(colors::fg_strong()),
                )
                .maybe_child(subtitle.map(|subtitle| {
                    label()
                        .text(subtitle)
                        .font_size(12.)
                        .color(colors::fg_secondary())
                        .into_element()
                })),
        )
        .maybe_child(settings)
        .child(
            Button::new()
                .ghost()
                .icon()
                .tooltip(t("oneclient.common.close"))
                .on_press(move |_| state.write().open = false)
                .child(Icon::new(IconType::X).size(18.)),
        )
}

#[derive(PartialEq)]
struct NoLobby {
    dialog: State<Option<Dialog>>,
}

impl Component for NoLobby {
    fn render(&self) -> impl IntoElement {
        let mut state = use_lobby();
        let dispatch = use_dispatch();
        let mut dialog = self.dialog;

        let create = move |_| {
            state.write().lobby = Some(Lobby::hosted());
            dispatch
                .notify(t("oneclient.components.lobby.toast.created"))
                .info()
                .icon(IconType::Users01)
                .toast_only()
                .send();
        };

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(10.)
            .child(header(
                t("oneclient.components.lobby.title").to_string(),
                None,
                None,
            ))
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .cross_align(Alignment::Center)
                    .spacing(10.)
                    .padding(Gaps::new(12., 16., 16., 16.))
                    .child(
                        rect()
                            .width(Size::px(52.))
                            .height(Size::px(52.))
                            .corner_radius(CornerRadius::new_all(14.))
                            .background(colors::component_bg())
                            .border(border_all_color(1., colors::component_border()))
                            .center()
                            .child(
                                Icon::new(IconType::Users01)
                                    .size(24.)
                                    .color(colors::fg_secondary()),
                            ),
                    )
                    .child(
                        label()
                            .text(t("oneclient.components.lobby.empty.title"))
                            .font_size(15.)
                            .font_weight(FontWeight::SEMI_BOLD)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text(t("oneclient.components.lobby.empty.body"))
                            .width(Size::fill())
                            .max_lines(3)
                            .text_align(TextAlign::Center)
                            .font_size(12.)
                            .color(colors::fg_secondary()),
                    )
                    .child(
                        rect()
                            .horizontal()
                            .spacing(8.)
                            .margin(Gaps::new(4., 0., 0., 0.))
                            .child(
                                Button::new()
                                    .primary()
                                    .on_press(create)
                                    .child(Icon::new(IconType::Plus).size(14.))
                                    .text(t("oneclient.components.lobby.empty.create")),
                            )
                            .child(
                                Button::new()
                                    .secondary()
                                    .on_press(move |_| dialog.set(Some(Dialog::Join)))
                                    .child(Icon::new(IconType::Key01).size(14.))
                                    .text(t("oneclient.components.lobby.empty.join")),
                            ),
                    ),
            )
    }
}

#[derive(PartialEq)]
struct InLobby {
    lobby: Lobby,
    dialog: State<Option<Dialog>>,
}

impl Component for InLobby {
    fn render(&self) -> impl IntoElement {
        let me = use_my_name();
        let mut show_settings = use_state(|| false);
        let lobby = &self.lobby;
        let mut dialog = self.dialog;

        let title = if lobby.is_leader() {
            t("oneclient.components.lobby.your_lobby").to_string()
        } else {
            tf(
                "oneclient.components.lobby.someones_lobby",
                &[&display_name(lobby.leader, &me)],
            )
        };
        let subtitle = tf(
            "oneclient.components.lobby.summary",
            &[
                &lobby.members.len(),
                &lobby.max_players,
                &lobby.ready_count(),
            ],
        );

        let settings_button = lobby.is_leader().then(|| {
            Button::new()
                .variant(if *show_settings.read() {
                    ButtonVariant::Secondary
                } else {
                    ButtonVariant::Ghost
                })
                .icon()
                .tooltip(t("oneclient.components.lobby.settings.title"))
                .on_press(move |_| show_settings.toggle())
                .child(Icon::new(IconType::Settings02).size(18.))
                .into_element()
        });

        let can_invite = lobby.can_invite() && !lobby.is_full();

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(10.)
            .child(header(title, Some(subtitle), settings_button))
            .child(
                ScrollArea::new()
                    .width(Size::fill())
                    .max_height(Size::window_percent(58.))
                    .spacing(14.)
                    .children((*show_settings.read() && lobby.is_leader()).then(|| {
                        LobbySettings {
                            lobby: lobby.clone(),
                        }
                        .into_element()
                    }))
                    .child(InstanceRow)
                    .child(
                        rect()
                            .vertical()
                            .width(Size::fill())
                            .spacing(4.)
                            .child(section_header(
                                t("oneclient.components.lobby.players"),
                                Some(format!("{}/{}", lobby.members.len(), lobby.max_players)),
                                None,
                            ))
                            .children(lobby.sorted_members().into_iter().map(|member| {
                                MemberRow {
                                    member,
                                    leader: lobby.leader,
                                    viewer_leads: lobby.is_leader(),
                                    dialog,
                                }
                                .into_element()
                            }))
                            .maybe_child(can_invite.then(|| {
                                InviteSlot {
                                    on_press: (move |()| dialog.set(Some(Dialog::Invite))).into(),
                                }
                                .into_element()
                            })),
                    )
                    .children((!lobby.invites.is_empty()).then(|| pending(lobby, &me)))
                    .child(invite_code(lobby.code)),
            )
            .child(divider())
            .child(footer(lobby, &me, dialog))
    }
}

#[derive(PartialEq)]
struct LobbySettings {
    lobby: Lobby,
}

impl Component for LobbySettings {
    fn render(&self) -> impl IntoElement {
        let mut state = use_lobby();
        let max = use_state(|| self.lobby.max_players);
        let min_allowed = self.lobby.smallest_allowed_max();

        use_side_effect_with_deps(&*max.read(), move |max| {
            if let Some(lobby) = state.write().lobby.as_mut() {
                lobby.max_players = *max;
            }
        });

        let members_can_invite = self.lobby.members_can_invite;
        let public = self.lobby.public;

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(10.)
            .padding(Gaps::new_all(12.))
            .corner_radius(CornerRadius::new_all(10.))
            .background(colors::card())
            .border(border_all_color(1., colors::component_border()))
            .child(section_label(t(
                "oneclient.components.lobby.settings.title",
            )))
            .child(setting_row(
                t("oneclient.components.lobby.settings.members_invite"),
                toggle_controlled(
                    members_can_invite,
                    (move |()| {
                        if let Some(lobby) = state.write().lobby.as_mut() {
                            lobby.members_can_invite = !lobby.members_can_invite;
                        }
                    })
                    .into(),
                )
                .into_element(),
            ))
            .child(setting_row(
                t("oneclient.components.lobby.settings.public"),
                toggle_controlled(
                    public,
                    (move |()| {
                        if let Some(lobby) = state.write().lobby.as_mut() {
                            lobby.public = !lobby.public;
                        }
                    })
                    .into(),
                )
                .into_element(),
            ))
            .child(setting_row(
                t("oneclient.components.lobby.settings.max_players"),
                SegmentedControl::new(max)
                    .height(28.)
                    .segments(super::MAX_PLAYER_CHOICES.into_iter().map(|choice| {
                        Segment::new(choice)
                            .label(choice.to_string())
                            .disabled(choice < min_allowed)
                    }))
                    .into_element(),
            ))
    }
}

fn setting_row(text: &'static str, control: Element) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .child(
            label()
                .text(text)
                .width(Size::flex(1.0))
                .font_size(13.)
                .color(colors::fg_primary()),
        )
        .child(control)
        .into_element()
}

#[derive(PartialEq)]
struct InstanceRow;

impl Component for InstanceRow {
    fn render(&self) -> impl IntoElement {
        let clusters = settled_or_loading(&use_clusters()).unwrap_or_default();
        let active = *use_active_cluster_id().read();
        let lobby = use_lobby().read().lobby.clone();
        let me = use_my_name();

        let name = active
            .and_then(|id| clusters.iter().find(|cluster| cluster.id == id))
            .or_else(|| clusters.first())
            .map_or_else(
                || t("oneclient.components.lobby.instance.none").to_string(),
                |cluster| {
                    if cluster.user_created {
                        cluster.name.clone()
                    } else {
                        format!("{} {}", cluster.mc_version, cluster.mc_loader)
                    }
                },
            );

        let hint = match lobby {
            Some(lobby) if !lobby.is_leader() => tf(
                "oneclient.components.lobby.instance.picked_by",
                &[&display_name(lobby.leader, &me)],
            ),
            _ => t("oneclient.components.lobby.instance.synced").to_string(),
        };

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(4.)
            .child(section_header(
                t("oneclient.components.lobby.instance.title"),
                None,
                None,
            ))
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .spacing(10.)
                    .padding(Gaps::new_all(10.))
                    .corner_radius(CornerRadius::new_all(10.))
                    .background(colors::card())
                    .border(border_all_color(1., colors::component_border()))
                    .child(
                        rect()
                            .width(Size::px(36.))
                            .height(Size::px(36.))
                            .corner_radius(CornerRadius::new_all(9.))
                            .background(colors::brand().with_a(40))
                            .center()
                            .child(
                                Icon::new(IconType::Rocket02)
                                    .size(18.)
                                    .color(colors::brand()),
                            ),
                    )
                    .child(
                        rect()
                            .vertical()
                            .width(Size::flex(1.0))
                            .spacing(2.)
                            .child(
                                label()
                                    .text(name)
                                    .font_size(13.)
                                    .font_weight(FontWeight::MEDIUM)
                                    .max_lines(1)
                                    .text_overflow(TextOverflow::Ellipsis)
                                    .color(colors::fg_primary()),
                            )
                            .child(
                                label()
                                    .text(hint)
                                    .font_size(11.)
                                    .max_lines(1)
                                    .text_overflow(TextOverflow::Ellipsis)
                                    .color(colors::fg_secondary()),
                            ),
                    ),
            )
    }
}

fn status_text(status: MemberStatus) -> (&'static str, Color) {
    match status {
        MemberStatus::Ready => (
            t("oneclient.components.lobby.status.ready"),
            colors::success(),
        ),
        MemberStatus::Syncing(_) => (
            t("oneclient.components.lobby.status.syncing"),
            colors::code_warn(),
        ),
        MemberStatus::NotReady => (
            t("oneclient.components.lobby.status.not_ready"),
            colors::fg_secondary(),
        ),
    }
}

fn tag(text: &'static str, color: Color) -> Element {
    rect()
        .padding(Gaps::new_symmetric(1., 6.))
        .corner_radius(CornerRadius::new_all(5.))
        .background(color.with_a(36))
        .child(
            label()
                .text(text)
                .font_size(10.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(color),
        )
        .into_element()
}

fn progress_bar(percent: u8) -> Element {
    rect()
        .width(Size::px(64.))
        .height(Size::px(4.))
        .corner_radius(CornerRadius::new_all(2.))
        .background(colors::component_bg_pressed())
        .child(
            rect()
                .width(Size::percent(f32::from(percent.min(100))))
                .height(Size::fill())
                .corner_radius(CornerRadius::new_all(2.))
                .background(colors::code_warn()),
        )
        .into_element()
}

#[derive(PartialEq)]
struct MemberRow {
    member: Member,
    leader: &'static str,
    viewer_leads: bool,
    dialog: State<Option<Dialog>>,
}

impl Component for MemberRow {
    fn render(&self) -> impl IntoElement {
        let mut state = use_lobby();
        let me = use_my_name();
        let me_uuid = use_my_uuid();
        let dispatch = use_dispatch();
        let menu = use_state(|| None::<(f32, f32)>);
        let mut dialog = self.dialog;

        let id = self.member.id;
        let is_me = id == ME;
        let name = display_name(id, &me);
        let (status_label, status_color) = status_text(self.member.status);
        let manageable = self.viewer_leads && !is_me;

        let context = (*menu.read()).map(|(x, y)| {
            let mut menu = menu;
            let promote_name = name.clone();
            let kick_name = name.clone();
            let profile_name = name.clone();
            let dispatch = dispatch.clone();
            ContextMenu::new(x, y)
                .title(name.clone())
                .on_close(move |()| menu.set(None))
                .action(
                    IconType::Key01,
                    t("oneclient.components.lobby.member.make_leader"),
                    move |()| {
                        menu.set(None);
                        dialog.set(Some(Dialog::Confirm(Confirm::promote(
                            id,
                            promote_name.clone(),
                        ))));
                    },
                )
                .action(
                    IconType::Users01,
                    t("oneclient.components.lobby.member.profile"),
                    move |()| {
                        menu.set(None);
                        dispatch
                            .notify(tf(
                                "oneclient.components.lobby.toast.profile",
                                &[&profile_name],
                            ))
                            .info()
                            .toast_only()
                            .send();
                    },
                )
                .separator()
                .danger_action(
                    IconType::LogOut01,
                    t("oneclient.components.lobby.member.kick"),
                    move |()| {
                        menu.set(None);
                        dialog.set(Some(Dialog::Confirm(Confirm::kick(id, kick_name.clone()))));
                    },
                )
                .into_element()
        });

        let trailing: Option<Element> = if is_me {
            Some(
                rect()
                    .horizontal()
                    .cross_align(Alignment::Center)
                    .spacing(8.)
                    .child(
                        label()
                            .text(t("oneclient.components.lobby.member.ready_toggle"))
                            .font_size(12.)
                            .color(colors::fg_secondary()),
                    )
                    .child(toggle_controlled(
                        self.member.status == MemberStatus::Ready,
                        (move |()| {
                            if let Some(lobby) = state.write().lobby.as_mut() {
                                let ready = lobby.my_status() != MemberStatus::Ready;
                                lobby.set_ready(ready);
                            }
                        })
                        .into(),
                    ))
                    .into_element(),
            )
        } else if manageable {
            Some(
                Button::new()
                    .ghost()
                    .icon()
                    .tooltip(t("oneclient.components.lobby.member.more"))
                    .on_press(open_menu_at(menu))
                    .child(Icon::new(IconType::DotsVertical).size(18.))
                    .into_element(),
            )
        } else {
            None
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(10.)
            .padding(Gaps::new_all(8.))
            .corner_radius(CornerRadius::new_all(10.))
            .background(colors::card())
            .border(border_all_color(1., colors::component_border()))
            .maybe(manageable, |el| el.on_secondary_down(open_menu_at(menu)))
            .child(
                rect()
                    .child(Avatar::new(player_uuid(id, &me_uuid)).size(32.))
                    .child(
                        rect()
                            .position(Position::new_absolute().bottom(-3.).right(-3.))
                            .width(Size::px(12.))
                            .height(Size::px(12.))
                            .corner_radius(CornerRadius::from(6.))
                            .background(status_color)
                            .border(border_all_color(2., colors::card()))
                            .layer(Layer::Relative(2)),
                    ),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(3.)
                    .child(
                        rect()
                            .horizontal()
                            .cross_align(Alignment::Center)
                            .spacing(6.)
                            .child(
                                label()
                                    .text(name.clone())
                                    .font_size(13.)
                                    .font_weight(FontWeight::MEDIUM)
                                    .max_lines(1)
                                    .text_overflow(TextOverflow::Ellipsis)
                                    .color(colors::fg_primary()),
                            )
                            .maybe_child((id == self.leader).then(|| {
                                tag(
                                    t("oneclient.components.lobby.member.leader"),
                                    colors::brand(),
                                )
                            }))
                            .maybe_child(is_me.then(|| {
                                tag(
                                    t("oneclient.components.lobby.member.you"),
                                    colors::fg_secondary(),
                                )
                            })),
                    )
                    .child(
                        rect()
                            .horizontal()
                            .cross_align(Alignment::Center)
                            .spacing(6.)
                            .child(
                                label()
                                    .text(status_label)
                                    .font_size(11.)
                                    .font_weight(FontWeight::MEDIUM)
                                    .color(status_color),
                            )
                            .maybe_child(match self.member.status {
                                MemberStatus::Syncing(percent) => Some(
                                    rect()
                                        .horizontal()
                                        .cross_align(Alignment::Center)
                                        .spacing(6.)
                                        .child(progress_bar(percent))
                                        .child(
                                            label()
                                                .text(format!("{percent}%"))
                                                .font_size(11.)
                                                .color(colors::fg_secondary()),
                                        )
                                        .into_element(),
                                ),
                                _ => None,
                            }),
                    ),
            )
            .maybe_child(trailing)
            .maybe_child(context)
    }
}

#[derive(PartialEq)]
struct InviteSlot {
    on_press: EventHandler<()>,
}

impl Component for InviteSlot {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let on_press = self.on_press.clone();
        let color = if *hovered.read() {
            colors::fg_primary()
        } else {
            colors::fg_secondary()
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(48.))
            .center()
            .spacing(8.)
            .corner_radius(CornerRadius::new_all(10.))
            .background(if *hovered.read() {
                colors::ghost_overlay_hover()
            } else {
                colors::ghost_overlay()
            })
            .border(border_all_color(1., colors::component_border()))
            .cursor(CursorIcon::Pointer)
            .a11y_role(AccessibilityRole::Button)
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_leave(move |_| hovered.set(false))
            .on_press(move |_| on_press.call(()))
            .child(Icon::new(IconType::Plus).size(16.).color(color))
            .child(
                label()
                    .text(t("oneclient.components.lobby.invite_slot"))
                    .font_size(13.)
                    .font_weight(FontWeight::MEDIUM)
                    .color(color),
            )
    }
}

fn pending(lobby: &Lobby, me: &str) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(4.)
        .child(section_header(
            t("oneclient.components.lobby.pending"),
            Some(lobby.invites.len().to_string()),
            None,
        ))
        .children(lobby.invites.iter().map(|invite| {
            PendingRow {
                id: invite.id,
                name: display_name(invite.id, me),
                by: if invite.by == ME {
                    t("oneclient.components.lobby.invited_by_you").to_string()
                } else {
                    tf(
                        "oneclient.components.lobby.invited_by",
                        &[&display_name(invite.by, me)],
                    )
                },
                when: relative_time(invite.sent),
                cancellable: lobby.is_leader() || invite.by == ME,
            }
            .into_element()
        }))
        .into_element()
}

#[derive(PartialEq)]
struct PendingRow {
    id: &'static str,
    name: String,
    by: String,
    when: String,
    cancellable: bool,
}

impl Component for PendingRow {
    fn render(&self) -> impl IntoElement {
        let mut state = use_lobby();
        let dispatch = use_dispatch();
        let me_uuid = use_my_uuid();
        let id = self.id;
        let name = self.name.clone();

        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(10.)
            .padding(Gaps::new_symmetric(6., 8.))
            .child(
                rect()
                    .opacity(0.6)
                    .child(Avatar::new(player_uuid(id, &me_uuid)).size(32.)),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(2.)
                    .child(
                        label()
                            .text(self.name.clone())
                            .font_size(13.)
                            .font_weight(FontWeight::MEDIUM)
                            .max_lines(1)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text(format!("{} · {}", self.by, self.when))
                            .font_size(11.)
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .color(colors::fg_secondary()),
                    ),
            )
            .maybe_child(self.cancellable.then(|| {
                Button::new()
                    .ghost()
                    .small()
                    .on_press(move |_| {
                        if let Some(lobby) = state.write().lobby.as_mut() {
                            lobby.cancel_invite(id);
                        }
                        dispatch
                            .notify(tf(
                                "oneclient.components.lobby.toast.invite_cancelled",
                                &[&name],
                            ))
                            .info()
                            .toast_only()
                            .send();
                    })
                    .text(t("oneclient.common.cancel"))
                    .into_element()
            }))
    }
}

fn invite_code(code: &'static str) -> Element {
    InviteCode { code }.into_element()
}

#[derive(PartialEq)]
struct InviteCode {
    code: &'static str,
}

impl Component for InviteCode {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let code = self.code;

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(4.)
            .child(section_header(
                t("oneclient.components.lobby.code.title"),
                None,
                None,
            ))
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .spacing(8.)
                    .child(
                        rect()
                            .width(Size::flex(1.0))
                            .height(Size::px(32.))
                            .padding(Gaps::new_symmetric(0., 12.))
                            .main_align(Alignment::Center)
                            .corner_radius(CornerRadius::new_all(8.))
                            .background(colors::component_bg())
                            .border(border_all_color(1., colors::component_border()))
                            .child(
                                label()
                                    .text(code)
                                    .font_family(theme::MONO_FONT)
                                    .font_size(13.)
                                    .letter_spacing(1.)
                                    .color(colors::fg_primary()),
                            ),
                    )
                    .child(
                        Button::new()
                            .secondary()
                            .icon()
                            .tooltip(t("oneclient.common.copy"))
                            .on_press(move |_| {
                                if freya::text_edit::Clipboard::set(code.to_string()).is_ok() {
                                    dispatch
                                        .notify(t("oneclient.components.lobby.toast.code_copied"))
                                        .info()
                                        .icon(IconType::ClipboardCheck)
                                        .toast_only()
                                        .send();
                                } else {
                                    dispatch
                                        .notify(t("oneclient.components.button.copy_failed.title"))
                                        .body(t("oneclient.components.button.copy_failed.body"))
                                        .error()
                                        .toast_only()
                                        .send();
                                }
                            })
                            .child(Icon::new(IconType::Copy01).size(14.)),
                    ),
            )
            .into_element()
    }
}

fn footer(lobby: &Lobby, me: &str, dialog: State<Option<Dialog>>) -> Element {
    let mut dialog = dialog;
    let invite = if lobby.can_invite() {
        Button::new()
            .primary()
            .width(Size::fill())
            .enabled(!lobby.is_full())
            .tooltip(if lobby.is_full() {
                t("oneclient.components.lobby.full")
            } else {
                t("oneclient.components.lobby.invite_players")
            })
            .on_press(move |_| dialog.set(Some(Dialog::Invite)))
            .child(Icon::new(IconType::Plus).size(16.))
            .text(t("oneclient.components.lobby.invite_players"))
            .into_element()
    } else {
        label()
            .text(tf(
                "oneclient.components.lobby.only_leader_invites",
                &[&display_name(lobby.leader, me)],
            ))
            .width(Size::fill())
            .text_align(TextAlign::Center)
            .font_size(12.)
            .color(colors::fg_secondary())
            .into_element()
    };

    let leave = Confirm::leave(lobby, me);
    let disband = Confirm::disband(lobby.members.len());

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(8.)
        .child(invite)
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::Flex)
                .spacing(8.)
                .child(
                    Button::new()
                        .secondary()
                        .width(Size::flex(1.0))
                        .on_press(move |_| dialog.set(Some(Dialog::Confirm(leave.clone()))))
                        .child(Icon::new(IconType::LogOut01).size(14.))
                        .text(t("oneclient.components.lobby.leave")),
                )
                .maybe_child(lobby.is_leader().then(|| {
                    Button::new()
                        .secondary()
                        .width(Size::flex(1.0))
                        .foreground(colors::danger())
                        .on_press(move |_| dialog.set(Some(Dialog::Confirm(disband.clone()))))
                        .child(
                            Icon::new(IconType::Trash01)
                                .size(14.)
                                .color(colors::danger()),
                        )
                        .text(t("oneclient.components.lobby.disband"))
                        .into_element()
                })),
        )
        .into_element()
}

#[derive(PartialEq)]
struct DialogHost {
    dialog: State<Option<Dialog>>,
}

impl Component for DialogHost {
    fn render(&self) -> impl IntoElement {
        let mut dialog = self.dialog;
        let close = move |()| dialog.set(None);

        match dialog.read().clone() {
            Some(Dialog::Invite) => InviteDialog {
                on_close: close.into(),
            }
            .into_element(),
            Some(Dialog::Join) => JoinDialog {
                on_close: close.into(),
            }
            .into_element(),
            Some(Dialog::Confirm(confirm)) => ConfirmDialog {
                confirm,
                on_close: close.into(),
            }
            .into_element(),
            None => rect().into_element(),
        }
    }
}

pub(super) fn apply_confirm(state: &mut State<LobbyState>, confirm: &Confirm) {
    let mut guard = state.write();
    match confirm.kind {
        super::dialogs::ConfirmKind::Kick(id) => {
            if let Some(lobby) = guard.lobby.as_mut() {
                lobby.kick(id);
            }
        }
        super::dialogs::ConfirmKind::Promote(id) => {
            if let Some(lobby) = guard.lobby.as_mut() {
                lobby.promote(id);
            }
        }
        super::dialogs::ConfirmKind::Leave | super::dialogs::ConfirmKind::Disband => {
            guard.lobby = None;
        }
    }
}
