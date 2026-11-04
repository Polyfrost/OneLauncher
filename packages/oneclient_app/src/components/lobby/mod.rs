mod dialogs;
mod panel;
mod widgets;

use std::future::Future;
use std::time::Duration;

use freya::prelude::*;
use freya::query::{Query, QueryCapability, UseQuery, use_query};
use futures_util::future::join_all;
use oneclient_auth::MinecraftAccount;
use oneclient_polyplus::PlusError;
use oneclient_polyplus::party::{
    self, InvitePolicy, InviteStatus, LeftReason, LiveEvent, MAX_PARTY_SIZE, Party, PartyInvite,
    PartyMember, PartyPacket,
};
use tokio::sync::broadcast::error::RecvError;
use uuid::Uuid;

use crate::components::IconType;
use crate::hooks::{
    Actions, settled_or_loading, try_default_account, use_current_account, use_dispatch,
    use_player_profile, wardrobe::plus_message,
};
use crate::i18n::{t, tf};

pub use panel::LobbyPanel;
pub use widgets::{LobbyHeroChip, NavbarLobbyButton};

const FRIENDS_STALE: Duration = Duration::from_secs(30);

#[derive(Clone, PartialEq, Default, Debug)]
pub struct LobbyState {
    pub open: bool,
    pub party: Option<Party>,
    pub incoming: Vec<PartyInvite>,
    pub outgoing: Vec<PartyInvite>,
    pub error: Option<String>,
    pub busy: bool,
    pub me: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Notice {
    Kicked,
    Disbanded,
    PartyLost,
    Invited(Uuid),
}

impl LobbyState {
    pub fn party_outgoing(&self) -> Vec<PartyInvite> {
        let Some(party) = &self.party else {
            return Vec::new();
        };
        self.outgoing
            .iter()
            .filter(|invite| invite.party_id == party.id)
            .cloned()
            .collect()
    }

    pub fn is_invited(&self, player: Uuid) -> bool {
        self.party_outgoing()
            .iter()
            .any(|invite| invite.recipient == player)
    }

    fn drop_invite(&mut self, id: Uuid) {
        self.incoming.retain(|invite| invite.id != id);
        self.outgoing.retain(|invite| invite.id != id);
    }

    fn lost_party(&self, member_as: Option<Uuid>) -> bool {
        member_as.is_some()
            && self.me == member_as
            && self.party.is_none()
            && self.error.is_none()
    }

