use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};

use oneclient_events::EventBus;
use oneclient_net::RequestClient;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::data::{MicrosoftLoginSession, MinecraftAccount};
use crate::error::{AuthError, AuthResult, MinecraftAuthError};
use crate::msa::{self, PendingBrowserLogin};
use crate::store::{self, CredentialsStore};

/// Stable id for the Microsoft login's progress so a front-end can recognise
/// it among everything else on the bus
pub const MICROSOFT_LOGIN_PROGRESS: Uuid =
    Uuid::from_u128(0x4D53_4141_5554_4800_0000_0000_0000_0001);

/// `finish_microsoft_login` takes the listener and runs with it while the
/// token stays here so a cancel from the UI has something to flip
struct PendingLogin {
    /// `None` once a flow owns the listener or once a cancel freed the port
    /// before any flow got that far
    browser: Option<PendingBrowserLogin>,
    cancel: CancellationToken,
}

pub struct AuthService {
    store: Mutex<CredentialsStore>,
    /// Keyed by CSRF state token
    pending_logins: Mutex<HashMap<String, PendingLogin>>,
    /// Serialises token renewal per account Microsoft rotates the refresh token
    /// on every use so concurrent renewals would sign the account out
    refresh_guards: StdMutex<HashMap<Uuid, Arc<Mutex<()>>>>,
    net: RequestClient,
    events: EventBus,
}

impl AuthService {
    /// A missing or unreadable `auth.json` yields an empty store rather than
    /// failing so a corrupt file means "sign in again" not "will not start"
    pub async fn load(net: RequestClient, events: EventBus) -> AuthResult<Self> {
        Ok(Self::with_store(
            CredentialsStore::load().await?,
            net,
            events,
        ))
    }

