mod error;
mod msa;
mod offline;
mod store;
mod data;

pub use error::{AuthError, MinecraftAuthError, MinecraftAuthStep};
pub use offline::{offline_account, offline_uuid, validate_offline_username};
pub use store::CredentialsStore;
pub use data::{
    AccountKind, BrowserLogin, DeviceCodeLogin, MicrosoftLoginSession, MinecraftAccount,
};
pub use msa::PendingBrowserLogin;

use uuid::Uuid;

use crate::LauncherResult;
use crate::notification::MicrosoftLoginStatus;
use crate::state::LauncherState;

pub async fn begin_microsoft_login() -> LauncherResult<MicrosoftLoginSession> {
    let state = LauncherState::get()?;
    let client = state.services.requester.http();

    let (browser, pending) = msa::begin_browser_login().await.map_err(AuthError::from)?;
    let device = msa::begin_device_login(client).await.map_err(AuthError::from)?;

    state
        .microsoft_logins
        .lock()
        .await
        .insert(browser.state.clone(), pending);

    Ok(MicrosoftLoginSession { browser, device })
}

pub async fn finish_microsoft_login(
    session: MicrosoftLoginSession,
) -> LauncherResult<MinecraftAccount> {
    let state = LauncherState::get()?;

    let pending = state
        .microsoft_logins
        .lock()
        .await
        .remove(&session.browser.state)
        .ok_or(AuthError::Minecraft(
            MinecraftAuthError::BrowserLoginNotFound,
        ))?;

    let client = state.services.requester.http();
    let notifier = state.services.notifier.clone();

    let result = msa::finish_dual_login(
        client,
        pending,
        &session.device,
        |label, current, total| {
            notifier.microsoft_login_status(Some(MicrosoftLoginStatus {
                label: label.to_string(),
                current,
                total,
            }));
        },
    )
    .await
    .map_err(AuthError::from);

    state.services.notifier.microsoft_login_status(None);

    let account = result?;
    let mut auth = state.auth.lock().await;
    auth.commit_account(account, &state.services).await
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
