use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{DateTime, FixedOffset, Utc};
use uuid::Uuid;

use oneclient_common::GameLoader;
use oneclient_polyplus::{
    BlockedPlayer, Friend, FriendRequest, GroupKind, GroupMessage, GroupSummary,
    IncomingSessionInvite, PlayerStatus,
};

use crate::state::{AsyncStatus, SocialTab};

#[must_use]
pub fn thread_in_view(visible: bool, tab: SocialTab, active: Option<i32>, group_id: i32) -> bool {
    visible && tab == SocialTab::Chat && active == Some(group_id)
}

#[must_use]
pub fn chat_visible(chat_window_focused: bool, social_open: bool, main_focused: bool) -> bool {
    chat_window_focused || (social_open && main_focused)
}

#[must_use]
pub fn short_player_id(player: Uuid) -> String {
    player.simple().to_string()[..8].to_string()
}

#[must_use]
pub fn chat_allowed(
    settings: &oneclient_core::settings::LauncherSettings,
    consent_declined: bool,
) -> bool {
    !consent_declined
        && !settings.declined_tos
        && settings.accepted_tos_version > 0
        && settings.accepted_privacy_version > 0
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FriendRequests {
    pub incoming: Vec<FriendRequest>,
    pub outgoing: Vec<FriendRequest>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChatRoster {
    pub friends: Vec<Friend>,
    pub requests: FriendRequests,
    pub blocked: Vec<BlockedPlayer>,
}

impl ChatRoster {
    #[must_use]
    pub fn is_blocked(&self, player: Uuid) -> bool {
        self.blocked.iter().any(|blocked| blocked.player == player)
    }
}

fn find_conversation(
    conversations: &[ChatConversation],
    group_id: i32,
) -> Option<&ChatConversation> {
    conversations
        .iter()
        .find(|conversation| conversation.id == group_id)
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChatConversation {
    pub id: i32,
    pub kind: GroupKind,
    pub name: Option<String>,
    pub members: Vec<Uuid>,
    pub unread: bool,
    pub preview: Option<String>,
    pub preview_sender: Option<Uuid>,
    pub last_activity: Option<DateTime<FixedOffset>>,
    pub special: bool,
}

impl From<GroupSummary> for ChatConversation {
    fn from(value: GroupSummary) -> Self {
        let (preview, preview_sender, last_activity) = match value.last_message {
            Some(last) => (Some(last.content), Some(last.sender), Some(last.sent_at)),
            None => (None, None, None),
        };

        Self {
            id: value.id,
            kind: value.kind,
            name: value.name,
            members: value.members,
            unread: value.unread,
            preview,
            preview_sender,
            last_activity,
            special: value.special,
        }
    }
}

impl ChatConversation {
    #[must_use]
    pub fn counterpart(&self, own_id: Uuid) -> Option<Uuid> {
        if self.kind != GroupKind::Dm {
            return None;
        }

        self.members
            .iter()
            .copied()
            .find(|member| *member != own_id)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChatMessage {
    pub id: i64,
    pub sender: Uuid,
    pub content: String,
    pub sent_at: Option<DateTime<FixedOffset>>,
    pub edited: bool,
    pub invite: Option<MessageInvite>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MessageInvite {
    pub id: i32,
    pub status: String,
}

impl MessageInvite {
    #[must_use]
    pub fn is_open(&self) -> bool {
        !matches!(
            self.status.to_ascii_lowercase().as_str(),
            "accepted" | "declined" | "expired" | "cancelled" | "canceled"
        )
    }

    #[must_use]
    pub fn new(id: i32, status: Option<String>) -> Self {
        Self {
            id,
            status: status.unwrap_or_else(|| "pending".to_string()),
        }
    }
}

impl From<GroupMessage> for ChatMessage {
    fn from(value: GroupMessage) -> Self {
        Self {
            id: value.id,
            sender: value.sender,
            content: value.content,
            sent_at: Some(value.sent_at),
            edited: value.edited_at.is_some(),
            invite: value
                .session_invite
                .map(|invite| MessageInvite::new(invite.id, Some(invite.status))),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PendingMessage {
    pub key: Uuid,
    pub content: String,
    pub failed: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum FriendRequestStatus {
    #[default]
    Idle,
    Sending(String),
    Sent(String),
    Failed(String),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InviteGame {
    pub version: Option<String>,
    pub loader: Option<GameLoader>,
}

impl InviteGame {
    #[must_use]
    pub fn known(&self) -> Option<(&str, GameLoader)> {
        Some((self.version.as_deref()?, self.loader?))
    }
}

impl From<&IncomingSessionInvite> for InviteGame {
    fn from(invite: &IncomingSessionInvite) -> Self {
        Self {
            version: invite.minecraft_version.clone(),
            loader: invite.loader.as_deref().and_then(|name| name.parse().ok()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChatInvite {
    pub id: i32,
    pub from: Uuid,
    pub group_id: i32,
    pub message_id: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChatToast {
    Message {
        group_id: i32,
        sender: Uuid,
        group_name: Option<String>,
        content: String,
    },
    Invite {
        group_id: i32,
        sender: Uuid,
    },
    FriendRequest {
        sender: Uuid,
    },
}

impl ChatToast {
    #[must_use]
    pub fn sender(&self) -> Uuid {
        match self {
            Self::Message { sender, .. }
            | Self::Invite { sender, .. }
            | Self::FriendRequest { sender } => *sender,
        }
    }

    #[must_use]
    pub fn group_id(&self) -> Option<i32> {
        match self {
            Self::Message { group_id, .. } | Self::Invite { group_id, .. } => Some(*group_id),
            Self::FriendRequest { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Received {
    pub unknown_group: bool,
    pub read: bool,
    pub toast: Option<ChatToast>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChatInbox {
    pub conversations: Vec<ChatConversation>,
    pub loading: bool,
    pub active: Option<i32>,
    pub active_gone: u64,
    pub active_opened: u64,
    pub open_friends: bool,
    pub connected: bool,
    pub error: Option<String>,
    pub names: Arc<HashMap<Uuid, String>>,
}

impl ChatInbox {
    #[must_use]
    pub fn conversation(&self, group_id: i32) -> Option<&ChatConversation> {
        find_conversation(&self.conversations, group_id)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum ThreadStatus {
    #[default]
    Idle,
    Loading,
    Error(String),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChatThread {
    pub messages: Arc<Vec<ChatMessage>>,
    pub pending: Vec<PendingMessage>,
    pub status: ThreadStatus,
    pub complete: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChatState {
    pub connected: bool,
    pub status: AsyncStatus,
    pub error: Option<String>,
    pub active: Option<i32>,
    pub active_gone: u64,
    pub active_opened: u64,
    pub open_friends: bool,
    pub owner: Option<Uuid>,
    pub self_status: PlayerStatus,
    pub friend_request: FriendRequestStatus,
    conversations: Vec<ChatConversation>,
    messages: HashMap<i32, Arc<Vec<ChatMessage>>>,
    pending: HashMap<i32, Vec<PendingMessage>>,
    presence: HashMap<Uuid, bool>,
    roster: ChatRoster,
    threads: HashMap<i32, ThreadStatus>,
    complete: HashMap<i32, bool>,
    synced: HashMap<i32, i64>,
    dismissed_invites: HashSet<i32>,
    names: Arc<HashMap<Uuid, String>>,
    invite_games: HashMap<i32, InviteGame>,
}

impl ChatState {
    #[must_use]
    pub fn conversations(&self) -> &[ChatConversation] {
        &self.conversations
    }

    #[must_use]
    pub fn conversation(&self, group_id: i32) -> Option<&ChatConversation> {
        find_conversation(&self.conversations, group_id)
    }

    #[must_use]
    pub fn is_blocked(&self, player: Uuid) -> bool {
        self.roster.is_blocked(player)
    }

    #[must_use]
    pub fn synced_to(&self, group_id: i32) -> Option<i64> {
        self.synced.get(&group_id).copied()
    }

    #[must_use]
    pub fn messages_for(&self, group_id: i32) -> Arc<Vec<ChatMessage>> {
        self.messages.get(&group_id).cloned().unwrap_or_default()
    }

    #[must_use]
    pub fn pending_for(&self, group_id: i32) -> &[PendingMessage] {
        self.pending
            .get(&group_id)
            .map_or(&[][..], |pending| pending.as_slice())
    }

    #[must_use]
    pub fn has_unread(&self) -> bool {
        self.conversations
            .iter()
            .any(|conversation| conversation.unread)
    }

    #[must_use]
    pub fn inbox(&self) -> ChatInbox {
        ChatInbox {
            conversations: self.conversations.clone(),
            loading: self.conversations.is_empty()
                && self.error.is_none()
                && matches!(self.status, AsyncStatus::Idle | AsyncStatus::Loading),
            active: self.active,
            active_gone: self.active_gone,
            active_opened: self.active_opened,
            open_friends: self.open_friends,
            connected: self.connected,
            error: self.error.clone(),
            names: Arc::clone(&self.names),
        }
    }

    #[must_use]
    pub fn unnamed_players(&self) -> Vec<Uuid> {
        let owner = self.owner.unwrap_or_default();
        let players: HashSet<Uuid> = self
            .conversations
            .iter()
            .filter_map(|conversation| conversation.counterpart(owner))
            .chain(self.roster.friends.iter().map(|friend| friend.player))
            .chain(
                self.roster
                    .requests
                    .incoming
                    .iter()
                    .chain(&self.roster.requests.outgoing)
                    .map(|request| request.player),
            )
            .filter(|player| !self.names.contains_key(player))
            .collect();
        players.into_iter().collect()
    }

    pub fn add_names(&mut self, names: impl IntoIterator<Item = (Uuid, String)>) {
        Arc::make_mut(&mut self.names).extend(names);
    }

    #[must_use]
    pub fn roster(&self) -> ChatRoster {
        self.roster.clone()
    }

    #[must_use]
    pub fn has_incoming_requests(&self) -> bool {
        !self.roster.requests.incoming.is_empty()
    }

    pub fn set_roster(&mut self, mut roster: ChatRoster) {
        let friends: HashSet<Uuid> = roster.friends.iter().map(|friend| friend.player).collect();
        roster
            .requests
            .incoming
            .retain(|request| !friends.contains(&request.player));

        self.presence.retain(|player, _| !friends.contains(player));
        self.roster = roster;
    }

    pub fn clear_presence(&mut self) {
        self.presence.clear();
    }

    #[must_use]
    pub fn online_count(&self, members: &[Uuid]) -> usize {
        members
            .iter()
            .filter(|member| {
                if self.owner == Some(**member) {
                    self.connected && self.self_status == PlayerStatus::Online
                } else {
                    self.is_online(**member)
                }
            })
            .count()
    }

    #[must_use]
    pub fn watched_players(&self) -> Vec<Uuid> {
        self.roster
            .friends
            .iter()
            .map(|friend| friend.player)
            .chain(
                self.conversations
                    .iter()
                    .filter(|conversation| conversation.kind.is_shared())
                    .flat_map(|conversation| conversation.members.iter().copied()),
            )
            .filter(|player| self.owner != Some(*player))
            .collect()
    }

    #[must_use]
    pub fn is_friend(&self, player: Uuid) -> bool {
        self.roster
            .friends
            .iter()
            .any(|friend| friend.player == player)
    }

    #[must_use]
    pub fn thread(&self, group_id: i32) -> ChatThread {
        ChatThread {
            messages: self.messages_for(group_id),
            pending: self.pending_for(group_id).to_vec(),
            status: self
                .threads
                .get(&group_id)
                .cloned()
                .unwrap_or(ThreadStatus::Idle),
            complete: self.complete.get(&group_id).copied().unwrap_or(false),
        }
    }

    pub fn begin_thread_load(&mut self, group_id: i32) {
        self.threads.insert(group_id, ThreadStatus::Loading);
    }

    pub fn settle_thread_load(&mut self, group_id: i32) {
        self.threads.insert(group_id, ThreadStatus::Idle);
    }

    pub fn fail_thread_load(&mut self, group_id: i32, message: String) {
        self.threads.insert(group_id, ThreadStatus::Error(message));
    }

    pub fn set_history_complete(&mut self, group_id: i32, complete: bool) {
        self.complete.insert(group_id, complete);
    }

    #[must_use]
    pub fn oldest_message(&self, group_id: i32) -> Option<i64> {
        self.messages
            .get(&group_id)
            .and_then(|messages| messages.first())
            .map(|message| message.id)
    }

    #[must_use]
    pub fn newest_message(&self, group_id: i32) -> Option<i64> {
        self.messages
            .get(&group_id)
            .and_then(|messages| messages.last())
            .map(|message| message.id)
    }

    pub fn set_conversations(&mut self, summaries: Vec<GroupSummary>) {
        let kept: HashSet<i32> = summaries.iter().map(|summary| summary.id).collect();
        let gone: Vec<i32> = self
            .conversations
            .iter()
            .map(|conversation| conversation.id)
            .filter(|id| !kept.contains(id))
            .collect();

        self.conversations = summaries.into_iter().map(ChatConversation::from).collect();
        for group_id in gone {
            self.forget_conversation(group_id);
        }
        let active = self.active;
        self.messages
            .retain(|group_id, _| kept.contains(group_id) || active == Some(*group_id));
        let messages = &self.messages;
        self.synced.retain(|group_id, _| messages.contains_key(group_id));
        self.sort();
        self.status = AsyncStatus::Ready;
        self.error = None;
    }

    pub fn set_messages(&mut self, group_id: i32, page: Vec<GroupMessage>, known: Option<i64>) {
        let mut messages: Vec<ChatMessage> = page.into_iter().map(ChatMessage::from).collect();
        messages.sort_by_key(|message| message.id);
        messages.dedup_by_key(|message| message.id);

        let first = messages
            .first()
            .map(|message| message.id)
            .filter(|first| known.is_some_and(|known| known >= *first));
        let last = messages.last().map(|message| message.id);
        let newer = last.max(known).unwrap_or(i64::MIN);
        messages.extend(
            self.messages_for(group_id)
                .iter()
                .filter(|message| {
                    first.is_some_and(|first| message.id < first) || message.id > newer
                })
                .cloned(),
        );
        messages.sort_by_key(|message| message.id);

        self.messages.insert(group_id, Arc::new(messages));
        if let Some(last) = last {
            self.synced.insert(group_id, last);
        }
    }

    pub fn prepend_messages(&mut self, group_id: i32, page: Vec<GroupMessage>) {
        let mut merged: Vec<ChatMessage> = page.into_iter().map(ChatMessage::from).collect();
        merged.extend(self.messages_for(group_id).iter().cloned());
        merged.sort_by_key(|message| message.id);
        merged.dedup_by_key(|message| message.id);
        self.messages.insert(group_id, Arc::new(merged));
    }

    pub fn upsert(&mut self, group_id: i32, message: ChatMessage) {
        let bucket = self.messages.entry(group_id).or_default();
        let messages = Arc::make_mut(bucket);

        match messages.binary_search_by_key(&message.id, |existing| existing.id) {
            Ok(index) => messages[index] = message,
            Err(index) => messages.insert(index, message),
        }
    }

    pub fn receive(
        &mut self,
        group_id: i32,
        message: ChatMessage,
        visible: bool,
        tab: SocialTab,
    ) -> Received {
        let sender = message.sender;
        let content = message.content.clone();
        let invited = message.invite.is_some();
        self.upsert(group_id, message);

        let viewing = thread_in_view(visible, tab, self.active, group_id);
        let own = self.owner == Some(sender);
        let conversation = self.conversation(group_id);
        let unknown_group = conversation.is_none();
        let group_name = conversation
            .filter(|conversation| conversation.kind.is_shared())
            .and_then(|conversation| conversation.name.clone());
        let blocked = self.roster.is_blocked(sender);
        let toast = (!viewing && !own && !blocked).then(|| {
            if invited {
                ChatToast::Invite { group_id, sender }
            } else {
                ChatToast::Message {
                    group_id,
                    sender,
                    group_name,
                    content: content.clone(),
                }
            }
        });

        self.note_activity(group_id, sender, content, !viewing && !own);

        Received {
            unknown_group,
            read: viewing && !own,
            toast,
        }
    }

    fn refresh_preview(&mut self, group_id: i32) {
        let last = self.messages_for(group_id).last().cloned();
        if let Some(conversation) = self
            .conversations
            .iter_mut()
            .find(|conversation| conversation.id == group_id)
        {
            conversation.preview_sender = last.as_ref().map(|message| message.sender);
            conversation.preview = last.map(|message| message.content);
        }
    }

    pub fn edit(
        &mut self,
        group_id: i32,
        message_id: i64,
        content: String,
        invite_status: Option<String>,
    ) {
        let newest = self.newest_message(group_id) == Some(message_id);
        let Some(bucket) = self.messages.get_mut(&group_id) else {
            return;
        };

        if let Some(message) = Arc::make_mut(bucket)
            .iter_mut()
            .find(|message| message.id == message_id)
        {
            message.content = content;
            message.edited |= invite_status.is_none();
            if let (Some(invite), Some(status)) = (message.invite.as_mut(), invite_status) {
                invite.status = status;
            }
        }

        if newest {
            self.refresh_preview(group_id);
        }
    }

    pub fn remove(&mut self, group_id: i32, message_id: i64) {
        let newest = self.newest_message(group_id) == Some(message_id);
        let Some(bucket) = self.messages.get_mut(&group_id) else {
            return;
        };

        Arc::make_mut(bucket).retain(|message| message.id != message_id);

        if newest {
            self.refresh_preview(group_id);
        }
    }

    #[must_use]
    pub fn invites(&self) -> Vec<ChatInvite> {
        let mut invites: Vec<ChatInvite> = self
            .messages
            .iter()
            .flat_map(|(group_id, messages)| {
                messages.iter().filter_map(move |message| {
                    let invite = message.invite.as_ref()?;
                    (invite.is_open()
                        && Some(message.sender) != self.owner
                        && !self.roster.is_blocked(message.sender)
                        && !self.dismissed_invites.contains(&invite.id))
                    .then_some(ChatInvite {
                        id: invite.id,
                        from: message.sender,
                        group_id: *group_id,
                        message_id: message.id,
                    })
                })
            })
            .collect();
        invites.sort_by_key(|invite| std::cmp::Reverse(invite.message_id));
        invites
    }

    pub fn dismiss_invite(&mut self, invite_id: i32) {
        self.dismissed_invites.insert(invite_id);
    }

    pub fn restore_invite(&mut self, invite_id: i32) {
        self.dismissed_invites.remove(&invite_id);
    }

    pub fn set_invite_games(&mut self, invites: &[IncomingSessionInvite]) {
        self.invite_games
            .extend(invites.iter().map(|invite| (invite.id, invite.into())));
    }

    #[must_use]
    pub fn invite_game(&self, invite_id: i32) -> Option<&InviteGame> {
        self.invite_games.get(&invite_id)
    }

    #[must_use]
    pub fn has_messages(&self, group_id: i32) -> bool {
        self.messages.contains_key(&group_id)
    }

    pub fn note_activity(&mut self, group_id: i32, sender: Uuid, preview: String, unread: bool) {
        let now = Utc::now().fixed_offset();

        if let Some(conversation) = self
            .conversations
            .iter_mut()
            .find(|conversation| conversation.id == group_id)
        {
            conversation.preview = Some(preview);
            conversation.preview_sender = Some(sender);
            conversation.last_activity = Some(now);
            conversation.unread |= unread;
        }

        self.sort();
    }

    #[must_use]
    pub fn is_online(&self, player: Uuid) -> bool {
        self.presence.get(&player).copied().unwrap_or_else(|| {
            self.roster
                .friends
                .iter()
                .any(|friend| friend.player == player && friend.online)
        })
    }

    pub fn set_presence(&mut self, player: Uuid, online: bool) {
        self.presence.insert(player, online);
    }

    pub fn forget_conversation(&mut self, group_id: i32) {
        self.conversations
            .retain(|conversation| conversation.id != group_id);
        self.messages.remove(&group_id);
        self.pending.remove(&group_id);
        self.threads.remove(&group_id);
        self.complete.remove(&group_id);
        self.synced.remove(&group_id);
        if self.active == Some(group_id) {
            self.active = None;
            self.active_gone = self.active_gone.wrapping_add(1);
        }
    }

    pub fn mark_read(&mut self, group_id: i32) {
        if let Some(conversation) = self
            .conversations
            .iter_mut()
            .find(|conversation| conversation.id == group_id)
        {
            conversation.unread = false;
        }
    }

    pub fn begin_send(&mut self, group_id: i32, key: Uuid, content: String) {
        self.pending
            .entry(group_id)
            .or_default()
            .push(PendingMessage {
                key,
                content,
                failed: false,
            });
    }

    pub fn finish_send(&mut self, group_id: i32, key: Uuid, message: ChatMessage) {
        self.discard_pending(group_id, key);
        self.upsert(group_id, message);
    }

    pub fn begin_retry(&mut self, group_id: i32, key: Uuid) -> Option<String> {
        let pending = self
            .pending
            .get_mut(&group_id)?
            .iter_mut()
            .find(|pending| pending.key == key)?;

        pending.failed = false;
        Some(pending.content.clone())
    }

    pub fn fail_send(&mut self, group_id: i32, key: Uuid) {
        if let Some(bucket) = self.pending.get_mut(&group_id)
            && let Some(pending) = bucket.iter_mut().find(|pending| pending.key == key)
        {
            pending.failed = true;
        }
    }

    pub fn discard_pending(&mut self, group_id: i32, key: Uuid) {
        let Some(bucket) = self.pending.get_mut(&group_id) else {
            return;
        };

        bucket.retain(|pending| pending.key != key);
        if bucket.is_empty() {
            self.pending.remove(&group_id);
        }
    }

    pub fn set_owner(&mut self, owner: Option<Uuid>) -> bool {
        if self.owner == owner {
            return false;
        }

        self.clear();
        self.owner = owner;
        true
    }

    pub fn clear(&mut self) {
        let connected = self.connected;
        let active_gone = self
            .active_gone
            .wrapping_add(u64::from(self.active.is_some()));
        let active_opened = self.active_opened;
        *self = Self::default();
        self.connected = connected;
        self.active_gone = active_gone;
        self.active_opened = active_opened;
    }

    fn sort(&mut self) {
        self.conversations.sort_by(|a, b| {
            b.last_activity
                .cmp(&a.last_activity)
                .then_with(|| b.id.cmp(&a.id))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_only_counts_as_visible_while_the_launcher_has_focus() {
        assert!(chat_visible(true, false, false));
        assert!(chat_visible(false, true, true));
        assert!(!chat_visible(false, true, false));
        assert!(!chat_visible(false, false, true));
    }

    fn unread(chat: &ChatState) -> usize {
        chat.conversations().iter().filter(|c| c.unread).count()
    }

    fn live(id: i64, sender: Uuid) -> ChatMessage {
        ChatMessage {
            id,
            sender,
            content: format!("message {id}"),
            sent_at: None,
            edited: false,
            invite: None,
        }
    }

    #[test]
    fn a_message_in_the_open_thread_is_read_without_a_toast() {
        let mut chat = ChatState::default();
        chat.set_conversations(vec![summary(1, false)]);
        chat.active = Some(1);

        let received = chat.receive(1, live(1, Uuid::from_u128(9)), true, SocialTab::Chat);

        assert!(received.read);
        assert_eq!(received.toast, None);
        assert_eq!(unread(&chat), 0);
    }

    #[test]
    fn a_message_out_of_view_toasts_and_counts_as_unread() {
        let mut chat = ChatState::default();
        chat.set_conversations(vec![summary(1, false)]);
        chat.active = Some(1);

        let received = chat.receive(1, live(1, Uuid::from_u128(9)), false, SocialTab::Chat);

        assert!(!received.read);
        assert!(matches!(received.toast, Some(ChatToast::Message { .. })));
        assert_eq!(unread(&chat), 1);
    }

    #[test]
    fn an_own_message_never_toasts_and_an_unlisted_group_is_flagged() {
        let own = Uuid::from_u128(9);
        let mut chat = ChatState::default();
        chat.owner = Some(own);

        let received = chat.receive(7, live(1, own), false, SocialTab::Chat);

        assert!(received.unknown_group);
        assert!(!received.read);
        assert_eq!(received.toast, None);
    }

    #[test]
    fn a_later_list_drops_threads_it_does_not_contain() {
        let mut chat = ChatState::default();
        chat.set_conversations(vec![summary(1, false)]);
        chat.upsert(1, live(1, Uuid::nil()));
        chat.upsert(2, live(2, Uuid::nil()));
        chat.upsert(3, live(3, Uuid::nil()));
        chat.active = Some(3);

        chat.set_conversations(vec![summary(1, false)]);

        assert!(chat.has_messages(1));
        assert!(!chat.has_messages(2));
        assert!(chat.has_messages(3));
    }

    #[test]
    fn an_invite_status_change_is_not_an_edit() {
        let mut chat = ChatState::default();
        let mut invite = live(1, Uuid::nil());
        invite.invite = Some(MessageInvite::new(4, None));
        chat.upsert(1, invite);

        chat.edit(1, 1, String::new(), Some("expired".to_string()));
        assert!(!chat.messages_for(1)[0].edited);

        chat.edit(1, 1, "changed".to_string(), None);
        assert!(chat.messages_for(1)[0].edited);
    }

    #[test]
    fn the_preview_follows_the_newest_message() {
        let sender = Uuid::from_u128(1);
        let mut chat = ChatState::default();
        chat.set_conversations(vec![summary(1, false)]);
        chat.upsert(1, live(1, sender));
        chat.upsert(1, live(2, sender));
        chat.note_activity(1, sender, "message 2".to_string(), false);

        chat.edit(1, 1, "older".to_string(), None);
        let preview = |chat: &ChatState| chat.conversation(1).and_then(|c| c.preview.clone());
        assert_eq!(preview(&chat).as_deref(), Some("message 2"));

        chat.edit(1, 2, "changed".to_string(), None);
        assert_eq!(preview(&chat).as_deref(), Some("changed"));

        chat.remove(1, 2);
        assert_eq!(preview(&chat).as_deref(), Some("older"));

        chat.remove(1, 1);
        assert_eq!(preview(&chat), None);
    }

    #[test]
    fn a_thread_behind_the_party_tab_is_not_in_view() {
        assert!(thread_in_view(true, SocialTab::Chat, Some(1), 1));
        assert!(!thread_in_view(true, SocialTab::Party, Some(1), 1));
        assert!(!thread_in_view(false, SocialTab::Chat, Some(1), 1));
        assert!(!thread_in_view(true, SocialTab::Chat, Some(2), 1));
    }

    fn summary(id: i32, unread: bool) -> GroupSummary {
        GroupSummary {
            id,
            kind: GroupKind::Dm,
            name: None,
            members: Vec::new(),
            last_message: None,
            unread,
            special: false,
        }
    }

    fn fetched(id: i64, content: &str, sent_at: &str) -> GroupMessage {
        GroupMessage {
            id,
            sender: Uuid::nil(),
            content: content.to_string(),
            sent_at: DateTime::parse_from_rfc3339(sent_at).expect("fixture timestamp"),
            edited_at: None,
            session_invite: None,
        }
    }

    fn message(id: i64, content: &str) -> ChatMessage {
        ChatMessage {
            id,
            sender: Uuid::nil(),
            content: content.to_string(),
            sent_at: None,
            edited: false,
            invite: None,
        }
    }

    #[test]
    fn messages_stay_ordered_however_they_arrive() {
        let mut chat = ChatState::default();

        chat.upsert(1, message(30, "third"));
        chat.upsert(1, message(10, "first"));
        chat.upsert(1, message(20, "second"));

        let ids: Vec<i64> = chat.messages_for(1).iter().map(|m| m.id).collect();
        assert_eq!(ids, vec![10, 20, 30]);
    }

    #[test]
    fn a_resent_message_replaces_rather_than_duplicates() {
        let mut chat = ChatState::default();

        chat.upsert(1, message(10, "original"));
        chat.upsert(1, message(10, "corrected"));

        let messages = chat.messages_for(1);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "corrected");
    }

    #[test]
    fn a_live_message_moves_its_conversation_to_the_top() {
        let mut chat = ChatState::default();
        chat.set_conversations(vec![summary(1, false), summary(2, false)]);

        chat.note_activity(1, Uuid::nil(), "hello".into(), true);

        let order: Vec<i32> = chat.conversations().iter().map(|c| c.id).collect();
        assert_eq!(order, vec![1, 2]);
        assert_eq!(unread(&chat), 1);
    }

    #[test]
    fn opening_a_conversation_clears_its_badge() {
        let mut chat = ChatState::default();
        chat.set_conversations(vec![summary(1, true), summary(2, true)]);

        chat.mark_read(1);

        assert_eq!(unread(&chat), 1);
        assert!(!chat.conversation(1).expect("conversation").unread);
    }

    #[test]
    fn a_confirmed_send_drops_its_optimistic_row() {
        let mut chat = ChatState::default();
        let key = Uuid::from_u128(7);

        chat.begin_send(1, key, "hi".into());
        assert_eq!(chat.pending_for(1).len(), 1);

        chat.finish_send(1, key, message(5, "hi"));

        assert!(chat.pending_for(1).is_empty());
        assert_eq!(chat.messages_for(1).len(), 1);
    }

    #[test]
    fn a_failed_send_is_kept_so_it_can_be_retried() {
        let mut chat = ChatState::default();
        let key = Uuid::from_u128(7);

        chat.begin_send(1, key, "hi".into());
        chat.fail_send(1, key);

        assert!(chat.pending_for(1)[0].failed);
    }

    #[test]
    fn retrying_clears_the_failed_marker_so_discard_is_out_of_reach() {
        let mut chat = ChatState::default();
        let key = Uuid::from_u128(7);

        chat.begin_send(1, key, "hi".into());
        chat.fail_send(1, key);

        assert_eq!(chat.begin_retry(1, key).as_deref(), Some("hi"));
        assert!(!chat.pending_for(1)[0].failed);
        assert!(chat.begin_retry(1, Uuid::from_u128(8)).is_none());
    }

    #[test]
    fn switching_accounts_drops_the_previous_inbox() {
        let mut chat = ChatState::default();
        let first = Uuid::from_u128(1);
        let second = Uuid::from_u128(2);

        assert!(chat.set_owner(Some(first)));
        chat.set_conversations(vec![summary(1, true)]);
        chat.upsert(1, message(10, "hello"));
        chat.begin_send(1, Uuid::from_u128(9), "draft".into());

        assert!(chat.set_owner(Some(second)));

        assert!(chat.conversations().is_empty());
        assert!(chat.messages_for(1).is_empty());
        assert!(chat.pending_for(1).is_empty());
        assert_eq!(unread(&chat), 0);
        assert_eq!(chat.owner, Some(second));
    }

    #[test]
    fn rechecking_the_same_account_is_not_a_switch() {
        let mut chat = ChatState::default();
        let account = Uuid::from_u128(1);

        assert!(chat.set_owner(Some(account)));
        chat.set_conversations(vec![summary(1, false)]);

        assert!(!chat.set_owner(Some(account)));
        assert_eq!(chat.conversations().len(), 1);
    }

    #[test]
    fn a_failed_load_is_reported_on_the_thread_not_the_inbox() {
        let mut chat = ChatState::default();

        chat.begin_thread_load(1);
        assert_eq!(chat.thread(1).status, ThreadStatus::Loading);

        chat.fail_thread_load(1, "offline".into());
        assert_eq!(chat.thread(1).status, ThreadStatus::Error("offline".into()));
        assert!(chat.error.is_none());

        chat.settle_thread_load(1);
        assert_eq!(chat.thread(1).status, ThreadStatus::Idle);
    }

    #[test]
    fn exhausted_history_latches_per_conversation() {
        let mut chat = ChatState::default();

        assert!(!chat.thread(1).complete);

        chat.set_history_complete(1, true);
        assert!(chat.thread(1).complete);
        assert!(!chat.thread(2).complete);
    }

    #[test]
    fn editing_an_invite_message_updates_its_status() {
        let mut chat = ChatState::default();
        let mut invite = message(10, "");
        invite.invite = Some(MessageInvite::new(4, None));
        chat.upsert(1, invite);

        chat.edit(1, 10, String::new(), Some("accepted".into()));

        let status = chat.messages_for(1)[0]
            .invite
            .as_ref()
            .map(|invite| invite.status.clone());
        assert_eq!(status.as_deref(), Some("accepted"));
    }

    #[test]
    fn an_edit_without_a_status_keeps_the_invite_as_it_was() {
        let mut chat = ChatState::default();
        let mut invite = message(10, "");
        invite.invite = Some(MessageInvite::new(4, Some("pending".into())));
        chat.upsert(1, invite);

        chat.edit(1, 10, "changed".into(), None);

        let message = &chat.messages_for(1)[0];
        assert_eq!(message.content, "changed");
        assert_eq!(
            message.invite.as_ref().map(|i| i.status.as_str()),
            Some("pending")
        );
    }

    #[test]
    fn open_invites_from_others_are_listed_newest_first() {
        let mut chat = ChatState::default();
        let me = Uuid::from_u128(1);
        let friend = Uuid::from_u128(2);
        chat.set_owner(Some(me));

        let invite = |id: i64, sender: Uuid, invite_id: i32, status: &str| ChatMessage {
            id,
            sender,
            content: String::new(),
            sent_at: None,
            edited: false,
            invite: Some(MessageInvite::new(invite_id, Some(status.into()))),
        };

        chat.upsert(7, invite(10, friend, 1, "pending"));
        chat.upsert(7, invite(11, friend, 2, "accepted"));
        chat.upsert(7, invite(12, me, 3, "pending"));
        chat.upsert(8, invite(20, friend, 4, "pending"));
        chat.upsert(8, message(21, "no invite"));

        let ids: Vec<i32> = chat.invites().iter().map(|invite| invite.id).collect();
        assert_eq!(ids, vec![4, 1]);
        assert_eq!(chat.invites()[0].group_id, 8);
    }

    fn settings(
        tos: u32,
        privacy: u32,
        declined: bool,
    ) -> oneclient_core::settings::LauncherSettings {
        oneclient_core::settings::LauncherSettings {
            accepted_tos_version: tos,
            accepted_privacy_version: privacy,
            declined_tos: declined,
            ..Default::default()
        }
    }

    #[test]
    fn chat_needs_both_documents_accepted() {
        assert!(chat_allowed(&settings(1, 1, false), false));
        assert!(!chat_allowed(&settings(0, 0, false), false));
        assert!(!chat_allowed(&settings(1, 0, false), false));
        assert!(!chat_allowed(&settings(0, 1, false), false));
    }

    #[test]
    fn declining_turns_chat_off() {
        assert!(!chat_allowed(&settings(0, 0, true), false));
        assert!(!chat_allowed(&settings(1, 1, false), true));
    }

    #[test]
    fn a_dismissed_invite_leaves_the_list() {
        let mut chat = ChatState::default();
        chat.set_owner(Some(Uuid::from_u128(1)));

        let mut invite = message(10, "");
        invite.sender = Uuid::from_u128(2);
        invite.invite = Some(MessageInvite::new(4, None));
        chat.upsert(7, invite);
        assert_eq!(chat.invites().len(), 1);

        chat.dismiss_invite(4);

        assert!(chat.invites().is_empty());
        assert!(chat.messages_for(7)[0].invite.is_some());
    }

    #[test]
    fn the_inbox_is_loading_until_its_first_list_arrives() {
        let mut chat = ChatState::default();
        assert!(chat.inbox().loading);

        chat.set_conversations(Vec::new());
        assert!(!chat.inbox().loading);
    }

    #[test]
    fn a_failed_first_load_is_not_reported_as_loading() {
        let chat = ChatState {
            status: AsyncStatus::Error,
            error: Some("offline".into()),
            ..ChatState::default()
        };

        assert!(!chat.inbox().loading);
    }

    #[test]
    fn reloading_the_newest_page_keeps_older_history_and_live_messages() {
        let mut chat = ChatState::default();
        chat.upsert(1, message(5, "older page"));
        chat.upsert(1, message(20, "stale copy"));
        chat.upsert(1, message(25, "deleted meanwhile"));
        chat.upsert(1, message(40, "arrived live"));

        chat.set_messages(
            1,
            vec![
                fetched(10, "newest page", "2026-01-01T00:00:00Z"),
                fetched(20, "fresh copy", "2026-01-01T00:01:00Z"),
                fetched(30, "newest page", "2026-01-01T00:02:00Z"),
            ],
            Some(25),
        );

        let messages = chat.messages_for(1);
        let ids: Vec<i64> = messages.iter().map(|m| m.id).collect();
        assert_eq!(ids, vec![5, 10, 20, 30, 40]);
        assert_eq!(messages[2].content, "fresh copy");
    }

    #[test]
    fn a_page_that_skips_messages_replaces_the_older_history() {
        let mut chat = ChatState::default();
        chat.upsert(1, message(10, "before the gap"));
        chat.upsert(1, message(100, "before the gap"));

        chat.set_messages(
            1,
            vec![
                fetched(131, "after the gap", "2026-01-01T00:00:00Z"),
                fetched(180, "after the gap", "2026-01-01T00:01:00Z"),
            ],
            chat.newest_message(1),
        );

        assert_eq!(
            chat.messages_for(1)
                .iter()
                .map(|m| m.id)
                .collect::<Vec<_>>(),
            vec![131, 180]
        );
    }

    #[test]
    fn a_live_message_cannot_hide_a_reconnect_gap() {
        let mut chat = ChatState::default();
        chat.set_messages(
            1,
            vec![
                fetched(1, "before the gap", "2026-01-01T00:00:00Z"),
                fetched(50, "before the gap", "2026-01-01T00:01:00Z"),
            ],
            None,
        );
        chat.upsert(1, message(300, "arrived live"));

        chat.set_messages(
            1,
            vec![
                fetched(251, "after the gap", "2026-01-01T00:02:00Z"),
                fetched(300, "after the gap", "2026-01-01T00:03:00Z"),
            ],
            chat.synced_to(1),
        );

        assert_eq!(
            chat.messages_for(1)
                .iter()
                .map(|m| m.id)
                .collect::<Vec<_>>(),
            vec![251, 300]
        );
        assert_eq!(chat.synced_to(1), Some(300));
    }

    #[test]
    fn blocked_senders_raise_no_toast() {
        let mut chat = ChatState::default();
        let incoming = message(1, "hello");
        chat.set_roster(ChatRoster {
            blocked: vec![oneclient_polyplus::BlockedPlayer {
                player: incoming.sender,
                since: DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").expect("timestamp"),
            }],
            ..ChatRoster::default()
        });

        let received = chat.receive(1, incoming, false, SocialTab::Chat);

        assert!(received.toast.is_none());
    }

    #[test]
    fn reloading_the_newest_page_drops_messages_deleted_while_away() {
        let mut chat = ChatState::default();
        chat.upsert(1, message(10, "kept"));
        chat.upsert(1, message(50, "deleted while away"));

        chat.set_messages(
            1,
            vec![fetched(10, "kept", "2026-01-01T00:00:00Z")],
            chat.newest_message(1),
        );
        assert_eq!(
            chat.messages_for(1)
                .iter()
                .map(|m| m.id)
                .collect::<Vec<_>>(),
            vec![10]
        );

        chat.upsert(1, message(60, "deleted too"));
        chat.set_messages(1, Vec::new(), Some(60));
        assert!(chat.messages_for(1).is_empty());
    }

    #[test]
    fn a_fresh_roster_overrides_old_presence() {
        let mut chat = ChatState::default();
        let friend = Uuid::from_u128(2);
        chat.set_presence(friend, true);

        chat.set_roster(ChatRoster {
            friends: vec![Friend {
                player: friend,
                kind: oneclient_polyplus::RelationshipKind::Friend,
                since: DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").expect("timestamp"),
                online: false,
            }],
            ..ChatRoster::default()
        });

        assert!(!chat.is_online(friend));
    }

    #[test]
    fn a_friend_request_from_an_existing_friend_is_hidden() {
        let since = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").expect("timestamp");
        let request = |id: i32, player: Uuid| oneclient_polyplus::FriendRequest {
            id,
            player,
            created_at: since,
        };
        let friend = Uuid::from_u128(2);
        let stranger = Uuid::from_u128(3);

        let mut chat = ChatState::default();
        chat.set_roster(ChatRoster {
            friends: vec![Friend {
                player: friend,
                kind: oneclient_polyplus::RelationshipKind::Friend,
                since,
                online: false,
            }],
            requests: FriendRequests {
                incoming: vec![request(1, friend), request(2, stranger)],
                outgoing: Vec::new(),
            },
            ..ChatRoster::default()
        });

        let incoming = chat.roster().requests.incoming;
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].player, stranger);
        assert!(chat.is_friend(friend));
        assert!(!chat.is_friend(stranger));
    }

    #[test]
    fn only_unknown_players_need_their_names_resolved() {
        let friend = Uuid::from_u128(2);
        let known = Uuid::from_u128(3);
        let since = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").expect("timestamp");
        let friend_of = |player| Friend {
            player,
            kind: oneclient_polyplus::RelationshipKind::Friend,
            since,
            online: false,
        };

        let mut chat = ChatState::default();
        chat.set_roster(ChatRoster {
            friends: vec![friend_of(friend), friend_of(known)],
            ..ChatRoster::default()
        });
        chat.add_names([(known, "Known".to_string())]);

        assert_eq!(chat.unnamed_players(), vec![friend]);
    }

    fn shared_group(id: i32, members: Vec<Uuid>) -> GroupSummary {
        GroupSummary {
            kind: GroupKind::Group,
            members,
            ..summary(id, false)
        }
    }

    #[test]
    fn the_online_count_includes_you_unless_you_are_invisible() {
        let me = Uuid::from_u128(1);
        let other = Uuid::from_u128(2);
        let members = [me, other];

        let mut chat = ChatState::default();
        chat.owner = Some(me);
        assert_eq!(chat.online_count(&members), 0);

        chat.connected = true;
        assert_eq!(chat.online_count(&members), 1);

        chat.set_presence(other, true);
        assert_eq!(chat.online_count(&members), 2);

        chat.self_status = PlayerStatus::Offline;
        assert_eq!(chat.online_count(&members), 1);
    }

    #[test]
    fn group_members_are_watched_but_not_you_or_direct_messages() {
        let me = Uuid::from_u128(1);
        let member = Uuid::from_u128(2);
        let direct = Uuid::from_u128(3);

        let mut chat = ChatState::default();
        chat.owner = Some(me);
        chat.set_conversations(vec![
            shared_group(1, vec![me, member]),
            GroupSummary {
                members: vec![me, direct],
                ..summary(2, false)
            },
        ]);

        assert_eq!(chat.watched_players(), vec![member]);
    }

    #[test]
    fn a_roster_reload_keeps_the_presence_of_non_friends() {
        let member = Uuid::from_u128(2);
        let mut chat = ChatState::default();
        chat.set_presence(member, true);

        chat.set_roster(ChatRoster::default());
        assert!(chat.is_online(member));

        chat.clear_presence();
        assert!(!chat.is_online(member));
    }

    #[test]
    fn a_failed_decline_brings_the_invite_back() {
        let mut chat = ChatState::default();
        chat.set_conversations(vec![summary(1, false)]);
        let mut invite = message(10, "");
        invite.invite = Some(MessageInvite::new(4, None));
        chat.upsert(1, invite);
        assert_eq!(chat.invites().len(), 1);

        chat.dismiss_invite(4);
        assert!(chat.invites().is_empty());

        chat.restore_invite(4);
        assert_eq!(chat.invites().len(), 1);
    }

    #[test]
    fn leaving_a_group_forgets_it_entirely() {
        let mut chat = ChatState::default();
        chat.set_conversations(vec![summary(1, false), summary(2, false)]);
        chat.upsert(1, message(10, "hello"));
        chat.begin_send(1, Uuid::from_u128(9), "draft".into());
        chat.active = Some(1);

        chat.forget_conversation(1);

        let ids: Vec<i32> = chat.conversations().iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![2]);
        assert!(!chat.has_messages(1));
        assert!(chat.pending_for(1).is_empty());
        assert_eq!(chat.active, None);
    }

    #[test]
    fn a_group_that_leaves_the_list_is_forgotten() {
        let mut chat = ChatState::default();
        chat.set_conversations(vec![summary(1, false), summary(2, false)]);
        chat.upsert(1, message(10, "hello"));
        chat.active = Some(1);

        chat.set_conversations(vec![summary(2, false)]);

        assert_eq!(chat.active, None);
        assert_eq!(chat.active_gone, 1);
        assert!(!chat.has_messages(1));
    }

    #[test]
    fn switching_owner_closes_the_open_thread() {
        let mut chat = ChatState {
            active: Some(1),
            active_opened: 3,
            ..ChatState::default()
        };

        chat.set_owner(Some(Uuid::from_u128(1)));

        assert_eq!(chat.active, None);
        assert_eq!(chat.active_gone, 1);
        assert_eq!(chat.active_opened, 3);
    }

    #[test]
    fn a_selected_group_not_listed_yet_survives_a_refresh() {
        let mut chat = ChatState::default();
        chat.set_conversations(vec![summary(1, false)]);
        chat.active = Some(2);

        chat.set_conversations(vec![summary(1, false)]);

        assert_eq!(chat.active, Some(2));
        assert_eq!(chat.active_gone, 0);
    }

    #[test]
    fn paging_backwards_merges_without_duplicating() {
        let mut chat = ChatState::default();
        chat.upsert(1, message(30, "newest"));

        chat.prepend_messages(
            1,
            vec![
                fetched(10, "older", "2026-01-01T00:00:00Z"),
                fetched(30, "newest", "2026-01-02T00:00:00Z"),
            ],
        );

        let ids: Vec<i64> = chat.messages_for(1).iter().map(|m| m.id).collect();
        assert_eq!(ids, vec![10, 30]);
    }
}
