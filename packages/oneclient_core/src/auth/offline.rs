use chrono::{Duration, Utc};
use md5::{Digest, Md5};
use uuid::Uuid;

use super::error::AuthError;
use super::data::{AccountKind, MinecraftAccount};
use crate::LauncherResult;

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

pub fn validate_offline_username(username: &str) -> LauncherResult<()> {
    let len = username.chars().count();
    if !(3..=16).contains(&len) {
        return Err(AuthError::InvalidOfflineUsername {
            reason: "username must be 3-16 characters".into(),
        }
        .into());
    }

    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(AuthError::InvalidOfflineUsername {
            reason: "username may only contain letters, digits, and underscores".into(),
        }
        .into());
    }

    Ok(())
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
    }
}
