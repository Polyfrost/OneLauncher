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
    /// Microsoft rejected the refresh token the entry stays listed but cannot
    /// launch until the user signs in to it again
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub signed_out: bool,
    /// Custom cosmetic data, used by offline accounts. Offline profiles have
    /// no session-server textures so the launcher stores and renders these
    /// itself. Microsoft accounts keep using the real Mojang wardrobe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appearance: Option<AccountAppearance>,
}

/// Custom skin and cape of an offline account.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountAppearance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skin: Option<CustomTexture>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cape: Option<CustomTexture>,
}

/// A texture the launcher applies itself. `source` is an https URL or an
/// absolute local path to a PNG file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomTexture {
    pub source: String,
    /// Skin model hint: `"slim"` (Alex) or `"wide"` (Steve). Capes ignore it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl CustomTexture {
    pub const SLIM: &str = "slim";
    pub const WIDE: &str = "wide";
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

    pub fn needs_sign_in(&self) -> bool {
        self.is_microsoft() && self.signed_out
    }

    pub fn is_expired(&self) -> bool {
        self.expires <= Utc::now() + chrono::TimeDelta::seconds(60)
    }

    pub fn appearance(&self) -> Option<&AccountAppearance> {
        self.appearance.as_ref()
    }

    pub fn custom_skin(&self) -> Option<&CustomTexture> {
        self.appearance.as_ref().and_then(|a| a.skin.as_ref())
    }

    pub fn custom_cape(&self) -> Option<&CustomTexture> {
        self.appearance.as_ref().and_then(|a| a.cape.as_ref())
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
