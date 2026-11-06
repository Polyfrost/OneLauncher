use std::sync::LazyLock;

use chrono::{DateTime, Utc};
use reqwest::{Method, StatusCode};
use serde::Deserialize;
use tokio::sync::broadcast;
use uuid::Uuid;

use oneclient_auth::MinecraftAccount;

use crate::PlusError;
use crate::api::{json_authorized, send_authorized};

pub const MAX_PARTY_SIZE: usize = 8;

static EVENTS: LazyLock<broadcast::Sender<LiveEvent>> =
    LazyLock::new(|| broadcast::channel(64).0);

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Party {
    pub id: Uuid,
    pub leader: Uuid,
    pub settings: PartySettings,
    #[serde(default)]
    pub chat_group_id: Option<i32>,
    pub members: Vec<PartyMember>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PartySettings {
    pub invite_policy: InvitePolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvitePolicy {
    Leader,
    Members,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PartyMember {
    pub player: Uuid,
    pub joined_at: DateTime<Utc>,
    pub online: bool,
    #[serde(default)]
    pub state: Option<MemberState>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MemberState {
    pub location: MemberLocation,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MemberLocation {
    pub kind: LocationKind,
    #[serde(default)]
    pub display: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocationKind {
    Menu,
    Singleplayer,
    Server,
    HostedWorld,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PartyInvite {
    pub id: Uuid,
    pub party_id: Uuid,
    pub sender: Uuid,
    pub recipient: Uuid,
    pub status: InviteStatus,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub party_size: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InviteStatus {
    Pending,
    Accepted,
    Declined,
    Cancelled,
    Expired,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeftReason {
    Left,
    Kicked,
    Disbanded,
    Expired,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "PascalCase")]
pub enum PartyPacket {
    PartyUpdated {
        party: Party,
    },
    PartyMemberStateChanged {
        party_id: Uuid,
        player: Uuid,
        state: MemberState,
    },
    PartyLeft {
        party_id: Uuid,
        reason: LeftReason,
    },
    PartyInviteReceived {
        invite: PartyInvite,
    },
    PartyInviteUpdated {
        invite_id: Uuid,
        status: InviteStatus,
    },
    FriendRemoved {
        player: Uuid,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum LiveEvent {
    Connected,
    Packet(Box<PartyPacket>),
}

pub fn subscribe() -> broadcast::Receiver<LiveEvent> {
    EVENTS.subscribe()
}

pub(crate) fn connected() {
    let _ = EVENTS.send(LiveEvent::Connected);
}

pub(crate) fn dispatch(text: &str) {
    if let Ok(packet) = serde_json::from_str::<PartyPacket>(text) {
        let _ = EVENTS.send(LiveEvent::Packet(Box::new(packet)));
    }
}

pub fn error_message(err: &PlusError) -> Option<String> {
    #[derive(Deserialize)]
    struct Body {
        message: String,
    }

    match err {
        PlusError::Api { status, body } if *status < 500 => serde_json::from_str::<Body>(body)
            .ok()
            .map(|body| body.message)
            .filter(|message| !message.trim().is_empty()),
        _ => None,
    }
}

pub async fn current(account: &MinecraftAccount) -> Result<Option<Party>, PlusError> {
    let response = send_authorized(account, Method::GET, "/v1/party", None).await?;
    if response.status() == StatusCode::NO_CONTENT {
        return Ok(None);
    }
    Ok(Some(response.json().await?))
}

pub async fn create(account: &MinecraftAccount) -> Result<Party, PlusError> {
    json_authorized(
        account,
        Method::POST,
        "/v1/party",
        Some(&serde_json::json!({})),
    )
    .await
}

pub async fn leave(account: &MinecraftAccount) -> Result<(), PlusError> {
    send_authorized(account, Method::POST, "/v1/party/leave", None).await?;
    Ok(())
}

pub async fn leave_for_sign_out(account: &MinecraftAccount) -> Result<(), PlusError> {
    if !crate::session::usable(account, crate::social_allowed()) {
        return Ok(());
    }

    if current(account).await?.is_some() {
        leave(account).await?;
    }
    Ok(())
}

pub async fn disband(account: &MinecraftAccount) -> Result<(), PlusError> {
    send_authorized(account, Method::DELETE, "/v1/party", None).await?;
    Ok(())
}

pub async fn transfer_leader(account: &MinecraftAccount, player: Uuid) -> Result<Party, PlusError> {
    json_authorized(
        account,
        Method::POST,
        "/v1/party/leader",
        Some(&serde_json::json!({ "player": player })),
    )
    .await
}

pub async fn kick(account: &MinecraftAccount, player: Uuid) -> Result<(), PlusError> {
    let path = format!("/v1/party/members/{player}");
    send_authorized(account, Method::DELETE, &path, None).await?;
    Ok(())
}

pub async fn invite(account: &MinecraftAccount, player: Uuid) -> Result<PartyInvite, PlusError> {
    json_authorized(
        account,
        Method::POST,
        "/v1/party/invites",
        Some(&serde_json::json!({ "player": player })),
    )
    .await
}

pub async fn incoming(account: &MinecraftAccount) -> Result<Vec<PartyInvite>, PlusError> {
    json_authorized(account, Method::GET, "/v1/party/invites/incoming", None).await
}

pub async fn outgoing(account: &MinecraftAccount) -> Result<Vec<PartyInvite>, PlusError> {
    json_authorized(account, Method::GET, "/v1/party/invites/outgoing", None).await
}

pub async fn accept(
    account: &MinecraftAccount,
    invite: Uuid,
    leave_current: bool,
) -> Result<Party, PlusError> {
    let path = format!("/v1/party/invites/{invite}/accept");
    json_authorized(
        account,
        Method::POST,
        &path,
        Some(&serde_json::json!({ "leave_current": leave_current })),
    )
    .await
}

pub async fn decline(account: &MinecraftAccount, invite: Uuid) -> Result<(), PlusError> {
    let path = format!("/v1/party/invites/{invite}/decline");
    send_authorized(account, Method::POST, &path, None).await?;
    Ok(())
}

pub async fn cancel_invite(account: &MinecraftAccount, invite: Uuid) -> Result<(), PlusError> {
    let path = format!("/v1/party/invites/{invite}");
    send_authorized(account, Method::DELETE, &path, None).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_party_updated_from_the_backend_shape() {
        let text = r#"{
            "type": "PartyUpdated",
            "party": {
                "id": "0192f0d0-0000-7000-8000-000000000001",
                "leader": "4b8c6a1e-2f3d-4e5a-9b7c-1d2e3f4a5b61",
                "created_at": "2026-11-02T18:12:30Z",
                "settings": { "voice_mode": "party", "invite_policy": "members" },
                "chat_group_id": null,
                "voice": { "room_name": "party-x", "transport": "rtc", "rtc_available": false },
                "members": [{
                    "player": "4b8c6a1e-2f3d-4e5a-9b7c-1d2e3f4a5b61",
                    "joined_at": "2026-11-02T18:12:30Z",
                    "online": true,
                    "eos_product_user_id": null,
                    "voice_hard_muted": false,
                    "state": {
                        "location": { "kind": "server", "key": "hypixel", "display": "Hypixel" },
                        "voice": {},
                        "svc_connected": false,
                        "scene": null
                    }
                }]
            }
        }"#;

        let PartyPacket::PartyUpdated { party } = serde_json::from_str(text).expect("parses")
        else {
            panic!("wrong packet");
        };
        assert_eq!(party.settings.invite_policy, InvitePolicy::Members);
        assert_eq!(party.chat_group_id, None);
        let location = &party.members[0].state.as_ref().expect("state").location;
        assert_eq!(location.kind, LocationKind::Server);
        assert_eq!(location.display.as_deref(), Some("Hypixel"));
    }

    #[test]
    fn reads_the_party_chat_group() {
        let party: Party = serde_json::from_str(
            r#"{
                "id": "0192f0d0-0000-7000-8000-000000000001",
                "leader": "4b8c6a1e-2f3d-4e5a-9b7c-1d2e3f4a5b61",
                "settings": { "invite_policy": "leader" },
                "chat_group_id": 38,
                "members": []
            }"#,
        )
        .expect("parses");
        assert_eq!(party.chat_group_id, Some(38));
    }

    #[test]
    fn ignores_unrelated_packets_and_tolerates_new_variants() {
        assert!(serde_json::from_str::<PartyPacket>(r#"{"type":"StopEmote"}"#).is_err());
        let left: PartyPacket = serde_json::from_str(
            r#"{"type":"PartyLeft","party_id":"0192f0d0-0000-7000-8000-000000000001","reason":"something_new"}"#,
        )
        .expect("parses");
        assert!(matches!(
            left,
            PartyPacket::PartyLeft {
                reason: LeftReason::Other,
                ..
            }
        ));
    }

    #[test]
    fn reads_the_backend_error_message() {
        let err = PlusError::Api {
            status: 409,
            body: r#"{"error":"party_full","message":"That party is full"}"#.into(),
        };
        assert_eq!(error_message(&err).as_deref(), Some("That party is full"));
        let err = PlusError::Api {
            status: 500,
            body: r#"{"error":"internal","message":"x"}"#.into(),
        };
        assert_eq!(error_message(&err), None);
    }
}
