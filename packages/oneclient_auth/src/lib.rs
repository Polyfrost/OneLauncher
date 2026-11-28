//! Construct [`AuthService`] in the composition layer and pass it down nothing
//! in here reaches for a global or a database
//! Accounts persist to `auth.json`

mod data;
mod diagnostics;
mod error;
mod msa;
mod offline;
mod service;
mod store;

pub use data::{
    AccountAppearance, AccountKind, BrowserLogin, CustomTexture, DeviceCodeLogin,
    MicrosoftLoginSession, MinecraftAccount,
};
pub use diagnostics::{AuthErrorGuidance, AuthErrorSample, diagnose_auth_error, preview_samples};
pub use error::{AuthError, AuthResult, MinecraftAuthError, MinecraftAuthStep};
pub use msa::PendingBrowserLogin;
pub use offline::{
    offline_account, offline_username_error, offline_username_warning, offline_uuid,
};
pub use service::{AuthService, MICROSOFT_LOGIN_PROGRESS};
pub use store::CredentialsStore;
