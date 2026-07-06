mod error;
mod msa;
mod offline;
mod store;
mod data;

pub use error::{AuthError, MinecraftAuthError, MinecraftAuthStep};
pub use offline::{offline_account, offline_uuid, validate_offline_username};
pub use store::CredentialsStore;
pub use data::{AccountKind, BrowserLogin, DeviceCodeLogin, MinecraftAccount, MinecraftLogin};
pub use msa::PendingBrowserLogin;

use uuid::Uuid;

use crate::LauncherResult;
use crate::state::LauncherState;

pub async fn begin_microsoft_login() -> LauncherResult<MinecraftLogin> {
    let state = LauncherState::get()?;
    let use_browser = state.settings.read().microsoft_login_use_browser;

    if use_browser {
        let (login, pending) = msa::begin_browser_login().await.map_err(AuthError::from)?;
        state
            .microsoft_logins
            .lock()
            .await
            .insert(login.state.clone(), pending);
        Ok(MinecraftLogin::Browser(login))
    } else {
        let client = state.services.requester.http();
        let flow = msa::begin_device_login(client).await.map_err(AuthError::from)?;
        Ok(MinecraftLogin::DeviceCode(flow))
    }
}

pub async fn finish_microsoft_login(flow: MinecraftLogin) -> LauncherResult<MinecraftAccount> {
    let state = LauncherState::get()?;

    match flow {
        MinecraftLogin::DeviceCode(flow) => {
            let mut auth = state.auth.lock().await;
            auth.finish_device_login(&flow, &state.services).await
        }
        MinecraftLogin::Browser(login) => {
            let pending = state
                .microsoft_logins
                .lock()
                .await
                .remove(&login.state)
                .ok_or(AuthError::Minecraft(
                    MinecraftAuthError::BrowserLoginNotFound,
                ))?;

            let client = state.services.requester.http();
            let progress_id = Uuid::new_v4();
            let notifier = state.services.notifier.clone();
            let account = msa::finish_browser_login(client, pending, |label, current, total| {
                notifier.send_progress(&progress_id, label, current, total);
            })
            .await
            .map_err(AuthError::from)?;

            let mut auth = state.auth.lock().await;
            auth.commit_account(account, &state.services).await
        }
    }
}

pub async fn cancel_microsoft_login(state_token: &str) {
    if let Ok(state) = LauncherState::get() {
        state.microsoft_logins.lock().await.remove(state_token);
    }
}

pub async fn add_offline_account(username: String) -> LauncherResult<MinecraftAccount> {
    let state = LauncherState::get()?;
    let mut auth = state.auth.lock().await;
    auth.add_offline_account_and_save(username).await
}

pub async fn list_accounts() -> LauncherResult<Vec<MinecraftAccount>> {
    let state = LauncherState::get()?;
    let auth = state.auth.lock().await;
    Ok(auth.list_accounts())
}

pub async fn get_account(id: Uuid) -> LauncherResult<Option<MinecraftAccount>> {
    let state = LauncherState::get()?;
    let auth = state.auth.lock().await;
    Ok(auth.get_account(id).cloned())
}

pub async fn get_default_account() -> LauncherResult<Option<MinecraftAccount>> {
    let state = LauncherState::get()?;
    let mut auth = state.auth.lock().await;
    auth.default_account(&state.services).await
}

pub async fn set_default_account(id: Option<Uuid>) -> LauncherResult<()> {
    let state = LauncherState::get()?;
    let mut auth = state.auth.lock().await;
    auth.set_default_user(id).await
}

pub async fn remove_account(id: Uuid) -> LauncherResult<()> {
    let state = LauncherState::get()?;
    let mut auth = state.auth.lock().await;
    auth.remove_account(id).await?;
    Ok(())
}

pub async fn refresh_account(id: Uuid) -> LauncherResult<Option<MinecraftAccount>> {
    let state = LauncherState::get()?;
    let mut auth = state.auth.lock().await;
    auth.refresh_microsoft_account(id, &state.services).await
}

pub async fn refresh_all_accounts() -> LauncherResult<Vec<MinecraftAccount>> {
    let state = LauncherState::get()?;
    let mut auth = state.auth.lock().await;
    let ids: Vec<Uuid> = auth.users.keys().copied().collect();
    let mut refreshed = Vec::with_capacity(ids.len());

    for id in ids {
        if let Some(account) = auth.refresh_microsoft_account(id, &state.services).await? {
            refreshed.push(account);
        }
    }

    Ok(refreshed)
}

pub async fn account_for_launch(id: Uuid) -> LauncherResult<MinecraftAccount> {
    let state = LauncherState::get()?;
    let mut auth = state.auth.lock().await;
    auth.account_for_launch(id, &state.services).await
}

pub async fn has_microsoft_account() -> LauncherResult<bool> {
    let state = LauncherState::get()?;
    let auth = state.auth.lock().await;
    Ok(auth.has_microsoft_account())
}