    #[must_use]
    pub fn with_store(store: CredentialsStore, net: RequestClient, events: EventBus) -> Self {
        Self {
            store: Mutex::new(store),
            pending_logins: Mutex::new(HashMap::new()),
            refresh_guards: StdMutex::new(HashMap::new()),
            net,
            events,
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn begin_microsoft_login(&self) -> AuthResult<MicrosoftLoginSession> {
        tracing::info!("beginning Microsoft login");
        let client = self.net.http();

        let (browser, pending) = msa::begin_browser_login().await?;
        let device = msa::begin_device_login(client).await?;

        let mut logins = self.pending_logins.lock().await;
        // A login cancelled before its flow ever started has nobody left to remove
        // it so the next login sweeps it up
        logins.retain(|_, login| !login.cancel.is_cancelled());
        logins.insert(
            browser.state.clone(),
            PendingLogin {
                browser: Some(pending),
                cancel: CancellationToken::new(),
            },
        );

        Ok(MicrosoftLoginSession { browser, device })
    }

    /// Only one caller can ever claim a login
    async fn claim_pending_login(
        &self,
        state_token: &str,
    ) -> AuthResult<(PendingBrowserLogin, CancellationToken)> {
        let mut logins = self.pending_logins.lock().await;

        // Cancelled between `begin` and here the listener is already gone and
        // there is nothing to run
        if logins
            .get(state_token)
            .is_some_and(|login| login.cancel.is_cancelled())
        {
            logins.remove(state_token);
            return Err(AuthError::LoginCancelled);
        }

        let login = logins.get_mut(state_token).ok_or(AuthError::Minecraft(
            MinecraftAuthError::BrowserLoginNotFound,
        ))?;
        let cancel = login.cancel.clone();
        let browser = login.browser.take().ok_or(AuthError::Minecraft(
            MinecraftAuthError::BrowserLoginNotFound,
        ))?;

        Ok((browser, cancel))
    }

    #[tracing::instrument(skip_all)]
    pub async fn finish_microsoft_login(
        &self,
        session: MicrosoftLoginSession,
    ) -> AuthResult<MinecraftAccount> {
        tracing::info!("finishing Microsoft login");

        let (pending, cancel) = self.claim_pending_login(&session.browser.state).await?;

        let events = self.events.clone();
        let flow = msa::finish_dual_login(
            self.net.http(),
            pending,
            &session.device,
            |label, current, total| {
                events.progress(MICROSOFT_LOGIN_PROGRESS, label, current, total);
            },
        );

        // Losing the race drops `flow` and with it the device-code poll the
        // loopback listener and whichever request was mid-flight
        let result = tokio::select! {
            biased;
            () = cancel.cancelled() => Err(AuthError::LoginCancelled),
            res = flow => res.map_err(AuthError::from),
        };

        // Whoever runs the flow owns the cleanup a cancel only flips the token
        self.pending_logins
            .lock()
            .await
            .remove(&session.browser.state);

        // Drives the card to 100% so the front-end can clear it a cancelled flow
        // needs this as much as a successful one
        self.events
            .progress(MICROSOFT_LOGIN_PROGRESS, "Signed in", 1, 1);

        // Re-read rather than trusting `result` the last handshake step can
        // complete in the same breath as a cancel
        if cancel.is_cancelled() {
            tracing::info!("Microsoft login cancelled by the user");
            return Err(AuthError::LoginCancelled);
        }

        if let Err(err) = &result {
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
        self.store
            .lock()
            .await
            .commit_account(account, &self.events)
            .await
    }

    /// The entry is deliberately left in the map `finish_microsoft_login` may
    /// not have claimed it yet and must find the cancelled token not a hole
    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn cancel_microsoft_login(&self, state_token: &str) {
        let mut logins = self.pending_logins.lock().await;
        let Some(login) = logins.get_mut(state_token) else {
            return;
        };

        login.cancel.cancel();
        // Frees the port now if no flow ever took the listener once one has the
        // flow drops it as it unwinds
        login.browser = None;
    }

    #[tracing::instrument(skip(self), fields(username = %username))]
    pub async fn add_offline_account(&self, username: String) -> AuthResult<MinecraftAccount> {
        self.store
            .lock()
            .await
            .add_offline_account_and_save(username)
            .await
    }

    pub async fn list_accounts(&self) -> Vec<MinecraftAccount> {
        self.store.lock().await.list_accounts()
    }

    pub async fn get_account(&self, id: Uuid) -> Option<MinecraftAccount> {
        self.store.lock().await.get_account(id).cloned()
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn default_account(&self) -> AuthResult<Option<MinecraftAccount>> {
        self.store.lock().await.default_account().await
    }

    #[tracing::instrument(level = "debug", skip(self), fields(?id))]
    pub async fn set_default_account(&self, id: Option<Uuid>) -> AuthResult<()> {
        self.store.lock().await.set_default_user(id).await
    }

    #[tracing::instrument(skip(self), fields(%id))]
    pub async fn remove_account(&self, id: Uuid) -> AuthResult<()> {
        self.store.lock().await.remove_account(id).await?;
        Ok(())
    }

    /// Debug aid puts the account straight into the state a rejected refresh
    /// token leaves behind
    #[tracing::instrument(skip(self), fields(%id))]
    pub async fn mark_signed_out(&self, id: Uuid) -> AuthResult<()> {
        let guard = self.refresh_guard(id);
        let _serialised = guard.lock().await;
        self.store.lock().await.mark_signed_out(id).await
    }

    /// Debug aid see [`CredentialsStore::corrupt_refresh_token`]
    #[tracing::instrument(skip(self), fields(%id))]
    pub async fn corrupt_refresh_token(&self, id: Uuid) -> AuthResult<()> {
        let guard = self.refresh_guard(id);
        let _serialised = guard.lock().await;
        self.store.lock().await.corrupt_refresh_token(id).await
    }

    pub async fn has_microsoft_account(&self) -> bool {
        self.store.lock().await.has_microsoft_account()
    }

    fn refresh_guard(&self, id: Uuid) -> Arc<Mutex<()>> {
        Arc::clone(
            self.refresh_guards
                .lock()
                .expect("refresh guard registry poisoned")
                .entry(id)
                .or_default(),
        )
    }

    async fn account_snapshot(&self, id: Uuid) -> AuthResult<MinecraftAccount> {
        self.store
            .lock()
            .await
            .get_account(id)
            .cloned()
            .ok_or(AuthError::AccountNotFound(id))
    }

    /// The store lock is never held across the Microsoft handshake only around
    /// the reads and the final write
    #[tracing::instrument(level = "debug", skip(self), fields(%id))]
    async fn renew_token(&self, id: Uuid, force: bool) -> AuthResult<MinecraftAccount> {
        let existing = self.account_snapshot(id).await?;
        if existing.needs_sign_in() {
            return Err(AuthError::SessionExpired {
                username: existing.username,
            });
        }
        if !existing.is_microsoft() || (!force && !existing.is_expired()) {
            return Ok(existing);
        }

        let guard = self.refresh_guard(id);
        let _serialised = guard.lock().await;

        // Re-read under the guard whoever held it may have just refreshed
        let existing = self.account_snapshot(id).await?;
        if existing.needs_sign_in() {
            return Err(AuthError::SessionExpired {
                username: existing.username,
            });
        }
        if !existing.is_microsoft() || (!force && !existing.is_expired()) {
            return Ok(existing);
        }

        tracing::info!(username = %existing.username, "renewing Microsoft access token");
        match msa::refresh_microsoft_account(self.net.http(), &existing).await {
            Ok(refreshed) => {
                self.store
                    .lock()
                    .await
                    .commit_refreshed_account(refreshed.clone())
                    .await?;
                Ok(refreshed)
            }
            Err(MinecraftAuthError::RefreshTokenRevoked { .. }) => {
                tracing::warn!(username = %existing.username, "refresh token rejected, marking account signed out");
                self.store.lock().await.mark_signed_out(id).await?;
                Err(AuthError::SessionExpired {
                    username: existing.username,
                })
            }
            Err(err) => {
                let err = AuthError::from(err);
                if store::is_transient_auth_error(&err) {
                    tracing::warn!("keeping existing token after transient renewal failure: {err}");
                    Ok(existing)
                } else {
                    Err(err)
                }
            }
        }
    }

    #[tracing::instrument(level = "debug", skip(self), fields(%id))]
    pub async fn refresh_account(&self, id: Uuid) -> AuthResult<MinecraftAccount> {
        self.renew_token(id, true).await
    }

    /// A signed-out account is skipped rather than failing the whole batch it
    /// already shows as needing a sign-in
    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn refresh_all_accounts(&self) -> AuthResult<Vec<MinecraftAccount>> {
        let ids: Vec<Uuid> = self.store.lock().await.users.keys().copied().collect();
        let mut refreshed = Vec::with_capacity(ids.len());

        for id in ids {
            match self.renew_token(id, true).await {
                Ok(account) => refreshed.push(account),
                Err(AuthError::SessionExpired { .. }) => {}
                Err(err) => return Err(err),
            }
        }

        Ok(refreshed)
    }

    /// A revoked refresh token is only discovered by trying it doing so at
    /// startup surfaces the sign-out before the user presses play
    /// Only lapsed tokens are tried a live one needs no renewal yet
    /// Returns whether any account was newly marked signed out
    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn check_expired_sessions(&self) -> bool {
        let ids: Vec<Uuid> = self
            .store
            .lock()
            .await
            .users
            .values()
            .filter(|a| a.is_microsoft() && !a.needs_sign_in() && a.is_expired())
            .map(|a| a.id)
            .collect();

        let mut signed_out = false;
        for id in ids {
            match self.renew_token(id, false).await {
                Ok(_) => {}
                Err(AuthError::SessionExpired { .. }) => signed_out = true,
                Err(err) => tracing::warn!(%id, "startup session check failed: {err}"),
            }
        }
        signed_out
    }

    #[tracing::instrument(level = "debug", skip(self), fields(%id))]
    pub async fn account_for_launch(&self, id: Uuid) -> AuthResult<MinecraftAccount> {
        let account = match self.renew_token(id, false).await {
            Ok(account) => account,
            // A signed-out account still plays under its saved name and uuid
            // with an empty token like an offline account singleplayer works
            // and the UI keeps asking for a sign-in
            Err(AuthError::SessionExpired { username }) => {
                tracing::info!(%username, "launching signed-out account without a token");
                self.account_snapshot(id).await?
            }
            Err(err) => return Err(err),
        };

        if account.is_offline() && !self.has_microsoft_account().await {
            return Err(AuthError::OfflineRequiresMicrosoft);
        }

        Ok(account)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn default_account_for_launch(&self) -> AuthResult<Option<MinecraftAccount>> {
        let Some(id) = self.store.lock().await.resolve_default_id().await? else {
            return Ok(None);
        };
        Ok(Some(self.account_for_launch(id).await?))
    }
}

#[cfg(test)]
mod tests {
    use oneclient_net::NetConfig;

    use super::*;
    use crate::data::DeviceCodeLogin;

    fn service(events: EventBus) -> AuthService {
        let net = RequestClient::new(NetConfig::default()).expect("net client");
        AuthService::with_store(CredentialsStore::default(), net, events)
    }

    /// Registers a login the way `begin_microsoft_login` does minus the device
    /// code request which is the only half that needs the network
    async fn register_login(service: &AuthService) -> MicrosoftLoginSession {
        let (browser, pending) = msa::begin_browser_login().await.expect("loopback bind");

        service.pending_logins.lock().await.insert(
            browser.state.clone(),
            PendingLogin {
                browser: Some(pending),
                cancel: CancellationToken::new(),
            },
        );

        MicrosoftLoginSession {
            browser,
            device: DeviceCodeLogin {
                user_code: "ABCD-EFGH".to_string(),
                device_code: "device-code".to_string(),
                verification_uri: "https://microsoft.com/link".to_string(),
                expires_in: 900,
                interval: 5,
                message: String::new(),
            },
        }
    }

    #[tokio::test]
    async fn a_cancel_that_beats_the_flow_is_still_a_cancel() {
        // A quick cancel can land before the flow claims the login
        let (events, _rx) = EventBus::channel();
        let service = service(events);
        let session = register_login(&service).await;

        service.cancel_microsoft_login(session.dedupe_key()).await;
        let err = service.finish_microsoft_login(session).await.unwrap_err();

        assert!(matches!(err, AuthError::LoginCancelled), "{err}");
        assert!(service.pending_logins.lock().await.is_empty());
        assert!(service.store.lock().await.list_accounts().is_empty());
    }

    #[tokio::test]
    async fn a_cancel_frees_the_loopback_port_it_was_holding() {
        let (events, _rx) = EventBus::channel();
        let service = service(events);
        let session = register_login(&service).await;

        service.cancel_microsoft_login(session.dedupe_key()).await;

        let logins = service.pending_logins.lock().await;
        let login = logins
            .get(session.dedupe_key())
            .expect("the entry is what a flow starting late reads the cancel from");
        assert!(login.cancel.is_cancelled());
        assert!(
            login.browser.is_none(),
            "the listener should be dropped rather than left bound"
        );
    }

    #[tokio::test]
    async fn a_cancel_reaches_a_flow_that_has_already_claimed_the_login() {
        // Once claimed the entry is the only channel a cancel has to the flow
        let (events, _rx) = EventBus::channel();
        let service = service(events);
        let session = register_login(&service).await;

        let (_listener, cancel) = service
            .claim_pending_login(session.dedupe_key())
            .await
            .expect("claim");
        service.cancel_microsoft_login(session.dedupe_key()).await;

        assert!(cancel.is_cancelled());
    }

    #[tokio::test]
    async fn a_login_can_only_be_claimed_once() {
        let (events, _rx) = EventBus::channel();
        let service = service(events);
        let session = register_login(&service).await;

        service
            .claim_pending_login(session.dedupe_key())
            .await
            .expect("claim");
        let claimed_again = service.claim_pending_login(session.dedupe_key()).await;

        assert!(matches!(
            claimed_again,
            Err(AuthError::Minecraft(
                MinecraftAuthError::BrowserLoginNotFound
            ))
        ));
    }

    fn signed_out_service() -> (AuthService, Uuid) {
        let mut account = crate::offline::offline_account("MsaUser".to_string());
        account.kind = crate::data::AccountKind::Microsoft;
        account.signed_out = true;
        let id = account.id;

        let mut store = CredentialsStore::default();
        store.users.insert(id, account);
        store.default_user = Some(id);

        let (events, _rx) = EventBus::channel();
        let net = RequestClient::new(NetConfig::default()).expect("net client");
        (AuthService::with_store(store, net, events), id)
    }

    #[tokio::test]
    async fn a_signed_out_account_still_launches_without_touching_the_network() {
        let (service, id) = signed_out_service();

        let account = service
            .default_account_for_launch()
            .await
            .expect("a signed-out account must still launch")
            .expect("the only account is the default");
        assert_eq!(account.id, id);
        assert!(account.needs_sign_in());
        assert!(account.access_token.is_empty());
    }

    #[tokio::test]
    async fn refresh_all_skips_signed_out_accounts() {
        let (service, id) = signed_out_service();

        let refreshed = service.refresh_all_accounts().await.unwrap();
        assert!(refreshed.is_empty());
        assert!(
            service.get_account(id).await.is_some(),
            "the entry must stay listed"
        );
    }

    #[tokio::test]
    async fn the_startup_check_leaves_live_and_signed_out_accounts_alone() {
        let (service, signed_out_id) = signed_out_service();

        let mut live = crate::offline::offline_account("LiveUser".to_string());
        live.kind = crate::data::AccountKind::Microsoft;
        live.expires = chrono::Utc::now() + chrono::TimeDelta::hours(12);
        let live_id = live.id;
        service.store.lock().await.users.insert(live_id, live);

        // Neither account qualifies so this must return without a request
        assert!(!service.check_expired_sessions().await);
        assert!(!service.get_account(live_id).await.unwrap().needs_sign_in());
        assert!(
            service
                .get_account(signed_out_id)
                .await
                .unwrap()
                .needs_sign_in()
        );
    }
}
