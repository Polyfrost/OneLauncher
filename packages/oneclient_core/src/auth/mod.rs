mod error;
mod msa;
mod offline;
mod store;
mod data;
mod diagnostics;

pub use error::{AuthError, MinecraftAuthError, MinecraftAuthStep};
pub use diagnostics::{
    diagnose_auth_error, preview_samples, AuthErrorGuidance, AuthErrorSample,
};
pub use offline::{offline_account, offline_uuid, validate_offline_username};
pub use store::CredentialsStore;
pub use data::{
    AccountKind, BrowserLogin, DeviceCodeLogin, MicrosoftLoginSession, MinecraftAccount,
};
pub use msa::PendingBrowserLogin;

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex as StdMutex};

use tokio::sync::Mutex;
use uuid::Uuid;

use crate::LauncherResult;
use crate::notification::MicrosoftLoginStatus;
use crate::state::LauncherState;

#[tracing::instrument(skip_all)]
pub async fn begin_microsoft_login() -> LauncherResult<MicrosoftLoginSession> {
    tracing::info!("beginning Microsoft login");
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

#[tracing::instrument(skip_all)]
pub async fn finish_microsoft_login(
    session: MicrosoftLoginSession,
) -> LauncherResult<MinecraftAccount> {
    tracing::info!("finishing Microsoft login");
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

    if let Err(err) = &result {
        tracing::warn!("Microsoft login failed: {err}");
        let mut chain = String::new();
        let mut source = std::error::Error::source(err);
        while let Some(cause) = source {
            chain.push_str(&format!("\n  caused by: {cause}"));
            source = cause.source();
        }
        tracing::warn!("Microsoft login failed: {err}{chain}");
    }

    let account = result?;
    tracing::info!(username = %account.username, "Microsoft login succeeded");
    let mut auth = state.auth.lock().await;
    auth.commit_account(account, &state.services).await
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn cancel_microsoft_login(state_token: &str) {
    if let Ok(state) = LauncherState::get() {
        state.microsoft_logins.lock().await.remove(state_token);
    }
}

#[tracing::instrument(skip_all, fields(username = %username))]
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

#[tracing::instrument(level = "debug", skip_all)]
pub async fn get_default_account() -> LauncherResult<Option<MinecraftAccount>> {
    let state = LauncherState::get()?;
    let mut auth = state.auth.lock().await;
    auth.default_account().await
}

#[tracing::instrument(level = "debug", skip_all, fields(?id))]
pub async fn set_default_account(id: Option<Uuid>) -> LauncherResult<()> {
    let state = LauncherState::get()?;
    let mut auth = state.auth.lock().await;
    auth.set_default_user(id).await
}

#[tracing::instrument(skip_all, fields(%id))]
pub async fn remove_account(id: Uuid) -> LauncherResult<()> {
    let state = LauncherState::get()?;
    let mut auth = state.auth.lock().await;
    auth.remove_account(id).await?;
    Ok(())
}

/// Serialises token renewal per account.
///
/// Microsoft rotates the refresh token on every use, so two concurrent renewals
/// of one account would race: the loser spends an already-consumed refresh token
/// and the account gets signed out. Clusters can launch in parallel, so this is
/// reachable — the guard makes the second caller wait and reuse the first
/// caller's result instead of starting its own chain.
static REFRESH_GUARDS: LazyLock<StdMutex<HashMap<Uuid, Arc<Mutex<()>>>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

fn refresh_guard(id: Uuid) -> Arc<Mutex<()>> {
    Arc::clone(
        REFRESH_GUARDS
            .lock()
            .expect("refresh guard registry poisoned")
            .entry(id)
            .or_default(),
    )
}

/// Clones an account out of the store, returning an owned value so no caller
/// keeps the credentials-store lock alive past this call.
async fn account_snapshot(
    state: &Arc<LauncherState>,
    id: Uuid,
) -> LauncherResult<MinecraftAccount> {
    let auth = state.auth.lock().await;
    auth.get_account(id)
        .cloned()
        .ok_or_else(|| AuthError::AccountNotFound(id).into())
}

/// Renews `id`'s access token if it has lapsed, returning a usable account.
///
/// The credentials-store lock is deliberately not held across the Microsoft
/// handshake, only around the reads and the final write. Holding it across
/// those six sequential round trips is what used to stall every other account
/// read behind a multi-second network call.
#[tracing::instrument(level = "debug", skip(state), fields(%id))]
async fn renew_token(
    state: &Arc<LauncherState>,
    id: Uuid,
    force: bool,
) -> LauncherResult<MinecraftAccount> {
    let existing = account_snapshot(state, id).await?;
    if !existing.is_microsoft() || (!force && !existing.is_expired()) {
        return Ok(existing);
    }

    let guard = refresh_guard(id);
    let _serialised = guard.lock().await;

    let existing = account_snapshot(state, id).await?;
    if !existing.is_microsoft() || (!force && !existing.is_expired()) {
        return Ok(existing);
    }

    tracing::info!(username = %existing.username, "renewing Microsoft access token");
    match msa::refresh_microsoft_account(state.services.requester.http(), &existing).await {
        Ok(refreshed) => {
            state
                .auth
                .lock()
                .await
                .commit_refreshed_account(refreshed.clone())
                .await?;
            Ok(refreshed)
        }
        Err(err) => {
            let err = crate::LauncherError::from(AuthError::from(err));
            if store::is_transient_auth_error(&err) {
                tracing::warn!("keeping existing token after transient renewal failure: {err}");
                Ok(existing)
            } else {
                Err(err)
            }
        }
    }
}

#[tracing::instrument(level = "debug", skip_all, fields(%id))]
pub async fn refresh_account(id: Uuid) -> LauncherResult<MinecraftAccount> {
    let state = LauncherState::get()?;
    renew_token(&state, id, true).await
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn refresh_all_accounts() -> LauncherResult<Vec<MinecraftAccount>> {
    let state = LauncherState::get()?;
    let ids: Vec<Uuid> = state.auth.lock().await.users.keys().copied().collect();
    let mut refreshed = Vec::with_capacity(ids.len());

    for id in ids {
        refreshed.push(renew_token(&state, id, true).await?);
    }

    Ok(refreshed)
}

#[tracing::instrument(level = "debug", skip_all, fields(%id))]
pub async fn account_for_launch(id: Uuid) -> LauncherResult<MinecraftAccount> {
    let state = LauncherState::get()?;
    let account = renew_token(&state, id, false).await?;

    if account.is_offline() && !state.auth.lock().await.has_microsoft_account() {
        return Err(AuthError::OfflineRequiresMicrosoft.into());
    }

    Ok(account)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn default_account_for_launch() -> LauncherResult<Option<MinecraftAccount>> {
    let state = LauncherState::get()?;
    let Some(id) = state.auth.lock().await.resolve_default_id().await? else {
        return Ok(None);
    };
    Ok(Some(account_for_launch(id).await?))
}

pub async fn has_microsoft_account() -> LauncherResult<bool> {
    let state = LauncherState::get()?;
    let auth = state.auth.lock().await;
    Ok(auth.has_microsoft_account())
}
