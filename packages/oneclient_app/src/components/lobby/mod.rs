mod dialogs;
mod panel;
mod widgets;

use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;

use freya::prelude::*;
use freya::query::{Query, QueryCapability, UseQuery, use_query};
use oneclient_auth::MinecraftAccount;
use oneclient_polyplus::PlusError;
use oneclient_polyplus::party::{
    self, InvitePolicy, InviteStatus, LeftReason, LiveEvent, MAX_PARTY_SIZE, Party, PartyInvite,
    PartyMember, PartyPacket,
};
use tokio::sync::broadcast::error::RecvError;
use uuid::Uuid;

use crate::components::IconType;
use crate::chat::short_player_id;
use crate::hooks::{
    Actions, player_name, settled_or_loading, try_default_account, use_chat_allowed,
    use_current_account, use_dispatch, use_player_profile, wardrobe::plus_message,
};
use crate::i18n::{t, tf};

pub(crate) use dialogs::modal;
pub use panel::PartyTab;
pub use widgets::LobbyHeroChip;

const FRIENDS_STALE: Duration = Duration::from_secs(30);

type Snapshot = (Uuid, Option<Party>, Vec<PartyInvite>, Vec<PartyInvite>);

#[derive(Clone, PartialEq, Default, Debug)]
pub struct LobbyState {
    pub party: Option<Party>,
    pub incoming: Vec<PartyInvite>,
    pub outgoing: Vec<PartyInvite>,
    pub error: Option<String>,
    pub busy: bool,
    pub me: Option<Uuid>,
    revision: u64,
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

    fn absorb(&mut self, seen: u64, result: Result<Snapshot, (Option<Uuid>, String)>) -> bool {
        if self.revision != seen {
            return false;
        }
        self.revision += 1;

        match result {
            Ok((me, party, incoming, outgoing)) => {
                self.me = Some(me);
                self.party = party;
                self.incoming = incoming;
                self.outgoing = outgoing;
                self.error = None;
            }
            Err((account, err)) => {
                if account.is_none() || account != self.me {
                    self.me = account;
                    self.party = None;
                    self.incoming.clear();
                    self.outgoing.clear();
                }
                self.error = Some(err);
            }
        }
        true
    }

