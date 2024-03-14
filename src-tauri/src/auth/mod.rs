use std::error::Error;

use serde::{Deserialize, Serialize};
use tauri::{plugin::TauriPlugin, AppHandle, Runtime};

mod microsoft_auth;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Account {
    pub username: String,
    pub uuid: String,
    pub skins: Vec<AccountSkin>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountSkin {
    pub id: String,
    pub state: String,
    pub url: String,
    pub variant: String,
}

pub trait AuthenticationMethod {
    async fn auth<R: Runtime>(handle: &AppHandle<R>) -> Result<Account, Box<dyn Error>>;
}

// Plugin for MSA
pub fn init<R: tauri::Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("auth")
    .invoke_handler(tauri::generate_handler![
        microsoft_auth::login_msa
    ])
    .build()
}