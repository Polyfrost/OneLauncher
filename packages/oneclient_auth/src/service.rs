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

/// Stable id for the Microsoft login's progress, so a front-end can recognise
/// it among everything else on the bus.
///
/// Naming it here is the core stating an identity, not a display decision. It
/// says nothing about whether this is a toast or a modal, which is the
/// front-end's call. It exists because the login is driven by a mutation with
/// no place to own a private bus.
pub const MICROSOFT_LOGIN_PROGRESS: Uuid =
	Uuid::from_u128(0x4D53_4141_5554_4800_0000_0000_0000_0001);

/// A Microsoft login that has been started but has not finished.
///
/// The two halves have different owners once the flow is under way, which is
/// the whole reason this is a struct: `finish_microsoft_login` takes the
/// listener and runs with it, while the token stays here so a cancel arriving
/// from the UI has something to flip.
struct PendingLogin {
	/// The loopback listener, until the flow claims it.
	///
	/// `None` once someone owns it, or once a cancel freed the port before any
	/// flow got that far.
	browser: Option<PendingBrowserLogin>,
	cancel: CancellationToken,
}

/// Owns everything the authentication flows need.
///
/// Constructed once by the composition layer and passed down, so nothing here
/// reaches for a global. No database; accounts live in `auth.json` beside the
/// launcher data.
pub struct AuthService {
	store: Mutex<CredentialsStore>,
	/// Browser logins waiting on their redirect, keyed by CSRF state token.
	pending_logins: Mutex<HashMap<String, PendingLogin>>,
	/// Serialises token renewal per account.
	///
	/// Microsoft rotates the refresh token on every use, so two concurrent
	/// renewals of one account would race: the loser spends an already-consumed
	/// refresh token and the account gets signed out. Clusters can launch in
	/// parallel, so this is reachable. The guard makes the second caller wait and
	/// reuse the first caller's result instead of starting its own chain.
	///
	/// A `std::sync::Mutex` because its critical section is one map lookup; the
	/// `tokio::Mutex` inside is what is held across the handshake.
	refresh_guards: StdMutex<HashMap<Uuid, Arc<Mutex<()>>>>,
	net: RequestClient,
	events: EventBus,
}

