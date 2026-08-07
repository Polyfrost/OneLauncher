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
	AccountKind, BrowserLogin, DeviceCodeLogin, MicrosoftLoginSession, MinecraftAccount,
};
pub use diagnostics::{AuthErrorGuidance, AuthErrorSample, diagnose_auth_error, preview_samples};
pub use error::{AuthError, AuthResult, MinecraftAuthError, MinecraftAuthStep};
pub use msa::PendingBrowserLogin;
pub use offline::{offline_account, offline_uuid, validate_offline_username};
pub use service::{AuthService, MICROSOFT_LOGIN_PROGRESS};
pub use store::CredentialsStore;