    pub fn apply(&mut self, packet: PartyPacket) -> Option<Notice> {
        match packet {
            PartyPacket::PartyUpdated { party } => {
                self.incoming.retain(|invite| invite.party_id != party.id);
                self.party = Some(party);
                None
            }
            PartyPacket::PartyMemberStateChanged {
                party_id,
                player,
                state,
            } => {
                if let Some(member) = self
                    .party
                    .as_mut()
                    .filter(|party| party.id == party_id)
                    .and_then(|party| party.members.iter_mut().find(|m| m.player == player))
                {
                    member.state = Some(state);
                }
                None
            }
            PartyPacket::PartyLeft { party_id, reason } => {
                let party = self.party.take_if(|party| party.id == party_id)?;
                match reason {
                    LeftReason::Kicked => Some(Notice::Kicked),
                    LeftReason::Disbanded if !is_leader(&party, self.me) => {
                        Some(Notice::Disbanded)
                    }
                    LeftReason::Expired => Some(Notice::PartyLost),
                    LeftReason::Disbanded | LeftReason::Left | LeftReason::Other => None,
                }
            }
            PartyPacket::PartyInviteReceived { invite } => {
                if self.incoming.iter().any(|known| known.id == invite.id) {
                    return None;
                }
                let sender = invite.sender;
                self.incoming.insert(0, invite);
                Some(Notice::Invited(sender))
            }
            PartyPacket::PartyInviteUpdated { invite_id, status } => {
                if status != InviteStatus::Pending {
                    self.drop_invite(invite_id);
                }
                None
            }
            PartyPacket::FriendRemoved { .. } => None,
        }
    }
}

pub fn is_leader(party: &Party, me: Option<Uuid>) -> bool {
    me == Some(party.leader)
}

pub fn can_invite(party: &Party, me: Option<Uuid>) -> bool {
    is_leader(party, me) || party.settings.invite_policy == InvitePolicy::Members
}

pub fn is_full(party: &Party) -> bool {
    party.members.len() >= MAX_PARTY_SIZE
}

pub fn online_count(party: &Party) -> usize {
    party.members.iter().filter(|member| member.online).count()
}

pub fn sorted_members(party: &Party, me: Option<Uuid>) -> Vec<PartyMember> {
    let mut members = party.members.clone();
    members.sort_by_key(|member| {
        (
            member.player != party.leader,
            Some(member.player) != me,
            member.joined_at,
        )
    });
    members
}

pub fn successor(party: &Party, me: Option<Uuid>) -> Option<Uuid> {
    let mut others: Vec<_> = party
        .members
        .iter()
        .filter(|member| Some(member.player) != me)
        .collect();
    others.sort_by_key(|member| (!member.online, member.joined_at));
    others.first().map(|member| member.player)
}

#[derive(Clone, Copy)]
struct LobbyContext(State<LobbyState>);

pub fn use_provide_lobby() {
    let state = use_state(LobbyState::default);
    let dispatch = use_dispatch();
    use_hook(move || {
        provide_root_context(LobbyContext(state));
        spawn(sync(state, dispatch));
    });
}

pub fn use_lobby() -> State<LobbyState> {
    consume_root_context::<LobbyContext>().0
}

async fn sync(mut state: State<LobbyState>, dispatch: Actions) {
    let mut events = party::subscribe();
    refresh(state).await;
    loop {
        match events.recv().await {
            Ok(LiveEvent::Connected) | Err(RecvError::Lagged(_)) => {
                let member_as = state.peek().party.as_ref().and(state.peek().me);
                refresh(state).await;
                if state.peek().lost_party(member_as) {
                    announce(Notice::PartyLost, &dispatch);
                }
            }
            Ok(LiveEvent::Packet(packet)) => {
                let notice = state.write().apply(*packet);
                if let Some(notice) = notice {
                    announce(notice, &dispatch);
                }
            }
            Err(RecvError::Closed) => break,
        }
    }
}

pub async fn refresh(mut state: State<LobbyState>) {
    let result = crate::launcher::off_ui(async {
        let account = party_account().await?;
        let party = party::current(&account).await.map_err(party_message)?;
        let incoming = party::incoming(&account).await.map_err(party_message)?;
        let outgoing = party::outgoing(&account).await.map_err(party_message)?;
        Ok::<_, String>((account.id, party, incoming, outgoing))
    })
    .await;

    let mut guard = state.write();
    match result {
        Ok((me, party, incoming, outgoing)) => {
            guard.me = Some(me);
            guard.party = party;
            guard.incoming = incoming;
            guard.outgoing = outgoing;
            guard.error = None;
        }
        Err(err) => {
            guard.party = None;
            guard.incoming.clear();
            guard.outgoing.clear();
            guard.error = Some(err);
        }
    }
}

fn announce(notice: Notice, dispatch: &Actions) {
    let message = match notice {
        Notice::Kicked => t("oneclient.components.lobby.toast.you_were_kicked").to_string(),
        Notice::Disbanded => t("oneclient.components.lobby.toast.disbanded_by_leader").to_string(),
        Notice::PartyLost => t("oneclient.components.lobby.toast.party_lost").to_string(),
        Notice::Invited(sender) => {
            let dispatch = dispatch.clone();
            spawn(async move {
                let name = player_name(sender).await;
                dispatch
                    .notify(tf("oneclient.components.lobby.toast.invite_received", &[&name]))
                    .info()
                    .icon(IconType::Users01)
                    .toast_only()
                    .send();
            });
            return;
        }
    };
    dispatch
        .notify(message)
        .info()
        .icon(IconType::Users01)
        .toast_only()
        .send();
}

pub fn party_message(err: PlusError) -> String {
    party::error_message(&err).unwrap_or_else(|| plus_message(err))
}

async fn party_account() -> Result<MinecraftAccount, String> {
    let state = crate::launcher::state().map_err(|err| err.to_string())?;
    let account = state
        .auth
        .default_account_for_launch()
        .await
        .map_err(|err| err.to_string())?
        .filter(MinecraftAccount::is_microsoft)
        .ok_or_else(|| t("oneclient.wardrobe.account.signed_out").to_string())?;
    if account.needs_sign_in() || account.access_token.is_empty() {
        return Err(t("oneclient.wardrobe.account.needs_sign_in").into());
    }
    Ok(account)
}

pub fn act<T, F, Fut>(
    mut state: State<LobbyState>,
    dispatch: Actions,
    work: F,
    done: impl FnOnce(&mut LobbyState, T) + 'static,
) where
    T: Send + 'static,
    F: FnOnce(MinecraftAccount) -> Fut + Send + 'static,
    Fut: Future<Output = Result<T, PlusError>> + Send + 'static,
{
    if state.peek().busy {
        return;
    }
    state.write().busy = true;
    spawn_forever(async move {
        let result = crate::launcher::off_ui(async move {
            let account = party_account().await?;
            work(account).await.map_err(party_message)
        })
        .await;

        state.write().busy = false;
        match result {
            Ok(value) => done(&mut state.write(), value),
            Err(err) => {
                dispatch
                    .notify(t("oneclient.components.lobby.toast.failed"))
                    .body(err)
                    .error()
                    .toast_only()
                    .send();
                refresh(state).await;
            }
        }
    });
}

pub fn use_my_uuid() -> Option<Uuid> {
    try_default_account(&use_current_account())
        .filter(MinecraftAccount::is_microsoft)
        .map(|account| account.id)
}

pub fn use_player_name(player: Uuid) -> String {
    let me = try_default_account(&use_current_account());
    let profile = settled_or_loading(&use_player_profile(player.to_string(), None::<String>));
    match me {
        Some(account) if account.id == player => account.username,
        _ => profile.map_or_else(|| short_uuid(player), |profile| profile.username),
    }
}

fn short_uuid(player: Uuid) -> String {
    player.simple().to_string()[..8].to_string()
}

async fn player_name(player: Uuid) -> String {
    crate::launcher::off_ui(async move {
        let state = crate::launcher::state().ok()?;
        oneclient_mc::fetch_player_profile_view(
            &state.services.requester,
            &player.to_string(),
            None,
        )
        .await
        .ok()
        .map(|profile| profile.username)
    })
    .await
    .unwrap_or_else(|| short_uuid(player))
}

#[derive(Clone, Debug, PartialEq)]
pub struct FriendView {
    pub player: Uuid,
    pub name: String,
    pub online: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FriendsQuery;

impl QueryCapability for FriendsQuery {
    type Ok = Vec<FriendView>;
    type Err = String;
    type Keys = Uuid;

