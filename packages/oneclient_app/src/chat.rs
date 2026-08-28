use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, FixedOffset, Utc};
use uuid::Uuid;

use oneclient_polyplus::{GroupKind, GroupMessage, GroupSummary};

use crate::state::AsyncStatus;

#[derive(Clone, Debug, PartialEq)]
pub struct ChatConversation {
    pub id: i32,
    pub kind: GroupKind,
    pub name: Option<String>,
    pub members: Vec<Uuid>,
    pub unread: bool,
    pub special: bool,
    pub preview: Option<String>,
    pub last_activity: Option<DateTime<FixedOffset>>,
}

impl From<GroupSummary> for ChatConversation {
    fn from(value: GroupSummary) -> Self {
        let (preview, last_activity) = match value.last_message {
            Some(last) => (Some(last.content), Some(last.sent_at)),
            None => (None, None),
        };

        Self {
            id: value.id,
            kind: value.kind,
            name: value.name,
            members: value.members,
            unread: value.unread,
            special: value.special,
            preview,
            last_activity,
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
}

impl From<GroupMessage> for ChatMessage {
    fn from(value: GroupMessage) -> Self {
        Self {
            id: value.id,
            sender: value.sender,
            content: value.content,
            sent_at: Some(value.sent_at),
            edited: value.edited_at.is_some(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PendingMessage {
    pub key: Uuid,
    pub content: String,
    pub failed: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChatState {
    pub connected: bool,
    pub status: AsyncStatus,
    pub error: Option<String>,
    pub active: Option<i32>,
    pub open_request: Option<i32>,
    pub owner: Option<Uuid>,
    conversations: Vec<ChatConversation>,
    messages: HashMap<i32, Arc<Vec<ChatMessage>>>,
    pending: HashMap<i32, Vec<PendingMessage>>,
    presence: HashMap<Uuid, bool>,
}

impl ChatState {
    #[must_use]
    pub fn conversations(&self) -> &[ChatConversation] {
        &self.conversations
    }

    #[must_use]
    pub fn conversation(&self, group_id: i32) -> Option<&ChatConversation> {
        self.conversations
            .iter()
            .find(|conversation| conversation.id == group_id)
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
    pub fn unread_count(&self) -> usize {
        self.conversations
            .iter()
            .filter(|conversation| conversation.unread)
            .count()
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
        self.conversations = summaries.into_iter().map(ChatConversation::from).collect();
        self.sort();
        self.status = AsyncStatus::Ready;
        self.error = None;
    }

    pub fn set_messages(&mut self, group_id: i32, page: Vec<GroupMessage>) {
        let mut messages: Vec<ChatMessage> = page.into_iter().map(ChatMessage::from).collect();
        messages.sort_by_key(|message| message.id);
        messages.dedup_by_key(|message| message.id);
        self.messages.insert(group_id, Arc::new(messages));
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

    pub fn edit(&mut self, group_id: i32, message_id: i64, content: String) {
        let Some(bucket) = self.messages.get_mut(&group_id) else {
            return;
        };

        if let Some(message) = Arc::make_mut(bucket)
            .iter_mut()
            .find(|message| message.id == message_id)
        {
            message.content = content;
            message.edited = true;
        }
    }

    pub fn remove(&mut self, group_id: i32, message_id: i64) {
        let Some(bucket) = self.messages.get_mut(&group_id) else {
            return;
        };

        Arc::make_mut(bucket).retain(|message| message.id != message_id);
    }

    pub fn note_activity(&mut self, group_id: i32, preview: String, unread: bool) {
        let now = Utc::now().fixed_offset();

        if let Some(conversation) = self
            .conversations
            .iter_mut()
            .find(|conversation| conversation.id == group_id)
        {
            conversation.preview = Some(preview);
            conversation.last_activity = Some(now);
            conversation.unread |= unread;
        }

        self.sort();
    }

    #[must_use]
    pub fn is_online(&self, player: Uuid) -> bool {
        self.presence.get(&player).copied().unwrap_or(false)
    }

    pub fn set_presence(&mut self, player: Uuid, online: bool) {
        self.presence.insert(player, online);
    }

    pub fn take_open_request(&mut self) -> Option<i32> {
        self.open_request.take()
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
        *self = Self::default();
        self.connected = connected;
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

        chat.note_activity(1, "hello".into(), true);

        let order: Vec<i32> = chat.conversations().iter().map(|c| c.id).collect();
        assert_eq!(order, vec![1, 2]);
        assert_eq!(chat.unread_count(), 1);
    }

    #[test]
    fn opening_a_conversation_clears_its_badge() {
        let mut chat = ChatState::default();
        chat.set_conversations(vec![summary(1, true), summary(2, true)]);

        chat.mark_read(1);

        assert_eq!(chat.unread_count(), 1);
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
        assert_eq!(chat.unread_count(), 0);
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
