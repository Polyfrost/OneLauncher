use chrono::{Duration, Utc};
use md5::{Digest, Md5};
use uuid::Uuid;

use super::data::{AccountKind, MinecraftAccount};

pub fn offline_uuid(username: &str) -> Uuid {
    let mut hasher = Md5::new();
    hasher.update(format!("OfflinePlayer:{username}").as_bytes());
    let digest = hasher.finalize();

    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest);
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

/// Fatal username problems. Only a name that cannot identify a player at all
/// (empty or blank) blocks creation, everything else is a warning.
pub fn offline_username_error(username: &str) -> Option<String> {
    if username.trim().is_empty() {
        return Some("username cannot be empty".into());
    }

    None
}

/// Non-fatal username problems. Vanilla profiles can never carry these names
/// so online servers will reject them, but they are still useful for local
/// testing and custom servers, so the launcher warns instead of refusing.
pub fn offline_username_warning(username: &str) -> Option<String> {
    let mut reasons: Vec<String> = Vec::new();

    let len = username.chars().count();
    if !(3..=16).contains(&len) {
        let reason = if len < 3 {
            "shorter than 3 characters"
        } else {
            "longer than 16 characters"
        };
        reasons.push(reason.to_string());
    }

    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        reasons.push("contains characters outside letters, digits, and underscores".to_string());
    }

    if username != username.trim() {
        reasons.push("has leading or trailing whitespace".to_string());
    }

    if reasons.is_empty() {
        None
    } else {
        Some(reasons.join(", "))
    }
}

#[tracing::instrument(level = "debug", fields(username = %username))]
pub fn offline_account(username: String) -> MinecraftAccount {
    tracing::info!("creating offline account");
    MinecraftAccount {
        id: offline_uuid(&username),
        username,
        access_token: String::new(),
        refresh_token: String::new(),
        expires: Utc::now() + Duration::days(3650),
        kind: AccountKind::Offline,
        signed_out: false,
        appearance: None,
    }
}