    async fn run(&self, _account: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        crate::launcher::off_ui(async {
            let account = party_account().await?;
            let friends = party::friends(&account).await.map_err(party_message)?;
            let names = join_all(friends.iter().map(|friend| player_name(friend.player))).await;
            let mut views: Vec<_> = friends
                .into_iter()
                .zip(names)
                .map(|(friend, name)| FriendView {
                    player: friend.player,
                    name,
                    online: friend.online,
                })
                .collect();
            views.sort_by_key(|friend| (!friend.online, friend.name.to_lowercase()));
            Ok(views)
        })
        .await
    }
}

pub fn use_friends(account: Uuid) -> UseQuery<FriendsQuery> {
    use_query(Query::new(account, FriendsQuery).stale_time(FRIENDS_STALE))
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use oneclient_polyplus::party::{LocationKind, MemberLocation, MemberState, PartySettings};

    use super::*;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn member(n: u128, joined: i64, online: bool) -> PartyMember {
        PartyMember {
            player: id(n),
            joined_at: Utc.timestamp_opt(joined, 0).unwrap(),
            online,
            state: None,
        }
    }

    fn party(leader: u128, members: Vec<PartyMember>) -> Party {
        Party {
            id: id(100),
            leader: id(leader),
            settings: PartySettings {
                invite_policy: InvitePolicy::Leader,
            },
            members,
        }
    }

    fn invite(n: u128, party_id: Uuid, recipient: u128) -> PartyInvite {
        PartyInvite {
            id: id(n),
            party_id,
            sender: id(1),
            recipient: id(recipient),
            status: InviteStatus::Pending,
            created_at: Utc::now(),
            expires_at: Utc::now(),
            party_size: 1,
        }
    }

    #[test]
    fn leader_sorts_first_then_me_then_by_join_time() {
        let party = party(
            3,
            vec![member(1, 10, true), member(2, 5, true), member(3, 20, true)],
        );
        let order: Vec<_> = sorted_members(&party, Some(id(1)))
            .iter()
            .map(|m| m.player)
            .collect();
        assert_eq!(order, [id(3), id(1), id(2)]);
    }

    #[test]
    fn successor_prefers_longest_standing_online_member() {
        let party = party(
            1,
            vec![member(1, 0, true), member(2, 5, false), member(3, 9, true)],
        );
        assert_eq!(successor(&party, Some(id(1))), Some(id(3)));
        let alone = party_with_offline();
        assert_eq!(successor(&alone, Some(id(1))), Some(id(2)));
    }

    fn party_with_offline() -> Party {
        party(1, vec![member(1, 0, true), member(2, 5, false)])
    }

    #[test]
    fn invite_rights_follow_policy() {
        let mut party = party(1, vec![member(1, 0, true), member(2, 1, true)]);
        assert!(can_invite(&party, Some(id(1))));
        assert!(!can_invite(&party, Some(id(2))));
        party.settings.invite_policy = InvitePolicy::Members;
        assert!(can_invite(&party, Some(id(2))));
        assert!(!is_full(&party));
    }

    #[test]
    fn packets_drive_the_state() {
        let mut state = LobbyState::default();
        let p = party(1, vec![member(1, 0, true)]);
        state.outgoing = vec![invite(7, p.id, 2), invite(8, id(999), 3)];
        assert_eq!(state.apply(PartyPacket::PartyUpdated { party: p.clone() }), None);
        assert_eq!(state.party_outgoing().len(), 1);
        assert!(state.is_invited(id(2)));
        assert!(!state.is_invited(id(3)));

        state.apply(PartyPacket::PartyMemberStateChanged {
            party_id: p.id,
            player: id(1),
            state: MemberState {
                location: MemberLocation {
                    kind: LocationKind::Menu,
                    display: None,
                },
            },
        });
        assert!(state.party.as_ref().unwrap().members[0].state.is_some());

        state.apply(PartyPacket::PartyInviteUpdated {
            invite_id: id(7),
            status: InviteStatus::Declined,
        });
        assert!(state.party_outgoing().is_empty());

        let notice = state.apply(PartyPacket::PartyInviteReceived {
            invite: invite(9, id(555), 1),
        });
        assert_eq!(notice, Some(Notice::Invited(id(1))));
        assert_eq!(state.incoming.len(), 1);

        let mut joined = p.clone();
        joined.id = id(555);
        state.apply(PartyPacket::PartyUpdated { party: joined });
        assert!(state.incoming.is_empty());
        state.apply(PartyPacket::PartyUpdated { party: p.clone() });

        assert_eq!(
            state.apply(PartyPacket::PartyLeft {
                party_id: id(12345),
                reason: LeftReason::Kicked,
            }),
            None
        );
        assert_eq!(
            state.apply(PartyPacket::PartyLeft {
                party_id: p.id,
                reason: LeftReason::Kicked,
            }),
            Some(Notice::Kicked)
        );
        assert!(state.party.is_none());
    }

    #[test]
    fn only_members_hear_that_the_leader_disbanded() {
        let p = party(1, vec![member(1, 0, true), member(2, 1, true)]);
        let disbanded = |me: u128| {
            let mut state = LobbyState {
                me: Some(id(me)),
                party: Some(p.clone()),
                ..LobbyState::default()
            };
            let notice = state.apply(PartyPacket::PartyLeft {
                party_id: p.id,
                reason: LeftReason::Disbanded,
            });
            assert!(state.party.is_none());
            notice
        };
        assert_eq!(disbanded(1), None);
        assert_eq!(disbanded(2), Some(Notice::Disbanded));
    }

    #[test]
    fn losing_the_party_needs_the_same_account_and_a_clean_refresh() {
        let mut state = LobbyState {
            me: Some(id(1)),
            ..LobbyState::default()
        };
        assert!(state.lost_party(Some(id(1))));
        assert!(!state.lost_party(None));
        assert!(!state.lost_party(Some(id(2))));

        state.error = Some("offline".to_string());
        assert!(!state.lost_party(Some(id(1))));

        state.error = None;
        state.party = Some(party(1, vec![member(1, 0, true)]));
        assert!(!state.lost_party(Some(id(1))));
    }
}
