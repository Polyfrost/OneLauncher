use serde::{Deserialize, Serialize};
use tauri::{plugin::TauriPlugin, AppHandle, Manager, Runtime};
use tauri_plugin_http::reqwest::Client;

use crate::{PolyError, PolyResult};

use self::microsoft_auth::MicrosoftAuthenticationMethod;

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

#[allow(async_fn_in_trait)]
pub trait AuthenticationMethod {
    /// Authenticate with a given method. Stage is a function that takes a string and a u8.
    /// The string is the message to display to the user, and the u8 is the progress of the authentication.
    async fn auth<R: Runtime, F>(handle: &AppHandle<R>, stage: F) -> PolyResult<Account>
        where F: Fn(String, u8, bool) -> ();

    async fn get_profile(access_token: String) -> PolyResult<Account> {
        let response = Client::new()
            .get("https://api.minecraftservices.com/minecraft/profile")
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?;
    
        let response = response.json::<serde_json::Value>().await?;
        if let Some(error) = response.get("error") {
            return Err(PolyError(error.to_string()));
        }
    
        let account = serde_json::from_value::<Account>(response)?;
        Ok(account)
    }
}

// Plugin for MSA
pub fn init<R: tauri::Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("auth")
    .invoke_handler(tauri::generate_handler![
        login_msa
    ])
    .build()
}

#[tauri::command]
async fn login_msa<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<Account, String> {
    match MicrosoftAuthenticationMethod::auth(&app, |status, stage, was_last| {
        let _ = app.emit("msa:status", (status, stage, was_last));
    }).await {
        Ok(account) => Ok(account),
        Err(err) => Err(err.to_string())
    }
}
