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
    Party,
    Unknown,
}

impl GroupKind {
    #[must_use]
    pub fn is_shared(self) -> bool {
        matches!(self, Self::Group | Self::Party)
    }
}

impl<'de> Deserialize<'de> for GroupKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match String::deserialize(deserializer)?.as_str() {
            "dm" => Self::Dm,
            "group" => Self::Group,
            "party" => Self::Party,
            _ => Self::Unknown,
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerStatus {
    #[default]
    Online,
    Offline,
}

impl<'de> Deserialize<'de> for PlayerStatus {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match String::deserialize(deserializer)?.as_str() {
            "offline" => Self::Offline,
            _ => Self::Online,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct StatusBody {
    pub status: PlayerStatus,
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
pub struct IncomingSessionInvite {
    pub id: i32,
    #[serde(default)]
    pub minecraft_version: Option<String>,
    #[serde(default)]
    pub loader: Option<String>,
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
    fn status_body_round_trips_with_the_server_shape() {
        let sent = serde_json::to_value(StatusBody {
            status: PlayerStatus::Offline,
        })
        .expect("status should serialize");
        assert_eq!(sent, serde_json::json!({ "status": "offline" }));

        let online: StatusBody =
            serde_json::from_str(r#"{"status":"online"}"#).expect("online should parse");
        let unknown: StatusBody =
            serde_json::from_str(r#"{"status":"away"}"#).expect("unknown should parse");
        assert_eq!(online.status, PlayerStatus::Online);
        assert_eq!(unknown.status, PlayerStatus::Online);
    }

    #[test]
    fn party_chats_are_their_own_shared_kind() {
        let summary: GroupSummary =
            serde_json::from_str(r#"{"id":1,"kind":"party","members":[],"unread":false}"#)
                .expect("a party chat should deserialize");

        assert_eq!(summary.kind, GroupKind::Party);
        assert!(summary.kind.is_shared());
        assert!(GroupKind::Group.is_shared());
        assert!(!GroupKind::Dm.is_shared());
        assert!(!GroupKind::Unknown.is_shared());
    }

    #[test]
    fn incoming_invites_read_the_game_and_tolerate_nulls() {
        let invites: Vec<IncomingSessionInvite> = serde_json::from_str(
            r#"[
                {"id":1,"session_id":"0192f0d0-0000-7000-8000-000000000001","sender":"0192f0d0-0000-7000-8000-000000000002","eos_product_user_id":null,"eos_session_id":"x","created_at":"2026-11-04T12:00:00Z","status":"pending","minecraft_version":"1.21.1","loader":"neoforge"},
                {"id":2,"minecraft_version":null,"loader":null}
            ]"#,
        )
        .expect("invites should parse");

        assert_eq!(invites[0].minecraft_version.as_deref(), Some("1.21.1"));
        assert_eq!(invites[0].loader.as_deref(), Some("neoforge"));
        assert_eq!(invites[1].minecraft_version, None);
    }

    #[test]
    fn unknown_group_kinds_do_not_fail_the_list() {
        let summary: GroupSummary =
            serde_json::from_str(r#"{"id":1,"kind":"broadcast","members":[],"unread":false}"#)
                .expect("an unrecognised kind should still deserialize");

        assert_eq!(summary.kind, GroupKind::Unknown);
    }
}