impl AuthService {
	/// Loads the credentials store from disk. A missing or unreadable file
	/// yields an empty store rather than failing: a corrupt `auth.json` should
	/// mean "sign in again", not "the launcher will not start".
	pub async fn load(net: RequestClient, events: EventBus) -> AuthResult<Self> {
		Ok(Self::with_store(CredentialsStore::load().await?, net, events))
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

	// --- Microsoft login ---------------------------------------------------

	#[tracing::instrument(skip_all)]
	pub async fn begin_microsoft_login(&self) -> AuthResult<MicrosoftLoginSession> {
		tracing::info!("beginning Microsoft login");
		let client = self.net.http();

		let (browser, pending) = msa::begin_browser_login().await?;
		let device = msa::begin_device_login(client).await?;

		let mut logins = self.pending_logins.lock().await;
		// A login cancelled before its flow ever started has nobody left to remove
		// it, so the next login sweeps it up.
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

	/// Takes the loopback listener for `state_token`'s flow, plus the token that
	/// cancels it. Only one caller can ever claim a login.
	async fn claim_pending_login(
		&self,
		state_token: &str,
	) -> AuthResult<(PendingBrowserLogin, CancellationToken)> {
		let mut logins = self.pending_logins.lock().await;

		// Cancelled between `begin` and here: the listener is already gone, and
		// there is nothing to run.
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

		// Racing the token is what makes a cancel prompt: losing the race drops
		// `flow`, and with it the device-code poll, the loopback listener and
		// whichever request was mid-flight. There is nothing left to check
		// between polls, and nothing keeps running behind the closed dialog.
		let result = tokio::select! {
			biased;
			() = cancel.cancelled() => Err(AuthError::LoginCancelled),
			res = flow => res.map_err(AuthError::from),
		};

		// Whoever runs the flow owns the cleanup; a cancel only flips the token,
		// so that an in-flight flow is the one to tear its own state down.
		self.pending_logins.lock().await.remove(&session.browser.state);

		// Drives the card to 100%, which is how a front-end knows the flow is
		// over and can clear whatever it was rendering. A cancelled flow needs
		// this just as much as a successful one, or the modal's progress line
		// would outlive the sign-in it belongs to.
		self.events
			.progress(MICROSOFT_LOGIN_PROGRESS, "Signed in", 1, 1);

		// Re-read rather than trusting `result`: the last handshake step can
		// complete in the same breath as a cancel, and an account the user has
		// already walked away from must not reach the store or the toast.
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

	/// Stops an in-flight Microsoft login and leaves nothing of it behind.
	///
	/// The entry is deliberately left in the map: `finish_microsoft_login` may
	/// not have claimed it yet, and it needs to find the cancelled token rather
	/// than a hole it would report as "this sign-in is no longer active".
	#[tracing::instrument(level = "debug", skip_all)]
	pub async fn cancel_microsoft_login(&self, state_token: &str) {
		let mut logins = self.pending_logins.lock().await;
		let Some(login) = logins.get_mut(state_token) else {
			return;
		};

		login.cancel.cancel();
		// Frees the port now if no flow ever took the listener; once one has, the
		// flow drops it as it unwinds.
		login.browser = None;
	}

	// --- account management ------------------------------------------------

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

	pub async fn has_microsoft_account(&self) -> bool {
		self.store.lock().await.has_microsoft_account()
	}

	// --- token renewal -----------------------------------------------------

	fn refresh_guard(&self, id: Uuid) -> Arc<Mutex<()>> {
		Arc::clone(
			self.refresh_guards
				.lock()
				.expect("refresh guard registry poisoned")
				.entry(id)
				.or_default(),
		)
	}

	/// Clones an account out of the store, so no caller holds the store lock
	/// past this call.
	async fn account_snapshot(&self, id: Uuid) -> AuthResult<MinecraftAccount> {
		self.store
			.lock()
			.await
			.get_account(id)
			.cloned()
			.ok_or(AuthError::AccountNotFound(id))
	}

	/// Renews `id`'s access token if it has lapsed, returning a usable account.
	///
	/// The store lock is not held across the Microsoft handshake, only around the
	/// reads and the final write. Holding it across those six sequential round
	/// trips stalls every other account read behind a multi-second network call.
	#[tracing::instrument(level = "debug", skip(self), fields(%id))]
	async fn renew_token(&self, id: Uuid, force: bool) -> AuthResult<MinecraftAccount> {
		let existing = self.account_snapshot(id).await?;
		if !existing.is_microsoft() || (!force && !existing.is_expired()) {
			return Ok(existing);
		}

		let guard = self.refresh_guard(id);
		let _serialised = guard.lock().await;

		// Re-read under the guard: whoever held it may have just refreshed.
		let existing = self.account_snapshot(id).await?;
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

	#[tracing::instrument(level = "debug", skip_all)]
	pub async fn refresh_all_accounts(&self) -> AuthResult<Vec<MinecraftAccount>> {
		let ids: Vec<Uuid> = self.store.lock().await.users.keys().copied().collect();
		let mut refreshed = Vec::with_capacity(ids.len());

		for id in ids {
			refreshed.push(self.renew_token(id, true).await?);
		}

		Ok(refreshed)
	}

	/// An account with a token good enough to launch with, renewing if needed.
	#[tracing::instrument(level = "debug", skip(self), fields(%id))]
	pub async fn account_for_launch(&self, id: Uuid) -> AuthResult<MinecraftAccount> {
		let account = self.renew_token(id, false).await?;

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

	/// Registers a login the way `begin_microsoft_login` does, minus the device
	/// code request: the browser half is the one that binds a port, and it is
	/// the only half that needs no network to set up.
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
		// The flow starts on its own task, so a quick cancel can land before it
		// claims the login. Answering that with "this sign-in is no longer
		// active" would show the user an error for working the dialog correctly.
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
		// Once claimed, the entry is the only channel a cancel has to the flow
		// that is running, so it has to survive the claim.
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
}
