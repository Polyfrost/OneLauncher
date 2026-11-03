mod dialogs;
mod panel;
mod widgets;

use std::time::Instant;

use freya::prelude::*;

use crate::hooks::{try_default_account, use_current_account};
use crate::i18n::t;

pub use panel::LobbyPanel;
pub use widgets::{LobbyHeroChip, LobbyLaunchButton, NavbarLobbyButton};

pub const ME: &str = "me";
const MAX_PLAYER_CHOICES: [u8; 3] = [4, 8, 12];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MemberStatus {
    Ready,
    Syncing(u8),
    NotReady,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Presence {
    Online,
    Playing(&'static str),
    Offline { hours_ago: u32 },
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Friend {
    pub id: &'static str,
    pub name: &'static str,
    pub uuid: &'static str,
    pub presence: Presence,
}

// shortcut: mock roster until the social backend exists, swap for the real friends query then
pub const FRIENDS: [Friend; 8] = [
    Friend {
        id: "mossy",
        name: "mossyblock",
        uuid: "4b8c6a1e-2f3d-4e5a-9b7c-1d2e3f4a5b61",
        presence: Presence::Online,
    },
    Friend {
        id: "fern",
        name: "Fernweh",
        uuid: "7d2e9f30-5a6b-4c7d-8e9f-0a1b2c3d4e72",
        presence: Presence::Online,
    },
    Friend {
        id: "lunar",
        name: "lunar_ox",
        uuid: "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c83",
        presence: Presence::Online,
    },
    Friend {
        id: "pine",
        name: "pixelpine",
        uuid: "c3d4e5f6-a7b8-4c9d-8e0f-1a2b3c4d5e94",
        presence: Presence::Online,
    },
    Friend {
        id: "rook",
        name: "Rookery",
        uuid: "e5f6a7b8-c9d0-4e1f-9a2b-3c4d5e6f7a05",
        presence: Presence::Playing("Hypixel"),
    },
    Friend {
        id: "tide",
        name: "tidepool",
        uuid: "f6a7b8c9-d0e1-4f2a-8b3c-4d5e6f7a8b16",
        presence: Presence::Online,
    },
    Friend {
        id: "ash",
        name: "ashfall22",
        uuid: "0a1b2c3d-4e5f-4a6b-9c7d-8e9f0a1b2c27",
        presence: Presence::Offline { hours_ago: 3 },
    },
    Friend {
        id: "wren",
        name: "wren",
        uuid: "1b2c3d4e-5f6a-4b7c-8d9e-0f1a2b3c4d38",
        presence: Presence::Offline { hours_ago: 50 },
    },
];

pub fn friend(id: &str) -> Option<&'static Friend> {
    FRIENDS.iter().find(|friend| friend.id == id)
}

#[derive(Clone, PartialEq, Debug)]
pub struct Member {
    pub id: &'static str,
    pub status: MemberStatus,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Invite {
    pub id: &'static str,
    pub by: &'static str,
    pub sent: Instant,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Lobby {
    pub leader: &'static str,
    pub members: Vec<Member>,
    pub invites: Vec<Invite>,
    pub code: &'static str,
    pub members_can_invite: bool,
    pub public: bool,
    pub max_players: u8,
}

impl Lobby {
    pub fn hosted() -> Self {
        Self {
            leader: ME,
            members: vec![Member {
                id: ME,
                status: MemberStatus::Ready,
            }],
            invites: Vec::new(),
            code: "OC-7F3K-92Q",
            members_can_invite: true,
            public: false,
            max_players: 8,
        }
    }

    pub fn joined() -> Self {
        Self {
            leader: "mossy",
            members: vec![
                Member {
                    id: "mossy",
                    status: MemberStatus::Ready,
                },
                Member {
                    id: ME,
                    status: MemberStatus::NotReady,
                },
                Member {
                    id: "fern",
                    status: MemberStatus::Syncing(64),
                },
                Member {
                    id: "lunar",
                    status: MemberStatus::NotReady,
                },
            ],
            invites: vec![Invite {
                id: "pine",
                by: "mossy",
                sent: Instant::now(),
            }],
            code: "OC-M0SS-4KQ",
            members_can_invite: true,
            public: false,
            max_players: 8,
        }
    }

    pub fn is_leader(&self) -> bool {
        self.leader == ME
    }

    pub fn seats_taken(&self) -> usize {
        self.members.len() + self.invites.len()
    }

    pub fn is_full(&self) -> bool {
        self.seats_taken() >= usize::from(self.max_players)
    }

    pub fn can_invite(&self) -> bool {
        self.is_leader() || self.members_can_invite
    }

    pub fn ready_count(&self) -> usize {
        self.members
            .iter()
            .filter(|member| member.status == MemberStatus::Ready)
            .count()
    }

    pub fn all_ready(&self) -> bool {
        self.ready_count() == self.members.len()
    }

    pub fn friends_count(&self) -> usize {
        self.members.len().saturating_sub(1)
    }

    pub fn my_status(&self) -> MemberStatus {
        self.members
            .iter()
            .find(|member| member.id == ME)
            .map_or(MemberStatus::NotReady, |member| member.status)
    }

    pub fn sorted_members(&self) -> Vec<Member> {
        let mut members = self.members.clone();
        members.sort_by_key(|member| (member.id != self.leader, member.id != ME));
        members
    }

    pub fn has(&self, id: &str) -> bool {
        self.members.iter().any(|member| member.id == id)
    }

    pub fn is_invited(&self, id: &str) -> bool {
        self.invites.iter().any(|invite| invite.id == id)
    }

    pub fn successor(&self) -> Option<&'static str> {
        self.members
            .iter()
            .find(|member| member.id != ME)
            .map(|member| member.id)
    }

    pub fn smallest_allowed_max(&self) -> u8 {
        MAX_PLAYER_CHOICES
            .into_iter()
            .find(|&max| usize::from(max) >= self.seats_taken())
            .unwrap_or(MAX_PLAYER_CHOICES[MAX_PLAYER_CHOICES.len() - 1])
    }

    pub fn set_ready(&mut self, ready: bool) {
        if let Some(member) = self.members.iter_mut().find(|member| member.id == ME) {
            member.status = if ready {
                MemberStatus::Ready
            } else {
                MemberStatus::NotReady
            };
        }
    }

    pub fn kick(&mut self, id: &str) {
        self.members.retain(|member| member.id != id);
    }

    pub fn promote(&mut self, id: &'static str) {
        if self.has(id) {
            self.leader = id;
        }
    }

    pub fn invite(&mut self, id: &'static str) {
        if !self.is_full() && !self.has(id) && !self.is_invited(id) {
            self.invites.push(Invite {
                id,
                by: ME,
                sent: Instant::now(),
            });
        }
    }

    pub fn cancel_invite(&mut self, id: &str) {
        self.invites.retain(|invite| invite.id != id);
    }
}

#[derive(Clone, PartialEq, Default, Debug)]
pub struct LobbyState {
    pub open: bool,
    pub lobby: Option<Lobby>,
}

#[derive(Clone, Copy)]
struct LobbyContext(State<LobbyState>);

pub fn use_provide_lobby() {
    let state = use_state(LobbyState::default);
    use_hook(move || provide_root_context(LobbyContext(state)));
}

pub fn use_lobby() -> State<LobbyState> {
    consume_root_context::<LobbyContext>().0
}

pub fn use_my_name() -> String {
    try_default_account(&use_current_account())
        .map(|account| account.username)
        .unwrap_or_else(|| t("oneclient.components.lobby.you").to_string())
}

pub fn use_my_uuid() -> String {
    try_default_account(&use_current_account())
        .map(|account| account.id.to_string())
        .unwrap_or_else(|| uuid::Uuid::nil().to_string())
}

pub fn display_name(id: &str, me: &str) -> String {
    if id == ME {
        me.to_string()
    } else {
        friend(id).map_or_else(|| id.to_string(), |friend| friend.name.to_string())
    }
}

pub fn player_uuid(id: &str, me_uuid: &str) -> String {
    if id == ME {
        me_uuid.to_string()
    } else {
        friend(id).map_or_else(
            || uuid::Uuid::nil().to_string(),
            |friend| friend.uuid.to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invites_respect_capacity_and_duplicates() {
        let mut lobby = Lobby::hosted();
        lobby.max_players = 2;
        lobby.invite("pine");
        lobby.invite("pine");
        lobby.invite("tide");
        assert_eq!(lobby.invites.len(), 1);
        assert!(lobby.is_full());
        assert_eq!(lobby.smallest_allowed_max(), 4);
    }

    #[test]
    fn leader_sorts_first_then_me() {
        let lobby = Lobby::joined();
        let order: Vec<_> = lobby.sorted_members().iter().map(|m| m.id).collect();
        assert_eq!(order, ["mossy", ME, "fern", "lunar"]);
        assert!(!lobby.is_leader());
        assert_eq!(lobby.ready_count(), 1);
    }

    #[test]
    fn promote_only_existing_members() {
        let mut lobby = Lobby::joined();
        lobby.promote("pine");
        assert_eq!(lobby.leader, "mossy");
        lobby.promote(ME);
        assert!(lobby.is_leader());
    }
}
