use uuid::Uuid;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ClientboundPacket {
    PlayerPresence {
        player: Uuid,
        online: bool,
    },

    SubscriptionSnapshot {
        #[serde(default)]
        users: Vec<Uuid>,
    },

    FriendRequestReceived {
        request_id: i32,
        sender: Uuid,
    },

    FriendRequestUpdated {
        request_id: i32,
        status: String,
    },

    FriendRemoved {
        player: Uuid,
    },

    GroupMessageReceived {
        group_id: i32,
        message_id: i64,
        sender: Uuid,
        content: String,
        #[serde(default)]
        session_invite_id: Option<i32>,
        #[serde(default)]
        session_invite_status: Option<String>,
    },

    GroupMessageEdited {
        group_id: i32,
        message_id: i64,
        content: String,
        #[serde(default)]
        session_invite_status: Option<String>,
    },

    GroupMessageDeleted {
        group_id: i32,
        message_id: i64,
    },

    Error {
        error_code: String,
        message: String,
        #[serde(default)]
        request_id: Option<u64>,
    },

    #[serde(other)]
    Unhandled,
}

#[derive(Debug, serde::Serialize)]
#[serde(tag = "type")]
pub(crate) enum ServerboundPacket<'a> {
    SubscribePlayers { players: &'a [Uuid] },
    UnsubscribePlayers { players: &'a [Uuid] },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscribing_matches_the_server_shape() {
        let player = Uuid::from_u128(1);
        let packet =
            serde_json::to_value(ServerboundPacket::SubscribePlayers { players: &[player] })
                .expect("packet should serialize");

        assert_eq!(
            packet,
            serde_json::json!({ "type": "SubscribePlayers", "players": [player] })
        );
    }

    #[test]
    fn parses_a_group_message() {
        let sender = Uuid::nil();
        let packet: ClientboundPacket = serde_json::from_str(&format!(
            r#"{{"type":"GroupMessageReceived","group_id":7,"message_id":42,"sender":"{sender}","content":"hi","session_invite_id":null,"session_invite_status":null}}"#
        ))
        .expect("packet should parse");

        match packet {
            ClientboundPacket::GroupMessageReceived {
                group_id,
                message_id,
                content,
                ..
            } => {
                assert_eq!(group_id, 7);
                assert_eq!(message_id, 42);
                assert_eq!(content, "hi");
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn cosmetic_packets_do_not_break_the_stream() {
        let packet: ClientboundPacket = serde_json::from_str(
            r#"{"type":"PlayerEmoteStarted","player":"00000000-0000-0000-0000-000000000000","emote_id":3}"#,
        )
        .expect("unknown packets should fall back rather than fail");

        assert!(matches!(packet, ClientboundPacket::Unhandled));
    }

    #[test]
    fn a_subscription_snapshot_lists_the_live_players() {
        let player = Uuid::from_u128(5);
        let packet: ClientboundPacket = serde_json::from_str(&format!(
            r#"{{"type":"SubscriptionSnapshot","equipped":{{}},"active_emotes":{{}},"particle_colors":{{}},"users":["{player}"],"rejected":[],"request_id":null}}"#
        ))
        .expect("packet should parse");

        assert!(matches!(
            packet,
            ClientboundPacket::SubscriptionSnapshot { users } if users == vec![player]
        ));
    }

    #[test]
    fn parses_a_server_error() {
        let packet: ClientboundPacket = serde_json::from_str(
            r#"{"type":"Error","error_code":"bad_request","message":"nope","request_id":null}"#,
        )
        .expect("packet should parse");

        assert!(matches!(packet, ClientboundPacket::Error { .. }));
    }
}
