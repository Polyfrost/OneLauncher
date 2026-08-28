use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

pub const MAX_MESSAGE_LENGTH: usize = 512;
pub const MAX_PAGE_SIZE: u64 = 200;
pub const MAX_RESOLVE_BATCH: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GroupKind {
    Dm,
    Group,
    Unknown,
}

impl<'de> Deserialize<'de> for GroupKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match String::deserialize(deserializer)?.as_str() {
            "dm" => Self::Dm,
            "group" => Self::Group,
            _ => Self::Unknown,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationshipKind {
    Friend,
    BestFriend,
    Unknown,
}

impl<'de> Deserialize<'de> for RelationshipKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match String::deserialize(deserializer)?.as_str() {
            "friend" => Self::Friend,
            "best_friend" => Self::BestFriend,
            _ => Self::Unknown,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LastMessage {
    pub content: String,
    pub sender: Uuid,
    pub sent_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct GroupSummary {
    pub id: i32,
    pub kind: GroupKind,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub members: Vec<Uuid>,
    #[serde(default)]
    pub last_message: Option<LastMessage>,
    #[serde(default)]
    pub unread: bool,
    #[serde(default)]
    pub special: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SessionInvite {
    pub id: i32,
    pub session_id: Uuid,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct GroupMessage {
    pub id: i64,
    pub sender: Uuid,
    pub content: String,
    pub sent_at: DateTime<FixedOffset>,
    #[serde(default)]
    pub edited_at: Option<DateTime<FixedOffset>>,
    #[serde(default)]
    pub session_invite: Option<SessionInvite>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Friend {
    pub player: Uuid,
    pub kind: RelationshipKind,
    pub since: DateTime<FixedOffset>,
    pub online: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FriendRequest {
    pub id: i32,
    pub player: Uuid,
    pub created_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BlockedPlayer {
    pub player: Uuid,
    pub since: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ResolvedPlayer {
    pub id: Uuid,
    pub username: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SpecialChatStatus {
    pub is_special_chat_target: bool,
    #[serde(default)]
    pub group_id: Option<i32>,
    #[serde(default)]
    pub cooldown_until: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SendMessageRequest<'a> {
    pub content: &'a str,
    pub idempotency_key: Uuid,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct EditMessageRequest<'a> {
    pub content: &'a str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CreateGroupRequest<'a> {
    pub name: &'a str,
    pub members: &'a [Uuid],
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ResolveRequest<'a> {
    pub ids: &'a [Uuid],
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) struct ResolveResponse {
    pub players: Vec<ResolvedPlayer>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_group_kinds_do_not_fail_the_list() {
        let summary: GroupSummary = serde_json::from_str(
            r#"{"id":1,"kind":"broadcast","members":[],"unread":false,"special":false}"#,
        )
        .expect("an unrecognised kind should still deserialize");

        assert_eq!(summary.kind, GroupKind::Unknown);
    }

    #[test]
    fn group_summaries_tolerate_a_missing_special_flag() {
        let summary: GroupSummary =
            serde_json::from_str(r#"{"id":4,"kind":"dm","members":[],"unread":true}"#)
                .expect("special is newer than the rest of the shape");

        assert_eq!(summary.kind, GroupKind::Dm);
        assert!(summary.unread);
        assert!(!summary.special);
    }
}