    pub fn apply(&mut self, packet: PartyPacket) -> Option<Notice> {
        if !matches!(packet, PartyPacket::PartyMemberStateChanged { .. }) {
            self.revision += 1;
        }

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
    let state = use_lobby();
    let dispatch = use_dispatch();
    let mut chat_key = use_state(|| None::<(Uuid, Option<i32>)>);
    let owner = use_my_uuid();
    let chat_allowed = use_chat_allowed();
    oneclient_polyplus::set_social_allowed(chat_allowed);
    let mut synced_for = use_state(|| (owner, chat_allowed));

    use_side_effect_with_deps(&(owner, chat_allowed), move |current| {
        if *synced_for.peek() != *current {
            synced_for.set(*current);
            spawn(refresh(state));
        }
    });

    use_side_effect({
        let dispatch = dispatch.clone();
        move || {
            let key = state
                .read()
                .party
                .as_ref()
                .map(|party| (party.id, party.chat_group_id));
            if *chat_key.peek() != key {
                chat_key.set(key);
                dispatch.refresh_chat();
            }
        }
    });

    use_hook(move || {
        spawn(sync(state, dispatch));
    });
}

pub fn use_lobby() -> State<LobbyState> {
    GlobalContexts::get()
        .get_context_or_insert(|| LobbyContext(State::create_global(LobbyState::default())))
        .0
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
    loop {
        let seen = state.peek().revision;
        let result = crate::launcher::off_ui(async {
            let account = lobby_account().await?;
            let id = account.id;
            let failed = |err: PlusError| (Some(id), party_message(err));
            let party = party::current(&account).await.map_err(failed)?;
            let incoming = party::incoming(&account).await.map_err(failed)?;
            let outgoing = party::outgoing(&account).await.map_err(failed)?;
            Ok::<Snapshot, (Option<Uuid>, String)>((id, party, incoming, outgoing))
        })
        .await;

        if state.write().absorb(seen, result) {
            break;
        }
    }
}

fn announce(notice: Notice, dispatch: &Actions) {
    let (title, body) = match notice {
        Notice::Kicked => (
            t("oneclient.components.lobby.toast.you_were_kicked"),
            t("oneclient.components.lobby.toast.you_were_kicked_body"),
        ),
        Notice::Disbanded => (
            t("oneclient.components.lobby.toast.disbanded_by_leader"),
            t("oneclient.components.lobby.toast.disbanded_by_leader_body"),
        ),
        Notice::PartyLost => (
            t("oneclient.components.lobby.toast.party_lost"),
            t("oneclient.components.lobby.toast.party_lost_body"),
        ),
        Notice::Invited(sender) => {
            if dispatch.station().peek().chat.is_blocked(sender) {
                return;
            }

            let dispatch = dispatch.clone();
            spawn(async move {
                let name = player_name(sender).await;
                dispatch
                    .notify(tf("oneclient.components.lobby.toast.invite_received", &[&name]))
                    .body(t("oneclient.components.lobby.toast.invite_received_body"))
                    .info()
                    .icon(IconType::Users01)
                    .toast_only()
                    .send();
            });
            return;
        }
    };
    dispatch
        .notify(title)
        .body(body)
        .info()
        .icon(IconType::Users01)
        .toast_only()
        .send();
}

pub fn party_message(err: PlusError) -> String {
    party::error_message(&err).unwrap_or_else(|| plus_message(err))
}

fn renewing_owner(stored: Option<MinecraftAccount>, allowed: bool) -> Option<Uuid> {
    stored
        .filter(|account| allowed && account.is_microsoft())
        .map(|account| account.id)
}

async fn lobby_account() -> Result<MinecraftAccount, (Option<Uuid>, String)> {
    let state = crate::launcher::state().map_err(|err| (None, err.to_string()))?;
    let allowed = oneclient_polyplus::social_allowed();
    let account = match state.auth.default_account_for_launch().await {
        Ok(account) => account,
        Err(err) => {
            let stored = state.auth.default_account().await.ok().flatten();
            return Err((renewing_owner(stored, allowed), err.to_string()));
        }
    }
    .filter(MinecraftAccount::is_microsoft)
    .filter(|_| allowed)
    .ok_or_else(|| (None, t("oneclient.wardrobe.account.signed_out").to_string()))?;
    if account.needs_sign_in() || account.access_token.is_empty() {
        return Err((None, t("oneclient.wardrobe.account.needs_sign_in").into()));
    }
    Ok(account)
}

async fn party_account() -> Result<MinecraftAccount, String> {
    lobby_account().await.map_err(|(_, err)| err)
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
    spawn_global(async move {
        let result = crate::launcher::off_ui(async move {
            let account = party_account().await?;
            let id = account.id;
            let value = work(account).await.map_err(party_message)?;
            Ok::<_, String>((id, value))
        })
        .await;

        state.write().busy = false;
        match result {
            Ok((id, _)) if state.peek().me != Some(id) => refresh(state).await,
            Ok((_, value)) => {
                let mut guard = state.write();
                guard.revision += 1;
                guard.error = None;
                done(&mut guard, value);
            }
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
        _ => profile.map_or_else(|| short_player_id(player), |profile| profile.username),
    }
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
            party_account().await?;
            let client = oneclient_polyplus::client()
                .ok_or_else(|| t("oneclient.chat.errors.unavailable").to_string())?;
            let friends = client.friends().await.map_err(party_message)?;
            let players: Vec<Uuid> = friends.iter().map(|friend| friend.player).collect();
            let names: HashMap<Uuid, String> = client
                .resolve_players(&players)
                .await
                .inspect_err(|err| tracing::warn!("could not resolve friend names: {err:#}"))
                .unwrap_or_default()
                .into_iter()
                .map(|player| (player.id, player.username))
                .collect();
            let mut views: Vec<_> = friends
                .into_iter()
                .map(|friend| FriendView {
                    player: friend.player,
                    name: names
                        .get(&friend.player)
                        .cloned()
                        .unwrap_or_else(|| short_player_id(friend.player)),
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

    fn snapshot(me: Uuid, party: Option<Party>) -> Result<Snapshot, (Option<Uuid>, String)> {
        Ok((me, party, Vec::new(), Vec::new()))
    }

    #[test]
    fn a_refresh_older_than_a_change_is_discarded() {
        let joined = party(1, vec![member(1, 0, true)]);
        let mut state = LobbyState::default();
        state.apply(PartyPacket::PartyUpdated {
            party: joined.clone(),
        });

        assert!(!state.absorb(0, snapshot(id(1), None)));
        assert_eq!(state.party, Some(joined));
    }

    #[test]
    fn a_failed_refresh_keeps_the_party_of_the_same_account() {
        let joined = party(1, vec![member(1, 0, true)]);
        let mut state = LobbyState::default();
        assert!(state.absorb(0, snapshot(id(1), Some(joined.clone()))));

        assert!(state.absorb(1, Err((Some(id(1)), "offline".into()))));

        assert_eq!(state.party, Some(joined));
        assert_eq!(state.error.as_deref(), Some("offline"));
    }

    #[test]
    fn a_failed_renewal_only_keeps_a_social_account() {
        let account = |kind| MinecraftAccount {
            id: id(1),
            username: "player".to_string(),
            access_token: "token".to_string(),
            refresh_token: String::new(),
            expires: Utc::now(),
            kind,
            signed_out: false,
        };
        let microsoft = oneclient_auth::AccountKind::Microsoft;
        let offline = oneclient_auth::AccountKind::Offline;

        assert_eq!(renewing_owner(Some(account(microsoft)), true), Some(id(1)));
        assert_eq!(renewing_owner(Some(account(microsoft)), false), None);
        assert_eq!(renewing_owner(Some(account(offline)), true), None);
        assert_eq!(renewing_owner(None, true), None);
    }

    #[test]
    fn a_failed_refresh_drops_the_party_of_another_account() {
        let joined = party(1, vec![member(1, 0, true)]);
        let mut state = LobbyState::default();
        assert!(state.absorb(0, snapshot(id(1), Some(joined.clone()))));
        assert!(state.absorb(1, Err((Some(id(2)), "offline".into()))));
        assert_eq!(state.party, None);

        assert!(state.absorb(2, snapshot(id(1), Some(joined))));
        assert!(state.absorb(3, Err((None, "signed out".into()))));
        assert_eq!(state.party, None);
        assert_eq!(state.me, None);
    }

    #[test]
    fn joining_a_party_keeps_invites_from_other_parties() {
        let mut state = LobbyState::default();
        let joined = party(1, vec![member(1, 0, true)]);
        let other = id(999);
        state.incoming = vec![invite(7, joined.id, 2), invite(8, other, 2)];

        state.apply(PartyPacket::PartyUpdated { party: joined });

        let left: Vec<Uuid> = state
            .incoming
            .iter()
            .map(|invite| invite.party_id)
            .collect();
        assert_eq!(left, vec![other]);
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
            chat_group_id: None,
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
