use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountKind {
    Microsoft,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftAccount {
    pub id: Uuid,
    pub username: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires: DateTime<Utc>,
    #[serde(default = "default_account_kind")]
    pub kind: AccountKind,
}

fn default_account_kind() -> AccountKind {
    AccountKind::Microsoft
}

impl MinecraftAccount {
    pub fn is_microsoft(&self) -> bool {
        self.kind == AccountKind::Microsoft
    }

    pub fn is_offline(&self) -> bool {
        self.kind == AccountKind::Offline
    }

    pub fn is_expired(&self) -> bool {
        self.expires <= Utc::now() + chrono::TimeDelta::seconds(60)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceCodeLogin {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BrowserLogin {
    pub auth_url: String,
    pub state: String,
    pub redirect_uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MicrosoftLoginSession {
    pub browser: BrowserLogin,
    pub device: DeviceCodeLogin,
}

impl MicrosoftLoginSession {
    pub fn dedupe_key(&self) -> &str {
        &self.browser.state
    }

    pub fn auth_url(&self) -> &str {
        &self.browser.auth_url
    }

    pub fn user_code(&self) -> &str {
        &self.device.user_code
    }

    pub fn verification_uri(&self) -> &str {
        &self.device.verification_uri
    }
}
